//! JSON-RPC 2.0 lewat stdio (satu pesan JSON per baris) untuk Agent Client Protocol (ACP).
//! Satu `Conn` = satu proses agent. Pembacaan stdout di thread sendiri; permintaan memblokir pemanggil
//! (panggil dari thread latar, jangan dari thread UI).
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

/// Cara menjalankan agent: program + argumen + lingkungan.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Variabel lingkungan yang dibuang (kredensial ambien yang tidak boleh ikut, mis. ANTHROPIC_API_KEY).
    pub env_remove: Vec<String>,
}

impl Launch {
    pub fn display(&self) -> String {
        let mut s = self.program.display().to_string();
        for a in &self.args {
            s.push(' ');
            s.push_str(a);
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), data: None }
    }
    /// Pesan untuk pengguna: `data.message` (bila ada) lebih spesifik daripada "Internal error".
    pub fn describe(&self) -> String {
        let inner = self.data.as_ref().and_then(|d| d.get("message").and_then(Value::as_str).map(String::from).or_else(|| d.as_str().map(String::from)));
        match inner {
            Some(m) if !m.is_empty() && m != self.message => format!("{} ({m})", self.message),
            _ => self.message.clone(),
        }
    }
    /// ACP: `auth_required` = -32000.
    pub fn is_auth(&self) -> bool {
        self.code == -32000 || self.describe().to_lowercase().contains("auth")
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.describe())
    }
}

/// Pesan dari agent yang bukan balasan atas permintaan kita.
#[derive(Debug)]
pub enum Incoming {
    Request { id: Value, method: String, params: Value },
    Notification { method: String, params: Value },
    /// Proses berhenti (stdout tertutup).
    Closed,
}

type Reply = mpsc::Sender<Result<Value, RpcError>>;

pub struct Conn {
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<u64, Reply>>,
    next: AtomicU64,
    alive: AtomicBool,
    child: Mutex<Child>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
}

const STDERR_LINES: usize = 40;

/// Cegah jendela konsol muncul saat aplikasi GUI menjalankan proses anak.
#[cfg(windows)]
fn hide_window(c: &mut Command) {
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}
#[cfg(not(windows))]
fn hide_window(_: &mut Command) {} // TODO(port): tidak perlu di unix

/// Siapkan `Command` dengan aturan yang sama untuk agent dan utilitas (npm): tanpa jendela, env dibersihkan.
pub fn command(launch: &Launch) -> Command {
    let mut c = Command::new(&launch.program);
    c.args(&launch.args);
    for k in &launch.env_remove {
        c.env_remove(k);
    }
    hide_window(&mut c);
    c
}

impl Conn {
    pub fn spawn(launch: &Launch, cwd: &std::path::Path, on_incoming: impl Fn(Incoming) + Send + Sync + 'static) -> Result<Arc<Conn>, String> {
        let mut c = command(launch);
        c.current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = c.spawn().map_err(|e| format!("gagal menjalankan {}: {e}", launch.display()))?;
        let stdin = child.stdin.take().ok_or("stdin agent tidak tersedia")?;
        let stdout = child.stdout.take().ok_or("stdout agent tidak tersedia")?;
        let stderr = child.stderr.take();
        let conn = Arc::new(Conn {
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            alive: AtomicBool::new(true),
            child: Mutex::new(child),
            stderr_tail: Arc::new(Mutex::new(VecDeque::new())),
        });
        if let Some(err) = stderr {
            let tail = conn.stderr_tail.clone();
            std::thread::Builder::new()
                .name("zandor-acp-stderr".into())
                .spawn(move || {
                    for line in BufReader::new(err).lines().map_while(Result::ok) {
                        let mut t = tail.lock().unwrap_or_else(|e| e.into_inner());
                        t.push_back(strip_ansi(&line));
                        while t.len() > STDERR_LINES {
                            t.pop_front();
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        let weak = Arc::downgrade(&conn);
        std::thread::Builder::new()
            .name("zandor-acp-read".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    let Ok(msg) = serde_json::from_str::<Value>(line) else { continue };
                    let Some(conn) = weak.upgrade() else { return };
                    match (msg.get("method").and_then(Value::as_str), msg.get("id")) {
                        (Some(m), Some(id)) if !id.is_null() => on_incoming(Incoming::Request { id: id.clone(), method: m.to_string(), params: msg.get("params").cloned().unwrap_or(Value::Null) }),
                        (Some(m), _) => on_incoming(Incoming::Notification { method: m.to_string(), params: msg.get("params").cloned().unwrap_or(Value::Null) }),
                        (None, Some(id)) => {
                            let Some(id) = id.as_u64() else { continue };
                            let reply = conn.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                            if let Some(tx) = reply {
                                let r = match msg.get("error") {
                                    Some(e) if !e.is_null() => Err(RpcError {
                                        code: e.get("code").and_then(Value::as_i64).unwrap_or(-32603),
                                        message: e.get("message").and_then(Value::as_str).unwrap_or("galat agent").to_string(),
                                        data: e.get("data").cloned(),
                                    }),
                                    _ => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
                                };
                                let _ = tx.send(r);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(conn) = weak.upgrade() {
                    conn.alive.store(false, Ordering::Relaxed);
                    let why = conn.exit_reason();
                    for (_, tx) in conn.pending.lock().unwrap_or_else(|e| e.into_inner()).drain() {
                        let _ = tx.send(Err(RpcError::new(-32099, why.clone())));
                    }
                }
                on_incoming(Incoming::Closed);
            })
            .map_err(|e| e.to_string())?;
        Ok(conn)
    }

    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Penjelasan saat proses berhenti: baris stderr terakhir yang bermakna.
    pub fn exit_reason(&self) -> String {
        let tail = self.stderr_tail.lock().unwrap_or_else(|e| e.into_inner());
        let last: Vec<&String> = tail.iter().rev().filter(|l| !l.trim().is_empty()).take(3).collect();
        if last.is_empty() {
            "proses agent berhenti".into()
        } else {
            format!("proses agent berhenti: {}", last.into_iter().rev().cloned().collect::<Vec<_>>().join(" | "))
        }
    }

    fn write(&self, v: &Value) -> Result<(), RpcError> {
        let mut s = serde_json::to_string(v).map_err(|e| RpcError::new(-32700, e.to_string()))?;
        s.push('\n');
        let mut w = self.stdin.lock().unwrap_or_else(|e| e.into_inner());
        w.write_all(s.as_bytes()).and_then(|_| w.flush()).map_err(|e| RpcError::new(-32099, format!("gagal menulis ke agent: {e}")))
    }

    /// Kirim permintaan dan tunggu balasannya. `timeout` None = tunggu sampai selesai (giliran prompt).
    pub fn request(&self, method: &str, params: Value, timeout: Option<Duration>) -> Result<Value, RpcError> {
        if !self.alive() {
            return Err(RpcError::new(-32099, self.exit_reason()));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).insert(id, tx);
        if let Err(e) = self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })) {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            return Err(e);
        }
        let r = match timeout {
            Some(t) => rx.recv_timeout(t).map_err(|_| RpcError::new(-32098, format!("agent tidak menjawab {method} dalam {} detik", t.as_secs()))),
            None => rx.recv().map_err(|_| RpcError::new(-32099, self.exit_reason())),
        };
        if r.is_err() {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
        }
        r?
    }

    pub fn notify(&self, method: &str, params: Value) {
        let _ = self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    pub fn respond(&self, id: Value, result: Result<Value, RpcError>) {
        let msg = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message } }),
        };
        let _ = self.write(&msg);
    }

    pub fn kill(&self) {
        self.alive.store(false, Ordering::Relaxed);
        let _ = self.child.lock().unwrap_or_else(|e| e.into_inner()).kill();
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        let _ = self.child.lock().unwrap_or_else(|e| e.into_inner()).kill();
    }
}

/// Buang kode warna ANSI (log tracing Rust/Node).
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_dibuang() {
        assert_eq!(strip_ansi("\u{1b}[2m2026\u{1b}[0m \u{1b}[31mERROR\u{1b}[0m x"), "2026 ERROR x");
    }

    #[test]
    fn pesan_galat_memakai_data() {
        let e = RpcError { code: -32603, message: "Internal error".into(), data: Some(json!({"message": "model butuh versi baru"})) };
        assert_eq!(e.describe(), "Internal error (model butuh versi baru)");
        assert!(RpcError::new(-32000, "Authentication required").is_auth());
        assert!(!RpcError::new(-32603, "x").is_auth());
    }

    /// Uji pulang-pergi nyata dengan proses anak yang menggemakan permintaan sebagai balasan.
    #[cfg(windows)]
    #[test]
    fn permintaan_dan_balasan() {
        // PowerShell membaca satu baris JSON lalu membalas {id, result}.
        let script = "$l=[Console]::In.ReadLine(); $o=$l|ConvertFrom-Json; [Console]::Out.WriteLine('{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"x\":1}}'); [Console]::Out.WriteLine('{\"jsonrpc\":\"2.0\",\"id\":'+$o.id+',\"result\":{\"ok\":true}}'); [Console]::Out.Flush(); Start-Sleep -Milliseconds 200";
        let launch = Launch { program: "powershell".into(), args: vec!["-NoProfile".into(), "-Command".into(), script.into()], env_remove: vec![] };
        let notes = Arc::new(Mutex::new(vec![]));
        let n2 = notes.clone();
        let conn = Conn::spawn(&launch, &std::env::temp_dir(), move |m| {
            if let Incoming::Notification { method, .. } = m {
                n2.lock().unwrap().push(method);
            }
        })
        .unwrap();
        let r = conn.request("initialize", json!({}), Some(Duration::from_secs(30))).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(notes.lock().unwrap().as_slice(), ["session/update"]);
        // proses selesai → permintaan berikutnya gagal dengan alasan, tidak menggantung
        std::thread::sleep(Duration::from_millis(800));
        assert!(conn.request("x", json!({}), Some(Duration::from_secs(5))).is_err());
    }
}

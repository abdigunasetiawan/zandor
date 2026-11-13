//! Spike S1 — supervisor untuk `opencode serve` sebagai sidecar.
//!
//! Tanggung jawab:
//! - start: spawn `opencode serve --hostname 127.0.0.1 --port 0` dengan token acak
//!   (`OPENCODE_SERVER_PASSWORD`, HTTP Basic user `opencode`), baca URL dari stdout.
//! - health: `GET /global/health` dengan auth, cek versi sesuai pin.
//! - monitor: thread yang mendeteksi engine mati tak terduga dan memanggil callback.
//! - stop: best-effort `POST /global/dispose`, lalu bunuh seluruh pohon proses.
//! - tanpa proses yatim:
//!   * Windows: proses dibuat `CREATE_SUSPENDED`, dimasukkan ke Job Object
//!     `KILL_ON_JOB_CLOSE`, baru di-resume. Kalau aplikasi crash / di-kill, handle job
//!     tertutup oleh kernel dan seluruh pohon (termasuk cucu: LSP, shell tool) ikut mati.
//!   * Linux: `PR_SET_PDEATHSIG(SIGKILL)` + process group sendiri.
//!   * macOS: process group sendiri; TIDAK ada jaminan saat parent crash (lihat ADR).
//!
//! Sengaja tanpa dependensi Tauri agar bisa diuji dengan `cargo test` biasa.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const BASIC_USER: &str = "opencode";
const LOG_CAP: usize = 500;

#[derive(Debug)]
pub enum EngineError {
    Spawn(std::io::Error),
    NotReady(String),
    Health(String),
    VersionMismatch { expected: String, got: String },
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Spawn(e) => write!(f, "gagal menjalankan engine: {e}"),
            EngineError::NotReady(s) => write!(f, "engine tidak siap: {s}"),
            EngineError::Health(s) => write!(f, "health check gagal: {s}"),
            EngineError::VersionMismatch { expected, got } => {
                write!(f, "versi engine {got}, yang di-pin {expected}")
            }
        }
    }
}
impl std::error::Error for EngineError {}

#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub bin: PathBuf,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub ready_timeout: Duration,
    /// Versi yang di-pin. Kalau `Some`, start gagal bila versi engine berbeda.
    pub expected_version: Option<String>,
}

impl EngineConfig {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            bin: bin.into(),
            cwd: None,
            env: Vec::new(),
            ready_timeout: Duration::from_secs(20),
            expected_version: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EngineInfo {
    pub url: String,
    pub host_port: String,
    pub password: String,
    pub pid: u32,
    pub version: String,
    pub startup: Duration,
}

impl EngineInfo {
    pub fn basic_auth_header(&self) -> String {
        format!("Basic {}", b64(format!("{BASIC_USER}:{}", self.password).as_bytes()))
    }
}

#[derive(Clone, Debug)]
pub struct ExitInfo {
    pub code: Option<i32>,
    pub last_logs: Vec<String>,
}

type ExitCallback = Box<dyn Fn(ExitInfo) + Send + 'static>;

pub struct Engine {
    child: Arc<Mutex<Option<Child>>>,
    info: EngineInfo,
    logs: Arc<Mutex<VecDeque<String>>>,
    stop_requested: Arc<AtomicBool>,
    exited: Arc<AtomicBool>,
    #[cfg(windows)]
    job: Arc<win::Job>,
}

impl Engine {
    pub fn start(
        cfg: EngineConfig,
        on_unexpected_exit: impl Fn(ExitInfo) + Send + 'static,
    ) -> Result<Engine, EngineError> {
        let t0 = Instant::now();
        let password = random_token();

        let mut cmd = Command::new(&cfg.bin);
        cmd.args(["serve", "--hostname", "127.0.0.1", "--port", "0"])
            .env("OPENCODE_SERVER_PASSWORD", &password)
            // Versi di-pin: engine tidak boleh meng-upgrade dirinya sendiri.
            .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in &cfg.env {
            cmd.env(k, v);
        }
        if let Some(cwd) = &cfg.cwd {
            cmd.current_dir(cwd);
        }

        #[cfg(windows)]
        let job = Arc::new(win::Job::new().map_err(EngineError::Spawn)?);
        #[cfg(windows)]
        let mut child = win::spawn_in_job(&mut cmd, &job).map_err(EngineError::Spawn)?;
        #[cfg(unix)]
        let mut child = unix::spawn(&mut cmd).map_err(EngineError::Spawn)?;

        let pid = child.id();
        let logs = Arc::new(Mutex::new(VecDeque::with_capacity(LOG_CAP)));
        let (url_tx, url_rx) = mpsc::channel::<String>();

        // Kuras stdout + stderr terus-menerus (pipe penuh = engine macet).
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        spawn_reader("stdout", stdout, logs.clone(), Some(url_tx));
        spawn_reader("stderr", stderr, logs.clone(), None);

        let child = Arc::new(Mutex::new(Some(child)));
        let stop_requested = Arc::new(AtomicBool::new(false));
        let exited = Arc::new(AtomicBool::new(false));

        let mut engine = Engine {
            child: child.clone(),
            info: EngineInfo {
                url: String::new(),
                host_port: String::new(),
                password,
                pid,
                version: String::new(),
                startup: Duration::ZERO,
            },
            logs: logs.clone(),
            stop_requested: stop_requested.clone(),
            exited: exited.clone(),
            #[cfg(windows)]
            job,
        };

        let url = match url_rx.recv_timeout(cfg.ready_timeout) {
            Ok(u) => u,
            Err(_) => {
                engine.stop();
                return Err(EngineError::NotReady(format!(
                    "tidak ada baris 'listening on' dalam {:?}. Log: {:?}",
                    cfg.ready_timeout,
                    engine.logs()
                )));
            }
        };
        engine.info.host_port = url
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        engine.info.url = url;

        // Server sudah listen; pastikan health 200 dengan token kita.
        let deadline = Instant::now() + cfg.ready_timeout;
        let version = loop {
            match engine.health() {
                Ok(v) => break v,
                Err(e) if Instant::now() > deadline => {
                    engine.stop();
                    return Err(e);
                }
                Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        };
        if let Some(exp) = &cfg.expected_version {
            if exp != &version {
                engine.stop();
                return Err(EngineError::VersionMismatch { expected: exp.clone(), got: version });
            }
        }
        engine.info.version = version;
        engine.info.startup = t0.elapsed();

        // Monitor: deteksi engine mati tanpa diminta.
        let cb: ExitCallback = Box::new(on_unexpected_exit);
        thread::Builder::new()
            .name("engine-monitor".into())
            .spawn(move || loop {
                thread::sleep(Duration::from_millis(250));
                let status = {
                    let mut g = child.lock().unwrap();
                    match g.as_mut() {
                        None => return,
                        Some(c) => c.try_wait(),
                    }
                };
                match status {
                    Ok(None) => continue,
                    Ok(Some(st)) => {
                        exited.store(true, Ordering::SeqCst);
                        if !stop_requested.load(Ordering::SeqCst) {
                            // beri reader waktu menampung baris terakhir
                            thread::sleep(Duration::from_millis(100));
                            cb(ExitInfo {
                                code: st.code(),
                                last_logs: logs.lock().unwrap().iter().cloned().collect(),
                            });
                        }
                        return;
                    }
                    Err(_) => return,
                }
            })
            .expect("spawn monitor");

        Ok(engine)
    }

    pub fn info(&self) -> &EngineInfo {
        &self.info
    }

    pub fn is_running(&self) -> bool {
        !self.exited.load(Ordering::SeqCst) && self.child.lock().unwrap().is_some()
    }

    pub fn logs(&self) -> Vec<String> {
        self.logs.lock().unwrap().iter().cloned().collect()
    }

    /// `GET /global/health` → versi engine.
    pub fn health(&self) -> Result<String, EngineError> {
        let (code, body) = http(
            &self.info.host_port,
            "GET",
            "/global/health",
            Some(&self.info.basic_auth_header()),
            Duration::from_secs(2),
        )
        .map_err(|e| EngineError::Health(e.to_string()))?;
        if code != 200 || !body.contains("\"healthy\":true") {
            return Err(EngineError::Health(format!("HTTP {code}: {body}")));
        }
        Ok(json_str_field(&body, "version").unwrap_or_default())
    }

    /// Simulasi crash engine (hanya proses utama). Untuk test monitor.
    pub fn kill_main_process_for_test(&self) {
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }

    /// Hentikan engine beserta seluruh pohon prosesnya. Idempoten.
    pub fn stop(&mut self) {
        if self.stop_requested.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut g = self.child.lock().unwrap();
        let Some(mut child) = g.take() else { return };

        if !self.info.host_port.is_empty() && matches!(child.try_wait(), Ok(None)) {
            // Beri engine kesempatan merapikan instance (best-effort, 1,5 s).
            let _ = http(
                &self.info.host_port,
                "POST",
                "/global/dispose",
                Some(&self.info.basic_auth_header()),
                Duration::from_millis(1500),
            );
        }

        #[cfg(windows)]
        {
            self.job.terminate();
            let _ = child.wait();
        }
        #[cfg(unix)]
        unix::terminate_group(&mut child, Duration::from_secs(3));

        self.exited.store(true, Ordering::SeqCst);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spawn_reader(
    name: &'static str,
    src: impl Read + Send + 'static,
    logs: Arc<Mutex<VecDeque<String>>>,
    url_tx: Option<mpsc::Sender<String>>,
) {
    thread::Builder::new()
        .name(format!("engine-{name}"))
        .spawn(move || {
            let mut url_tx = url_tx;
            for line in BufReader::new(src).lines() {
                let Ok(line) = line else { break };
                if let Some(tx) = &url_tx {
                    if let Some(idx) = line.find("listening on ") {
                        let url = line[idx + "listening on ".len()..].trim().to_string();
                        let _ = tx.send(url);
                        url_tx = None;
                    }
                }
                let mut l = logs.lock().unwrap();
                if l.len() == LOG_CAP {
                    l.pop_front();
                }
                l.push_back(format!("[{name}] {line}"));
            }
        })
        .expect("spawn reader");
}

fn random_token() -> String {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).expect("OS RNG");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// HTTP/1.1 minimal (hanya loopback, tanpa TLS) supaya spike tidak butuh crate HTTP.
pub fn http(
    host_port: &str,
    method: &str,
    path: &str,
    auth: Option<&str>,
    timeout: Duration,
) -> std::io::Result<(u16, String)> {
    http_json(host_port, method, path, auth, None, timeout)
}

/// Sama seperti [`http`] tetapi dengan body JSON opsional.
pub fn http_json(
    host_port: &str,
    method: &str,
    path: &str,
    auth: Option<&str>,
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<(u16, String)> {
    let addr = host_port
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other("alamat tidak valid"))?;
    let mut s = TcpStream::connect_timeout(&addr, timeout)?;
    s.set_read_timeout(Some(timeout))?;
    s.set_write_timeout(Some(timeout))?;
    let payload = body.unwrap_or("");
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        payload.len()
    );
    if body.is_some() {
        req.push_str("Content-Type: application/json\r\n");
    }
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: {a}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(payload);
    s.write_all(req.as_bytes())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    let code = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let body = text.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
    Ok((code, body))
}

fn json_str_field(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":\"");
    let i = body.find(&pat)? + pat.len();
    let j = body[i..].find('"')?;
    Some(body[i..i + j].to_string())
}

fn b64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(windows)]
mod win {
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};

    #[link(name = "ntdll")]
    extern "system" {
        fn NtResumeProcess(process: HANDLE) -> i32;
    }

    pub struct Job(HANDLE);
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> std::io::Result<Self> {
            unsafe {
                let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if h.is_null() {
                    return Err(std::io::Error::last_os_error());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    let e = std::io::Error::last_os_error();
                    CloseHandle(h);
                    return Err(e);
                }
                Ok(Job(h))
            }
        }

        pub fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    /// Spawn tersuspensi → masuk job → resume. Tidak ada jendela di mana engine (atau
    /// anaknya) berjalan di luar job.
    pub fn spawn_in_job(cmd: &mut Command, job: &Job) -> std::io::Result<Child> {
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = cmd.spawn()?;
        let h = child.as_raw_handle() as HANDLE;
        unsafe {
            if AssignProcessToJobObject(job.0, h) == 0 {
                let e = std::io::Error::last_os_error();
                let _ = child.kill();
                return Err(e);
            }
            let st = NtResumeProcess(h);
            if st < 0 {
                let _ = child.kill();
                return Err(std::io::Error::other(format!("NtResumeProcess NTSTATUS {st:#x}")));
            }
        }
        Ok(child)
    }
}

#[cfg(unix)]
mod unix {
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};
    use std::time::{Duration, Instant};

    pub fn spawn(cmd: &mut Command) -> std::io::Result<Child> {
        unsafe {
            cmd.pre_exec(|| {
                // Process group sendiri → bisa mematikan seluruh pohon sekaligus.
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // Linux: kernel kirim SIGKILL ke engine kalau thread parent mati.
                // Catatan: PDEATHSIG terikat ke *thread* pembuat, jadi spawn harus dari
                // thread yang hidup sepanjang aplikasi (atau thread utama).
                #[cfg(target_os = "linux")]
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        cmd.spawn()
    }

    pub fn terminate_group(child: &mut Child, grace: Duration) {
        let pgid = child.id() as i32;
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_rfc4648() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"opencode:x"), "b3BlbmNvZGU6eA==");
    }

    #[test]
    fn token_is_random_64_hex() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn parses_json_field() {
        assert_eq!(
            json_str_field(r#"{"healthy":true,"version":"1.19.5"}"#, "version").as_deref(),
            Some("1.19.5")
        );
    }
}

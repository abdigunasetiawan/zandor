//! Klien HTTP + SSE ke engine opencode — port packages/core/src/engine/api.ts (jalur & bentuk sama dengan
//! SDK @opencode-ai/sdk v2 yang di-pin). Blocking (ureq); panggil dari background executor.
#![allow(dead_code)]
use super::attachments::{attachment_parts, FileAttachment};
use super::types::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionInfo {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub created: f64,
    pub updated: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionFileDiff {
    pub file: String,
    pub patch: String,
    pub additions: u32,
    pub deletions: u32,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub absolute: String,
    pub is_dir: bool,
    pub ignored: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ModelOption {
    pub provider_id: String,
    pub provider_name: String,
    pub model_id: String,
    pub name: String,
    pub variants: Vec<String>,
    pub context_limit: Option<u64>,
    pub output_limit: Option<u64>,
    pub reasoning: Option<bool>,
    pub tool_call: Option<bool>,
    pub attachment: Option<bool>,
    pub inputs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EngineCommand {
    pub name: String,
    pub description: Option<String>,
    pub source: Option<String>,
    pub hints: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelRef {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CatalogProvider {
    pub id: String,
    pub name: String,
    pub env: Vec<String>,
    pub connected: bool,
    pub model_count: usize,
}

#[derive(Debug)]
pub struct EngineError(pub String);
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EngineError {}
pub type Result<T> = std::result::Result<T, EngineError>;

/// Judul bawaan engine ("New session - 2026-…") = belum diberi nama.
pub fn is_default_session_title(title: &str) -> bool {
    let Some(rest) = title.strip_prefix("New session - ") else { return false };
    let b = rest.as_bytes();
    b.len() >= 11
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'T'
}

fn b64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in input.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn basic_auth_header(info: &ConnectionInfo) -> String {
    format!("Basic {}", b64(format!("{}:{}", info.username, info.password).as_bytes()))
}

fn q(v: &str) -> String {
    let mut o = String::new();
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => o.push(b as char),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

fn sv(o: &Value, k: &str) -> String {
    o.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// Teks balasan asisten dari respons `POST /session/:id/message` (`{info, parts}`): gabungan bagian bertipe teks.
fn reply_text(v: &Value) -> String {
    v.get("parts")
        .and_then(Value::as_array)
        .map(|ps| ps.iter().filter(|p| p.get("type").and_then(Value::as_str) == Some("text")).filter_map(|p| p.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("
"))
        .unwrap_or_default()
}

fn to_session(v: &Value) -> SessionInfo {
    let t = v.get("time").cloned().unwrap_or(Value::Null);
    SessionInfo {
        id: sv(v, "id"),
        title: sv(v, "title"),
        directory: sv(v, "directory"),
        created: t.get("created").and_then(Value::as_f64).unwrap_or(0.),
        updated: t.get("updated").and_then(Value::as_f64).unwrap_or(0.),
    }
}

fn arr(v: Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a,
        _ => vec![],
    }
}

/// Rute engine remote: kunci path project → (URL dasar, header Authorization). Port klien gabungan (remote.ts):
/// permintaan untuk directory project yang memakai engine remote dikirim ke engine itu, sisanya ke engine lokal.
pub type Routes = Arc<std::sync::RwLock<Vec<(String, String, String)>>>;

fn tls_agent(timeout: Option<Duration>, status_err: bool) -> ureq::Agent {
    // Hanya fitur native-tls yang aktif: penyedia TLS harus dipilih eksplisit agar https (engine remote) berfungsi.
    ureq::Agent::config_builder()
        .timeout_global(timeout)
        .http_status_as_error(status_err)
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

#[derive(Clone)]
pub struct EngineApi {
    info: ConnectionInfo,
    auth: String,
    agent: ureq::Agent,
    routes: Routes,
}

impl EngineApi {
    pub fn new(info: ConnectionInfo) -> Self {
        Self { auth: basic_auth_header(&info), info, agent: tls_agent(Some(Duration::from_secs(120)), false), routes: Default::default() }
    }

    /// Klien untuk satu engine saja (tanpa rute), mis. untuk cek health engine remote.
    pub fn single(info: ConnectionInfo) -> Self {
        let mut a = Self::new(info);
        a.agent = tls_agent(Some(Duration::from_secs(15)), false);
        a
    }

    /// Ganti tabel rute (dibagi antar klon).
    pub fn set_routes(&self, list: Vec<(String, ConnectionInfo)>) {
        let v = list.into_iter().map(|(k, i)| (k, i.url.trim_end_matches('/').to_string(), basic_auth_header(&i))).collect();
        if let Ok(mut r) = self.routes.write() {
            *r = v;
        }
    }

    /// (URL dasar, auth) untuk directory.
    fn target(&self, dir: Option<&str>) -> (String, String) {
        if let Some(d) = dir {
            let k = super::projects::path_key(d);
            if let Ok(r) = self.routes.read() {
                // port routeOf: project yang memuat direktori (atau sama persis), path terpanjang menang
                let inside = |p: &str| {
                    let base = p.trim_end_matches(['/', '\\']);
                    k == p || k.starts_with(&format!("{base}/")) || k.starts_with(&format!("{base}\\"))
                };
                if let Some((_, u, a)) = r.iter().filter(|(p, _, _)| inside(p)).max_by_key(|(p, _, _)| p.len()) {
                    return (u.clone(), a.clone());
                }
            }
        }
        (self.info.url.trim_end_matches('/').to_string(), self.auth.clone())
    }

    fn auth_for(&self, url: &str) -> String {
        if let Ok(r) = self.routes.read() {
            if let Some((_, _, a)) = r.iter().find(|(_, u, _)| url.starts_with(&format!("{u}/"))) {
                return a.clone();
            }
        }
        self.auth.clone()
    }

    fn url(&self, path: &str, dir: Option<&str>, extra: &[(&str, String)]) -> String {
        let mut u = format!("{}{}", self.target(dir).0, path);
        let mut qs: Vec<String> = Vec::new();
        if let Some(d) = dir {
            qs.push(format!("directory={}", q(d)));
        }
        for (k, v) in extra {
            qs.push(format!("{k}={}", q(v)));
        }
        if !qs.is_empty() {
            u.push('?');
            u.push_str(&qs.join("&"));
        }
        u
    }

    fn call(&self, method: &str, url: &str, body: Option<Value>) -> Result<Value> {
        let e = |x: ureq::Error| EngineError(format!("{method} {url}: {x}"));
        let auth = self.auth_for(url);
        let mut resp = match (method, body) {
            ("GET", _) => self.agent.get(url).header("Authorization", &auth).call().map_err(e)?,
            ("DELETE", _) => self.agent.delete(url).header("Authorization", &auth).call().map_err(e)?,
            (m, b) => {
                let req = match m {
                    "PATCH" => self.agent.patch(url),
                    "PUT" => self.agent.put(url),
                    _ => self.agent.post(url),
                };
                req.header("Authorization", &auth).send_json(b.unwrap_or(json!({}))).map_err(e)?
            }
        };
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(EngineError(format!("engine {status} untuk {method} {url}: {text}")));
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|x| EngineError(format!("JSON tidak valid dari engine: {x}")))
    }

    fn get(&self, path: &str, dir: Option<&str>, extra: &[(&str, String)]) -> Result<Value> {
        self.call("GET", &self.url(path, dir, extra), None)
    }
    fn post(&self, path: &str, dir: Option<&str>, body: Value) -> Result<Value> {
        self.call("POST", &self.url(path, dir, &[]), Some(body))
    }

    pub fn health(&self) -> Result<String> {
        Ok(sv(&self.get("/global/health", None, &[])?, "version"))
    }
    pub fn list_sessions(&self, dir: &str) -> Result<Vec<SessionInfo>> {
        let acp = super::acp::host();
        Ok(arr(self.get("/session", Some(dir), &[])?)
            .iter()
            .map(to_session)
            .map(|mut s| {
                // Giliran agent ACP tidak menyentuh waktu sesi di engine: pakai aktivitas ACP terakhir bila lebih baru.
                if let Some(t) = acp.last_activity(&s.id) {
                    s.updated = s.updated.max(t);
                }
                s
            })
            .collect())
    }
    pub fn create_session(&self, dir: &str, title: Option<&str>) -> Result<SessionInfo> {
        let body = title.map(|t| json!({ "title": t })).unwrap_or(json!({}));
        Ok(to_session(&self.post("/session", Some(dir), body)?))
    }
    pub fn rename_session(&self, dir: &str, id: &str, title: &str) -> Result<SessionInfo> {
        let u = self.url(&format!("/session/{id}"), Some(dir), &[]);
        Ok(to_session(&self.call("PATCH", &u, Some(json!({ "title": title })))?))
    }
    // ---- PTY engine (ADR-024): terminal hidup di engine, pulih setelah aplikasi dimuat ulang.
    pub fn pty_list(&self, dir: &str) -> Result<Vec<PtyInfo>> {
        Ok(arr(self.get("/pty", Some(dir), &[])?).iter().filter_map(to_pty).collect())
    }
    pub fn pty_create(&self, dir: &str, command: &str, args: &[String], title: Option<&str>) -> Result<PtyInfo> {
        let mut body = json!({ "command": command, "cwd": dir });
        if !args.is_empty() {
            body["args"] = json!(args);
        }
        if let Some(t) = title {
            body["title"] = json!(t);
        }
        to_pty(&self.post("/pty", Some(dir), body)?).ok_or_else(|| EngineError("respons /pty tidak dikenali".into()))
    }
    pub fn pty_resize(&self, dir: &str, id: &str, cols: u16, rows: u16) -> Result<()> {
        self.call("PUT", &self.url(&format!("/pty/{}", q(id)), Some(dir), &[]), Some(json!({ "size": { "cols": cols, "rows": rows } }))).map(|_| ())
    }
    pub fn pty_remove(&self, dir: &str, id: &str) -> Result<()> {
        self.call("DELETE", &self.url(&format!("/pty/{}", q(id)), Some(dir), &[]), None).map(|_| ())
    }
    /// URL `ws://…/connect` siap pakai (tiket sekali pakai sudah diminta). None bila engine bukan http polos.
    pub fn pty_connect_url(&self, dir: &str, id: &str, cursor: u64) -> Result<String> {
        let u = self.url(&format!("/pty/{}/connect-token", q(id)), Some(dir), &[]);
        let auth = self.auth_for(&u);
        let mut resp = self.agent.post(&u).header("Authorization", &auth).header("x-opencode-ticket", "1").send_json(json!({})).map_err(|x| EngineError(format!("POST {u}: {x}")))?;
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let ticket = v["ticket"].as_str().filter(|t| !t.is_empty()).ok_or_else(|| EngineError("engine tidak memberi tiket terminal".into()))?;
        let base = self.target(Some(dir)).0;
        let ws = base
            .strip_prefix("http://")
            .map(|r| format!("ws://{r}"))
            .or_else(|| base.strip_prefix("https://").map(|r| format!("wss://{r}")))
            .ok_or_else(|| EngineError(format!("URL engine tidak dikenal: {base}")))?;
        Ok(format!("{ws}/pty/{}/connect?ticket={}&cursor={cursor}&directory={}", q(id), q(ticket), q(dir)))
    }
    pub fn delete_session(&self, dir: &str, id: &str) -> Result<()> {
        self.call("DELETE", &self.url(&format!("/session/{id}"), Some(dir), &[]), None)?;
        super::acp::host().forget(id);
        Ok(())
    }
    pub fn list_files(&self, dir: &str, path: &str) -> Result<Vec<FileNode>> {
        Ok(arr(self.get("/file", Some(dir), &[("path", path.into())])?)
            .iter()
            .map(|n| FileNode {
                name: sv(n, "name"),
                path: sv(n, "path"),
                absolute: sv(n, "absolute"),
                is_dir: sv(n, "type") == "directory",
                ignored: n.get("ignored").and_then(Value::as_bool).unwrap_or(false),
            })
            .collect())
    }
    pub fn list_models(&self) -> Result<Vec<ModelOption>> {
        let r = self.get("/config/providers", None, &[])?;
        let mut out = Vec::new();
        for p in r.get("providers").and_then(Value::as_array).into_iter().flatten() {
            for m in p.get("models").and_then(Value::as_object).into_iter().flat_map(|o| o.values()) {
                let caps = m.get("capabilities");
                let limit = m.get("limit");
                out.push(ModelOption {
                    provider_id: sv(p, "id"),
                    provider_name: sv(p, "name"),
                    model_id: sv(m, "id"),
                    name: sv(m, "name"),
                    variants: m.get("variants").and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default(),
                    context_limit: limit.and_then(|l| l.get("context")).and_then(Value::as_u64).filter(|v| *v > 0),
                    output_limit: limit.and_then(|l| l.get("output")).and_then(Value::as_u64).filter(|v| *v > 0),
                    reasoning: caps.map(|c| c.get("reasoning").and_then(Value::as_bool).unwrap_or(false)),
                    tool_call: caps.map(|c| c.get("toolcall").and_then(Value::as_bool).unwrap_or(false)),
                    attachment: caps.map(|c| c.get("attachment").and_then(Value::as_bool).unwrap_or(false)),
                    inputs: caps
                        .and_then(|c| c.get("input"))
                        .and_then(Value::as_object)
                        .map(|o| o.iter().filter(|(_, v)| v.as_bool() == Some(true)).map(|(k, _)| k.clone()).collect())
                        .unwrap_or_default(),
                });
            }
        }
        // Model agent CLI (ACP) yang diaktifkan pengguna: memakai login langganan CLI masing-masing.
        out.extend(super::acp::host().models());
        Ok(out)
    }
    /// Pesan sesi: dari engine, digabung dengan giliran agent ACP (urut menurut id menaik).
    pub fn list_messages(&self, dir: &str, id: &str) -> Result<Vec<ChatMessage>> {
        let mut msgs: Vec<ChatMessage> = arr(self.get(&format!("/session/{id}/message"), Some(dir), &[])?).iter().filter_map(to_message).collect();
        let acp = super::acp::host().messages(id);
        if !acp.is_empty() {
            msgs.extend(acp);
            msgs.sort_by(|a, b| a.info.id.cmp(&b.info.id));
        }
        Ok(msgs)
    }
    pub fn list_permissions(&self, dir: &str) -> Result<Vec<PermissionRequest>> {
        let mut v: Vec<PermissionRequest> = arr(self.get("/permission", Some(dir), &[])?).iter().filter_map(to_permission).collect();
        v.extend(super::acp::host().permissions(dir));
        Ok(v)
    }
    pub fn list_questions(&self, dir: &str) -> Result<Vec<QuestionRequest>> {
        Ok(arr(self.get("/question", Some(dir), &[])?).iter().filter_map(to_question).collect())
    }
    /// Status jalan sesi menurut engine (`GET /session/status`: hanya sesi yang sedang jalan yang tercantum).
    /// Ini sumber kebenaran: pesan asisten tanpa `completed` bisa saja yatim karena engine sempat dimatikan.
    pub fn session_status(&self, dir: &str, id: &str) -> Result<SessionRunStatus> {
        if super::acp::host().busy(id) {
            return Ok(SessionRunStatus::Busy);
        }
        let v = self.get("/session/status", Some(dir), &[])?;
        Ok(v.get(id).and_then(Value::as_object).map(parse_run_status).unwrap_or(SessionRunStatus::Idle))
    }
    pub fn list_todos(&self, dir: &str, id: &str) -> Result<Vec<Todo>> {
        if let Some(t) = super::acp::host().todos(id) {
            return Ok(t);
        }
        Ok(to_todos(Some(&self.get(&format!("/session/{id}/todo"), Some(dir), &[])?)))
    }
    pub fn reply_question(&self, dir: &str, req: &str, answers: Vec<Vec<String>>) -> Result<()> {
        self.post(&format!("/question/{req}/reply"), Some(dir), json!({ "answers": answers })).map(|_| ())
    }
    pub fn reject_question(&self, dir: &str, req: &str) -> Result<()> {
        self.post(&format!("/question/{req}/reject"), Some(dir), json!({})).map(|_| ())
    }
    pub fn send_prompt(
        &self,
        dir: &str,
        id: &str,
        text: &str,
        model: Option<&ModelRef>,
        variant: Option<&str>,
        files: &[FileAttachment],
    ) -> Result<()> {
        self.send_prompt_with(dir, id, text, model, variant, files, &[])
    }
    /// Seperti `send_prompt`, plus teks sintetis tambahan (tidak tampil sebagai teks pengguna; mis. catatan lanjutan).
    #[allow(clippy::too_many_arguments)]
    pub fn send_prompt_with(
        &self,
        dir: &str,
        id: &str,
        text: &str,
        model: Option<&ModelRef>,
        variant: Option<&str>,
        files: &[FileAttachment],
        synthetic: &[String],
    ) -> Result<()> {
        let acp = super::acp::host();
        if let Some(m) = model.filter(|m| super::acp::is_acp_provider(&m.provider_id)) {
            // Agent ACP: riwayat sesi (engine + ACP) dioper bila agent belum melihatnya.
            let history = self.list_messages(dir, id).unwrap_or_default();
            // OpenCode tidak melihat giliran ACP, jadi tidak membuat judul dari pesan pertama. Beri judul sementara
            // dari pesan itu (hanya bila judul masih bawaan); event session.updated menandainya sebagai judul
            // otomatis sehingga penamaan ulang Zandor (autotitle) tetap bisa memperbaikinya.
            if !history.iter().any(|m| m.info.role == super::types::Role::User) {
                if let Some(title) = super::acp::convert::provisional_title(text) {
                    let (api, dir, id) = (self.clone(), dir.to_string(), id.to_string());
                    std::thread::spawn(move || {
                        let current = api.get(&format!("/session/{id}"), Some(&dir), &[]).ok().and_then(|v| v["title"].as_str().map(str::to_string));
                        if current.as_deref().is_some_and(is_default_session_title) {
                            let _ = api.rename_session(&dir, &id, &title);
                        }
                    });
                }
            }
            return acp.prompt(dir, id, text, &m.provider_id, &m.model_id, variant, files, synthetic, history).map_err(EngineError);
        }
        let mut parts = vec![json!({ "type": "text", "text": text })];
        parts.extend(attachment_parts(files));
        parts.extend(synthetic.iter().map(|s| json!({ "type": "text", "synthetic": true, "text": s })));
        // Kembali dari agent ACP ke model engine: giliran ACP yang belum dilihat engine dioper sebagai catatan.
        if let Some(note) = acp.opencode_handoff(id) {
            parts.push(json!({ "type": "text", "synthetic": true, "text": note }));
        }
        let mut body = json!({ "parts": parts });
        if let Some(m) = model {
            body["model"] = json!({ "providerID": m.provider_id, "modelID": m.model_id });
        }
        if let Some(v) = variant {
            body["variant"] = json!(v);
        }
        self.post(&format!("/session/{id}/prompt_async"), Some(dir), body).map(|_| ())
    }
    pub fn session_diff(&self, dir: &str, id: &str) -> Result<Vec<SessionFileDiff>> {
        self.session_diff_for(dir, id, None)
    }
    /// Diff satu giliran (`message_id` = pesan pengguna pembuka giliran); None = seluruh sesi.
    pub fn session_diff_for(&self, dir: &str, id: &str, message_id: Option<&str>) -> Result<Vec<SessionFileDiff>> {
        let extra: Vec<(&str, String)> = message_id.map(|m| ("messageID", m.to_string())).into_iter().collect();
        Ok(arr(self.get(&format!("/session/{id}/diff"), Some(dir), &extra)?)
            .iter()
            .map(|d| SessionFileDiff {
                file: sv(d, "file"),
                patch: sv(d, "patch"),
                additions: d.get("additions").and_then(Value::as_u64).unwrap_or(0) as u32,
                deletions: d.get("deletions").and_then(Value::as_u64).unwrap_or(0) as u32,
                status: d.get("status").and_then(Value::as_str).unwrap_or("modified").into(),
            })
            .collect())
    }
    pub fn get_revert(&self, dir: &str, id: &str) -> Result<Option<String>> {
        let r = self.get(&format!("/session/{id}"), Some(dir), &[])?;
        Ok(r.get("revert").and_then(|x| x.get("messageID")).and_then(Value::as_str).map(String::from))
    }
    /// Waktu dibuat pesan engine pertama yang dibuat sejak `t` (untuk memotong bagian engine di sesi campuran).
    fn engine_message_from(&self, dir: &str, id: &str, t: f64) -> Option<String> {
        arr(self.get(&format!("/session/{id}/message"), Some(dir), &[]).ok()?).iter().filter_map(to_message).filter(|m| m.info.created >= t).min_by(|a, b| a.info.id.cmp(&b.info.id)).map(|m| m.info.id)
    }
    pub fn revert(&self, dir: &str, id: &str, message_id: &str) -> Result<()> {
        let acp = super::acp::host();
        if acp.owns_message(id, message_id) {
            // Pesan agent ACP: potong transkrip lokal (tidak bisa di-unrevert) dan bagian engine sesudahnya.
            let t = acp.message_created(id, message_id).unwrap_or(0.);
            acp.truncate(id, Some(message_id), None);
            if let Some(m) = self.engine_message_from(dir, id, t) {
                self.post(&format!("/session/{id}/revert"), Some(dir), json!({ "messageID": m }))?;
            }
            return Ok(());
        }
        self.post(&format!("/session/{id}/revert"), Some(dir), json!({ "messageID": message_id }))?;
        // Giliran ACP sesudah pesan ini ikut dibuang.
        if let Some(t) = self.list_messages(dir, id).ok().and_then(|v| v.into_iter().find(|m| m.info.id == message_id)).map(|m| m.info.created) {
            acp.truncate(id, None, Some(t));
        }
        Ok(())
    }
    pub fn delete_message(&self, dir: &str, id: &str, message_id: &str) -> Result<()> {
        let acp = super::acp::host();
        if acp.owns_message(id, message_id) {
            // Transkrip ACP hanya bisa dipotong dari satu pesan ke akhir. Rewind menghapus dari pesan terakhir ke awal,
            // jadi memotong dari pesan ini sama dengan menghapusnya.
            acp.truncate(id, Some(message_id), None);
            return Ok(());
        }
        self.call("DELETE", &self.url(&format!("/session/{id}/message/{message_id}"), Some(dir), &[]), None).map(|_| ())
    }
    pub fn unrevert(&self, dir: &str, id: &str) -> Result<()> {
        self.post(&format!("/session/{id}/unrevert"), Some(dir), json!({})).map(|_| ())
    }
    pub fn fork(&self, dir: &str, id: &str, message_id: Option<&str>) -> Result<SessionInfo> {
        let acp = super::acp::host();
        // Batas cabang: waktu pesan itu. Pesan ACP bukan milik engine, jadi engine diberi pesan engine pertama sesudahnya.
        let cutoff = message_id.and_then(|m| self.list_messages(dir, id).ok()?.into_iter().find(|x| x.info.id == m).map(|x| x.info.created));
        let engine_mid = match (message_id, cutoff) {
            (Some(m), Some(t)) if acp.owns_message(id, m) => self.engine_message_from(dir, id, t),
            (m, _) => m.map(String::from),
        };
        let body = engine_mid.map(|m| json!({ "messageID": m })).unwrap_or(json!({}));
        let s = to_session(&self.post(&format!("/session/{id}/fork"), Some(dir), body)?);
        acp.copy_for_fork(id, &s.id, dir, cutoff);
        Ok(s)
    }
    pub fn find_files(&self, dir: &str, query: &str, limit: u32) -> Result<Vec<String>> {
        let r = self.get("/find/file", Some(dir), &[("query", query.into()), ("dirs", "false".into()), ("limit", limit.to_string())])?;
        Ok(arr(r).into_iter().filter_map(|v| v.as_str().map(String::from)).collect())
    }
    pub fn list_commands(&self, dir: &str) -> Result<Vec<EngineCommand>> {
        Ok(arr(self.get("/command", Some(dir), &[])?)
            .iter()
            .map(|c| EngineCommand {
                name: sv(c, "name"),
                description: c.get("description").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from),
                source: c.get("source").and_then(Value::as_str).map(String::from),
                hints: c.get("hints").and_then(Value::as_array).map(|a| a.iter().filter_map(|h| h.as_str().map(String::from)).collect()).unwrap_or_default(),
            })
            .collect())
    }
    pub fn run_command(
        &self,
        dir: &str,
        id: &str,
        command: &str,
        arguments: &str,
        model: Option<&ModelRef>,
        variant: Option<&str>,
        files: &[FileAttachment],
    ) -> Result<()> {
        if model.is_some_and(|m| super::acp::is_acp_provider(&m.provider_id)) {
            // Agent ACP menangani perintah garis miring sendiri (Claude Code, Codex, Antigravity).
            let text = if arguments.trim().is_empty() { format!("/{command}") } else { format!("/{command} {arguments}") };
            return self.send_prompt(dir, id, &text, model, variant, files);
        }
        let all = attachment_parts(files);
        let file_parts: Vec<Value> = all.iter().filter(|p| p["type"] == "file").cloned().collect();
        let notes: Vec<String> = all.iter().filter(|p| p["type"] == "text").filter_map(|p| p["text"].as_str().map(String::from)).collect();
        let mut args: Vec<String> = vec![arguments.to_string()];
        args.extend(notes);
        let args: Vec<String> = args.into_iter().filter(|s| !s.is_empty()).collect();
        let mut body = json!({ "command": command, "arguments": args.join("\n\n") });
        if let Some(m) = model {
            body["model"] = json!(format!("{}/{}", m.provider_id, m.model_id));
        }
        if let Some(v) = variant {
            body["variant"] = json!(v);
        }
        if !file_parts.is_empty() {
            body["parts"] = json!(file_parts);
        }
        self.post(&format!("/session/{id}/command"), Some(dir), body).map(|_| ())
    }
    /// Satu panggilan model sinkron (mis. membuat judul): sesi sementara berjudul eksplisit (agar engine tidak membuat
    /// judul otomatis untuknya) → `POST /session/:id/message` (menunggu balasan) → sesi selalu dihapus lagi.
    pub fn complete_once(&self, dir: &str, prompt: &str, model: Option<&ModelRef>) -> Result<String> {
        if let Some(m) = model.filter(|m| super::acp::is_acp_provider(&m.provider_id)) {
            return super::acp::host().complete_once(&m.provider_id, &m.model_id, prompt).map_err(EngineError);
        }
        let s = self.create_session(dir, Some("Zandor · tugas singkat"))?;
        let run = || {
            let mut body = json!({ "parts": [{ "type": "text", "text": prompt }] });
            if let Some(m) = model {
                body["model"] = json!({ "providerID": m.provider_id, "modelID": m.model_id });
            }
            self.post(&format!("/session/{}/message", s.id), Some(dir), body).map(|v| reply_text(&v))
        };
        let out = run();
        let _ = self.delete_session(dir, &s.id);
        out
    }
    pub fn summarize(&self, dir: &str, id: &str, model: &ModelRef) -> Result<()> {
        if super::acp::is_acp_provider(&model.provider_id) {
            // Agent ACP mengelola konteksnya sendiri (Claude Code/Codex meringkas otomatis, atau perintah /compact).
            return self.send_prompt(dir, id, "/compact", Some(model), None, &[]);
        }
        self.post(&format!("/session/{id}/summarize"), Some(dir), json!({ "providerID": model.provider_id, "modelID": model.model_id })).map(|_| ())
    }
    pub fn abort(&self, dir: &str, id: &str) -> Result<()> {
        if super::acp::host().cancel(id) {
            return Ok(());
        }
        self.post(&format!("/session/{id}/abort"), Some(dir), json!({})).map(|_| ())
    }
    pub fn dispose_instance(&self, dir: &str) -> Result<()> {
        self.post("/instance/dispose", Some(dir), json!({})).map(|_| ())
    }
    pub fn reply_permission(&self, dir: &str, req: &str, reply: PermissionReply) -> Result<()> {
        if super::acp::host::AcpHost::owns_permission(req) {
            return super::acp::host().reply_permission(req, reply).map_err(EngineError);
        }
        self.post(&format!("/permission/{req}/reply"), Some(dir), json!({ "reply": reply.as_str() })).map(|_| ())
    }
    pub fn list_catalog(&self) -> Result<Vec<CatalogProvider>> {
        let r = self.get("/provider", None, &[])?;
        let connected: Vec<String> = r.get("connected").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        Ok(r.get("all")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|p| {
                let id = sv(p, "id");
                CatalogProvider {
                    connected: connected.contains(&id),
                    name: sv(p, "name"),
                    env: p.get("env").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
                    model_count: p.get("models").and_then(Value::as_object).map(|o| o.len()).unwrap_or(0),
                    id,
                }
            })
            .collect())
    }
    pub fn list_auth_methods(&self) -> Result<Value> {
        self.get("/provider/auth", None, &[])
    }
    pub fn oauth_authorize(&self, provider: &str, method: u32, inputs: Option<Value>) -> Result<(String, String, String)> {
        let mut body = json!({ "method": method });
        if let Some(i) = inputs {
            body["inputs"] = i;
        }
        let d = self.post(&format!("/provider/{provider}/oauth/authorize"), None, body)?;
        let m = if sv(&d, "method") == "code" { "code" } else { "auto" };
        Ok((sv(&d, "url"), m.into(), sv(&d, "instructions")))
    }
    pub fn oauth_callback(&self, provider: &str, method: u32, code: Option<&str>) -> Result<()> {
        let mut body = json!({ "method": method });
        if let Some(c) = code {
            body["code"] = json!(c);
        }
        self.post(&format!("/provider/{provider}/oauth/callback"), None, body).map(|_| ())
    }

    /// Berlangganan SSE satu direktori dengan reconnect otomatis (500ms → maks 5s).
    /// `on_state(true)` setiap (re)koneksi berhasil: pemanggil wajib sinkron ulang.
    /// Kembalikan flag berhenti.
    pub fn subscribe(
        &self,
        dir: &str,
        on_event: impl Fn(EngineEvent) + Send + Sync + 'static,
        on_state: impl Fn(bool) + Send + 'static,
    ) -> Arc<AtomicBool> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        // Event agent ACP untuk direktori ini lewat jalur yang sama.
        let on_event: Arc<dyn Fn(EngineEvent) + Send + Sync> = Arc::new(on_event);
        super::acp::host().subscribe(dir, stop.clone(), on_event.clone());
        let url = self.url("/event", Some(dir), &[]);
        let auth = self.target(Some(dir)).1;
        std::thread::Builder::new()
            .name("zandor-sse".into())
            .spawn(move || {
                let agent = tls_agent(None, true);
                let mut delay = 500u64;
                while !stop2.load(Ordering::Relaxed) {
                    if let Ok(resp) = agent.get(&url).header("Authorization", &auth).header("Accept", "text/event-stream").call() {
                        delay = 500;
                        on_state(true);
                        let reader = BufReader::new(resp.into_body().into_reader());
                        let mut data = String::new();
                        for line in reader.lines() {
                            if stop2.load(Ordering::Relaxed) {
                                return;
                            }
                            let Ok(line) = line else { break };
                            if line.is_empty() {
                                if !data.is_empty() {
                                    if let Ok(v) = serde_json::from_str::<Value>(&data) {
                                        // Bentuk event: {type, properties} atau dibungkus {payload:{...}}.
                                        let v = v.get("payload").cloned().unwrap_or(v);
                                        if let Some(ev) = to_engine_event(&v) {
                                            on_event(ev);
                                        }
                                    }
                                    data.clear();
                                }
                            } else if let Some(d) = line.strip_prefix("data:") {
                                if !data.is_empty() {
                                    data.push('\n');
                                }
                                data.push_str(d.trim_start());
                            }
                        }
                    }
                    if stop2.load(Ordering::Relaxed) {
                        return;
                    }
                    on_state(false);
                    std::thread::sleep(Duration::from_millis(delay));
                    delay = (delay * 2).min(5000);
                }
            })
            .expect("thread SSE");
        stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn judul_bawaan() {
        assert!(is_default_session_title("New session - 2026-11-18T01:02:03.000Z"));
        assert!(!is_default_session_title("Rapat"));
        let r = json!({"info":{"id":"m"},"parts":[{"type":"reasoning","text":"abaikan"},{"type":"text","text":"Judul A"},{"type":"text","text":"baris 2"}]});
        assert_eq!(reply_text(&r), "Judul A
baris 2");
        assert_eq!(reply_text(&json!({})), "");
    }
    #[test]
    fn auth() {
        let i = ConnectionInfo { url: "x".into(), username: "opencode".into(), password: "pw".into() };
        assert_eq!(basic_auth_header(&i), "Basic b3BlbmNvZGU6cHc=");
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PtyInfo {
    pub id: String,
    pub title: String,
    pub command: String,
    pub cwd: String,
    pub running: bool,
}

fn to_pty(v: &Value) -> Option<PtyInfo> {
    let id = v["id"].as_str()?.to_string();
    Some(PtyInfo {
        id,
        title: v["title"].as_str().unwrap_or("terminal").to_string(),
        command: sv(v, "command"),
        cwd: sv(v, "cwd"),
        running: v["status"].as_str() != Some("exited"),
    })
}

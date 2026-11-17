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
                if let Some((_, u, a)) = r.iter().find(|(p, _, _)| *p == k) {
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
        Ok(arr(self.get("/session", Some(dir), &[])?).iter().map(to_session).collect())
    }
    pub fn create_session(&self, dir: &str, title: Option<&str>) -> Result<SessionInfo> {
        let body = title.map(|t| json!({ "title": t })).unwrap_or(json!({}));
        Ok(to_session(&self.post("/session", Some(dir), body)?))
    }
    pub fn rename_session(&self, dir: &str, id: &str, title: &str) -> Result<SessionInfo> {
        let u = self.url(&format!("/session/{id}"), Some(dir), &[]);
        Ok(to_session(&self.call("PATCH", &u, Some(json!({ "title": title })))?))
    }
    pub fn delete_session(&self, dir: &str, id: &str) -> Result<()> {
        self.call("DELETE", &self.url(&format!("/session/{id}"), Some(dir), &[]), None).map(|_| ())
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
        Ok(out)
    }
    pub fn list_messages(&self, dir: &str, id: &str) -> Result<Vec<ChatMessage>> {
        Ok(arr(self.get(&format!("/session/{id}/message"), Some(dir), &[])?).iter().filter_map(to_message).collect())
    }
    pub fn list_permissions(&self, dir: &str) -> Result<Vec<PermissionRequest>> {
        Ok(arr(self.get("/permission", Some(dir), &[])?).iter().filter_map(to_permission).collect())
    }
    pub fn list_questions(&self, dir: &str) -> Result<Vec<QuestionRequest>> {
        Ok(arr(self.get("/question", Some(dir), &[])?).iter().filter_map(to_question).collect())
    }
    pub fn list_todos(&self, dir: &str, id: &str) -> Result<Vec<Todo>> {
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
        let mut parts = vec![json!({ "type": "text", "text": text })];
        parts.extend(attachment_parts(files));
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
        Ok(arr(self.get(&format!("/session/{id}/diff"), Some(dir), &[])?)
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
    pub fn revert(&self, dir: &str, id: &str, message_id: &str) -> Result<()> {
        self.post(&format!("/session/{id}/revert"), Some(dir), json!({ "messageID": message_id })).map(|_| ())
    }
    pub fn unrevert(&self, dir: &str, id: &str) -> Result<()> {
        self.post(&format!("/session/{id}/unrevert"), Some(dir), json!({})).map(|_| ())
    }
    pub fn fork(&self, dir: &str, id: &str, message_id: Option<&str>) -> Result<SessionInfo> {
        let body = message_id.map(|m| json!({ "messageID": m })).unwrap_or(json!({}));
        Ok(to_session(&self.post(&format!("/session/{id}/fork"), Some(dir), body)?))
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
    pub fn summarize(&self, dir: &str, id: &str, model: &ModelRef) -> Result<()> {
        self.post(&format!("/session/{id}/summarize"), Some(dir), json!({ "providerID": model.provider_id, "modelID": model.model_id })).map(|_| ())
    }
    pub fn abort(&self, dir: &str, id: &str) -> Result<()> {
        self.post(&format!("/session/{id}/abort"), Some(dir), json!({})).map(|_| ())
    }
    pub fn dispose_instance(&self, dir: &str) -> Result<()> {
        self.post("/instance/dispose", Some(dir), json!({})).map(|_| ())
    }
    pub fn reply_permission(&self, dir: &str, req: &str, reply: PermissionReply) -> Result<()> {
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
        on_event: impl Fn(EngineEvent) + Send + 'static,
        on_state: impl Fn(bool) + Send + 'static,
    ) -> Arc<AtomicBool> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
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
    }
    #[test]
    fn auth() {
        let i = ConnectionInfo { url: "x".into(), username: "opencode".into(), password: "pw".into() };
        assert_eq!(basic_auth_header(&i), "Basic b3BlbmNvZGU6cHc=");
    }
}

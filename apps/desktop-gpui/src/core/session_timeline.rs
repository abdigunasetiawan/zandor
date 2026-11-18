//! Port shell/session-timeline.ts: logika murni untuk Panel "Aktivitas sesi" (garis waktu seluruh kejadian sesi) dan
//! "Berkas sesi" (berkas yang disentuh agent). Dibangun dari pesan sesi — tanpa data tambahan dan tanpa data karangan.
use super::engine::SessionFileDiff;
use super::tool_view::{step_of, tool_summary, Step};
use super::types::*;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    User,
    Reply,
    Thinking,
    Tool,
    Error,
    Permission,
    Question,
}
impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Kind::User => "user",
            Kind::Reply => "reply",
            Kind::Thinking => "thinking",
            Kind::Tool => "tool",
            Kind::Error => "error",
            Kind::Permission => "permission",
            Kind::Question => "question",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    /// ms epoch; 0 bila tidak diketahui.
    pub at: f64,
    pub kind: Kind,
    pub title: String,
    pub step: Option<Step>,
    /// pending | running | completed | error
    pub status: Option<&'static str>,
    pub duration_ms: Option<f64>,
    pub message_id: Option<String>,
    pub detail: Option<String>,
}

const DETAIL_MAX: usize = 4000;

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() > n { format!("{}…", s.chars().take(n).collect::<String>()) } else { s.to_string() }
}
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn status_str(s: ToolStatus) -> &'static str {
    match s {
        ToolStatus::Pending => "pending",
        ToolStatus::Running => "running",
        ToolStatus::Completed => "completed",
        ToolStatus::Error => "error",
    }
}

fn text_of(m: &ChatMessage) -> String {
    m.parts
        .iter()
        .filter_map(|p| match &p.kind {
            PartKind::Text { text, synthetic: false } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_detail(st: &ToolState) -> String {
    let mut parts = vec![];
    let input: Vec<String> = st
        .input
        .iter()
        .filter(|(_, v)| !v.is_null() && v.as_str() != Some(""))
        .map(|(k, v)| format!("{k}: {}", v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
        .collect();
    if !input.is_empty() {
        parts.push(input.join("\n"));
    }
    if let Some(e) = &st.error {
        parts.push(e.clone());
    } else if let Some(o) = st.output.as_ref().filter(|o| !o.is_empty()) {
        parts.push(o.clone());
    }
    clip(&parts.join("\n\n"), DETAIL_MAX)
}

/// Garis waktu kronologis: pesan pengguna, berpikir, tool, jawaban, galat; lalu izin/pertanyaan yang masih menunggu.
pub fn build_timeline(messages: &[ChatMessage], permissions: &[PermissionRequest], questions: &[QuestionRequest]) -> Vec<Entry> {
    let mut out = vec![];
    let mut last: f64 = 0.;
    let e = |id: String, at: f64, kind: Kind, title: String| Entry { id, at, kind, title, step: None, status: None, duration_ms: None, message_id: None, detail: None };
    for m in messages {
        let base = m.info.created;
        last = last.max(m.info.completed.unwrap_or(base));
        let mid = Some(m.info.id.clone());
        if m.info.role == Role::User {
            let text = one_line(&text_of(m));
            let files = m.parts.iter().filter(|p| matches!(p.kind, PartKind::File { .. })).count();
            let title = clip(if text.is_empty() && files > 0 { format!("{files} lampiran") } else { text.clone() }.as_str(), 160);
            out.push(Entry { message_id: mid, detail: (text.chars().count() > 160).then(|| clip(&text, DETAIL_MAX)), ..e(m.info.id.clone(), base, Kind::User, title) });
            continue;
        }
        let mut reply = String::new();
        for p in &m.parts {
            match &p.kind {
                PartKind::Reasoning { text } => {
                    if let Some(step) = step_of(p) {
                        out.push(Entry { step: Some(step), message_id: mid.clone(), detail: (!text.is_empty()).then(|| clip(text, DETAIL_MAX)), ..e(p.id.clone(), base, Kind::Thinking, clip(&one_line(text), 160)) });
                    }
                }
                PartKind::Tool { tool, state, .. } => {
                    let (start, end) = match state.time {
                        Some((s, en)) => (s, en),
                        None => (base, None),
                    };
                    out.push(Entry {
                        step: step_of(p),
                        status: Some(status_str(state.status)),
                        duration_ms: end.map(|en| (en - start).max(0.)),
                        message_id: mid.clone(),
                        detail: Some(tool_detail(state)),
                        ..e(p.id.clone(), start, Kind::Tool, clip(&tool_summary(tool, &state.input, state.title.as_deref()), 200))
                    });
                    last = last.max(end.unwrap_or(start));
                }
                PartKind::Text { text, synthetic: false } => {
                    if !reply.is_empty() {
                        reply.push('\n');
                    }
                    reply.push_str(text);
                }
                _ => {}
            }
        }
        let at = m.info.completed.unwrap_or(base);
        if !reply.trim().is_empty() {
            out.push(Entry { message_id: mid.clone(), detail: (reply.chars().count() > 160).then(|| clip(&reply, DETAIL_MAX)), ..e(format!("{}:reply", m.info.id), at, Kind::Reply, clip(&one_line(&reply), 160)) });
        }
        if let Some(err) = &m.info.error {
            let msg = if err.message.is_empty() { &err.name } else { &err.message };
            out.push(Entry { status: Some("error"), message_id: mid, detail: Some(format!("{}: {}", err.name, err.message)), ..e(format!("{}:error", m.info.id), at, Kind::Error, clip(&one_line(msg), 200)) });
        }
    }
    for p in permissions {
        let title = if p.patterns.is_empty() { p.permission.clone() } else { format!("{}: {}", p.permission, p.patterns.join(", ")) };
        out.push(Entry { status: Some("pending"), ..e(format!("perm:{}", p.id), last, Kind::Permission, title) });
    }
    for q in questions {
        out.push(Entry { status: Some("pending"), ..e(format!("q:{}", q.id), last, Kind::Question, q.questions.iter().map(|x| x.question.clone()).collect::<Vec<_>>().join(" · ")) });
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct TouchedFile {
    pub path: String,
    /// read | edit | write
    pub ops: Vec<&'static str>,
    pub count: usize,
    pub last_at: f64,
    pub failed: bool,
}

/// Berkas yang disentuh agent lewat tool read/edit/write sesi ini; terbaru dulu.
pub fn touched_files(messages: &[ChatMessage]) -> Vec<TouchedFile> {
    let mut list: Vec<TouchedFile> = vec![];
    for m in messages {
        for p in &m.parts {
            let PartKind::Tool { tool, state, .. } = &p.kind else { continue };
            let op = match tool.to_lowercase().as_str() {
                "read" => "read",
                "write" => "write",
                "edit" | "multiedit" | "patch" => "edit",
                _ => continue,
            };
            let raw = ["filePath", "path", "file"].iter().find_map(|k| state.input.get(*k).and_then(Value::as_str)).unwrap_or("");
            if raw.is_empty() {
                continue;
            }
            let at = state.time.map(|(s, e)| e.unwrap_or(s)).unwrap_or(m.info.created);
            let cur = match list.iter_mut().find(|f| f.path == raw) {
                Some(c) => c,
                None => {
                    list.push(TouchedFile { path: raw.to_string(), ops: vec![], count: 0, last_at: 0., failed: false });
                    list.last_mut().unwrap()
                }
            };
            if !cur.ops.contains(&op) {
                cur.ops.push(op);
            }
            cur.count += 1;
            cur.last_at = cur.last_at.max(at);
            if state.status == ToolStatus::Error {
                cur.failed = true;
            }
        }
    }
    list.sort_by(|a, b| b.last_at.partial_cmp(&a.last_at).unwrap_or(std::cmp::Ordering::Equal));
    list
}

fn norm_path(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn is_abs(p: &str) -> bool {
    let b = p.as_bytes();
    p.starts_with('/') || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
}

/// Lengkapi daftar dari tool dengan perubahan menurut snapshot engine (berkas dari `bash` dsb.). Berkas terhapus dilewati.
pub fn merge_snapshot_files(touched: Vec<TouchedFile>, diffs: &[SessionFileDiff], directory: &str, at: f64) -> Vec<TouchedFile> {
    let mut seen: Vec<String> = touched.iter().map(|f| norm_path(&f.path)).collect();
    let mut out = touched;
    for d in diffs {
        if d.status == "deleted" {
            continue;
        }
        let abs = if is_abs(&d.file) { d.file.clone() } else { format!("{}/{}", directory.trim_end_matches(['\\', '/']), d.file) };
        let key = norm_path(&abs);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        out.push(TouchedFile { path: abs, ops: vec![if d.status == "added" { "write" } else { "edit" }], count: 1, last_at: at, failed: false });
    }
    out.sort_by(|a, b| b.last_at.partial_cmp(&a.last_at).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Path relatif terhadap folder kerja sesi (garis miring maju); path di luar folder tetap utuh.
pub fn relative_to(dir: &str, path: &str) -> String {
    let (d, p) = (dir.replace('\\', "/"), path.replace('\\', "/"));
    let d = d.trim_end_matches('/');
    if !d.is_empty() && p.to_lowercase().starts_with(&format!("{}/", d.to_lowercase())) {
        return p[d.len() + 1..].to_string();
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tool(id: &str, name: &str, path: &str, status: ToolStatus, t: (f64, Option<f64>)) -> MessagePart {
        let mut input = Obj::new();
        input.insert("filePath".into(), Value::String(path.into()));
        MessagePart { id: id.into(), message_id: "m".into(), session_id: "s".into(), kind: PartKind::Tool { tool: name.into(), call_id: id.into(), state: ToolState { status, input, output: Some("ok".into()), error: None, title: None, metadata: None, time: Some(t) } } }
    }
    fn msg(id: &str, role: Role, parts: Vec<MessagePart>) -> ChatMessage {
        ChatMessage { info: MessageInfo { id: id.into(), session_id: "s".into(), role, created: 1., completed: Some(5.), parent_id: None, agent: None, provider_id: None, model_id: None, finish: None, error: None, tokens: None, cost: None }, parts }
    }
    #[test]
    fn berkas_dan_garis_waktu() {
        let txt = |s: &str| MessagePart { id: "t".into(), message_id: "u".into(), session_id: "s".into(), kind: PartKind::Text { text: s.into(), synthetic: false } };
        let ms = vec![
            msg("u", Role::User, vec![txt("halo   dunia")]),
            msg("a", Role::Assistant, vec![tool("p1", "read", "C:/x/a.rs", ToolStatus::Completed, (2., Some(3.))), tool("p2", "edit", "C:/x/a.rs", ToolStatus::Error, (4., Some(6.))), tool("p3", "write", "C:/x/b.rs", ToolStatus::Completed, (3., None)), txt("selesai")]),
        ];
        let f = touched_files(&ms);
        assert_eq!(f[0].path, "C:/x/a.rs");
        assert_eq!(f[0].ops, ["read", "edit"]);
        assert!(f[0].failed && f[0].count == 2);
        let diffs = vec![SessionFileDiff { file: "c.md".into(), patch: String::new(), additions: 1, deletions: 0, status: "added".into() }, SessionFileDiff { file: "C:\\x\\b.rs".into(), patch: String::new(), additions: 1, deletions: 0, status: "modified".into() }];
        let all = merge_snapshot_files(f, &diffs, "C:/x/", 9.);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].path, "C:/x/c.md");
        let tl = build_timeline(&ms, &[], &[]);
        assert_eq!(tl[0].title, "halo dunia");
        assert_eq!(tl.iter().filter(|e| e.kind == Kind::Tool).count(), 3);
        assert_eq!(tl[2].duration_ms, Some(2.));
        assert_eq!(tl.last().unwrap().kind, Kind::Reply);
        assert_eq!(relative_to("C:\\x", "C:/x/sub/a.rs"), "sub/a.rs");
    }
}

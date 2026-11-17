//! Model data chat — port 1:1 packages/core/src/chat/types.ts.
//! Normalisasi defensif atas JSON engine (input tak tepercaya): data tak dikenal tidak merusak tampilan.
#![allow(dead_code)]
use serde_json::{Map, Value};

pub type Obj = Map<String, Value>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TokenUsage {
    pub input: f64,
    pub output: f64,
    pub reasoning: f64,
    pub total: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessageError {
    pub name: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessageInfo {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub created: f64,
    pub completed: Option<f64>,
    pub parent_id: Option<String>,
    pub agent: Option<String>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub finish: Option<String>,
    pub error: Option<MessageError>,
    pub tokens: Option<TokenUsage>,
    pub cost: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    Pending,
    Running,
    Completed,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolState {
    pub status: ToolStatus,
    pub input: Obj,
    pub output: Option<String>,
    pub error: Option<String>,
    pub title: Option<String>,
    pub metadata: Option<Obj>,
    pub time: Option<(f64, Option<f64>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PartKind {
    Text { text: String, synthetic: bool },
    Reasoning { text: String },
    Tool { tool: String, call_id: String, state: ToolState },
    StepFinish { reason: String, cost: f64, tokens: TokenUsage },
    File { mime: String, filename: Option<String>, url: String },
    Other { raw_type: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessagePart {
    pub id: String,
    pub message_id: String,
    pub session_id: String,
    pub kind: PartKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub info: MessageInfo,
    pub parts: Vec<MessagePart>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolRef {
    pub message_id: String,
    pub call_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PermissionRequest {
    pub id: String,
    pub session_id: String,
    pub permission: String,
    pub patterns: Vec<String>,
    pub metadata: Obj,
    pub always: Vec<String>,
    pub tool: Option<ToolRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionReply {
    Once,
    Always,
    Reject,
}
impl PermissionReply {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Always => "always",
            Self::Reject => "reject",
        }
    }
    fn parse(v: &Value) -> Option<Self> {
        match v.as_str()? {
            "once" => Some(Self::Once),
            "always" => Some(Self::Always),
            "reject" => Some(Self::Reject),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct QuestionItem {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    pub multiple: bool,
    pub custom: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct QuestionRequest {
    pub id: String,
    pub session_id: String,
    pub questions: Vec<QuestionItem>,
    pub tool: Option<ToolRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    High,
    Medium,
    Low,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Todo {
    pub content: String,
    pub status: TodoStatus,
    pub priority: Priority,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionRunStatus {
    Idle,
    Busy,
    Retry { attempt: f64, message: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineEvent {
    MessageUpdated(MessageInfo),
    MessageRemoved { session_id: String, message_id: String },
    PartUpdated(MessagePart),
    PartRemoved { session_id: String, message_id: String, part_id: String },
    PartDelta { session_id: String, message_id: String, part_id: String, field: String, delta: String },
    SessionStatus { session_id: String, status: SessionRunStatus },
    SessionRevert { session_id: String, revert: Option<String> },
    SessionError { session_id: Option<String>, error: MessageError },
    PermissionAsked(PermissionRequest),
    PermissionReplied { session_id: String, request_id: String, reply: PermissionReply },
    QuestionAsked(QuestionRequest),
    QuestionClosed { session_id: String, request_id: String, answered: bool },
    TodoUpdated { session_id: String, todos: Vec<Todo> },
}

impl EngineEvent {
    /// Sesi yang dituju event (bila ada).
    pub fn session_id(&self) -> Option<&str> {
        use EngineEvent::*;
        Some(match self {
            MessageUpdated(i) => &i.session_id,
            MessageRemoved { session_id, .. }
            | PartRemoved { session_id, .. }
            | PartDelta { session_id, .. }
            | SessionStatus { session_id, .. }
            | SessionRevert { session_id, .. }
            | PermissionReplied { session_id, .. }
            | QuestionClosed { session_id, .. }
            | TodoUpdated { session_id, .. } => session_id,
            PartUpdated(p) => &p.session_id,
            PermissionAsked(r) => &r.session_id,
            QuestionAsked(q) => &q.session_id,
            SessionError { session_id, .. } => return session_id.as_deref(),
        })
    }
}

// ---------- normalisasi ----------

fn s(o: &Obj, k: &str) -> Option<String> {
    o.get(k).and_then(Value::as_str).map(String::from)
}
fn ne(o: &Obj, k: &str) -> Option<String> {
    s(o, k).filter(|x| !x.is_empty())
}
fn n(o: &Obj, k: &str, d: f64) -> f64 {
    o.get(k).and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(d)
}
fn obj<'a>(o: &'a Obj, k: &str) -> Option<&'a Obj> {
    o.get(k).and_then(Value::as_object)
}
fn strs(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}
fn tool_ref(o: &Obj) -> Option<ToolRef> {
    let t = obj(o, "tool")?;
    Some(ToolRef { message_id: ne(t, "messageID")?, call_id: ne(t, "callID")? })
}

fn usage(v: Option<&Obj>) -> TokenUsage {
    let e = Obj::new();
    let o = v.unwrap_or(&e);
    let (input, output, reasoning) = (n(o, "input", 0.), n(o, "output", 0.), n(o, "reasoning", 0.));
    TokenUsage { input, output, reasoning, total: n(o, "total", input + output + reasoning) }
}

pub fn to_message_error(v: Option<&Value>) -> Option<MessageError> {
    let o = v?.as_object()?;
    let e = Obj::new();
    let data = obj(o, "data").unwrap_or(&e);
    let name = s(o, "name").unwrap_or_else(|| "UnknownError".into());
    let message = s(data, "message").or_else(|| s(o, "message")).unwrap_or_else(|| name.clone());
    Some(MessageError { name, message })
}

pub fn to_message_info(raw: Option<&Value>) -> Option<MessageInfo> {
    let o = raw?.as_object()?;
    let id = ne(o, "id")?;
    let session_id = ne(o, "sessionID")?;
    let role = match o.get("role")?.as_str()? {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        _ => return None,
    };
    let e = Obj::new();
    let t = obj(o, "time").unwrap_or(&e);
    let model = obj(o, "model").unwrap_or(&e);
    Some(MessageInfo {
        id,
        session_id,
        role,
        created: n(t, "created", 0.),
        completed: t.get("completed").and_then(Value::as_f64),
        parent_id: ne(o, "parentID"),
        agent: ne(o, "agent"),
        provider_id: ne(o, "providerID").or_else(|| ne(model, "providerID")),
        model_id: ne(o, "modelID").or_else(|| ne(model, "modelID")),
        finish: ne(o, "finish"),
        error: to_message_error(o.get("error")),
        tokens: obj(o, "tokens").map(|t| usage(Some(t))),
        cost: o.get("cost").and_then(Value::as_f64),
    })
}

fn to_tool_state(raw: Option<&Value>) -> ToolState {
    let e = Obj::new();
    let o = raw.and_then(Value::as_object).unwrap_or(&e);
    let status = match o.get("status").and_then(Value::as_str) {
        Some("running") => ToolStatus::Running,
        Some("completed") => ToolStatus::Completed,
        Some("error") => ToolStatus::Error,
        _ => ToolStatus::Pending,
    };
    ToolState {
        status,
        input: obj(o, "input").cloned().unwrap_or_default(),
        output: s(o, "output"),
        error: s(o, "error"),
        title: s(o, "title"),
        metadata: obj(o, "metadata").cloned(),
        time: obj(o, "time").map(|t| (n(t, "start", 0.), t.get("end").and_then(Value::as_f64))),
    }
}

pub fn to_part(raw: Option<&Value>) -> Option<MessagePart> {
    let o = raw?.as_object()?;
    let id = ne(o, "id")?;
    let message_id = ne(o, "messageID")?;
    let session_id = ne(o, "sessionID")?;
    let ty = ne(o, "type")?;
    let kind = match ty.as_str() {
        "text" => PartKind::Text {
            text: s(o, "text").unwrap_or_default(),
            synthetic: o.get("synthetic").and_then(Value::as_bool) == Some(true),
        },
        "reasoning" => PartKind::Reasoning { text: s(o, "text").unwrap_or_default() },
        "tool" => PartKind::Tool {
            tool: s(o, "tool").unwrap_or_else(|| "?".into()),
            call_id: s(o, "callID").unwrap_or_else(|| id.clone()),
            state: to_tool_state(o.get("state")),
        },
        "step-finish" => PartKind::StepFinish {
            reason: s(o, "reason").unwrap_or_default(),
            cost: n(o, "cost", 0.),
            tokens: usage(obj(o, "tokens")),
        },
        "file" => PartKind::File {
            mime: s(o, "mime").unwrap_or_else(|| "application/octet-stream".into()),
            url: s(o, "url").unwrap_or_default(),
            filename: ne(o, "filename"),
        },
        _ => PartKind::Other { raw_type: ty },
    };
    Some(MessagePart { id, message_id, session_id, kind })
}

pub fn to_message(raw: &Value) -> Option<ChatMessage> {
    let o = raw.as_object()?;
    let info = to_message_info(o.get("info"))?;
    let mut parts: Vec<MessagePart> = o
        .get("parts")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|p| to_part(Some(p))).collect())
        .unwrap_or_default();
    parts.sort_by(|a, b| a.id.cmp(&b.id));
    Some(ChatMessage { info, parts })
}

pub fn to_permission(raw: &Value) -> Option<PermissionRequest> {
    let o = raw.as_object()?;
    Some(PermissionRequest {
        id: ne(o, "id")?,
        session_id: ne(o, "sessionID")?,
        permission: s(o, "permission").unwrap_or_else(|| "unknown".into()),
        patterns: strs(o.get("patterns")),
        metadata: obj(o, "metadata").cloned().unwrap_or_default(),
        always: strs(o.get("always")),
        tool: tool_ref(o),
    })
}

pub fn to_question(raw: &Value) -> Option<QuestionRequest> {
    let o = raw.as_object()?;
    let id = ne(o, "id")?;
    let session_id = ne(o, "sessionID")?;
    // Model kadang mengirim `questions` sebagai string JSON. Tetap dibaca bila bisa.
    let list: Value = match o.get("questions") {
        Some(Value::String(txt)) => serde_json::from_str(txt).unwrap_or(Value::Array(vec![])),
        Some(v) => v.clone(),
        None => Value::Array(vec![]),
    };
    let mut questions = Vec::new();
    for q in list.as_array().into_iter().flatten() {
        let Some(q) = q.as_object() else { continue };
        let Some(text) = ne(q, "question") else { continue };
        let options = q
            .get("options")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|o| {
                        let o = o.as_object()?;
                        Some(QuestionOption { label: ne(o, "label")?, description: s(o, "description").unwrap_or_default() })
                    })
                    .collect()
            })
            .unwrap_or_default();
        questions.push(QuestionItem {
            question: text,
            header: s(q, "header").unwrap_or_default().chars().take(60).collect(),
            options,
            multiple: q.get("multiple").and_then(Value::as_bool) == Some(true),
            custom: q.get("custom").and_then(Value::as_bool) != Some(false),
        });
    }
    // Permintaan rusak tetap dikembalikan: engine menunggu jawabannya, pengguna harus bisa melewatinya.
    if questions.is_empty() {
        questions.push(QuestionItem { question: String::new(), header: String::new(), options: vec![], multiple: false, custom: true });
    }
    Some(QuestionRequest { id, session_id, questions, tool: tool_ref(o) })
}

pub fn to_todos(raw: Option<&Value>) -> Vec<Todo> {
    let mut out = Vec::new();
    for t in raw.and_then(Value::as_array).into_iter().flatten() {
        let Some(t) = t.as_object() else { continue };
        let Some(content) = ne(t, "content") else { continue };
        let status = match t.get("status").and_then(Value::as_str) {
            Some("in_progress") => TodoStatus::InProgress,
            Some("completed") => TodoStatus::Completed,
            Some("cancelled") => TodoStatus::Cancelled,
            _ => TodoStatus::Pending,
        };
        let priority = match t.get("priority").and_then(Value::as_str) {
            Some("high") => Priority::High,
            Some("low") => Priority::Low,
            _ => Priority::Medium,
        };
        out.push(Todo { content: content.chars().take(500).collect(), status, priority });
        if out.len() == 100 {
            break;
        }
    }
    out
}

/// Event SSE mentah → EngineEvent. Tidak relevan/tidak dikenal → None.
pub fn to_engine_event(raw: &Value) -> Option<EngineEvent> {
    let o = raw.as_object()?;
    let ty = o.get("type")?.as_str()?;
    let e = Obj::new();
    let p = obj(o, "properties").unwrap_or(&e);
    use EngineEvent::*;
    Some(match ty {
        "message.updated" => MessageUpdated(to_message_info(p.get("info"))?),
        "message.removed" => MessageRemoved { session_id: ne(p, "sessionID")?, message_id: ne(p, "messageID")? },
        "message.part.updated" => PartUpdated(to_part(p.get("part"))?),
        "message.part.removed" => PartRemoved {
            session_id: ne(p, "sessionID")?,
            message_id: ne(p, "messageID")?,
            part_id: ne(p, "partID")?,
        },
        "message.part.delta" => PartDelta {
            session_id: ne(p, "sessionID")?,
            message_id: ne(p, "messageID")?,
            part_id: ne(p, "partID")?,
            field: ne(p, "field")?,
            delta: s(p, "delta")?,
        },
        "session.status" => {
            let session_id = ne(p, "sessionID")?;
            let st = obj(p, "status").unwrap_or(&e);
            let status = match st.get("type").and_then(Value::as_str) {
                Some("busy") => SessionRunStatus::Busy,
                Some("retry") => SessionRunStatus::Retry { attempt: n(st, "attempt", 0.), message: s(st, "message").unwrap_or_default() },
                _ => SessionRunStatus::Idle,
            };
            SessionStatus { session_id, status }
        }
        "session.updated" => {
            let info = obj(p, "info")?;
            let session_id = ne(p, "sessionID").or_else(|| ne(info, "id"))?;
            let revert = obj(info, "revert").and_then(|r| s(r, "messageID"));
            SessionRevert { session_id, revert }
        }
        "session.idle" => SessionStatus { session_id: ne(p, "sessionID")?, status: SessionRunStatus::Idle },
        "session.error" => SessionError {
            session_id: ne(p, "sessionID"),
            error: to_message_error(p.get("error"))
                .unwrap_or(MessageError { name: "UnknownError".into(), message: "galat tidak diketahui".into() }),
        },
        "permission.asked" => PermissionAsked(to_permission(&Value::Object(p.clone()))?),
        "permission.replied" => PermissionReplied {
            session_id: ne(p, "sessionID")?,
            request_id: ne(p, "requestID")?,
            reply: PermissionReply::parse(p.get("reply")?)?,
        },
        "question.asked" => QuestionAsked(to_question(&Value::Object(p.clone()))?),
        "question.replied" | "question.rejected" => QuestionClosed {
            session_id: ne(p, "sessionID")?,
            request_id: ne(p, "requestID")?,
            answered: ty == "question.replied",
        },
        "todo.updated" => TodoUpdated { session_id: ne(p, "sessionID")?, todos: to_todos(p.get("todos")) },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn delta_dan_status() {
        let d = to_engine_event(&json!({"type":"message.part.delta","properties":{"sessionID":"s","messageID":"m","partID":"p","field":"text","delta":"hai"}}));
        assert!(matches!(d, Some(EngineEvent::PartDelta { ref delta, .. }) if delta == "hai"));
        let st = to_engine_event(&json!({"type":"session.idle","properties":{"sessionID":"s"}}));
        assert_eq!(st, Some(EngineEvent::SessionStatus { session_id: "s".into(), status: SessionRunStatus::Idle }));
        assert_eq!(to_engine_event(&json!({"type":"x"})), None);
    }

    #[test]
    fn pertanyaan_string_json_dan_rusak() {
        let q = to_question(&json!({"id":"q","sessionID":"s","questions":"[{\"question\":\"A?\",\"options\":[{\"label\":\"Ya\"}]}]"})).unwrap();
        assert_eq!(q.questions[0].options[0].label, "Ya");
        assert!(q.questions[0].custom);
        let r = to_question(&json!({"id":"q","sessionID":"s","questions":"rusak"})).unwrap();
        assert_eq!(r.questions.len(), 1);
    }

    #[test]
    fn pesan_part_terurut() {
        let m = to_message(&json!({"info":{"id":"m","sessionID":"s","role":"assistant","time":{"created":1}},
            "parts":[{"id":"b","messageID":"m","sessionID":"s","type":"text","text":"2"},
                     {"id":"a","messageID":"m","sessionID":"s","type":"weird"}]})).unwrap();
        assert_eq!(m.parts[0].id, "a");
        assert!(matches!(m.parts[0].kind, PartKind::Other { .. }));
    }
}

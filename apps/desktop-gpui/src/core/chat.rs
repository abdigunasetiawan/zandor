//! Reducer state chat satu sesi — port 1:1 packages/core/src/chat/reducer.ts.
#![allow(dead_code)]
use super::types::*;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ChatState {
    pub session_id: String,
    pub loaded: bool,
    pub messages: Vec<ChatMessage>,
    pub status: SessionRunStatus,
    pub revert: Option<String>,
    pub permissions: Vec<PermissionRequest>,
    pub questions: Vec<QuestionRequest>,
    pub todos: Vec<Todo>,
    pub error: Option<String>,
    pending: HashMap<String, String>,
}

impl ChatState {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            loaded: false,
            messages: vec![],
            status: SessionRunStatus::Idle,
            revert: None,
            permissions: vec![],
            questions: vec![],
            todos: vec![],
            error: None,
            pending: HashMap::new(),
        }
    }

    pub fn busy(&self) -> bool {
        !matches!(self.status, SessionRunStatus::Idle)
    }

    /// Ganti riwayat penuh (muat awal / sinkron ulang).
    pub fn load(
        &mut self,
        mut messages: Vec<ChatMessage>,
        permissions: Vec<PermissionRequest>,
        revert: Option<String>,
        questions: Option<Vec<QuestionRequest>>,
        todos: Option<Vec<Todo>>,
    ) {
        messages.sort_by(|a, b| a.info.id.cmp(&b.info.id));
        let running = messages.last().is_some_and(|m| m.info.role == Role::Assistant && m.info.completed.is_none());
        self.loaded = true;
        self.revert = revert;
        self.messages = messages;
        self.permissions = permissions;
        if let Some(q) = questions {
            self.questions = q;
        }
        if let Some(t) = todos {
            self.todos = t;
        }
        self.pending.clear();
        if running && self.status == SessionRunStatus::Idle {
            self.status = SessionRunStatus::Busy;
        }
    }

    fn upsert_message(&mut self, next: ChatMessage) {
        if let Some(m) = self.messages.iter_mut().find(|m| m.info.id == next.info.id) {
            *m = next;
        } else {
            self.messages.push(next);
            self.messages.sort_by(|a, b| a.info.id.cmp(&b.info.id));
        }
    }

    /// Terapkan event; kembalikan true bila state berubah.
    pub fn apply(&mut self, ev: &EngineEvent) -> bool {
        let sid = self.session_id.clone();
        match ev {
            EngineEvent::MessageUpdated(info) => {
                if info.session_id != sid {
                    return false;
                }
                let parts = self.messages.iter().find(|m| m.info.id == info.id).map(|m| m.parts.clone()).unwrap_or_default();
                self.upsert_message(ChatMessage { info: info.clone(), parts });
            }
            EngineEvent::MessageRemoved { session_id, message_id } => {
                if *session_id != sid {
                    return false;
                }
                self.messages.retain(|m| m.info.id != *message_id);
            }
            EngineEvent::PartUpdated(part) => {
                if part.session_id != sid {
                    return false;
                }
                let mut incoming = part.clone();
                if let Some(buf) = self.pending.remove(&part.id) {
                    if let PartKind::Text { text, .. } | PartKind::Reasoning { text } = &mut incoming.kind {
                        if !text.ends_with(&buf) {
                            text.push_str(&buf);
                        }
                    }
                }
                if let Some(t) = todos_from_part(&incoming) {
                    self.todos = t;
                }
                let base = self.messages.iter().find(|m| m.info.id == part.message_id).cloned().unwrap_or_else(|| ChatMessage {
                    info: MessageInfo {
                        id: part.message_id.clone(),
                        session_id: part.session_id.clone(),
                        role: Role::Assistant,
                        created: 0.,
                        completed: None,
                        parent_id: None,
                        agent: None,
                        provider_id: None,
                        model_id: None,
                        finish: None,
                        error: None,
                        tokens: None,
                        cost: None,
                    },
                    parts: vec![],
                });
                let mut parts = base.parts;
                if let Some(p) = parts.iter_mut().find(|p| p.id == incoming.id) {
                    *p = incoming;
                } else {
                    parts.push(incoming);
                    parts.sort_by(|a, b| a.id.cmp(&b.id));
                }
                self.upsert_message(ChatMessage { info: base.info, parts });
            }
            EngineEvent::PartRemoved { session_id, message_id, part_id } => {
                if *session_id != sid {
                    return false;
                }
                if let Some(m) = self.messages.iter_mut().find(|m| m.info.id == *message_id) {
                    m.parts.retain(|p| p.id != *part_id);
                }
            }
            EngineEvent::PartDelta { session_id, message_id, part_id, field, delta } => {
                if *session_id != sid || field != "text" {
                    return false;
                }
                let part = self
                    .messages
                    .iter_mut()
                    .find(|m| m.info.id == *message_id)
                    .and_then(|m| m.parts.iter_mut().find(|p| p.id == *part_id));
                match part {
                    None => self.pending.entry(part_id.clone()).or_default().push_str(delta),
                    Some(p) => match &mut p.kind {
                        PartKind::Text { text, .. } | PartKind::Reasoning { text } => text.push_str(delta),
                        _ => return false,
                    },
                }
            }
            EngineEvent::SessionStatus { session_id, status } => {
                if *session_id != sid || self.status == *status {
                    return false;
                }
                self.status = status.clone();
            }
            EngineEvent::SessionRevert { session_id, revert } => {
                if *session_id != sid || *revert == self.revert {
                    return false;
                }
                self.revert = revert.clone();
            }
            EngineEvent::SessionError { session_id, error } => {
                if session_id.as_ref().is_some_and(|s| *s != sid) {
                    return false;
                }
                self.error = Some(error.message.clone());
            }
            EngineEvent::PermissionAsked(r) => {
                if r.session_id != sid || self.permissions.iter().any(|p| p.id == r.id) {
                    return false;
                }
                self.permissions.push(r.clone());
            }
            EngineEvent::QuestionAsked(q) => {
                if q.session_id != sid || self.questions.iter().any(|x| x.id == q.id) {
                    return false;
                }
                self.questions.push(q.clone());
            }
            EngineEvent::QuestionClosed { session_id, request_id, .. } => {
                if *session_id != sid || !self.questions.iter().any(|q| q.id == *request_id) {
                    return false;
                }
                self.questions.retain(|q| q.id != *request_id);
            }
            EngineEvent::TodoUpdated { session_id, todos } => {
                if *session_id != sid {
                    return false;
                }
                self.todos = todos.clone();
            }
            EngineEvent::PermissionReplied { session_id, request_id, .. } => {
                if *session_id != sid || !self.permissions.iter().any(|p| p.id == *request_id) {
                    return false;
                }
                self.permissions.retain(|p| p.id != *request_id);
            }
        }
        true
    }

    /// Pesan terlihat (yang sejak revert disembunyikan).
    pub fn visible(&self) -> impl Iterator<Item = &ChatMessage> {
        let cut = self.revert.clone();
        self.messages.iter().filter(move |m| cut.as_ref().is_none_or(|c| m.info.id < *c))
    }
}

fn todos_from_part(p: &MessagePart) -> Option<Vec<Todo>> {
    let PartKind::Tool { tool, state, .. } = &p.kind else { return None };
    if tool.to_lowercase() != "todowrite" || state.status == ToolStatus::Pending {
        return None;
    }
    let t = to_todos(state.input.get("todos"));
    (!t.is_empty()).then_some(t)
}

/// Teks gabungan pesan (part teks non-sintetis).
pub fn message_text(m: &ChatMessage) -> String {
    m.parts
        .iter()
        .filter_map(|p| match &p.kind {
            PartKind::Text { text, synthetic: false } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn part(id: &str, text: &str) -> MessagePart {
        MessagePart {
            id: id.into(),
            message_id: "m1".into(),
            session_id: "s".into(),
            kind: PartKind::Text { text: text.into(), synthetic: false },
        }
    }
    #[test]
    fn delta_sebelum_part_ditahan() {
        let mut s = ChatState::new("s");
        assert!(s.apply(&EngineEvent::PartDelta {
            session_id: "s".into(),
            message_id: "m1".into(),
            part_id: "p1".into(),
            field: "text".into(),
            delta: "lo".into()
        }));
        s.apply(&EngineEvent::PartUpdated(part("p1", "hal")));
        assert_eq!(message_text(&s.messages[0]), "hallo");
        s.apply(&EngineEvent::PartDelta {
            session_id: "s".into(),
            message_id: "m1".into(),
            part_id: "p1".into(),
            field: "text".into(),
            delta: "!".into(),
        });
        assert_eq!(message_text(&s.messages[0]), "hallo!");
    }
    #[test]
    fn sesi_lain_diabaikan() {
        let mut s = ChatState::new("x");
        assert!(!s.apply(&EngineEvent::PartUpdated(part("p1", "a"))));
    }
}

/// Satu giliran tampilan (port turns.ts TurnMessage): pesan asisten berurutan digabung.
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    pub message: ChatMessage,
    pub source_ids: Vec<String>,
}

/// Port mergeAssistantTurns: identitas/model/status mengikuti pesan TERAKHIR, waktu mulai dari yang pertama.
pub fn merge_assistant_turns<'a>(messages: impl IntoIterator<Item = &'a ChatMessage>) -> Vec<Turn> {
    let mut out: Vec<Turn> = vec![];
    for m in messages {
        match out.last_mut() {
            Some(prev) if m.info.role == Role::Assistant && prev.message.info.role == Role::Assistant => {
                let mut info = m.info.clone();
                info.created = prev.message.info.created;
                info.error = m.info.error.clone().or_else(|| prev.message.info.error.clone());
                info.tokens = m.info.tokens.clone();
                let cost = prev.message.info.cost.unwrap_or(0.) + m.info.cost.unwrap_or(0.);
                info.cost = (cost > 0.).then_some(cost);
                prev.message.info = info;
                prev.message.parts.extend(m.parts.iter().cloned());
                prev.source_ids.push(m.info.id.clone());
            }
            _ => out.push(Turn { message: m.clone(), source_ids: vec![m.info.id.clone()] }),
        }
    }
    out
}

#[cfg(test)]
mod turn_tests {
    use super::*;
    fn msg(id: &str, role: Role, created: f64, completed: Option<f64>) -> ChatMessage {
        ChatMessage {
            info: MessageInfo { id: id.into(), session_id: "s".into(), role, created, completed, parent_id: None, agent: None, provider_id: None, model_id: Some(id.into()), finish: None, error: None, tokens: None, cost: Some(1.) },
            parts: vec![MessagePart { id: format!("p{id}"), message_id: id.into(), session_id: "s".into(), kind: PartKind::Text { text: id.into(), synthetic: false } }],
        }
    }
    #[test]
    fn merge() {
        let ms = [msg("u1", Role::User, 1., None), msg("a1", Role::Assistant, 2., Some(3.)), msg("a2", Role::Assistant, 4., Some(5.)), msg("u2", Role::User, 6., None), msg("a3", Role::Assistant, 7., None)];
        let t = merge_assistant_turns(ms.iter());
        assert_eq!(t.len(), 4);
        assert_eq!(t[1].source_ids, vec!["a1".to_string(), "a2".to_string()]);
        assert_eq!((t[1].message.info.created, t[1].message.info.completed), (2., Some(5.)));
        assert_eq!(t[1].message.info.model_id.as_deref(), Some("a2"));
        assert_eq!(t[1].message.parts.len(), 2);
        assert_eq!(t[1].message.info.cost, Some(2.));
        assert_eq!(t[3].message.info.completed, None);
    }
}

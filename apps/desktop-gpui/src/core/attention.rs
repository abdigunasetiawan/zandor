//! Indikator sesi dan pelacak perhatian — port 1:1 packages/core/src/chat/attention.ts. Murni terhadap state chat:
//! menurunkan status tampilan dan mendeteksi PERALIHAN yang layak diberitahukan (selesai/gagal/batas/butuh aksi).
use super::chat::ChatState;
use super::types::{Role, SessionRunStatus};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indicator {
    Idle,
    Busy,
    Action,
    Done,
    Failed,
    Limit,
}
impl Indicator {
    pub fn key(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::Action => "action",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Limit => "limit",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttentionEvent {
    Done { session_id: String },
    Failed { session_id: String, limit: bool, message: String },
    Action { session_id: String, permission: bool, detail: String },
}
impl AttentionEvent {
    pub fn session_id(&self) -> &str {
        match self {
            Self::Done { session_id } | Self::Failed { session_id, .. } | Self::Action { session_id, .. } => session_id,
        }
    }
}

pub fn is_limit_error(message: &str) -> bool {
    regex::Regex::new(r"(?i)rate.?limit|too many requests|\b429\b|quota|insufficient|credit|billing|usage limit|limit (reached|exceeded)|overloaded|context (length|window)|maximum context|token limit")
        .expect("regex")
        .is_match(message)
}

fn busy_of(s: &ChatState, sending: bool) -> bool {
    !matches!(s.status, SessionRunStatus::Idle) || sending
}

/// Kegagalan giliran terakhir: (batas?, pesan).
pub fn failure_of(s: &ChatState) -> Option<(bool, String)> {
    let mut message = String::new();
    match s.messages.last() {
        Some(last) if last.info.role == Role::Assistant && last.info.error.as_ref().is_some_and(|e| e.name != "MessageAbortedError") => {
            let e = last.info.error.as_ref().unwrap();
            message = if e.message.is_empty() { e.name.clone() } else { e.message.clone() };
        }
        _ => {
            if let Some(e) = &s.error {
                message = e.clone();
            }
        }
    }
    if message.is_empty() {
        return None;
    }
    Some((is_limit_error(&message), message))
}

pub const SETTLE_MS: f64 = 300.;

#[derive(Default)]
pub struct AttentionTracker {
    pub unseen: HashMap<String, ()>,
    prev: HashMap<String, (bool, usize)>,
    ending: HashMap<String, f64>,
}

impl AttentionTracker {
    pub fn has_pending(&self) -> bool {
        !self.ending.is_empty()
    }
    pub fn mark_seen(&mut self, id: &str) {
        self.unseen.remove(id);
    }
    pub fn indicator(&self, id: &str, s: Option<&ChatState>, sending: bool) -> Indicator {
        let Some(s) = s else { return if self.unseen.contains_key(id) { Indicator::Done } else { Indicator::Idle } };
        if s.permissions.len() + s.questions.len() > 0 {
            return Indicator::Action;
        }
        if busy_of(s, sending) {
            return Indicator::Busy;
        }
        if let Some((limit, _)) = failure_of(s) {
            return if limit { Indicator::Limit } else { Indicator::Failed };
        }
        if self.unseen.contains_key(id) { Indicator::Done } else { Indicator::Idle }
    }

    /// `sessions`: (id, state, sedang mengirim).
    pub fn observe(&mut self, sessions: &[(&str, &ChatState, bool)], viewing: Option<&str>, focused: bool, at: f64) -> Vec<AttentionEvent> {
        let mut events = vec![];
        for (id, s, sending) in sessions {
            let now = (busy_of(s, *sending), s.permissions.len() + s.questions.len());
            let before = self.prev.insert(id.to_string(), now);
            let Some(before) = before else { continue };
            if now.1 > before.1 {
                let perm = s.permissions.last();
                let q = s.questions.last();
                let ev = match perm {
                    Some(p) if q.is_none() || s.permissions.len() >= s.questions.len() => AttentionEvent::Action {
                        session_id: id.to_string(),
                        permission: true,
                        detail: if p.patterns.is_empty() { p.permission.clone() } else { format!("{}: {}", p.permission, p.patterns.join(", ")) },
                    },
                    _ => AttentionEvent::Action {
                        session_id: id.to_string(),
                        permission: false,
                        detail: q.and_then(|q| q.questions.first()).map(|x| x.question.clone()).unwrap_or_default(),
                    },
                };
                events.push(ev);
            }
            if before.0 && !now.0 {
                self.ending.insert(id.to_string(), at);
            }
            if now.0 {
                self.ending.remove(*id);
            }
            if !before.0 && now.0 {
                self.unseen.remove(*id);
            }
        }
        let ending: Vec<(String, f64)> = self.ending.iter().map(|(k, v)| (k.clone(), *v)).collect();
        for (id, since) in ending {
            let Some((_, s, sending)) = sessions.iter().find(|(x, ..)| *x == id) else {
                self.ending.remove(&id);
                continue;
            };
            if busy_of(s, *sending) {
                self.ending.remove(&id);
                continue;
            }
            if at - since < SETTLE_MS {
                continue;
            }
            self.ending.remove(&id);
            match failure_of(s) {
                Some((limit, message)) => events.push(AttentionEvent::Failed { session_id: id.clone(), limit, message }),
                None => {
                    events.push(AttentionEvent::Done { session_id: id.clone() });
                    if !(viewing == Some(id.as_str()) && focused) {
                        self.unseen.insert(id.clone(), ());
                    }
                }
            }
        }
        if let (Some(v), true) = (viewing, focused) {
            self.unseen.remove(v);
        }
        events
    }

    pub fn forget(&mut self, id: &str) {
        self.prev.remove(id);
        self.mark_seen(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::*;
    fn st(busy: bool) -> ChatState {
        let mut s = ChatState::new("s");
        s.status = if busy { SessionRunStatus::Busy } else { SessionRunStatus::Idle };
        s
    }
    #[test]
    fn peralihan() {
        let mut a = AttentionTracker::default();
        assert!(a.observe(&[("s", &st(true), false)], None, true, 0.).is_empty());
        assert!(a.observe(&[("s", &st(false), false)], None, true, 10.).is_empty());
        assert!(a.has_pending());
        let ev = a.observe(&[("s", &st(false), false)], None, true, 400.);
        assert_eq!(ev, vec![AttentionEvent::Done { session_id: "s".into() }]);
        assert_eq!(a.indicator("s", Some(&st(false)), false), Indicator::Done);
        let mut f = st(false);
        f.error = Some("429 Too Many Requests".into());
        assert_eq!(a.indicator("x", Some(&f), false), Indicator::Limit);
        assert!(is_limit_error("quota exceeded") && !is_limit_error("boom"));
        a.observe(&[("s", &st(false), false)], Some("s"), true, 500.);
        assert_eq!(a.indicator("s", Some(&st(false)), false), Indicator::Idle);
    }
}

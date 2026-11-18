//! Pencarian global lintas sesi — port packages/core/src/chat/session-search.ts (fungsi murni: snippetAt,
//! searchSession, rankResults). Pemuatan pesan dan cache per sesi ada di shell/global_search.rs.
use super::chat::message_text;
use super::engine::SessionInfo;
use super::projects::session_title;
use super::types::{ChatMessage, Role};

pub const MAX_HITS: usize = 3;
const CONTEXT: usize = 48;

#[derive(Clone, Debug, PartialEq)]
pub struct SessionHit {
    pub message_id: String,
    pub user: bool,
    pub snippet: String,
    /// [awal, akhir) dalam karakter di `snippet`.
    pub mat: (usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionResult {
    pub key: String,
    pub session: SessionInfo,
    pub title_match: bool,
    pub hits: Vec<SessionHit>,
    pub total: usize,
}

fn squash(s: &str) -> String {
    let mut out = String::new();
    let mut ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !ws {
                out.push(' ');
            }
            ws = true;
        } else {
            out.push(c);
            ws = false;
        }
    }
    out
}

/// Potongan di sekitar kecocokan (indeks dalam karakter).
pub fn snippet_at(text: &[char], at: usize, len: usize) -> (String, (usize, usize)) {
    let start = at.saturating_sub(CONTEXT);
    let end = (at + len + CONTEXT).min(text.len());
    let pre = format!("{}{}", if start > 0 { "…" } else { "" }, squash(&text[start..at].iter().collect::<String>()).trim_start());
    let hit: String = text[at..at + len].iter().collect();
    let post = format!("{}{}", squash(&text[at + len..end].iter().collect::<String>()).trim_end(), if end < text.len() { "…" } else { "" });
    let a = pre.chars().count();
    (format!("{pre}{hit}{post}"), (a, a + len))
}

pub fn search_session(key: &str, session: &SessionInfo, messages: &[ChatMessage], query: &str) -> Option<SessionResult> {
    let q: Vec<char> = query.trim().to_lowercase().chars().collect();
    if q.is_empty() {
        return None;
    }
    let qs: String = q.iter().collect();
    let title_match = session_title(session).unwrap_or("").to_lowercase().contains(&qs);
    let mut hits = vec![];
    let mut total = 0;
    for m in messages {
        let text: Vec<char> = message_text(m).chars().collect();
        // perbandingan per karakter huruf kecil (panjang karakter tetap untuk huruf umum)
        let lower: Vec<char> = text.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
        let mut i = 0;
        while i + q.len() <= lower.len() {
            if lower[i..i + q.len()] == q[..] {
                total += 1;
                if hits.len() < MAX_HITS && !hits.iter().any(|h: &SessionHit| h.message_id == m.info.id) {
                    let (snippet, mat) = snippet_at(&text, i, q.len());
                    hits.push(SessionHit { message_id: m.info.id.clone(), user: m.info.role == Role::User, snippet, mat });
                }
                i += q.len();
            } else {
                i += 1;
            }
        }
    }
    if !title_match && total == 0 {
        return None;
    }
    Some(SessionResult { key: key.to_string(), session: session.clone(), title_match, hits, total })
}

pub fn rank_results(mut r: Vec<SessionResult>) -> Vec<SessionResult> {
    r.sort_by(|a, b| b.title_match.cmp(&a.title_match).then(b.total.cmp(&a.total)).then(b.session.updated.partial_cmp(&a.session.updated).unwrap_or(std::cmp::Ordering::Equal)));
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn potongan() {
        let t: Vec<char> = "halo   dunia ini teks".chars().collect();
        let (s, m) = snippet_at(&t, 7, 5);
        assert_eq!(s, "halo dunia ini teks");
        assert_eq!(&s.chars().collect::<Vec<_>>()[m.0..m.1].iter().collect::<String>(), "dunia");
        let long: Vec<char> = format!("{}X{}", "a".repeat(100), "b".repeat(100)).chars().collect();
        let (s, _) = snippet_at(&long, 100, 1);
        assert!(s.starts_with('…') && s.ends_with('…'));
    }
}

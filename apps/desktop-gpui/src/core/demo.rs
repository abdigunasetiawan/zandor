//! Tiga sesi DEMO bawaan (disematkan, permanen, hanya-baca) — port apps/desktop/src/shell/demo-sessions.ts.
//! Isinya diekspor dari sumber React yang sama (`bun apps/desktop-gpui/scripts/export-demos.ts` →
//! `assets/demo-sessions.json`) sehingga hanya ada satu sumber kebenaran. Statis, diberi label "Demo", tidak dikirim ke
//! engine dan tidak disimpan di riwayat sesi (ADR-043).
use super::chat::ChatState;
use super::types::{to_message, to_permission, to_question, to_todos};
use serde_json::Value;
use std::sync::OnceLock;

/// Kunci "project" untuk sesi demo (bukan id project nyata maupun CHAT_KEY).
pub const DEMO_KEY: &str = "demo";

#[derive(Clone, Debug)]
pub struct DemoDef {
    pub id: String,
    pub title_key: String,
    pub subtitle_key: String,
    /// Keadaan awal (izin/pertanyaan bisa dijawab lokal lalu di-reset).
    pub state: ChatState,
}

fn parse(raw: &str) -> Vec<DemoDef> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) else { return vec![] };
    items
        .iter()
        .filter_map(|d| {
            let id = d["id"].as_str()?.to_string();
            let mut st = ChatState::new(d["sessionID"].as_str().unwrap_or(&id));
            st.loaded = true;
            st.messages = d["messages"].as_array().map(|a| a.iter().filter_map(to_message).collect()).unwrap_or_default();
            st.permissions = d["permissions"].as_array().map(|a| a.iter().filter_map(to_permission).collect()).unwrap_or_default();
            st.questions = d["questions"].as_array().map(|a| a.iter().filter_map(to_question).collect()).unwrap_or_default();
            st.todos = to_todos(d.get("todos"));
            Some(DemoDef { id, title_key: d["titleKey"].as_str()?.to_string(), subtitle_key: d["subtitleKey"].as_str().unwrap_or_default().to_string(), state: st })
        })
        .collect()
}

/// Daftar sesi demo (dibaca sekali).
pub fn demos() -> &'static [DemoDef] {
    static D: OnceLock<Vec<DemoDef>> = OnceLock::new();
    D.get_or_init(|| parse(include_str!("../../assets/demo-sessions.json")))
}

pub fn demo_by_id(id: &str) -> Option<&'static DemoDef> {
    demos().iter().find(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn tiga_demo_terbaca_utuh() {
        let d = super::demos();
        assert_eq!(d.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["demo-trajectory", "demo-render", "demo-ask"]);
        assert!(d.iter().all(|x| !x.state.messages.is_empty() && x.state.loaded));
        let ask = super::demo_by_id("demo-ask").unwrap();
        assert_eq!((ask.state.permissions.len(), ask.state.questions.len(), ask.state.todos.len()), (3, 1, 5));
        // pesan asisten demo membawa bagian (teks, tool, berkas) — parser engine tidak membuang isinya
        assert!(d.iter().all(|x| x.state.messages.iter().all(|m| !m.parts.is_empty())));
    }
}

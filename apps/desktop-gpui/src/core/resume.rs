//! Lanjutkan giliran yang terputus (aplikasi ditutup, engine mati, atau dimatikan paksa di tengah giliran).
//!
//! Giliran terputus = pesan asisten tanpa `completed` dan tanpa galat, padahal engine melaporkan sesi idle. Engine
//! tidak pernah menutup pesan seperti itu, jadi UI harus mengenalinya sendiri (timer berhenti, kartu "terputus").
//!
//! "Lanjutkan" mengirim pesan pengguna baru berisi teks pendek yang terlihat + satu part teks **sintetis** berawalan
//! [`TAG`] (pola sama dengan `<zandor-attachments>`): permintaan awal pengguna dan ringkasan langkah yang sudah
//! dikerjakan, supaya model tidak mengulang dari nol. Part sintetis itu ditampilkan di detail proses giliran berikutnya.
use super::types::{ChatMessage, MessageInfo, MessagePart, PartKind, Role, Todo, TodoStatus, ToolStatus};

pub const TAG: &str = "<zandor-resume>";
const TAG_END: &str = "</zandor-resume>";
/// Batas langkah yang diringkas (yang terbaru); sisanya cukup disebut jumlahnya.
const MAX_STEPS: usize = 40;
const CLIP: usize = 160;

/// Pesan asisten ini terputus? (`session_busy` = engine masih menjalankan sesi; selama itu pesan memang belum selesai.)
pub fn is_interrupted(info: &MessageInfo, session_busy: bool) -> bool {
    info.role == Role::Assistant && info.completed.is_none() && info.error.is_none() && !session_busy
}

/// Waktu aktivitas terakhir yang tercatat di part (akhir/mulai tool). Dipakai sebagai akhir durasi giliran terputus,
/// supaya timer tidak terus berjalan sampai sekarang.
pub fn last_activity(parts: &[&MessagePart]) -> Option<f64> {
    parts
        .iter()
        .filter_map(|p| match &p.kind {
            PartKind::Tool { state, .. } => state.time.map(|(a, b)| b.unwrap_or(a)),
            _ => None,
        })
        .max_by(f64::total_cmp)
}

/// Isi catatan lanjutan pada pesan pengguna (part sintetis berawalan [`TAG`]), bila ada.
pub fn resume_note(m: &ChatMessage) -> Option<&str> {
    m.parts.iter().find_map(|p| match &p.kind {
        PartKind::Text { text, synthetic: true } if text.starts_with(TAG) => Some(text.as_str()),
        _ => None,
    })
}

fn clip(s: &str) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > CLIP { format!("{}…", one.chars().take(CLIP).collect::<String>()) } else { one }
}

/// Satu baris per tool call: tanda status · nama tool — ringkasan masukan → potongan keluaran / galat.
fn step_line(tool: &str, p: &PartKind) -> Option<String> {
    let PartKind::Tool { state, .. } = p else { return None };
    let what = clip(&super::trajectory::tool_summary(tool, &state.input, state.title.as_deref()));
    let what = if what.is_empty() { String::new() } else { format!(" — {what}") };
    Some(match state.status {
        ToolStatus::Completed => {
            let out = state.output.as_deref().map(clip).filter(|s| !s.is_empty()).map(|s| format!(" → {s}")).unwrap_or_default();
            format!("[selesai] {tool}{what}{out}")
        }
        ToolStatus::Error => format!("[galat] {tool}{what} → {}", clip(state.error.as_deref().unwrap_or(""))),
        ToolStatus::Running | ToolStatus::Pending => format!("[terputus, mungkin setengah jadi] {tool}{what}"),
    })
}

/// Susun part sintetis untuk melanjutkan. `request` = teks permintaan awal pengguna; `progress` = semua pesan asisten
/// sejak permintaan itu (termasuk percobaan lanjutan sebelumnya); `todos` = rencana sesi saat ini.
pub fn build_note(request: &str, progress: &[&ChatMessage], todos: &[Todo]) -> String {
    let mut steps: Vec<String> = vec![];
    let mut last_text: Option<String> = None;
    for m in progress.iter().filter(|m| m.info.role == Role::Assistant) {
        for p in &m.parts {
            match &p.kind {
                PartKind::Tool { tool, .. } => steps.extend(step_line(tool, &p.kind)),
                PartKind::Text { text, synthetic: false } if !text.trim().is_empty() => last_text = Some(text.clone()),
                _ => {}
            }
        }
    }
    let mut out = vec![
        TAG.to_string(),
        "Giliran sebelumnya terputus sebelum selesai (aplikasi ditutup atau engine berhenti). Lanjutkan pekerjaan dari titik terakhir.".to_string(),
        "Jangan mengulang langkah yang sudah selesai. Periksa dulu keadaan berkas/hasil yang relevan, karena langkah yang terputus bisa setengah jadi.".to_string(),
        String::new(),
        "Permintaan awal pengguna:".to_string(),
        request.trim().to_string(),
        String::new(),
    ];
    if steps.is_empty() {
        out.push("Progres sebelumnya: belum ada langkah tool yang tercatat.".into());
    } else {
        out.push(format!("Progres sebelumnya ({} langkah):", steps.len()));
        let skip = steps.len().saturating_sub(MAX_STEPS);
        if skip > 0 {
            out.push(format!("({skip} langkah awal tidak dicantumkan)"));
        }
        out.extend(steps.iter().enumerate().skip(skip).map(|(n, s)| format!("{}. {s}", n + 1)));
    }
    if !todos.is_empty() {
        out.push(String::new());
        out.push("Rencana saat terputus:".into());
        out.extend(todos.iter().map(|t| {
            let mark = match t.status {
                TodoStatus::Completed => "[x]",
                TodoStatus::InProgress => "[sedang]",
                TodoStatus::Cancelled => "[batal]",
                TodoStatus::Pending => "[ ]",
            };
            format!("- {mark} {}", t.content)
        }));
    }
    if let Some(t) = last_text {
        out.push(String::new());
        out.push(format!("Teks terakhir asisten: {}", clip(&t)));
    }
    out.push(TAG_END.into());
    out.join("\n")
}

/// Cari bahan lanjutan dari riwayat yang terlihat: (permintaan awal, pesan asisten sejak itu). Pesan pengguna yang
/// sendiri adalah pesan "Lanjutkan" dilewati agar lanjutan berulang tetap merujuk ke permintaan asli.
pub fn gather(visible: &[ChatMessage]) -> Option<(String, Vec<&ChatMessage>)> {
    let ix = visible.iter().rposition(|m| m.info.role == Role::User && resume_note(m).is_none())?;
    let request = super::chat::message_text(&visible[ix]);
    let progress = visible[ix + 1..].iter().filter(|m| m.info.role == Role::Assistant).collect();
    Some((request, progress))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Priority, ToolState};

    fn info(id: &str, role: Role, completed: Option<f64>) -> MessageInfo {
        MessageInfo { id: id.into(), session_id: "s".into(), role, created: 1., completed, parent_id: None, agent: None, provider_id: None, model_id: None, finish: None, error: None, tokens: None, cost: None }
    }
    fn tool(id: &str, name: &str, status: ToolStatus, cmd: &str, out: Option<&str>, time: (f64, Option<f64>)) -> MessagePart {
        let mut input = serde_json::Map::new();
        input.insert("command".into(), serde_json::json!(cmd));
        MessagePart { id: id.into(), message_id: "a".into(), session_id: "s".into(), kind: PartKind::Tool { tool: name.into(), call_id: id.into(), state: ToolState { status, input, output: out.map(String::from), error: None, title: None, metadata: None, time: Some(time) } } }
    }
    fn text(id: &str, t: &str, synthetic: bool) -> MessagePart {
        MessagePart { id: id.into(), message_id: "m".into(), session_id: "s".into(), kind: PartKind::Text { text: t.into(), synthetic } }
    }

    #[test]
    fn deteksi_terputus() {
        let mut a = info("a", Role::Assistant, None);
        assert!(is_interrupted(&a, false));
        assert!(!is_interrupted(&a, true), "masih berjalan di engine");
        a.completed = Some(2.);
        assert!(!is_interrupted(&a, false));
        let u = info("u", Role::User, None);
        assert!(!is_interrupted(&u, false));
    }

    #[test]
    fn aktivitas_terakhir_dari_tool() {
        let a = tool("t1", "bash", ToolStatus::Completed, "ls", None, (10., Some(20.)));
        let b = tool("t2", "bash", ToolStatus::Running, "npm i", None, (30., None));
        assert_eq!(last_activity(&[&a, &b]), Some(30.));
        assert_eq!(last_activity(&[]), None);
    }

    #[test]
    fn catatan_berisi_permintaan_progres_dan_rencana() {
        let user = ChatMessage { info: info("u1", Role::User, None), parts: vec![text("p0", "buat website", false)] };
        let asst = ChatMessage {
            info: info("a1", Role::Assistant, None),
            parts: vec![
                tool("t1", "bash", ToolStatus::Completed, "mkdir site", Some("ok"), (1., Some(2.))),
                text("p1", "Saya buat struktur dulu.", false),
                tool("t2", "bash", ToolStatus::Running, "npm install", None, (3., None)),
            ],
        };
        let todos = vec![Todo { content: "Struktur".into(), status: TodoStatus::Completed, priority: Priority::Medium }, Todo { content: "Halaman utama".into(), status: TodoStatus::Pending, priority: Priority::Medium }];
        let msgs = vec![user, asst];
        let (req, progress) = gather(&msgs).unwrap();
        let note = build_note(&req, &progress, &todos);
        assert!(note.starts_with(TAG) && note.ends_with(TAG_END));
        assert!(note.contains("buat website"));
        assert!(note.contains("1. [selesai] bash — mkdir site → ok"));
        assert!(note.contains("2. [terputus, mungkin setengah jadi] bash — npm install"));
        assert!(note.contains("- [x] Struktur") && note.contains("- [ ] Halaman utama"));
        assert!(note.contains("Teks terakhir asisten: Saya buat struktur dulu."));
    }

    #[test]
    fn lanjutan_berulang_merujuk_permintaan_asli() {
        let msgs = vec![
            ChatMessage { info: info("u1", Role::User, None), parts: vec![text("p0", "buat website", false)] },
            ChatMessage { info: info("a1", Role::Assistant, None), parts: vec![tool("t1", "write", ToolStatus::Completed, "x", None, (1., Some(2.)))] },
            ChatMessage { info: info("u2", Role::User, None), parts: vec![text("p1", "Lanjutkan", false), text("p2", &format!("{TAG}\n…\n{TAG_END}"), true)] },
            ChatMessage { info: info("a2", Role::Assistant, None), parts: vec![tool("t2", "bash", ToolStatus::Running, "y", None, (3., None))] },
        ];
        let (req, progress) = gather(&msgs).unwrap();
        assert_eq!(req, "buat website");
        assert_eq!(progress.len(), 2, "progres mencakup giliran asli dan lanjutan");
        assert!(resume_note(&msgs[2]).is_some());
        assert!(resume_note(&msgs[0]).is_none());
    }
}

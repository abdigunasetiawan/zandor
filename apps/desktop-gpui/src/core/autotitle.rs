//! Penamaan ulang sesi otomatis. Judul bawaan engine dibuat dari pesan pertama saja, jadi sesi yang dibuka dengan
//! sapaan ("halo") bertahan dengan judul sapaan walau topik sebenarnya baru muncul di pesan berikutnya.
//!
//! Tidak ada daftar kata sapaan: modellah yang menilai. Satu panggilan model kecil menerima awal percakapan dan judul
//! saat ini, lalu menjawab salah satu dari tiga hal (`Verdict`): judul baru, "judul sekarang sudah mewakili topik"
//! ([`KEEP`]), atau "belum ada topik" ([`NO_TOPIC`]). Hanya dua penanda itu yang berupa protokol tetap.
//!
//! Hanya judul yang berasal dari engine/Zandor yang boleh diubah; judul yang pernah diganti pengguna tidak disentuh
//! (lihat `ProjectStore::begin_retitle`).
use super::types::{ChatMessage, PartKind, Role};

/// Jumlah giliran pengguna pertama yang masih boleh memicu penamaan ulang.
pub const MAX_TURNS: usize = 3;
/// Penanda jawaban model: percakapan belum punya topik (mis. baru sapaan) — jangan ubah judul, coba lagi nanti.
pub const NO_TOPIC: &str = "BELUM ADA TOPIK";
/// Penanda jawaban model: judul saat ini sudah mewakili topik — berhenti menilai ulang.
pub const KEEP: &str = "TETAP";
const MAX_TITLE_CHARS: usize = 60;
const CLIP_CHARS: usize = 400;

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Keep,
    NoTopic,
    Title(String),
}

/// Jumlah pesan pengguna (bukan sintetis) pada riwayat.
pub fn user_turns(msgs: &[ChatMessage]) -> usize {
    msgs.iter().filter(|m| m.info.role == Role::User && !text_of(m).trim().is_empty()).count()
}

/// Teks pesan: gabungan bagian teks non-sintetis.
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

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}…")
}

/// Awal percakapan: tiap pesan pengguna (maks `MAX_TURNS`) beserta ringkasan balasan asisten, masing-masing dipotong.
pub fn transcript(msgs: &[ChatMessage]) -> String {
    let mut out = String::new();
    let mut users = 0;
    for m in msgs {
        let t = text_of(m);
        if t.trim().is_empty() {
            continue;
        }
        match m.info.role {
            Role::User => {
                if users == MAX_TURNS {
                    break;
                }
                users += 1;
                out.push_str(&format!("Pengguna: {}\n", clip(&t, CLIP_CHARS)));
            }
            _ if users > 0 => out.push_str(&format!("Asisten: {}\n", clip(&t, CLIP_CHARS))),
            _ => {}
        }
    }
    out
}

/// Prompt untuk model kecil.
pub fn prompt(current_title: &str, transcript: &str) -> String {
    format!(
        "Anda memberi judul pada sebuah percakapan. Berikut awal percakapannya:\n\n{transcript}\n\
         Judul saat ini: \"{current_title}\"\n\n\
         Tulis judul singkat (maksimal 6 kata, bahasa yang sama dengan pengguna, tanpa tanda kutip dan tanpa titik) \
         yang menggambarkan topik atau tujuan pengguna.\n\
         - Jika judul saat ini sudah mewakili topik dengan baik, balas persis: {KEEP}\n\
         - Jika percakapan belum punya topik yang jelas (misalnya baru sapaan atau basa-basi), balas persis: {NO_TOPIC}\n\
         Balas hanya dengan judulnya atau salah satu penanda di atas, tanpa penjelasan."
    )
}

/// Tafsirkan jawaban model. Jawaban kosong dianggap belum ada topik (tidak mengubah apa pun).
pub fn parse_reply(raw: &str) -> Verdict {
    let line = raw.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let line = line.trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '*' | '#' | '“' | '”' | '‘' | '’')).trim().trim_end_matches('.').trim();
    if line.is_empty() || line.to_uppercase().contains(NO_TOPIC) {
        return Verdict::NoTopic;
    }
    if line.eq_ignore_ascii_case(KEEP) {
        return Verdict::Keep;
    }
    Verdict::Title(clip(line, MAX_TITLE_CHARS).trim_end_matches('…').trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{MessageInfo, MessagePart};

    fn msg(id: &str, role: Role, text: &str) -> ChatMessage {
        ChatMessage {
            info: MessageInfo {
                id: id.into(),
                session_id: "s".into(),
                role,
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
            parts: vec![MessagePart { id: format!("p{id}"), message_id: id.into(), session_id: "s".into(), kind: PartKind::Text { text: text.into(), synthetic: false } }],
        }
    }

    #[test]
    fn jawaban_model_ditafsirkan() {
        assert_eq!(parse_reply("TETAP"), Verdict::Keep);
        assert_eq!(parse_reply("  tetap. "), Verdict::Keep);
        assert_eq!(parse_reply("BELUM ADA TOPIK"), Verdict::NoTopic);
        assert_eq!(parse_reply("\"Belum ada topik.\""), Verdict::NoTopic);
        assert_eq!(parse_reply(""), Verdict::NoTopic);
        assert_eq!(parse_reply("\"Belajar Rust dasar\"\nPenjelasan lain"), Verdict::Title("Belajar Rust dasar".into()));
        assert_eq!(parse_reply("**Tetap semangat belajar**"), Verdict::Title("Tetap semangat belajar".into()));
        let panjang = "kata ".repeat(40);
        let Verdict::Title(t) = parse_reply(&panjang) else { panic!("harus judul") };
        assert!(t.chars().count() <= MAX_TITLE_CHARS);
    }

    #[test]
    fn transkrip_dibatasi_giliran_dan_panjang() {
        let mut m = vec![];
        for i in 0..5 {
            m.push(msg(&format!("u{i}"), Role::User, &format!("pesan {i}")));
            m.push(msg(&format!("a{i}"), Role::Assistant, &"x".repeat(1000)));
        }
        let t = transcript(&m);
        assert_eq!(t.matches("Pengguna:").count(), MAX_TURNS);
        assert!(!t.contains("pesan 3"));
        assert!(t.contains('…'));
        assert_eq!(user_turns(&m), 5);
    }

    #[test]
    fn prompt_memuat_penanda_dan_judul() {
        let p = prompt("Sapaan", "Pengguna: halo\n");
        assert!(p.contains(KEEP) && p.contains(NO_TOPIC) && p.contains("\"Sapaan\"") && p.contains("halo"));
    }
}

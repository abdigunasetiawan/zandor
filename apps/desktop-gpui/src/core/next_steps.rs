//! Kesimpulan + saran aksi di akhir jawaban asisten ("langkah berikutnya").
//!
//! Dirancang agar jalan di model apa pun, murah/mahal, pintar/bodoh: model yang menjawab TIDAK diminta menulisnya
//! (model lemah sering lupa atau salah format). Setelah giliran selesai, Zandor membuat satu panggilan singkat ke model
//! kecil (peran `small_model`, atau model sesi bila tidak diatur) dengan permintaan pengguna + jawaban, memakai format
//! BARIS sederhana (bukan JSON) yang diurai longgar dan divalidasi ketat (panjang, jumlah, duplikat). Bila panggilan
//! gagal atau balasannya tak terbaca, dipakai heuristik: paragraf penutup jawaban panjang sebagai ringkasan, tanpa aksi.
//! Aksi tidak pernah dikarang tanpa model: lebih baik tanpa tombol daripada tombol asal.
use super::types::{ChatMessage, PartKind, Role};

/// Jawaban obrolan/penjelasan lebih pendek dari ini (karakter) tidak diberi kartu: ringkasannya cuma mengulang.
pub const MIN_ANSWER_CHARS: usize = 500;
/// Giliran yang memakai tool (agent mengerjakan sesuatu) layak diringkas walau teks akhirnya pendek.
pub const MIN_WORK_ANSWER_CHARS: usize = 80;
/// Penanda balasan model: jawaban tidak butuh tindak lanjut, jangan tampilkan kartu.
pub const SKIP: &str = "TIDAK PERLU";

/// Layak dibuatkan kartu? (murah, sebelum memanggil model)
pub fn worth_asking(answer: &str, worked: bool) -> bool {
    answer.chars().count() >= if worked { MIN_WORK_ANSWER_CHARS } else { MIN_ANSWER_CHARS }
}

/// Model menilai jawaban tidak butuh kartu (baris pertama berisi penanda, tanpa kunci RINGKAS/AKSI).
pub fn is_skip(raw: &str) -> bool {
    let up = raw.to_uppercase();
    let first = up.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    first.contains(SKIP) && !up.contains("RINGKAS:") && !up.contains("AKSI: ") || first.trim_matches(|c: char| !c.is_alphanumeric()) == "SKIP"
}
/// Jawaban panjang tanpa hasil model memakai heuristik paragraf penutup.
pub const FALLBACK_MIN_CHARS: usize = 600;
pub const MAX_ACTIONS: usize = 3;
const MAX_RECAP: usize = 280;
const MAX_ASK: usize = 120;
const MAX_LABEL: usize = 24;
const MAX_MESSAGE: usize = 300;
const CLIP_REQUEST: usize = 600;
const CLIP_ANSWER: usize = 2500;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub label: String,
    /// Pesan yang dikirim sebagai pesan pengguna bila tombol diklik.
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct NextSteps {
    /// Id pesan pengguna yang jawabannya diringkas (kartu hanya tampil selama itu pesan pengguna terakhir).
    pub for_msg: String,
    pub recap: String,
    pub ask: Option<String>,
    pub actions: Vec<Action>,
    /// Indeks aksi yang dipilih pengguna (kartu menetap di riwayat sebagai catatan jawaban).
    pub chosen: Option<usize>,
}

/// Pertukaran terakhir: (id pesan pengguna, permintaan, jawaban asisten, memakai tool). None bila belum ada jawaban yang selesai & utuh.
pub fn last_exchange(msgs: &[ChatMessage]) -> Option<(String, String, String, bool)> {
    let u = msgs.iter().rposition(|m| m.info.role == Role::User && !text_of(m).trim().is_empty())?;
    let replies = &msgs[u + 1..];
    let last = replies.iter().rev().find(|m| m.info.role == Role::Assistant)?;
    if last.info.completed.is_none() || last.info.error.is_some() {
        return None;
    }
    let answer = replies.iter().filter(|m| m.info.role == Role::Assistant).map(text_of).filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n\n");
    if answer.trim().is_empty() {
        return None;
    }
    let worked = replies.iter().any(|m| m.parts.iter().any(|p| matches!(p.kind, PartKind::Tool { .. })));
    Some((msgs[u].info.id.clone(), text_of(&msgs[u]), answer, worked))
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

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    // potong di batas kata bila ada
    let cut = match cut.rfind(char::is_whitespace) {
        Some(i) if i > max / 2 => cut[..i].to_string(),
        _ => cut,
    };
    format!("{}…", cut.trim_end_matches(|c: char| c.is_whitespace() || ",;:".contains(c)))
}

/// Ekor teks (kesimpulan biasanya di akhir).
fn tail(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.trim().to_string();
    }
    format!("…{}", s.chars().skip(n - max).collect::<String>().trim())
}

/// Prompt untuk model kecil.
pub fn prompt(request: &str, answer: &str) -> String {
    format!(
        "Anda membantu pengguna melanjutkan percakapan dengan asisten AI.\n\n\
         Permintaan pengguna:\n{}\n\n\
         Jawaban asisten:\n{}\n\n\
         Tulis kesimpulan dan saran langkah lanjut dengan FORMAT BARIS PERSIS ini, dalam bahasa yang sama dengan pengguna, \
         tanpa markdown dan tanpa teks lain:\n\
         RINGKAS: <kesimpulan 1-2 kalimat: apa hasilnya atau apa yang sudah dilakukan>\n\
         TANYA: <satu pertanyaan pendek tentang langkah berikutnya, atau tanda ->\n\
         AKSI: <label 1-3 kata> | <kalimat lengkap yang akan dikirim pengguna sebagai pesan berikutnya>\n\
         AKSI: <label> | <pesan>\n\n\
         ATURAN AKSI (wajib):\n\
         - Setiap aksi harus langsung berlanjut dari jawaban asisten di atas dan tujuan pengguna di permintaannya.\n\
         - Jangan membuka topik baru, jangan menebak keinginan pengguna, dan jangan menyebut nama/hal yang tidak tertulis \
         di percakapan ini.\n\
         - Lebih baik 1-2 aksi yang pasti nyambung daripada {MAX_ACTIONS} yang menebak.\n\
         - Pesan aksi ditulis sebagai pengguna berbicara kepada asisten dan harus bisa dikerjakan di percakapan ini.\n\
         - Bila jawaban berupa daftar/rekap banyak hal, hanya boleh menyebut item yang benar-benar ada di daftar itu.\n\
         Jika tidak ada langkah lanjut yang berguna, tulis satu baris: AKSI: -\n\
         Jika jawaban hanya penjelasan, obrolan, atau jawaban lengkap yang tidak butuh tindak lanjut (misalnya pertanyaan \
         pengetahuan atau sapaan), jangan tulis format di atas; balas persis: {SKIP}",
        clip(request, CLIP_REQUEST),
        tail(answer, CLIP_ANSWER)
    )
}

fn is_empty_value(v: &str) -> bool {
    let v = v.trim().trim_matches(|c: char| matches!(c, '-' | '—' | '–' | '.' | '"' | '*' | '`')).trim().to_lowercase();
    v.is_empty() || matches!(v.as_str(), "tidak ada" | "none" | "n/a" | "na" | "null" | "kosong")
}

fn clean(s: &str) -> String {
    s.trim().trim_matches(|c: char| matches!(c, '*' | '`' | '"' | '“' | '”' | '_')).trim().to_string()
}

fn derive_label(message: &str) -> String {
    clip(&message.split_whitespace().take(3).collect::<Vec<_>>().join(" "), MAX_LABEL)
}

fn push_action(out: &mut Vec<Action>, label: &str, message: &str) {
    let message = clean(message);
    let label = clean(label);
    if is_empty_value(&message) || out.len() >= MAX_ACTIONS {
        return;
    }
    let label = if is_empty_value(&label) { derive_label(&message) } else { clip(&label, MAX_LABEL) };
    let message = clip(&message, MAX_MESSAGE);
    if label.is_empty() || out.iter().any(|a| a.label.eq_ignore_ascii_case(&label) || a.message == message) {
        return;
    }
    out.push(Action { label, message });
}

/// Tafsirkan balasan model. Longgar terhadap huruf besar/kecil, bullet, markdown, dan JSON; ketat pada hasil akhir.
pub fn parse_reply(raw: &str, for_msg: &str) -> Option<NextSteps> {
    let t = raw.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    if t.starts_with('{') {
        if let Some(ns) = parse_json(t, for_msg) {
            return Some(ns);
        }
    }
    let mut ns = NextSteps { for_msg: for_msg.to_string(), ..Default::default() };
    for line in t.lines() {
        let l = line.trim().trim_start_matches(|c: char| matches!(c, '-' | '*' | '•' | '#' | '>' | ' ') || c.is_ascii_digit() || c == '.' || c == ')').trim();
        let l = l.trim_start_matches(['*', '_']);
        let Some((k, v)) = l.split_once(':') else { continue };
        let key = k.trim().trim_matches(['*', '_', '`']).to_uppercase();
        let v = v.trim();
        match key.as_str() {
            "RINGKAS" | "RINGKASAN" | "KESIMPULAN" | "RECAP" | "SUMMARY" if ns.recap.is_empty() && !is_empty_value(v) => ns.recap = clip(&clean(v), MAX_RECAP),
            "TANYA" | "PERTANYAAN" | "ASK" | "QUESTION" if ns.ask.is_none() && !is_empty_value(v) => ns.ask = Some(clip(&clean(v), MAX_ASK)),
            "AKSI" | "ACTION" | "LANGKAH" | "NEXT" => match v.split_once('|') {
                Some((label, message)) => push_action(&mut ns.actions, label, message),
                None => push_action(&mut ns.actions, "", v),
            },
            _ => {}
        }
    }
    (!ns.recap.is_empty() || !ns.actions.is_empty()).then_some(ns)
}

fn parse_json(t: &str, for_msg: &str) -> Option<NextSteps> {
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let s = |keys: &[&str]| keys.iter().find_map(|k| v.get(*k)?.as_str()).map(clean).filter(|x| !is_empty_value(x));
    let mut ns = NextSteps { for_msg: for_msg.to_string(), ..Default::default() };
    ns.recap = clip(&s(&["recap", "ringkas", "ringkasan", "summary"]).unwrap_or_default(), MAX_RECAP);
    ns.ask = s(&["ask", "tanya", "question"]).map(|a| clip(&a, MAX_ASK));
    for a in v.get("actions").or_else(|| v.get("aksi")).and_then(|a| a.as_array()).into_iter().flatten() {
        match a {
            serde_json::Value::String(m) => push_action(&mut ns.actions, "", m),
            o => push_action(&mut ns.actions, o.get("label").and_then(|x| x.as_str()).unwrap_or(""), o.get("message").or_else(|| o.get("pesan")).and_then(|x| x.as_str()).unwrap_or("")),
        }
    }
    (!ns.recap.is_empty() || !ns.actions.is_empty()).then_some(ns)
}

const STOP_WORDS: &[&str] = &[
    "yang", "untuk", "dengan", "dari", "pada", "adalah", "itu", "ini", "dan", "atau", "saya", "aku", "kamu", "anda", "tolong", "mohon", "bisa", "dulu", "lagi",
    "lanjut", "lanjutkan", "jalankan", "buat", "buatkan", "tambah", "tambahkan", "cek", "periksa", "lakukan", "kerjakan", "mulai", "sekarang", "juga", "ada",
    "the", "and", "for", "with", "that", "this", "please", "next", "step", "now", "can", "you", "run", "make", "add", "check", "continue", "do",
];

fn content_words(s: &str) -> std::collections::HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() >= 3 && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// Buang aksi yang tidak terhubung ke percakapan: tiap aksi harus berbagi kata isi dengan permintaan pengguna atau ujung
/// jawaban (tempat kesimpulan/tawaran biasanya berada); aksi panjang butuh sepertiga kata isinya ada di sana.
/// Bila semua aksi gugur, pertanyaan ikut dibuang (pertanyaan tanpa pilihan yang nyambung hanya mengganggu).
/// Ini penyaring leksikal kasar, bukan pemahaman: pelindung utamanya tetap aturan di prompt.
pub fn ground(mut ns: NextSteps, request: &str, answer: &str) -> NextSteps {
    let mut ctx = content_words(request);
    ctx.extend(content_words(&tail(answer, 1500)));
    ns.actions.retain(|a| {
        let w = content_words(&format!("{} {}", a.label, a.message));
        let hit = w.iter().filter(|x| ctx.contains(*x)).count();
        hit >= 1 && (w.len() < 4 || hit * 3 >= w.len())
    });
    if ns.actions.is_empty() {
        ns.ask = None;
    }
    ns
}

/// Cadangan tanpa model: paragraf penutup jawaban panjang sebagai ringkasan (tanpa aksi). Blok kode/daftar tidak dipakai.
pub fn fallback(answer: &str, for_msg: &str) -> Option<NextSteps> {
    if answer.chars().count() < FALLBACK_MIN_CHARS {
        return None;
    }
    let paras: Vec<&str> = answer.split("\n\n").map(str::trim).collect();
    let p = paras.into_iter().rev().find(|p| p.chars().count() >= 40 && !p.starts_with("```") && !p.starts_with('|') && !p.lines().all(|l| l.trim_start().starts_with(['-', '*', '#']) || l.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit())))?;
    let recap = clip(&p.replace('\n', " "), MAX_RECAP);
    Some(NextSteps { for_msg: for_msg.to_string(), recap, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{MessageInfo, MessagePart};

    fn msg(id: &str, role: Role, text: &str, done: bool) -> ChatMessage {
        ChatMessage {
            info: MessageInfo {
                id: id.into(),
                session_id: "s".into(),
                role,
                created: 0.,
                completed: done.then_some(1.),
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
    fn format_baris_standar() {
        let r = "RINGKAS: Fungsi sudah dibuat dan lulus tes.\nTANYA: Mau saya tambahkan dokumentasi?\nAKSI: Tambah dokumentasi | Tambahkan dokumentasi untuk fungsi tadi\nAKSI: Commit | Buat commit untuk perubahan ini";
        let ns = parse_reply(r, "u1").unwrap();
        assert_eq!(ns.for_msg, "u1");
        assert_eq!(ns.recap, "Fungsi sudah dibuat dan lulus tes.");
        assert_eq!(ns.ask.as_deref(), Some("Mau saya tambahkan dokumentasi?"));
        assert_eq!(ns.actions.len(), 2);
        assert_eq!(ns.actions[1], Action { label: "Commit".into(), message: "Buat commit untuk perubahan ini".into() });
    }

    #[test]
    fn model_lemah_markdown_bullet_dan_huruf_kecil_tetap_terbaca() {
        let r = "Tentu! Berikut hasilnya:\n\n- **ringkas**: \"Server sudah jalan di port 3000.\"\n- **Tanya:** -\n1. aksi: Buka browser | Buka http://localhost:3000 di browser\n2. AKSI : -";
        let ns = parse_reply(r, "u").unwrap();
        assert_eq!(ns.recap, "Server sudah jalan di port 3000.");
        assert_eq!(ns.ask, None);
        assert_eq!(ns.actions, vec![Action { label: "Buka browser".into(), message: "Buka http://localhost:3000 di browser".into() }]);
    }

    #[test]
    fn json_dan_pagar_kode_diterima() {
        let r = "```json\n{\"recap\":\"Selesai.\",\"actions\":[{\"label\":\"Tes\",\"message\":\"Jalankan tes\"},\"Perbaiki galat lint\"]}\n```";
        let ns = parse_reply(r, "u").unwrap();
        assert_eq!(ns.recap, "Selesai.");
        assert_eq!(ns.actions.len(), 2);
        assert_eq!(ns.actions[1].label, "Perbaiki galat lint");
    }

    #[test]
    fn batas_dan_duplikat_dijaga() {
        let panjang = "kata ".repeat(200);
        let r = format!("RINGKAS: {panjang}\nAKSI: A | pesan satu\nAKSI: a | pesan dua\nAKSI: B | pesan satu\nAKSI: C | c\nAKSI: D | d\nAKSI: E | e");
        let ns = parse_reply(&r, "u").unwrap();
        assert!(ns.recap.chars().count() <= MAX_RECAP);
        assert!(ns.actions.len() <= MAX_ACTIONS);
        assert_eq!(ns.actions[0].label, "A");
        assert!(!ns.actions.iter().any(|a| a.label == "B"), "pesan duplikat dibuang");
    }

    #[test]
    fn sampah_atau_kosong_ditolak() {
        assert_eq!(parse_reply("", "u"), None);
        assert_eq!(parse_reply("Maaf, saya tidak bisa membantu.", "u"), None);
        assert_eq!(parse_reply("RINGKAS: -\nTANYA: -\nAKSI: -", "u"), None);
    }

    #[test]
    fn cadangan_hanya_untuk_jawaban_panjang_dan_tanpa_aksi() {
        assert_eq!(fallback("Halo! Ada yang bisa dibantu?", "u"), None);
        let a = format!("{}\n\n```rust\nfn main() {{}}\n```\n\nIntinya: fungsi sudah diperbaiki dan semua tes lulus sekarang.", "Penjelasan panjang. ".repeat(40));
        let ns = fallback(&a, "u").unwrap();
        assert!(ns.recap.starts_with("Intinya"));
        assert!(ns.actions.is_empty() && ns.ask.is_none());
    }

    #[test]
    fn pertukaran_terakhir_hanya_jawaban_utuh() {
        let m = vec![msg("u1", Role::User, "buat fungsi", false), msg("a1", Role::Assistant, "Ini fungsinya", true), msg("a2", Role::Assistant, "Dan tesnya", true)];
        let (id, req, ans, worked) = last_exchange(&m).unwrap();
        assert!(!worked);
        assert_eq!((id.as_str(), req.as_str()), ("u1", "buat fungsi"));
        assert!(ans.contains("Ini fungsinya") && ans.contains("Dan tesnya"));
        let belum = vec![msg("u1", Role::User, "x", false), msg("a1", Role::Assistant, "sebagian", false)];
        assert!(last_exchange(&belum).is_none());
        assert!(last_exchange(&[msg("u1", Role::User, "x", false)]).is_none());
    }

    #[test]
    fn aksi_liar_dibuang_yang_nyambung_dipertahankan() {
        let ns = NextSteps {
            for_msg: "u".into(),
            recap: "Server sudah dibuat.".into(),
            ask: Some("Mau lanjut?".into()),
            actions: vec![
                Action { label: "Jalankan tes".into(), message: "Jalankan tes untuk server tadi".into() },
                Action { label: "Deploy ke AWS".into(), message: "Deploy aplikasi ke AWS lambda dengan terraform".into() },
            ],
            chosen: None,
        };
        let g = ground(ns.clone(), "buat server express sederhana", "Server express sudah dibuat di server.js dan tes sudah ditulis.");
        assert_eq!(g.actions.len(), 1);
        assert_eq!(g.actions[0].label, "Jalankan tes");
        assert!(g.ask.is_some());
        let semua = ground(NextSteps { actions: vec![ns.actions[1].clone()], ..ns }, "buat server express", "Server express sudah dibuat.");
        assert!(semua.actions.is_empty() && semua.ask.is_none());
    }

    #[test]
    fn ambang_dan_penanda_lewati() {
        assert!(!worth_asking("pendek", false) && worth_asking(&"x".repeat(500), false));
        assert!(worth_asking(&"x".repeat(100), true) && !worth_asking("hai", true));
        assert!(is_skip("TIDAK PERLU") && is_skip("  \"Tidak perlu.\" ") && is_skip("skip"));
        assert!(!is_skip("RINGKAS: Selesai.
AKSI: Tes | jalankan tes"));
        assert!(!is_skip("RINGKAS: Itu tidak perlu dilakukan lagi.
AKSI: -"));
    }

    #[test]
    fn prompt_memuat_format_dan_konteks() {
        let p = prompt("buat server", "Server dibuat.");
        assert!(p.contains("RINGKAS:") && p.contains("AKSI:") && p.contains("buat server") && p.contains("Server dibuat."));
    }
}

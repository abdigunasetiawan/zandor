//! Deteksi niat "ini sebaiknya project" untuk pesan pertama di sesi Chat (tanpa project).
//! Murni dan tanpa model: skor dari sinyal (ID + EN) yang menandakan pekerjaan berkas jamak / instalasi / berulang,
//! bukan hanya kode: aplikasi, data, riset, dokumen panjang, media massal, otomasi. Pertanyaan, terjemahan, ringkasan,
//! dan potongan kecil (satu fungsi, satu puisi) sengaja tidak memicu.
use super::re::re;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Build,
    Data,
    Research,
    Document,
    Media,
    Automation,
}

impl Kind {
    /// Kunci i18n: `projSuggest.kind.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Build => "build",
            Kind::Data => "data",
            Kind::Research => "research",
            Kind::Document => "document",
            Kind::Media => "media",
            Kind::Automation => "automation",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Intent {
    pub kind: Kind,
    pub score: i32,
    /// Id sinyal yang cocok (untuk uji dan penjelasan).
    pub signals: Vec<&'static str>,
    /// Nama folder usulan (slug).
    pub name: String,
}

const THRESHOLD: i32 = 4;

struct Signal {
    id: &'static str,
    weight: i32,
    kind: Kind,
    pattern: &'static str,
}

const SIGNALS: &[Signal] = &[
    Signal { id: "install", weight: 3, kind: Kind::Build, pattern: r"\b(npm|pnpm|yarn|bun|pip3?|uv|cargo|composer|gem|go|dotnet)\s+(install|add|init|create|new|run|build)\b|\b(install|instal|memasang|pasang|setup|siapkan|konfigurasi)\w*\s+(dependen\w*|dependencies|package|paket|library|pustaka|framework|environment|venv|docker|database|requirements)\b|package\.json|requirements\.txt|cargo\.toml|docker-?compose|dockerfile" },
    Signal { id: "build-app", weight: 3, kind: Kind::Build, pattern: r"\b(buat|bikin|bangun|kembangkan|create|build|make|develop|scaffold|generate)\w*\b.{0,40}?\b(aplikasi|app|website|web ?site|situs|web|api|backend|frontend|bot|game|plugin|extension|ekstensi|cli|landing page|dashboard|sistem|crud|saas|toko online|e-?commerce)\b" },
    Signal { id: "framework", weight: 2, kind: Kind::Build, pattern: r"\b(react|next\.?js|vue|nuxt|svelte|angular|django|flask|fastapi|laravel|express|nest\.?js|flutter|tauri|electron|spring|rails|astro|vite|tailwind)\b" },
    Signal { id: "repo", weight: 2, kind: Kind::Build, pattern: r"\bgit clone\b|\bclone\s+repo\w*|github\.com/" },
    Signal { id: "multi-file", weight: 2, kind: Kind::Build, pattern: r"\b(beberapa|banyak|multi|semua|seluruh)\s+(file|berkas|folder|halaman|modul)\b|struktur\s+(folder|project|proyek)|multiple files|folder structure|project structure" },
    Signal { id: "run-local", weight: 1, kind: Kind::Build, pattern: r"\blocalhost\b|\bdev server\b|\bport \d{2,5}\b|\bdeploy\w*\b" },
    Signal { id: "data", weight: 3, kind: Kind::Data, pattern: r"\b(dataset|csv|xlsx|excel|spreadsheet|dataframe|pandas|scrap\w*|crawl\w*|etl|jupyter|notebook|analisis data|data analysis|visualisasi data)\b" },
    Signal { id: "research", weight: 3, kind: Kind::Research, pattern: r"\b(riset|research|literature review|tinjauan pustaka|deep research|studi kasus|whitepaper)\b|kumpulkan\s+(referensi|sumber|data|artikel)|banyak sumber|multiple sources" },
    Signal { id: "document", weight: 2, kind: Kind::Document, pattern: r"\b(skripsi|tesis|disertasi|thesis|buku|ebook|e-book|novel|modul ajar|kurikulum|proposal|company profile|business plan|rencana bisnis|handbook|panduan lengkap|laporan lengkap)\b" },
    Signal { id: "long-output", weight: 2, kind: Kind::Document, pattern: r"\b\d{2,}\s*(halaman|slide|bab|chapter|pages)\b|\bbab\s+\d" },
    Signal { id: "media", weight: 4, kind: Kind::Media, pattern: r"\b(edit video|video editing|ffmpeg|batch (convert|resize|rename)|konversi (semua|batch)|kompres (semua|banyak)|subtitle|podcast|bulk)\b|\b(convert|ubah|konversi)\s+semua\b" },
    Signal { id: "automation", weight: 2, kind: Kind::Automation, pattern: r"\b(otomatis\w*|automat\w*|jadwal\w*|schedule[dr]?|cron|webhook|workflow|n8n|scraper|monitoring)\b|\btiap\s+(jam|hari|minggu)\b|\bevery\s+(hour|day|week)\b" },
];

const NEGATIVES: &[(&str, i32, &str)] = &[
    ("question", -2, r"^\s*(apa|apakah|siapa|kenapa|mengapa|bagaimana|gimana|jelaskan|terangkan|tolong jelaskan|what|why|how|explain|who|when|is|are|does|do)\b"),
    ("snippet", -3, r"\b(satu|1)\s+(fungsi|function|regex|query|rumus|formula|snippet|kalimat|paragraf|email|caption)\b|\b(buatkan|tulis\w*|bikin\w*)\s+(sebuah\s+)?(fungsi|function|regex|query|rumus|puisi|caption|email|surat|pantun)\b"),
    ("transform", -4, r"^\s*(terjemah\w*|translate|ringkas\w*|summari[sz]e|rangkum\w*|koreksi|proofread|perbaiki (kalimat|tata bahasa))\b"),
];

/// Daftar bullet/bernomor ≥ 3 baris = tugas bertahap.
fn list_lines(text: &str) -> usize {
    let r = re(r"(?m)^\s*(\d+[.)]|[-*•])\s+\S");
    r.find_iter(text).count()
}

/// Nilai deteksi untuk pesan. `attachments` = jumlah berkas yang dilampirkan.
pub fn detect(text: &str, attachments: usize) -> Option<Intent> {
    let t = text.trim();
    if t.chars().count() < 25 {
        return None;
    }
    let low = t.to_lowercase();
    let mut score = 0;
    let mut signals = vec![];
    let mut by_kind = [0i32; 6];
    for s in SIGNALS {
        if re(s.pattern).is_match(&low) {
            score += s.weight;
            signals.push(s.id);
            by_kind[s.kind as usize] += s.weight;
        }
    }
    if signals.is_empty() {
        return None;
    }
    for (id, w, p) in NEGATIVES {
        if re(p).is_match(&low) {
            score += w;
            signals.push(id);
        }
    }
    if list_lines(t) >= 3 {
        score += 2;
        signals.push("steps");
    }
    let n = t.chars().count();
    if n > 900 {
        score += 2;
        signals.push("long");
    } else if n > 400 {
        score += 1;
        signals.push("long");
    }
    if attachments > 0 {
        score += 1;
        signals.push("attached");
    }
    if score < THRESHOLD {
        return None;
    }
    let kinds = [Kind::Build, Kind::Data, Kind::Research, Kind::Document, Kind::Media, Kind::Automation];
    let kind = kinds.iter().copied().max_by_key(|k| (by_kind[*k as usize], -(*k as i32))).unwrap_or(Kind::Build);
    Some(Intent { kind, score, signals, name: suggest_name(t, kind) })
}

const STOP: &[&str] = &[
    "buat", "buatkan", "buatin", "bikin", "bikinin", "tolong", "saya", "aku", "dong", "ya", "please", "create", "build", "make", "a", "an", "the", "me", "i", "want", "to", "sebuah", "satu", "bantu", "carikan", "tentang", "about", "on", "mau", "ingin", "kamu", "can", "you", "could", "need",
];
const BOUNDARY: &[&str] = &["pakai", "dengan", "menggunakan", "memakai", "with", "using", "yang", "untuk", "for", "and", "dan", "di", "in", "lalu", "kemudian", "then", "agar", "supaya", "dari", "from"];

/// Slug folder dari klausa pertama pesan (maks 4 kata bermakna, ASCII huruf kecil + tanda hubung).
pub fn suggest_name(text: &str, kind: Kind) -> String {
    let clause = text.split(['\n', ',', '.', ':', ';', '?', '!']).find(|c| !c.trim().is_empty()).unwrap_or("");
    let mut words: Vec<String> = vec![];
    for w in clause.split_whitespace() {
        let w: String = w.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
        if w.is_empty() || STOP.contains(&w.as_str()) {
            continue;
        }
        if BOUNDARY.contains(&w.as_str()) {
            if words.len() >= 2 {
                break;
            }
            continue;
        }
        words.push(w);
        if words.len() == 4 {
            break;
        }
    }
    let slug = slugify(&words.join("-"));
    if slug.is_empty() { format!("project-{}", kind.id()) } else { slug }
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(40).collect::<String>().trim_matches('-').to_string()
}

/// Folder induk project buatan Zandor: `<Documents>/Zandor Projects` (cadangan: folder data aplikasi).
pub fn projects_root(fallback: &Path) -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    match home {
        Some(h) if h.join("Documents").is_dir() => h.join("Documents").join("Zandor Projects"),
        _ => fallback.join("projects"),
    }
}

/// Buat folder baru `<root>/<name>` (akhiran -2, -3 … bila sudah ada). Tidak pernah memakai folder yang sudah ada.
pub fn create_unique_dir(root: &Path, name: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(root).map_err(|e| format!("gagal membuat folder induk: {e}"))?;
    for n in 1..1000 {
        let dir = root.join(if n == 1 { name.to_string() } else { format!("{name}-{n}") });
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("gagal membuat folder project: {e}")),
        }
    }
    Err("terlalu banyak folder dengan nama serupa".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(t: &str) -> Option<Kind> {
        detect(t, 0).map(|i| i.kind)
    }

    #[test]
    fn memicu_untuk_pekerjaan_project() {
        assert_eq!(kind("buatkan aplikasi todo list pakai react, install dependensi nya dan jalankan di localhost"), Some(Kind::Build));
        assert_eq!(kind("Create a REST API with FastAPI, pip install the requirements and add docker-compose"), Some(Kind::Build));
        assert_eq!(kind("analisis dataset penjualan csv ini, bersihkan data lalu buat visualisasi dan simpan ke beberapa file"), Some(Kind::Data));
        assert_eq!(kind("riset mendalam pasar kendaraan listrik, kumpulkan referensi dari banyak sumber dan susun laporan 20 halaman"), Some(Kind::Research));
        assert_eq!(kind("tolong tulis skripsi tentang literasi digital, bab 1 sampai bab 5 sebanyak 40 halaman"), Some(Kind::Document));
        assert_eq!(kind("convert semua video di folder ini ke mp4 pakai ffmpeg lalu tambahkan subtitle"), Some(Kind::Media));
        assert!(kind("buat bot telegram yang otomatis kirim laporan tiap jam 8 pagi").is_some());
    }

    #[test]
    fn tidak_memicu_untuk_obrolan_biasa() {
        for t in [
            "apa itu react?",
            "jelaskan perbedaan npm dan yarn secara singkat",
            "bagaimana cara install dependensi python dengan pip?",
            "terjemahkan paragraf ini ke bahasa Inggris: halo dunia apa kabar",
            "buatkan fungsi python untuk menghitung faktorial",
            "buatkan puisi tentang hujan di sore hari",
            "ringkas artikel ini menjadi tiga poin utama saja",
            "what is the difference between a vector and an array?",
        ] {
            assert_eq!(detect(t, 0), None, "{t}");
        }
    }

    #[test]
    fn daftar_bertahap_dan_lampiran_menambah_skor() {
        let t = "siapkan environment\n1. buat folder\n2. install paket\n3. jalankan skrip";
        assert!(detect(t, 0).is_some());
        assert!(detect("tolong rapikan folder unduhan saya\n- pindahkan\n- ganti nama\n- hapus duplikat", 0).is_none());
        let t2 = "analisis csv terlampir lalu:\n- bersihkan data\n- hitung ringkasan\n- buat grafik";
        assert_eq!(detect(t2, 1).map(|i| i.kind), Some(Kind::Data));
    }

    #[test]
    fn nama_usulan_berupa_slug() {
        assert_eq!(suggest_name("buatkan aplikasi todo list pakai react", Kind::Build), "aplikasi-todo-list");
        assert_eq!(suggest_name("Riset pasar EV di Indonesia, lengkap", Kind::Research), "riset-pasar-ev");
        assert_eq!(suggest_name("!!!", Kind::Data), "project-data");
        assert_eq!(slugify("  Halo  Dunia!! "), "halo-dunia");
    }

    #[test]
    fn folder_unik_tidak_menimpa() {
        let root = std::env::temp_dir().join(format!("zandor-pi-{}", std::process::id()));
        let a = create_unique_dir(&root, "demo").unwrap();
        let b = create_unique_dir(&root, "demo").unwrap();
        assert_ne!(a, b);
        assert!(b.ends_with("demo-2"));
        std::fs::remove_dir_all(&root).ok();
    }
}

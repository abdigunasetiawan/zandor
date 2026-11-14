// Akses berkas untuk Panel 3 (Fase 7): baca byte persis, salin ke lokasi pilihan pengguna, dan buka di aplikasi lain.
// Aman secara bawaan: path HARUS berada di dalam root yang terdaftar (project di registry atau folder Chat) setelah
// dikanonisasi (menolak `..`, path absolut di luar root, dan symlink yang keluar). Berkas dibatasi ukurannya, dan berkas
// yang bisa dieksekusi tidak pernah dibuka lewat "buka di aplikasi lain".
use std::path::{Path, PathBuf};

pub const MAX_READ_BYTES: u64 = 100 * 1024 * 1024;

/// Ekstensi yang menjalankan kode bila dibuka lewat shell; ditolak untuk "buka di aplikasi lain".
const EXECUTABLE_EXT: &[&str] = &[
    "exe",
    "com",
    "scr",
    "bat",
    "cmd",
    "msi",
    "msp",
    "ps1",
    "psm1",
    "vbs",
    "vbe",
    "js",
    "jse",
    "wsf",
    "wsh",
    "hta",
    "lnk",
    "url",
    "reg",
    "jar",
    "dll",
    "cpl",
    "gadget",
    "pif",
    "appx",
    "msix",
    "application",
    "sh",
];

/// Root yang diizinkan: path project di `projects.json` ditambah folder Chat.
pub fn allowed_roots(app_data: &Path) -> Vec<PathBuf> {
    let mut roots = vec![app_data.join("chats")];
    if let Ok(text) = std::fs::read_to_string(app_data.join("projects.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(arr) = v.get("projects").and_then(|p| p.as_array()) {
                for p in arr {
                    if let Some(path) = p.get("path").and_then(|x| x.as_str()) {
                        roots.push(PathBuf::from(path));
                    }
                }
            }
        }
    }
    roots
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (dunce(a), dunce(b)) {
        (Some(x), Some(y)) => {
            if cfg!(windows) {
                x.to_string_lossy().to_lowercase() == y.to_string_lossy().to_lowercase()
            } else {
                x == y
            }
        }
        _ => false,
    }
}

/// Kanonisasi tanpa awalan verbatim `\\?\` (Windows) supaya perbandingan dan tampilan konsisten.
fn dunce(p: &Path) -> Option<PathBuf> {
    let c = std::fs::canonicalize(p).ok()?;
    let s = c.to_string_lossy();
    Some(match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => c,
    })
}

/// Pastikan `root` terdaftar, lalu resolve `rel` di dalamnya. Mengembalikan path berkas yang sudah dikanonisasi.
pub fn resolve_in_root(allowed: &[PathBuf], root: &str, rel: &str) -> Result<PathBuf, String> {
    if rel.is_empty() || rel.contains('\0') {
        return Err("path tidak valid".into());
    }
    let root_path = PathBuf::from(root);
    if !allowed.iter().any(|a| same_dir(a, &root_path)) {
        return Err("folder ini bukan project yang terdaftar".into());
    }
    let root_canon = dunce(&root_path).ok_or("folder project tidak ditemukan")?;
    let rel_path = Path::new(rel);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("path harus relatif terhadap project dan tidak boleh memakai '..'".into());
    }
    let full = dunce(&root_canon.join(rel_path)).ok_or("berkas tidak ditemukan")?;
    if !full.starts_with(&root_canon) {
        return Err("berkas berada di luar project".into());
    }
    Ok(full)
}

/// Root terdaftar → path folder yang sudah dikanonisasi (untuk perintah yang bekerja di dalam folder, mis. git).
pub fn resolve_dir(allowed: &[PathBuf], root: &str) -> Result<PathBuf, String> {
    let root_path = PathBuf::from(root);
    if !allowed.iter().any(|a| same_dir(a, &root_path)) {
        return Err("folder ini bukan project yang terdaftar".into());
    }
    dunce(&root_path).ok_or_else(|| "folder project tidak ditemukan".to_string())
}

/// Seperti `resolve_dir`, tetapi `dir` juga boleh berada DI DALAM root terdaftar (mis. repo git di anak folder project).
pub fn resolve_dir_within(allowed: &[PathBuf], dir: &str) -> Result<PathBuf, String> {
    let dir_path = PathBuf::from(dir);
    let canon = dunce(&dir_path).ok_or("folder tidak ditemukan")?;
    if !canon.is_dir() {
        return Err("bukan folder".into());
    }
    for a in allowed {
        if let Some(root) = dunce(a) {
            let (c, r) = if cfg!(windows) {
                (
                    canon.to_string_lossy().to_lowercase(),
                    root.to_string_lossy().to_lowercase(),
                )
            } else {
                (
                    canon.to_string_lossy().into_owned(),
                    root.to_string_lossy().into_owned(),
                )
            };
            let sep = std::path::MAIN_SEPARATOR;
            if c == r || c.starts_with(&format!("{}{}", r.trim_end_matches(sep), sep)) {
                return Ok(canon);
            }
        }
    }
    Err("folder ini bukan bagian dari project yang terdaftar".into())
}

pub fn read_limited(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("tidak bisa membaca berkas: {e}"))?;
    if !meta.is_file() {
        return Err("bukan berkas".into());
    }
    if meta.len() > max {
        return Err(format!(
            "berkas terlalu besar ({} MB; batas {} MB)",
            meta.len() / 1024 / 1024,
            max / 1024 / 1024
        ));
    }
    std::fs::read(path).map_err(|e| format!("gagal membaca berkas: {e}"))
}

/// Batas teks yang boleh disimpan dari editor (UTF-8).
pub const MAX_WRITE_BYTES: usize = 5 * 1024 * 1024;

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct FileStat {
    pub size: u64,
    /// ms sejak epoch
    pub modified: u64,
}

pub fn stat(path: &Path) -> Result<FileStat, String> {
    let m = std::fs::metadata(path).map_err(|e| format!("tidak bisa membaca berkas: {e}"))?;
    if !m.is_file() {
        return Err("bukan berkas".into());
    }
    let modified = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Ok(FileStat {
        size: m.len(),
        modified,
    })
}

/// Tulis ulang berkas teks yang SUDAH ada (tidak membuat berkas baru) secara atomik: tulis ke berkas sementara di folder
/// yang sama lalu ganti. Bila `expected_modified` diberikan dan berkas di disk sudah berubah sejak dibaca, ditolak dengan
/// awalan `KONFLIK:` supaya UI bisa menanyakan pengguna (tidak menimpa diam-diam). Berkas yang bisa dieksekusi dan
/// berkas read-only ditolak. Mengembalikan waktu modifikasi baru.
pub fn write_text(path: &Path, text: &str, expected_modified: Option<u64>) -> Result<u64, String> {
    if text.len() > MAX_WRITE_BYTES {
        return Err(format!(
            "teks terlalu besar (maks {} MB)",
            MAX_WRITE_BYTES / 1024 / 1024
        ));
    }
    if is_executable_ext(path) {
        return Err("berkas yang dapat dieksekusi tidak diedit dari Zandor".into());
    }
    let meta = std::fs::metadata(path).map_err(|e| format!("tidak bisa membaca berkas: {e}"))?;
    if !meta.is_file() {
        return Err("bukan berkas".into());
    }
    if meta.permissions().readonly() {
        return Err("berkas bersifat read-only".into());
    }
    let current = stat(path)?;
    if let Some(exp) = expected_modified {
        if exp != current.modified {
            return Err("KONFLIK: berkas berubah di disk sejak dibuka".into());
        }
    }
    let dir = path.parent().ok_or("path tidak valid")?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("nama berkas tidak valid")?;
    let tmp = dir.join(format!(".{name}.zandor-{}.tmp", std::process::id()));
    std::fs::write(&tmp, text.as_bytes()).map_err(|e| format!("gagal menulis: {e}"))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("gagal mengganti berkas: {e}"));
    }
    Ok(stat(path)?.modified)
}

/// Nama berkas/folder baru yang valid: satu segmen (tanpa pemisah path), bukan `.`/`..`, tanpa karakter terlarang Windows,
/// tidak berakhir spasi/titik, 1–255 karakter.
pub fn valid_entry_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty()
        && n == name
        && n.chars().count() <= 255
        && n != "."
        && n != ".."
        && !n.ends_with('.')
        && !n
            .chars()
            .any(|c| c.is_control() || r#"<>:"/\|?*"#.contains(c))
}

/// Buat berkas kosong atau folder baru bernama `name` di dalam `parent` (folder yang sudah ada). Tidak pernah menimpa:
/// nama yang sudah dipakai ditolak. Berkas dengan ekstensi yang dapat dieksekusi ditolak. Mengembalikan path barunya.
pub fn create_entry(parent: &Path, name: &str, dir: bool) -> Result<PathBuf, String> {
    if !valid_entry_name(name) {
        return Err("nama tidak valid".into());
    }
    if !parent.is_dir() {
        return Err("folder tujuan tidak ditemukan".into());
    }
    let target = parent.join(name);
    if target.exists() {
        return Err("sudah ada berkas atau folder dengan nama itu".into());
    }
    if dir {
        std::fs::create_dir(&target).map_err(|e| format!("gagal membuat folder: {e}"))?;
    } else {
        if is_executable_ext(&target) {
            return Err("berkas yang dapat dieksekusi tidak dibuat dari Zandor".into());
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| format!("gagal membuat berkas: {e}"))?;
    }
    Ok(target)
}

/// Nama folder sesi Chat yang valid: 1–64 karakter huruf/angka/`.`/`_`/`-`, tidak diawali titik.
pub fn valid_chat_folder_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Buat folder sesi Chat `root/name` (root = folder Chat). Mengembalikan path lengkapnya.
pub fn chat_folder_create(root: &Path, name: &str) -> Result<PathBuf, String> {
    if !valid_chat_folder_name(name) {
        return Err(format!("nama folder sesi tidak valid: {name:?}"));
    }
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("gagal membuat folder sesi: {e}"))?;
    Ok(dir)
}

/// Semua folder sesi (anak langsung folder Chat), urut nama.
pub fn chat_folder_list(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Hapus folder sesi bila KOSONG. Hanya anak langsung folder Chat yang boleh; folder berisi berkas tidak pernah dihapus.
pub fn chat_folder_remove_if_empty(root: &Path, dir: &Path) -> Result<(), String> {
    let parent_ok = dir.parent().is_some_and(|p| same_dir(p, root));
    let name_ok = dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(valid_chat_folder_name);
    if !parent_ok || !name_ok {
        return Err("bukan folder sesi Chat".into());
    }
    std::fs::remove_dir(dir).map_err(|e| format!("folder tidak dihapus: {e}"))
}

/// Batas ukuran berkas hasil ekspor (teks/gambar/SVG dari satu respons).
pub const MAX_SAVE_BYTES: usize = 32 * 1024 * 1024;

/// Tulis byte ke lokasi yang dipilih pengguna lewat dialog "Simpan sebagai" (ekspor respons). Berkas yang dapat dieksekusi
/// ditolak; folder tujuan harus sudah ada.
pub fn save_bytes(dest: &Path, data: &[u8]) -> Result<(), String> {
    if data.len() > MAX_SAVE_BYTES {
        return Err(format!(
            "berkas terlalu besar ({} MB maksimum)",
            MAX_SAVE_BYTES / 1024 / 1024
        ));
    }
    if is_executable_ext(dest) {
        return Err("berkas yang dapat dieksekusi tidak disimpan dari Zandor".into());
    }
    match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() && !p.is_dir() => {
            return Err("folder tujuan tidak ada".into())
        }
        _ => {}
    }
    std::fs::write(dest, data).map_err(|e| format!("gagal menyimpan: {e}"))
}

pub fn is_executable_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXECUTABLE_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_dir_within_menerima_anak_folder_dan_menolak_di_luar_root() {
        let base = std::env::temp_dir().join(format!("zandor-within-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("mono");
        std::fs::create_dir_all(root.join("api")).unwrap();
        std::fs::create_dir_all(base.join("mono-lain")).unwrap(); // awalan nama sama, bukan anak
        let allowed = vec![root.clone()];
        assert!(resolve_dir_within(&allowed, root.to_str().unwrap()).is_ok());
        assert!(resolve_dir_within(&allowed, root.join("api").to_str().unwrap()).is_ok());
        assert!(resolve_dir_within(&allowed, base.join("mono-lain").to_str().unwrap()).is_err());
        assert!(resolve_dir_within(&allowed, base.to_str().unwrap()).is_err());
        assert!(resolve_dir_within(&allowed, root.join("tidak-ada").to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn create_entry_membuat_tanpa_menimpa_dan_menolak_nama_buruk() {
        let base = std::env::temp_dir().join(format!("zandor-create-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let f = create_entry(&base, "baru.md", false).unwrap();
        assert!(f.is_file());
        let d = create_entry(&base, "folder", true).unwrap();
        assert!(d.is_dir());
        assert!(create_entry(&base, "baru.md", false).is_err()); // tidak menimpa
        for bad in ["", "..", "a/b", "a\\b", "x:y", "titik.", " spasi", "a*b"] {
            assert!(create_entry(&base, bad, false).is_err(), "{bad}");
        }
        assert!(create_entry(&base, "skrip.exe", false).is_err());
        assert!(create_entry(&base.join("tidak-ada"), "x.txt", false).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    fn setup(tag: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("zandor-files-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("proj");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("a.txt"), "halo\n").unwrap();
        std::fs::write(root.join("sub").join("b.bin"), [0u8, 1, 2, 255]).unwrap();
        std::fs::write(base.join("rahasia.txt"), "jangan").unwrap();
        (base, root)
    }

    #[test]
    fn resolves_files_inside_a_registered_root() {
        let (_b, root) = setup("ok");
        let allowed = vec![root.clone()];
        let p = resolve_in_root(&allowed, root.to_str().unwrap(), "sub/b.bin").unwrap();
        assert_eq!(read_limited(&p, 1024).unwrap(), vec![0, 1, 2, 255]);
        // byte persis, termasuk newline akhir
        let a = resolve_in_root(&allowed, root.to_str().unwrap(), "a.txt").unwrap();
        assert_eq!(read_limited(&a, 1024).unwrap(), b"halo\n");
    }

    #[test]
    fn rejects_traversal_absolute_paths_and_unregistered_roots() {
        let (base, root) = setup("deny");
        let allowed = vec![root.clone()];
        let r = root.to_str().unwrap();
        assert!(resolve_in_root(&allowed, r, "../rahasia.txt").is_err());
        assert!(resolve_in_root(&allowed, r, "sub/../../rahasia.txt").is_err());
        let abs = base.join("rahasia.txt");
        assert!(resolve_in_root(&allowed, r, abs.to_str().unwrap()).is_err());
        assert!(resolve_in_root(&allowed, r, "").is_err());
        assert!(resolve_in_root(&allowed, r, "tidak-ada.txt").is_err());
        // root yang tidak terdaftar
        let other = base.to_str().unwrap();
        assert!(resolve_in_root(&allowed, other, "rahasia.txt").is_err());
        assert!(resolve_in_root(&[], r, "a.txt").is_err());
    }

    #[test]
    fn root_match_ignores_case_and_trailing_separator_on_windows() {
        let (_b, root) = setup("case");
        let allowed = vec![root.clone()];
        if cfg!(windows) {
            let shouty = format!("{}\\", root.to_string_lossy().to_uppercase());
            assert!(resolve_in_root(&allowed, &shouty, "a.txt").is_ok());
        }
    }

    #[test]
    fn resolve_dir_requires_a_registered_root() {
        let (base, root) = setup("dir");
        let allowed = vec![root.clone()];
        assert!(resolve_dir(&allowed, root.to_str().unwrap()).is_ok());
        assert!(resolve_dir(&allowed, base.to_str().unwrap()).is_err());
        assert!(resolve_dir(&[], root.to_str().unwrap()).is_err());
    }

    #[test]
    fn size_limit_and_directories_are_refused() {
        let (_b, root) = setup("size");
        assert!(read_limited(&root.join("a.txt"), 2).is_err());
        assert!(read_limited(&root.join("sub"), 1024).is_err());
    }

    #[test]
    fn write_text_replaces_atomically_and_detects_conflicts() {
        let (root, _) = setup("write");
        let f = root.join("a.txt");
        std::fs::write(&f, "lama").unwrap();
        let before = stat(&f).unwrap();
        // tanpa pemeriksaan
        let m1 = write_text(&f, "baru\n", None).unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "baru\n");
        assert!(m1 > 0);
        // modifikasi sama → berhasil
        let m2 = write_text(&f, "lagi", Some(m1)).unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "lagi");
        // modifikasi usang → konflik, isi tidak berubah
        let err = write_text(&f, "timpa", Some(before.modified.wrapping_add(12345))).unwrap_err();
        assert!(err.starts_with("KONFLIK"), "{err}");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "lagi");
        let _ = m2;
        // tidak ada sisa berkas sementara
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    // Windows-first (ADR-027): set_readonly(false) hanya dipakai agar berkas uji bisa dihapus. TODO(port): unix → PermissionsExt.
    #[allow(clippy::permissions_set_readonly_false)]
    #[test]
    fn write_text_refuses_missing_executable_readonly_and_huge() {
        let (root, _) = setup("write2");
        assert!(write_text(&root.join("tidak-ada.txt"), "x", None).is_err());
        let exe = root.join("a.bat");
        std::fs::write(&exe, "echo").unwrap();
        assert!(write_text(&exe, "x", None)
            .unwrap_err()
            .contains("dieksekusi"));
        let ro = root.join("ro.txt");
        std::fs::write(&ro, "x").unwrap();
        let mut perm = std::fs::metadata(&ro).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&ro, perm.clone()).unwrap();
        assert!(write_text(&ro, "y", None)
            .unwrap_err()
            .contains("read-only"));
        perm.set_readonly(false);
        std::fs::set_permissions(&ro, perm).unwrap();
        let big = root.join("big.txt");
        std::fs::write(&big, "x").unwrap();
        assert!(write_text(&big, &"x".repeat(MAX_WRITE_BYTES + 1), None).is_err());
        assert!(write_text(&root, "x", None).is_err()); // direktori
    }

    #[test]
    fn save_bytes_writes_and_refuses_executables_missing_folders_and_huge_data() {
        let (base, _) = setup("savebytes");
        let ok = base.join("hasil.png");
        save_bytes(&ok, &[1, 2, 3]).unwrap();
        assert_eq!(std::fs::read(&ok).unwrap(), vec![1, 2, 3]);
        assert!(save_bytes(&base.join("x.exe"), &[1])
            .unwrap_err()
            .contains("dieksekusi"));
        assert!(save_bytes(&base.join("tidak-ada").join("a.txt"), &[1])
            .unwrap_err()
            .contains("folder"));
        let huge = vec![0u8; MAX_SAVE_BYTES + 1];
        assert!(save_bytes(&base.join("besar.bin"), &huge)
            .unwrap_err()
            .contains("terlalu besar"));
    }

    #[test]
    fn chat_folders_are_created_listed_and_only_removed_when_empty_and_direct_children() {
        let (base, _) = setup("chatfolders");
        let root = base.join("chats");
        std::fs::create_dir_all(&root).unwrap();
        assert!(valid_chat_folder_name("chat-20261114-090507-abcd12"));
        for bad in ["", ".x", "a/b", "a\\b", "..", "spasi nama", &"x".repeat(65)] {
            assert!(!valid_chat_folder_name(bad), "{bad:?}");
        }
        assert!(chat_folder_create(&root, "../kabur").is_err());
        let a = chat_folder_create(&root, "chat-a").unwrap();
        let b = chat_folder_create(&root, "chat-b").unwrap();
        assert_eq!(chat_folder_list(&root), vec![a.clone(), b.clone()]);
        std::fs::write(b.join("hasil.md"), "isi").unwrap();
        assert!(chat_folder_remove_if_empty(&root, &b).is_err()); // berisi berkas → tidak dihapus
        assert!(b.join("hasil.md").exists());
        chat_folder_remove_if_empty(&root, &a).unwrap(); // kosong → dihapus
        assert!(!a.exists());
        assert!(chat_folder_remove_if_empty(&root, &base).is_err()); // bukan anak langsung folder Chat
        assert!(chat_folder_remove_if_empty(&root, &root).is_err());
    }

    #[test]
    fn executable_extensions_are_flagged_case_insensitively() {
        for n in ["x.exe", "x.BAT", "a.ps1", "s.lnk", "m.msi", "r.sh"] {
            assert!(is_executable_ext(Path::new(n)), "{n}");
        }
        for n in ["x.pdf", "a.md", "noext", "gambar.png"] {
            assert!(!is_executable_ext(Path::new(n)), "{n}");
        }
    }

    #[test]
    fn allowed_roots_come_from_registry_and_chat_dir() {
        let base = std::env::temp_dir().join(format!("zandor-roots-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(
            base.join("projects.json"),
            r#"{"version":1,"projects":[{"id":"a","path":"C:\\Work\\alpha","name":"alpha"}],"selection":{}}"#,
        )
        .unwrap();
        let roots = allowed_roots(&base);
        assert!(roots.contains(&base.join("chats")));
        assert!(roots.iter().any(|r| r.to_string_lossy().contains("alpha")));
        let _ = std::fs::remove_dir_all(&base);
    }
}

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

pub fn is_executable_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXECUTABLE_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

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

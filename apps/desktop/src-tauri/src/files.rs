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
    fn size_limit_and_directories_are_refused() {
        let (_b, root) = setup("size");
        assert!(read_limited(&root.join("a.txt"), 2).is_err());
        assert!(read_limited(&root.join("sub"), 1024).is_err());
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

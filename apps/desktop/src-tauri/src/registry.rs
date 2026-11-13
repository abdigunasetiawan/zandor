//! Penyimpanan registry project (JSON milik Zandor di direktori data aplikasi).
//! Menulis atomik: tulis ke berkas sementara lalu ganti nama, jadi crash di tengah penulisan tidak merusak data.

use std::path::{Path, PathBuf};

const MAX_BYTES: usize = 5 * 1024 * 1024;

pub fn registry_path(app_data: &Path) -> PathBuf {
    app_data.join("projects.json")
}

pub fn load(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("gagal membaca registry: {e}")),
    }
}

pub fn save(path: &Path, text: &str) -> Result<(), String> {
    if text.len() > MAX_BYTES {
        return Err(format!("registry terlalu besar ({} byte)", text.len()));
    }
    let dir = path.parent().ok_or("path registry tidak valid")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("gagal membuat direktori: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("gagal menulis registry: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("gagal mengganti registry: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "zandor-registry-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn missing_file_is_none() {
        let d = temp();
        assert_eq!(load(&registry_path(&d)).unwrap(), None);
    }

    #[test]
    fn save_then_load_roundtrip_and_overwrite() {
        let d = temp();
        let p = registry_path(&d);
        save(&p, "{\"v\":1}").unwrap();
        assert_eq!(load(&p).unwrap().as_deref(), Some("{\"v\":1}"));
        save(&p, "{\"v\":2,\"ü\":\"✓\"}").unwrap();
        assert_eq!(load(&p).unwrap().as_deref(), Some("{\"v\":2,\"ü\":\"✓\"}"));
        assert!(
            !p.with_extension("json.tmp").exists(),
            "berkas sementara tidak boleh tersisa"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rejects_oversized_payload() {
        let d = temp();
        let big = "x".repeat(MAX_BYTES + 1);
        assert!(save(&registry_path(&d), &big).is_err());
        assert!(!registry_path(&d).exists());
    }
}

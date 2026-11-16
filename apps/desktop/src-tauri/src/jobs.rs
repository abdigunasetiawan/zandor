//! Background job agent (ADR-072): dibaca dari `<app_data>/jobs` yang ditulis plugin Zandor di engine.
//! UI hanya membaca meta/log dan menulis berkas kontrol (`control/<id>.<aksi>`); proses dikelola plugin.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// id job dari plugin: `job-` + 6..16 huruf kecil/angka. Mencegah traversal path.
pub fn valid_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("job-") else {
        return false;
    };
    (6..=16).contains(&rest.len())
        && rest
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Semua meta job (JSON mentah dari plugin), terbaru dulu.
pub fn list(dir: &Path) -> Vec<serde_json::Value> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<serde_json::Value> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let id = name.strip_suffix(".json")?;
            if !valid_id(id) {
                return None;
            }
            let text = fs::read_to_string(e.path()).ok()?;
            serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .filter(|v| v.get("id").and_then(|x| x.as_str()) == Some(id))
        })
        .collect();
    out.sort_by_key(|v| {
        std::cmp::Reverse(v.get("startedAt").and_then(|x| x.as_f64()).unwrap_or(0.0) as i64)
    });
    out
}

/// `max` byte terakhir log job (UTF-8, potongan awal yang rusak dibuang).
pub fn log(dir: &Path, id: &str, max: u64) -> Result<String, String> {
    if !valid_id(id) {
        return Err("id job tidak valid".into());
    }
    let path = dir.join(format!("{id}.log"));
    let Ok(mut f) = fs::File::open(&path) else {
        return Ok(String::new());
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(max);
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// Minta plugin melakukan aksi (stop/restart/remove) untuk job.
pub fn control(dir: &Path, id: &str, action: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("id job tidak valid".into());
    }
    if !matches!(action, "stop" | "restart" | "remove") {
        return Err(format!("aksi tidak dikenal: {action}"));
    }
    let ctl = dir.join("control");
    fs::create_dir_all(&ctl).map_err(|e| e.to_string())?;
    fs::write(ctl.join(format!("{id}.{action}")), b"").map_err(|e| e.to_string())
}

/// Hapus berkas job yang sudah tidak dikelola plugin mana pun (status lost/berhenti dari sesi engine lama).
pub fn forget(dir: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("id job tidak valid".into());
    }
    for ext in ["json", "log"] {
        let p = dir.join(format!("{id}.{ext}"));
        if p.exists() {
            fs::remove_file(p).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("zandor-jobs-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn id_rejects_traversal() {
        assert!(valid_id("job-abc123"));
        assert!(!valid_id("job-../x"));
        assert!(!valid_id("job-AB1234"));
        assert!(!valid_id("x-abc123"));
        assert!(!valid_id("job-abc"));
    }

    #[test]
    fn list_log_control_forget() {
        let d = tmp("rt");
        fs::write(
            d.join("job-aaaaaa11.json"),
            r#"{"id":"job-aaaaaa11","startedAt":1}"#,
        )
        .unwrap();
        fs::write(
            d.join("job-bbbbbb22.json"),
            r#"{"id":"job-bbbbbb22","startedAt":2}"#,
        )
        .unwrap();
        fs::write(d.join("job-cccccc33.json"), r#"{"id":"lain"}"#).unwrap();
        fs::write(d.join("job-aaaaaa11.log"), "halo dunia").unwrap();
        let l = list(&d);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0]["id"], "job-bbbbbb22");
        assert_eq!(log(&d, "job-aaaaaa11", 5).unwrap(), "dunia");
        assert_eq!(log(&d, "job-bbbbbb22", 5).unwrap(), "");
        control(&d, "job-aaaaaa11", "stop").unwrap();
        assert!(d.join("control").join("job-aaaaaa11.stop").exists());
        assert!(control(&d, "job-aaaaaa11", "rm -rf").is_err());
        forget(&d, "job-aaaaaa11").unwrap();
        assert_eq!(list(&d).len(), 1);
        let _ = fs::remove_dir_all(&d);
    }
}

//! Kontrol aplikasi oleh agent (ADR-077): plugin Zandor di engine menulis permintaan ke
//! `<app_data>/control/requests/<id>.json`; UI mengambilnya lewat `take`, menjalankannya, lalu menjawab lewat `reply`
//! (`responses/<id>.json`, ditulis atomik). Isi permintaan tidak dipercaya: UI memvalidasi setiap aksi.
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const MAX_BATCH: usize = 16;
/// Permintaan yang lebih tua dari ini (aplikasi sempat tertutup) dibuang: tool di engine sudah berhenti menunggu.
const STALE: Duration = Duration::from_secs(300);
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

/// id dari plugin: `ctl-` + 6..32 huruf kecil/angka. Mencegah traversal path.
pub fn valid_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("ctl-") else {
        return false;
    };
    (6..=32).contains(&rest.len())
        && rest
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Ambil (dan hapus) permintaan yang menunggu, terlama dulu.
pub fn take(dir: &Path) -> Vec<serde_json::Value> {
    let req_dir = dir.join("requests");
    let Ok(rd) = fs::read_dir(&req_dir) else {
        return Vec::new();
    };
    let mut files: Vec<(SystemTime, std::path::PathBuf, String)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let id = name.strip_suffix(".json")?.to_string();
            valid_id(&id).then_some(())?;
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((modified, e.path(), id))
        })
        .collect();
    files.sort_by_key(|f| f.0);
    let now = SystemTime::now();
    let mut out = Vec::new();
    for (modified, path, id) in files.into_iter().take(MAX_BATCH) {
        let fresh = now
            .duration_since(modified)
            .map(|d| d < STALE)
            .unwrap_or(true);
        let small = fs::metadata(&path)
            .map(|m| m.len() <= MAX_REQUEST_BYTES)
            .unwrap_or(false);
        let value = (fresh && small)
            .then(|| fs::read_to_string(&path).ok())
            .flatten()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .filter(|v| v.get("id").and_then(|x| x.as_str()) == Some(id.as_str()));
        let _ = fs::remove_file(&path);
        if let Some(v) = value {
            out.push(v);
        }
    }
    out
}

/// Tulis jawaban untuk permintaan `id` (atomik: tmp lalu rename).
pub fn reply(dir: &Path, id: &str, response: &serde_json::Value) -> Result<(), String> {
    if !valid_id(id) {
        return Err(format!("id kontrol tidak valid: {id}"));
    }
    let text = response.to_string();
    if text.len() > MAX_RESPONSE_BYTES {
        return Err("jawaban kontrol terlalu besar".into());
    }
    let res_dir = dir.join("responses");
    fs::create_dir_all(&res_dir).map_err(|e| e.to_string())?;
    let target = res_dir.join(format!("{id}.json"));
    let tmp = res_dir.join(format!("{id}.json.tmp"));
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &target).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_valid_menolak_traversal() {
        assert!(valid_id("ctl-abc123xyz"));
        assert!(!valid_id("ctl-../x"));
        assert!(!valid_id("job-abc123"));
        assert!(!valid_id("ctl-ABC123"));
    }

    #[test]
    fn take_membaca_lalu_menghapus_dan_reply_menulis() {
        let dir = std::env::temp_dir().join(format!("zandor-ctl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("requests")).unwrap();
        fs::write(
            dir.join("requests/ctl-aaaaaa1.json"),
            r#"{"id":"ctl-aaaaaa1","action":"status"}"#,
        )
        .unwrap();
        // id di isi berbeda dari nama berkas → dibuang
        fs::write(
            dir.join("requests/ctl-bbbbbb2.json"),
            r#"{"id":"ctl-zzzzzz9"}"#,
        )
        .unwrap();
        fs::write(dir.join("requests/bukan.json"), "{}").unwrap();
        let got = take(&dir);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0]["action"], "status");
        assert!(!dir.join("requests/ctl-aaaaaa1.json").exists());
        assert!(!dir.join("requests/ctl-bbbbbb2.json").exists());
        assert!(take(&dir).is_empty());
        reply(&dir, "ctl-aaaaaa1", &serde_json::json!({"ok": true})).unwrap();
        let text = fs::read_to_string(dir.join("responses/ctl-aaaaaa1.json")).unwrap();
        assert_eq!(text, r#"{"ok":true}"#);
        assert!(reply(&dir, "../x", &serde_json::json!({})).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}

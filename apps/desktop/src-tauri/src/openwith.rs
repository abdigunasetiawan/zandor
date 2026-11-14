//! "Buka dengan": daftar aplikasi yang terdaftar untuk satu ekstensi (beserta ikonnya) dan peluncuran berkas di aplikasi
//! pilihan. Windows: registri dibaca lewat skrip PowerShell `openwith.ps1` (hanya baca; ikon diambil dari exe). Platform
//! lain belum didukung (ADR-027) → daftar kosong. Peluncuran hanya untuk aplikasi yang ADA di daftar ekstensi itu, jadi
//! jalur yang dikirim dari UI tidak bisa dipakai menjalankan exe sembarang.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct OpenApp {
    pub name: String,
    pub path: String,
    /// PNG base64 (tanpa awalan data:); kosong bila ikon tidak bisa diambil.
    #[serde(default)]
    pub icon: String,
    #[serde(default, rename = "isDefault")]
    pub is_default: bool,
}

/// Ekstensi yang valid: 1–16 karakter huruf/angka (tanpa titik).
pub fn valid_ext(ext: &str) -> bool {
    !ext.is_empty() && ext.len() <= 16 && ext.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Parse keluaran skrip (array JSON; satu objek tunggal ditoleransi karena PowerShell 5 bisa meratakannya).
pub fn parse_apps(json: &str) -> Result<Vec<OpenApp>, String> {
    let t = json.trim();
    if t.is_empty() {
        return Ok(Vec::new());
    }
    if t.starts_with('[') {
        serde_json::from_str(t).map_err(|e| format!("daftar aplikasi tidak terbaca: {e}"))
    } else {
        serde_json::from_str::<OpenApp>(t)
            .map(|a| vec![a])
            .map_err(|e| format!("daftar aplikasi tidak terbaca: {e}"))
    }
}

/// Apakah `app_path` ada di daftar (tanpa membedakan huruf besar/kecil, sesuai Windows).
pub fn app_allowed(apps: &[OpenApp], app_path: &str) -> bool {
    apps.iter().any(|a| a.path.eq_ignore_ascii_case(app_path))
}

type Cache = Mutex<HashMap<String, Vec<OpenApp>>>;
fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Daftar aplikasi untuk ekstensi (di-cache selama aplikasi berjalan; pembacaan registri + ikon bisa memakan beberapa detik).
pub fn apps_for(ext: &str, text_like: bool, force: bool) -> Result<Vec<OpenApp>, String> {
    if !valid_ext(ext) {
        return Err("ekstensi tidak valid".into());
    }
    let key = format!("{}|{}", ext.to_ascii_lowercase(), text_like);
    if !force {
        if let Some(hit) = cache().lock().map_err(|e| e.to_string())?.get(&key) {
            return Ok(hit.clone());
        }
    }
    let apps = enumerate(ext, text_like)?;
    cache()
        .lock()
        .map_err(|e| e.to_string())?
        .insert(key, apps.clone());
    Ok(apps)
}

#[cfg(windows)]
fn enumerate(ext: &str, text_like: bool) -> Result<Vec<OpenApp>, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Skrip ditulis ke folder sementara (bukan -Command) supaya tidak ada masalah kutip/escape argumen.
    let script = std::env::temp_dir().join(format!("zandor-openwith-{}.ps1", std::process::id()));
    std::fs::write(&script, include_str!("openwith.ps1"))
        .map_err(|e| format!("gagal menyiapkan skrip: {e}"))?;
    let mut cmd = std::process::Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ])
    .arg(&script)
    .args(["-Ext", ext])
    .creation_flags(CREATE_NO_WINDOW);
    if text_like {
        cmd.arg("-TextLike");
    }
    let out = cmd
        .output()
        .map_err(|e| format!("gagal membaca daftar aplikasi: {e}"));
    let _ = std::fs::remove_file(&script);
    let out = out?;
    if !out.status.success() {
        return Err("pembacaan daftar aplikasi gagal".into());
    }
    parse_apps(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(windows))]
fn enumerate(_ext: &str, _text_like: bool) -> Result<Vec<OpenApp>, String> {
    Ok(Vec::new())
}

/// Buka `file` dengan aplikasi `app` (argumen terpisah, bukan lewat shell).
pub fn launch(app: &str, file: &Path) -> Result<(), String> {
    std::process::Command::new(app)
        .arg(file)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("gagal membuka aplikasi: {e}"))
}

/// Tampilkan berkas di File Explorer (terpilih).
pub fn reveal(file: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", file.display()))
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("gagal membuka File Explorer: {e}"))
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Err("belum didukung di platform ini (ADR-027)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ekstensi_valid_hanya_alfanumerik_pendek() {
        assert!(valid_ext("md"));
        assert!(valid_ext("docx"));
        for bad in ["", ".md", "a/b", "a b", "x;calc", &"a".repeat(17)] {
            assert!(!valid_ext(bad), "{bad}");
        }
    }

    #[test]
    fn parse_array_objek_tunggal_dan_kosong() {
        let arr = r#"[{"name":"Word","path":"C:\\A\\WINWORD.EXE","icon":"AAA","isDefault":true},{"name":"Notepad","path":"C:\\N.exe","icon":"","isDefault":false}]"#;
        let apps = parse_apps(arr).unwrap();
        assert_eq!(apps.len(), 2);
        assert!(apps[0].is_default && apps[0].icon == "AAA");
        let single = parse_apps(r#"{"name":"X","path":"C:\\x.exe"}"#).unwrap();
        assert_eq!(single.len(), 1);
        assert!(parse_apps("  ").unwrap().is_empty());
        assert!(parse_apps("bukan json").is_err());
    }

    #[test]
    fn peluncuran_hanya_untuk_aplikasi_di_daftar() {
        let apps = parse_apps(r#"[{"name":"Word","path":"C:\\Office\\WINWORD.EXE"}]"#).unwrap();
        assert!(app_allowed(&apps, "c:\\office\\winword.exe"));
        assert!(!app_allowed(&apps, "C:\\Windows\\System32\\cmd.exe"));
    }
}

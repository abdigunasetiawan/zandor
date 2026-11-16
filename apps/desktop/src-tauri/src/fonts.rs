//! Daftar keluarga font yang terinstal di sistem (untuk pemilih "Jenis huruf" di kustomisasi Panel 2).
//! Windows: `InstalledFontCollection` lewat PowerShell (nama keluarga, bukan nama berkas/face). Platform lain:
//! `fc-list : family`. Hanya membaca; hasil di-cache selama aplikasi berjalan.

use std::sync::{Mutex, OnceLock};

/// Rapikan keluaran: satu nama per baris, koma (fc-list: alias) dipecah, kosong/duplikat dibuang, diurutkan.
pub fn parse_fonts(out: &str) -> Vec<String> {
    let mut list: Vec<String> = out
        .lines()
        .flat_map(|l| l.split(','))
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty() && s.len() <= 120 && !s.chars().any(|c| c.is_control()))
        .collect();
    list.sort_by_key(|s| s.to_lowercase());
    list.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    list
}

fn cache() -> &'static Mutex<Option<Vec<String>>> {
    static C: OnceLock<Mutex<Option<Vec<String>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

pub fn list(force: bool) -> Result<Vec<String>, String> {
    if !force {
        if let Some(hit) = cache().lock().map_err(|e| e.to_string())?.as_ref() {
            return Ok(hit.clone());
        }
    }
    let fonts = parse_fonts(&enumerate()?);
    *cache().lock().map_err(|e| e.to_string())? = Some(fonts.clone());
    Ok(fonts)
}

#[cfg(windows)]
fn enumerate() -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Add-Type -AssemblyName System.Drawing; (New-Object System.Drawing.Text.InstalledFontCollection).Families | ForEach-Object { $_.Name }",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("gagal membaca font sistem: {e}"))?;
    if !out.status.success() {
        return Err("gagal membaca font sistem".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(not(windows))]
fn enumerate() -> Result<String, String> {
    let out = std::process::Command::new("fc-list")
        .args([":", "family"])
        .output()
        .map_err(|e| format!("gagal membaca font sistem (fc-list): {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_urut_unik_dan_pecah_alias() {
        let got = parse_fonts("Segoe UI\r\nArial\narial\n\nDejaVu Sans,DejaVu Sans Condensed\n");
        assert_eq!(
            got,
            vec!["Arial", "DejaVu Sans", "DejaVu Sans Condensed", "Segoe UI"]
        );
    }
}

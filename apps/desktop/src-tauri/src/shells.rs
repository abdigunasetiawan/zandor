//! Deteksi shell yang terpasang di mesin pengguna untuk pemilih shell panel Terminal. Daftar engine (`GET /pty/shells`)
//! tidak lengkap (mis. tidak menyebut WSL per distro atau PowerShell 7), jadi aplikasi desktop memeriksa sendiri dan
//! UI menggabungkan keduanya. Windows: skrip `shells.ps1` (hanya membaca berkas/PATH/registri; ikon diambil dari exe).
//! Platform lain: `/etc/shells` + shell umum di PATH, tanpa ikon. Hasil di-cache selama aplikasi berjalan.

use std::sync::{Mutex, OnceLock};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ShellInfo {
    /// Jenis shell stabil (pwsh, powershell, cmd, git-bash, wsl, bash, zsh, …) untuk ikon cadangan di UI.
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// PNG base64 (tanpa awalan data:); kosong bila ikon tidak bisa diambil.
    #[serde(default)]
    pub icon: String,
}

/// Parse keluaran skrip (array JSON; satu objek tunggal ditoleransi karena PowerShell 5 bisa meratakannya).
pub fn parse_shells(json: &str) -> Result<Vec<ShellInfo>, String> {
    let t = json.trim();
    if t.is_empty() {
        return Ok(Vec::new());
    }
    let list: Vec<ShellInfo> = if t.starts_with('[') {
        serde_json::from_str(t).map_err(|e| format!("daftar shell tidak terbaca: {e}"))?
    } else {
        vec![serde_json::from_str(t).map_err(|e| format!("daftar shell tidak terbaca: {e}"))?]
    };
    Ok(list
        .into_iter()
        .filter(|s| !s.path.is_empty() && !s.name.is_empty())
        .collect())
}

fn cache() -> &'static Mutex<Option<Vec<ShellInfo>>> {
    static C: OnceLock<Mutex<Option<Vec<ShellInfo>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

pub fn detect(force: bool) -> Result<Vec<ShellInfo>, String> {
    if !force {
        if let Some(hit) = cache().lock().map_err(|e| e.to_string())?.as_ref() {
            return Ok(hit.clone());
        }
    }
    let list = enumerate()?;
    *cache().lock().map_err(|e| e.to_string())? = Some(list.clone());
    Ok(list)
}

#[cfg(windows)]
fn enumerate() -> Result<Vec<ShellInfo>, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Skrip ditulis ke folder sementara (bukan -Command) supaya tidak ada masalah kutip/escape argumen.
    let script = std::env::temp_dir().join(format!("zandor-shells-{}.ps1", std::process::id()));
    std::fs::write(&script, include_str!("shells.ps1"))
        .map_err(|e| format!("gagal menyiapkan skrip: {e}"))?;
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("gagal mendeteksi shell: {e}"));
    let _ = std::fs::remove_file(&script);
    let out = out?;
    if !out.status.success() {
        return Err("deteksi shell gagal".into());
    }
    parse_shells(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(windows))]
fn enumerate() -> Result<Vec<ShellInfo>, String> {
    use std::path::Path;
    let mut paths: Vec<String> = std::fs::read_to_string("/etc/shells")
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('/'))
        .map(String::from)
        .collect();
    let path_var = std::env::var("PATH").unwrap_or_default();
    for name in ["zsh", "bash", "fish", "nu", "pwsh", "elvish", "xonsh", "sh"] {
        if let Some(p) = path_var
            .split(':')
            .map(|d| Path::new(d).join(name))
            .find(|p| p.is_file())
        {
            paths.push(p.to_string_lossy().into_owned());
        }
    }
    let mut out: Vec<ShellInfo> = Vec::new();
    for p in paths {
        let path = Path::new(&p);
        if !path.is_file() {
            continue;
        }
        // tautan simbolik (mis. /bin → /usr/bin) jangan menggandakan entri
        let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let real_s = real.to_string_lossy().into_owned();
        if out.iter().any(|s| {
            s.path == p
                || std::fs::canonicalize(&s.path)
                    .map(|r| r == real)
                    .unwrap_or(false)
        }) {
            continue;
        }
        let base = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or(real_s);
        let name = match base.as_str() {
            "pwsh" => "PowerShell".to_string(),
            "nu" => "Nushell".to_string(),
            other => other.to_string(),
        };
        out.push(ShellInfo {
            id: base,
            name,
            path: p,
            args: Vec::new(),
            icon: String::new(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_array_objek_tunggal_dan_entri_rusak() {
        let arr = r#"[{"id":"cmd","name":"Command Prompt","path":"C:\\Windows\\system32\\cmd.exe","args":[],"icon":"AAA"},
                     {"id":"wsl","name":"WSL: Ubuntu","path":"C:\\Windows\\System32\\wsl.exe","args":["-d","Ubuntu"]},
                     {"id":"x","name":"","path":"C:\\x.exe"}]"#;
        let list = parse_shells(arr).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].icon, "AAA");
        assert_eq!(list[1].args, vec!["-d", "Ubuntu"]);
        assert_eq!(list[1].icon, "");
        let one =
            parse_shells(r#"{"id":"cmd","name":"Command Prompt","path":"C:\\cmd.exe"}"#).unwrap();
        assert_eq!(one.len(), 1);
        assert!(one[0].args.is_empty());
        assert!(parse_shells("  ").unwrap().is_empty());
        assert!(parse_shells("bukan json").is_err());
    }
}

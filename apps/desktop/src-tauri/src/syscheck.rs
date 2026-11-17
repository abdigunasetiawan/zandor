//! Pemeriksaan stack untuk halaman onboarding: alat yang dipakai agent/skill bawaan (Python + pustaka dokumen, Git,
//! Node.js) dan winget untuk memasangnya. Hanya membaca versi (perintah `--version`), tidak memasang apa pun.
use serde::Serialize;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);
/// Pustaka Python yang dipakai skill dokumen bawaan untuk MENGEDIT (membaca sudah lewat `Zandor office-check`).
pub const PY_MODULES: [&str; 4] = ["docx", "openpyxl", "pptx", "pypdf"];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub found: bool,
    pub version: Option<String>,
    /// Perintah yang dipakai (mis. `py -3`), atau daftar yang kurang untuk pustaka.
    pub detail: Option<String>,
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(c.output());
    });
    let out = rx.recv_timeout(TIMEOUT).ok()?.ok()?;
    if !out.status.success() {
        return None;
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Some(text.trim().to_string())
}

/// Ambil nomor versi pertama (mis. "Python 3.12.1" → "3.12.1", "v20.11.0" → "20.11.0").
pub fn parse_version(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == 'v')
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()) && w.contains('.'))
        .map(|w| {
            w.trim_end_matches(|c: char| !c.is_ascii_digit())
                .to_string()
        })
}

fn simple(id: &str, cmd: &str, args: &[&str]) -> Check {
    let out = run(cmd, args);
    Check {
        id: id.into(),
        found: out.is_some(),
        version: out.as_deref().and_then(parse_version),
        detail: None,
    }
}

/// Alat yang di Windows sering berupa skrip `.cmd`/`.ps1` (npm, opencode dari npm): dijalankan lewat `cmd /C`.
fn script(id: &str, name: &str) -> Check {
    #[cfg(windows)]
    let out = run("cmd", &["/C", name, "--version"]);
    #[cfg(not(windows))]
    let out = run(name, &["--version"]);
    Check {
        id: id.into(),
        found: out.is_some(),
        version: out.as_deref().and_then(parse_version),
        detail: None,
    }
}

/// LibreOffice (konversi/pratinjau dokumen): `soffice` di PATH atau lokasi pasang bawaan Windows.
fn libreoffice() -> Check {
    let mut paths = vec!["soffice".to_string()];
    #[cfg(windows)]
    for base in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(dir) = std::env::var(base) {
            let p = format!("{dir}/LibreOffice/program/soffice.com");
            if std::path::Path::new(&p).exists() {
                paths.insert(0, p);
            }
        }
    }
    let found = paths
        .iter()
        .find_map(|p| run(p, &["--version"]).map(|o| (p.clone(), o)));
    Check {
        id: "libreoffice".into(),
        found: found.is_some(),
        version: found.as_ref().and_then(|(_, o)| parse_version(o)),
        detail: found.map(|(p, _)| p),
    }
}

/// Perintah Python yang berfungsi. `python` di Windows bisa berupa alias Microsoft Store yang gagal → dilewati.
fn python() -> Option<(Vec<&'static str>, String)> {
    let candidates: [&[&str]; 3] = [&["python"], &["py", "-3"], &["python3"]];
    candidates.iter().find_map(|c| {
        let mut args = c[1..].to_vec();
        args.push("--version");
        let out = run(c[0], &args)?;
        out.contains("Python").then(|| (c.to_vec(), out))
    })
}

pub fn check_all() -> Vec<Check> {
    let mut out = Vec::new();
    match python() {
        Some((cmd, text)) => {
            let label = cmd.join(" ");
            out.push(Check {
                id: "python".into(),
                found: true,
                version: parse_version(&text),
                detail: Some(label),
            });
            let mut pip = cmd[1..].to_vec();
            pip.extend(["-m", "pip", "--version"]);
            let pip_out = run(cmd[0], &pip);
            out.push(Check {
                id: "pip".into(),
                found: pip_out.is_some(),
                version: pip_out.as_deref().and_then(parse_version),
                detail: None,
            });
            let script = format!(
                "import importlib.util as u;print(','.join(m for m in {:?} if u.find_spec(m) is None))",
                PY_MODULES
            );
            let mut args = cmd[1..].to_vec();
            args.extend(["-c", script.as_str()]);
            let missing = run(cmd[0], &args);
            out.push(Check {
                id: "pythonOffice".into(),
                found: missing.as_deref() == Some(""),
                version: None,
                detail: missing.filter(|m| !m.is_empty()),
            });
        }
        None => {
            out.push(Check {
                id: "python".into(),
                found: false,
                version: None,
                detail: None,
            });
            out.push(Check {
                id: "pip".into(),
                found: false,
                version: None,
                detail: None,
            });
            out.push(Check {
                id: "pythonOffice".into(),
                found: false,
                version: None,
                detail: Some(PY_MODULES.join(",")),
            });
        }
    }
    out.push(simple("git", "git", &["--version"]));
    out.push(script("gh", "gh"));
    out.push(simple("node", "node", &["--version"]));
    out.push(script("npm", "npm"));
    out.push(script("bun", "bun"));
    out.push(script("uv", "uv"));
    out.push(script("opencode", "opencode"));
    out.push(libreoffice());
    out.push(simple("winget", "winget", &["--version"]));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versi_diurai_dari_keluaran_umum() {
        assert_eq!(parse_version("Python 3.12.1").as_deref(), Some("3.12.1"));
        assert_eq!(parse_version("v20.11.0").as_deref(), Some("20.11.0"));
        assert_eq!(
            parse_version("git version 2.45.1.windows.1").as_deref(),
            Some("2.45.1.windows.1")
        );
        assert_eq!(parse_version("tidak ada"), None);
    }

    #[test]
    fn perintah_tak_ada_dianggap_tidak_terpasang() {
        assert!(run("zandor-tidak-ada-perintah-ini", &["--version"]).is_none());
    }
}

#[cfg(test)]
mod live {
    /// Manual: `cargo test syscheck::live -- --ignored --nocapture` mencetak hasil pemeriksaan di mesin ini.
    #[test]
    #[ignore]
    fn cetak_hasil() {
        for c in super::check_all() {
            println!("{c:?}");
        }
    }
}

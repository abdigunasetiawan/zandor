//! Skill bawaan dokumen (ADR-070): `zandor-docx`, `zandor-xlsx`, `zandor-pptx`, `zandor-pdf`; plus `zandor-app`
//! (mengoperasikan/mengonfigurasi Zandor, ADR-077).
//! Isinya ditanam di biner (`include_str!`) dan disinkronkan ke folder skill global engine
//! (`<engine_root>/config/opencode/skills/<nama>/`) setiap engine dimulai. Berkas hanya ditulis bila isinya berbeda,
//! jadi pembaruan aplikasi otomatis memperbarui skill. `{{SKILL_DIR}}` diganti path folder skill sebenarnya,
//! `{{OFFICE_CHECK}}` diganti perintah pemeriksa bawaan (`"<Zandor.exe>" office-check`, ADR-074) sehingga membaca dan
//! memverifikasi dokumen tidak butuh Python.
use std::path::Path;

/// Berkas pemeriksa Python lama (ADR-070) yang dihapus dari folder skill saat pemasangan.
const LEGACY_CHECKER: &str = "office_check.py";

/// Perintah shell pemeriksa dokumen: biner aplikasi ini dengan subperintah `office-check`.
pub fn checker_command() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| "Zandor".into());
    format!("\"{exe}\" office-check")
}

pub const SKILLS: [(&str, &str); 5] = [
    (
        "zandor-docx",
        include_str!("../bundled-skills/zandor-docx/SKILL.md"),
    ),
    (
        "zandor-xlsx",
        include_str!("../bundled-skills/zandor-xlsx/SKILL.md"),
    ),
    (
        "zandor-pptx",
        include_str!("../bundled-skills/zandor-pptx/SKILL.md"),
    ),
    (
        "zandor-pdf",
        include_str!("../bundled-skills/zandor-pdf/SKILL.md"),
    ),
    (
        "zandor-app",
        include_str!("../bundled-skills/zandor-app/SKILL.md"),
    ),
];

fn write_if_changed(path: &Path, content: &str) -> std::io::Result<bool> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(content) {
        return Ok(false);
    }
    std::fs::write(path, content)?;
    Ok(true)
}

/// Pasang/perbarui skill bawaan di `skills_root`. Mengembalikan jumlah berkas yang ditulis.
pub fn install(skills_root: &Path, checker: &str) -> Result<usize, String> {
    let mut written = 0;
    for (name, body) in SKILLS {
        let dir = skills_root.join(name);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{name}: {e}"))?;
        let dir_str = dir.to_string_lossy().replace('\\', "/");
        // Checkout git di Windows bisa berakhiran CRLF; frontmatter dinormalisasi ke LF.
        let md = body
            .replace("\r\n", "\n")
            .replace("{{SKILL_DIR}}", &dir_str)
            .replace("{{OFFICE_CHECK}}", checker);
        if write_if_changed(&dir.join("SKILL.md"), &md)
            .map_err(|e| format!("{name}/SKILL.md: {e}"))?
        {
            written += 1;
        }
        let legacy = dir.join(LEGACY_CHECKER);
        if legacy.is_file() {
            std::fs::remove_file(&legacy).map_err(|e| format!("{name}/{LEGACY_CHECKER}: {e}"))?;
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::valid_skill_name;

    #[test]
    fn skills_have_valid_names_and_frontmatter() {
        for (name, body) in SKILLS {
            let body = body.replace("\r\n", "\n");
            assert!(valid_skill_name(name), "{name}");
            assert!(
                body.starts_with(&format!("---\nname: {name}\ndescription: \"")),
                "{name}"
            );
            // Skill dokumen memakai pemeriksa bawaan; skill aplikasi (zandor-app) tidak.
            assert!(
                name == "zandor-app" || body.contains("{{OFFICE_CHECK}}"),
                "{name}"
            );
            assert!(!body.contains("office_check.py"), "{name}");
            assert!(body.len() <= crate::memory::MAX_SKILL_BYTES, "{name}");
        }
    }

    #[test]
    fn install_writes_then_is_idempotent() {
        let tmp = std::env::temp_dir().join(format!("zandor-bundled-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        // Sisa pemeriksa Python dari versi lama dibersihkan.
        std::fs::create_dir_all(tmp.join("zandor-pdf")).unwrap();
        std::fs::write(tmp.join("zandor-pdf").join(LEGACY_CHECKER), "lama").unwrap();
        let cmd = "\"C:/Apps/Zandor.exe\" office-check";
        assert_eq!(install(&tmp, cmd).unwrap(), SKILLS.len());
        let md = std::fs::read_to_string(tmp.join("zandor-docx").join("SKILL.md")).unwrap();
        assert!(!md.contains("{{SKILL_DIR}}") && !md.contains("{{OFFICE_CHECK}}"));
        assert!(md.contains("`\"C:/Apps/Zandor.exe\" office-check berkas.docx --text`"));
        assert!(!tmp.join("zandor-pdf").join(LEGACY_CHECKER).exists());
        assert_eq!(install(&tmp, cmd).unwrap(), 0);
        // Berkas yang diubah/rusak dipulihkan.
        std::fs::write(tmp.join("zandor-xlsx").join("SKILL.md"), "rusak").unwrap();
        assert_eq!(install(&tmp, cmd).unwrap(), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

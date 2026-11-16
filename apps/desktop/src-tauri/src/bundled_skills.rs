//! Skill bawaan dokumen (ADR-070): `zandor-docx`, `zandor-xlsx`, `zandor-pptx`, `zandor-pdf`.
//! Isinya ditanam di biner (`include_str!`) dan disinkronkan ke folder skill global engine
//! (`<engine_root>/config/opencode/skills/<nama>/`) setiap engine dimulai. Berkas hanya ditulis bila isinya berbeda,
//! jadi pembaruan aplikasi otomatis memperbarui skill. `{{SKILL_DIR}}` diganti path folder skill sebenarnya.
use std::path::Path;

const CHECKER: &str = include_str!("../bundled-skills/office_check.py");

pub const SKILLS: [(&str, &str); 4] = [
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
];

fn write_if_changed(path: &Path, content: &str) -> std::io::Result<bool> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(content) {
        return Ok(false);
    }
    std::fs::write(path, content)?;
    Ok(true)
}

/// Pasang/perbarui skill bawaan di `skills_root`. Mengembalikan jumlah berkas yang ditulis.
pub fn install(skills_root: &Path) -> Result<usize, String> {
    let mut written = 0;
    for (name, body) in SKILLS {
        let dir = skills_root.join(name);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{name}: {e}"))?;
        let dir_str = dir.to_string_lossy().replace('\\', "/");
        // Checkout git di Windows bisa berakhiran CRLF; frontmatter dinormalisasi ke LF.
        let md = body
            .replace("\r\n", "\n")
            .replace("{{SKILL_DIR}}", &dir_str);
        for (file, content) in [("SKILL.md", md.as_str()), ("office_check.py", CHECKER)] {
            if write_if_changed(&dir.join(file), content)
                .map_err(|e| format!("{name}/{file}: {e}"))?
            {
                written += 1;
            }
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
            assert!(body.contains("{{SKILL_DIR}}/office_check.py"), "{name}");
            assert!(body.len() <= crate::memory::MAX_SKILL_BYTES, "{name}");
        }
    }

    #[test]
    fn install_writes_then_is_idempotent() {
        let tmp = std::env::temp_dir().join(format!("zandor-bundled-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        assert_eq!(install(&tmp).unwrap(), 8);
        let md = std::fs::read_to_string(tmp.join("zandor-docx").join("SKILL.md")).unwrap();
        assert!(!md.contains("{{SKILL_DIR}}"));
        assert!(md.contains("zandor-docx/office_check.py"));
        assert!(tmp.join("zandor-pdf").join("office_check.py").is_file());
        assert_eq!(install(&tmp).unwrap(), 0);
        // Berkas yang diubah/rusak dipulihkan.
        std::fs::write(tmp.join("zandor-xlsx").join("SKILL.md"), "rusak").unwrap();
        assert_eq!(install(&tmp).unwrap(), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

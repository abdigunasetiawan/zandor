//! Memori dan skill (Fase 9, ADR-037). Data memori ada di `<app_data>/memory` (mesin engine) dan dibaca plugin Zandor
//! setiap giliran. Modul ini hanya mengizinkan berkas yang dikenal (daftar putih), jadi UI tidak bisa menulis
//! ke tempat lain lewat perintah ini. Skill terpasang berada di folder skill opencode (project atau global).

use serde::Serialize;
use std::path::{Component, Path, PathBuf};

pub const MAX_WRITE_BYTES: usize = 200 * 1024;
pub const MAX_SKILL_BYTES: usize = 16 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct MemoryFile {
    pub rel: String,
    pub size: u64,
    pub modified: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SkillInfo {
    pub name: String,
    pub scope: String,
    pub path: String,
}

fn is_hex16(s: &str) -> bool {
    s.len() == 16
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `rel` harus salah satu berkas memori yang dikenal. Mengembalikan path absolut di dalam `dir`.
pub fn resolve_memory(dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let ok = match rel {
        "settings.json" | "global.md" | "skill-queue.jsonl" | "meta.json" => true,
        _ => {
            if let Some(name) = rel
                .strip_prefix("projects/")
                .and_then(|r| r.strip_suffix(".md"))
            {
                is_hex16(name)
            } else if let Some(name) = rel
                .strip_prefix("pins/")
                .and_then(|r| r.strip_suffix(".md"))
            {
                is_safe_id(name)
            } else {
                false
            }
        }
    };
    if !ok {
        return Err(format!("berkas memori tidak dikenal: {rel}"));
    }
    Ok(dir.join(rel))
}

pub fn list_memory(dir: &Path) -> Vec<MemoryFile> {
    let mut out = Vec::new();
    let mut push = |rel: String, p: &Path| {
        if let Ok(m) = std::fs::metadata(p) {
            if m.is_file() {
                let modified = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                out.push(MemoryFile {
                    rel,
                    size: m.len(),
                    modified,
                });
            }
        }
    };
    for f in [
        "settings.json",
        "global.md",
        "skill-queue.jsonl",
        "meta.json",
    ] {
        push(f.to_string(), &dir.join(f));
    }
    for sub in ["projects", "pins"] {
        if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
            let mut names: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect();
            names.sort();
            for n in names {
                let rel = format!("{sub}/{n}");
                if resolve_memory(dir, &rel).is_ok() {
                    push(rel, &dir.join(sub).join(&n));
                }
            }
        }
    }
    out
}

pub fn read_memory(dir: &Path, rel: &str) -> Result<String, String> {
    let p = resolve_memory(dir, rel)?;
    match std::fs::read(&p) {
        Ok(b) => Ok(String::from_utf8_lossy(&b).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("gagal membaca: {e}")),
    }
}

pub fn write_memory(dir: &Path, rel: &str, text: &str) -> Result<(), String> {
    if rel == "meta.json" {
        return Err("meta.json hanya dibaca".into());
    }
    if text.len() > MAX_WRITE_BYTES {
        return Err("isi terlalu besar (maks 200 KB)".into());
    }
    let p = resolve_memory(dir, rel)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("gagal membuat folder: {e}"))?;
    }
    std::fs::write(&p, text).map_err(|e| format!("gagal menulis: {e}"))
}

pub fn delete_memory(dir: &Path, rel: &str) -> Result<(), String> {
    if !(rel == "global.md" || rel.starts_with("projects/") || rel.starts_with("pins/")) {
        return Err("berkas ini tidak bisa dihapus".into());
    }
    let p = resolve_memory(dir, rel)?;
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("gagal menghapus: {e}")),
    }
}

/// Nama skill: kebab-case huruf kecil/angka, maks 64.
pub fn valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Folder skill: `<project>/.opencode/skills` atau `<engine_root>/config/opencode/skills`.
pub fn skills_root(
    scope: &str,
    project_dir: Option<&Path>,
    engine_root: &Path,
) -> Result<PathBuf, String> {
    match scope {
        "global" => Ok(engine_root.join("config").join("opencode").join("skills")),
        "project" => {
            let p = project_dir.ok_or("project wajib untuk skill project")?;
            Ok(p.join(".opencode").join("skills"))
        }
        _ => Err("scope harus global atau project".into()),
    }
}

fn skill_file(root: &Path, name: &str) -> Result<PathBuf, String> {
    if !valid_skill_name(name) {
        return Err("nama skill tidak valid".into());
    }
    let f = root.join(name).join("SKILL.md");
    // Pertahanan berlapis: tidak boleh keluar dari root.
    if f.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("nama skill tidak valid".into());
    }
    Ok(f)
}

pub fn list_skills(root: &Path, scope: &str) -> Vec<SkillInfo> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            let f = e.path().join("SKILL.md");
            if valid_skill_name(&name) && f.is_file() {
                out.push(SkillInfo {
                    name,
                    scope: scope.to_string(),
                    path: f.to_string_lossy().into_owned(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn read_skill(root: &Path, name: &str) -> Result<String, String> {
    let f = skill_file(root, name)?;
    std::fs::read_to_string(&f).map_err(|e| format!("gagal membaca skill: {e}"))
}

pub fn write_skill(
    root: &Path,
    name: &str,
    markdown: &str,
    overwrite: bool,
) -> Result<String, String> {
    if markdown.len() > MAX_SKILL_BYTES {
        return Err("isi skill terlalu besar (maks 16 KB)".into());
    }
    let f = skill_file(root, name)?;
    if f.exists() && !overwrite {
        return Err(format!("skill \"{name}\" sudah ada"));
    }
    std::fs::create_dir_all(f.parent().expect("punya induk"))
        .map_err(|e| format!("gagal membuat folder: {e}"))?;
    std::fs::write(&f, markdown).map_err(|e| format!("gagal menulis skill: {e}"))?;
    Ok(f.to_string_lossy().into_owned())
}

pub fn delete_skill(root: &Path, name: &str) -> Result<(), String> {
    let f = skill_file(root, name)?;
    let dir = f.parent().expect("punya induk");
    // Hanya SKILL.md yang dihapus; folder dihapus bila jadi kosong. Berkas tambahan pengguna tidak disentuh.
    match std::fs::remove_file(&f) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("gagal menghapus skill: {e}")),
    }
    let _ = std::fs::remove_dir(dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zandor-mem-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn only_known_memory_files_are_reachable() {
        let d = tmp("whitelist");
        for ok in [
            "settings.json",
            "global.md",
            "skill-queue.jsonl",
            "meta.json",
            "projects/0123456789abcdef.md",
            "pins/ses_ab-1.md",
        ] {
            assert!(resolve_memory(&d, ok).is_ok(), "{ok}");
        }
        for bad in [
            "../x.md",
            "projects/../global.md",
            "projects/abc.md",
            "projects/0123456789ABCDEF.md",
            "pins/../../x.md",
            "pins/a/b.md",
            "pins/.md",
            "index.sqlite",
            "C:/Windows/win.ini",
            "global.md/../x",
        ] {
            assert!(resolve_memory(&d, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn write_read_list_delete_roundtrip() {
        let d = tmp("rt");
        assert_eq!(read_memory(&d, "global.md").unwrap(), "");
        write_memory(&d, "global.md", "- halo\n").unwrap();
        write_memory(&d, "projects/0123456789abcdef.md", "- p\n").unwrap();
        write_memory(&d, "pins/s1.md", "pin").unwrap();
        assert_eq!(read_memory(&d, "global.md").unwrap(), "- halo\n");
        let rels: Vec<String> = list_memory(&d).into_iter().map(|f| f.rel).collect();
        assert_eq!(
            rels,
            ["global.md", "projects/0123456789abcdef.md", "pins/s1.md"]
        );
        delete_memory(&d, "pins/s1.md").unwrap();
        delete_memory(&d, "pins/s1.md").unwrap(); // idempoten
        assert!(!d.join("pins/s1.md").exists());
        assert!(delete_memory(&d, "settings.json").is_err());
        assert!(write_memory(&d, "meta.json", "{}").is_err());
        assert!(write_memory(&d, "global.md", &"x".repeat(MAX_WRITE_BYTES + 1)).is_err());
    }

    #[test]
    fn skill_names_are_strict_kebab_case() {
        for ok in ["a", "tulis-changelog", "x1-y2"] {
            assert!(valid_skill_name(ok), "{ok}");
        }
        for bad in [
            "",
            "-a",
            "a-",
            "a--b",
            "A",
            "a_b",
            "../x",
            "a/b",
            "a b",
            &"a".repeat(65),
        ] {
            assert!(!valid_skill_name(bad), "{bad}");
        }
    }

    #[test]
    fn skill_install_list_read_overwrite_delete() {
        let d = tmp("skill");
        let root = skills_root("project", Some(&d), &d).unwrap();
        assert!(root.ends_with(".opencode/skills") || root.ends_with(r".opencode\skills"));
        let path = write_skill(
            &root,
            "tulis-changelog",
            "---\nname: tulis-changelog\n---\nisi",
            false,
        )
        .unwrap();
        assert!(Path::new(&path).is_file());
        assert!(write_skill(&root, "tulis-changelog", "lain", false)
            .unwrap_err()
            .contains("sudah ada"));
        write_skill(&root, "tulis-changelog", "baru", true).unwrap();
        assert_eq!(read_skill(&root, "tulis-changelog").unwrap(), "baru");
        assert_eq!(
            list_skills(&root, "project")
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["tulis-changelog"]
        );
        assert!(write_skill(&root, "../evil", "x", true).is_err());
        assert!(write_skill(&root, "ok", &"x".repeat(MAX_SKILL_BYTES + 1), false).is_err());
        // berkas tambahan pengguna dipertahankan
        std::fs::write(
            root.join("tulis-changelog").join("catatan.txt"),
            "milik user",
        )
        .unwrap();
        delete_skill(&root, "tulis-changelog").unwrap();
        assert!(!root.join("tulis-changelog").join("SKILL.md").exists());
        assert!(root.join("tulis-changelog").join("catatan.txt").exists());
    }

    #[test]
    fn global_scope_lives_under_engine_config() {
        let e = Path::new("/data/engine");
        let r = skills_root("global", None, e).unwrap();
        assert!(
            r.starts_with(e)
                && r.to_string_lossy()
                    .replace('\\', "/")
                    .ends_with("config/opencode/skills")
        );
        assert!(skills_root("project", None, e).is_err());
        assert!(skills_root("lain", None, e).is_err());
    }
}

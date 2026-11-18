//! Fungsi murni memori — port packages/core/src/memory/store.ts (bagian paritas dengan plugin): kunci project, pemindai
//! secret, nama berkas memori/sematan, pengaturan memori, antrean skill, dan draf skill.
use serde_json::Value;
use std::collections::BTreeMap;

pub const FILE_GLOBAL: &str = "global.md";
pub const FILE_SETTINGS: &str = "settings.json";
pub const FILE_QUEUE: &str = "skill-queue.jsonl";
pub const FILE_META: &str = "meta.json";

pub fn project_file(key: &str) -> String {
    format!("projects/{key}.md")
}
pub fn pin_file(session_id: &str) -> String {
    let safe: String = session_id.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    format!("pins/{safe}.md")
}

pub fn normalize_project_path(path: &str) -> String {
    let p = path.replace('\\', "/");
    let p = p.strip_prefix("//?/").unwrap_or(&p).to_string();
    let p = if p.len() > 1 { p.trim_end_matches('/').to_string() } else { p };
    p.to_lowercase()
}

/// sha256 16 heks dari path ternormalisasi (sama dengan plugin).
pub fn project_key(path: &str) -> String {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(normalize_project_path(path).as_bytes());
    h.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub fn scan_secrets(text: &str) -> Vec<&'static str> {
    let rules: [(&str, &str); 10] = [
        ("kunci API bergaya sk-", r"\bsk-[A-Za-z0-9_-]{16,}"),
        ("token GitHub", r"\bgh[pousr]_[A-Za-z0-9]{30,}"),
        ("token Slack", r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),
        ("kunci akses AWS", r"\bAKIA[0-9A-Z]{16}\b"),
        ("kunci Google API", r"\bAIza[0-9A-Za-z_-]{30,}"),
        ("token JWT", r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
        ("kunci privat", r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
        ("header Authorization", r"(?i)\bauthorization\s*[:=]\s*(bearer|basic)\s+[A-Za-z0-9._~+/=-]{12,}"),
        ("kredensial di URL", r"(?i)\b[a-z][a-z0-9+.-]*://[^\s/:@]+:[^\s/@]{3,}@"),
        ("variabel rahasia", r#"\b[A-Z0-9_]*(?:SECRET|TOKEN|PASSWORD|API_?KEY)[A-Z0-9_]*\s*[=:]\s*["']?[^\s"']{8,}"#),
    ];
    rules.iter().filter(|(_, re)| regex::Regex::new(re).expect("regex").is_match(text)).map(|(n, _)| *n).collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemorySettings {
    pub default_enabled: bool,
    pub projects: BTreeMap<String, bool>,
    pub sessions: BTreeMap<String, bool>,
    pub skill_creator: bool,
    pub skill_creator_projects: BTreeMap<String, bool>,
    pub skill_creator_sessions: BTreeMap<String, bool>,
}

fn bool_map(v: Option<&Value>) -> BTreeMap<String, bool> {
    v.and_then(Value::as_object).map(|o| o.iter().filter_map(|(k, x)| x.as_bool().map(|b| (k.clone(), b))).collect()).unwrap_or_default()
}

pub fn parse_settings(text: &str) -> MemorySettings {
    let v: Value = serde_json::from_str(if text.is_empty() { "{}" } else { text }).unwrap_or(Value::Null);
    MemorySettings {
        default_enabled: v.get("defaultEnabled") == Some(&Value::Bool(true)),
        projects: bool_map(v.get("projects")),
        sessions: bool_map(v.get("sessions")),
        skill_creator: v.get("skillCreator") == Some(&Value::Bool(true)),
        skill_creator_projects: bool_map(v.get("skillCreatorProjects")),
        skill_creator_sessions: bool_map(v.get("skillCreatorSessions")),
    }
}

pub fn serialize_settings(s: &MemorySettings) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "defaultEnabled": s.default_enabled,
        "projects": s.projects,
        "sessions": s.sessions,
        "skillCreator": s.skill_creator,
        "skillCreatorProjects": s.skill_creator_projects,
        "skillCreatorSessions": s.skill_creator_sessions,
    }))
    .unwrap_or_default()
}

/// Sesi → project → bawaan.
pub fn memory_enabled(s: &MemorySettings, key: Option<&str>, session: Option<&str>) -> bool {
    session.and_then(|x| s.sessions.get(x).copied()).or_else(|| key.and_then(|k| s.projects.get(k).copied())).unwrap_or(s.default_enabled)
}
pub fn skill_creator_enabled(s: &MemorySettings, key: Option<&str>, session: Option<&str>) -> bool {
    session.and_then(|x| s.skill_creator_sessions.get(x).copied()).or_else(|| key.and_then(|k| s.skill_creator_projects.get(k).copied())).unwrap_or(s.skill_creator)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SkillDraft {
    pub name: String,
    pub description: String,
    pub body: String,
}

pub fn validate_skill_draft(d: &SkillDraft) -> Option<String> {
    let name_ok = regex::Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").expect("regex").is_match(&d.name) && d.name.len() <= 64;
    if !name_ok {
        return Some("Nama skill harus huruf kecil/angka dengan tanda hubung (maks 64), mis. `tulis-changelog`.".into());
    }
    let dl = d.description.chars().count();
    if d.description.trim().chars().count() < 10 || dl > 300 {
        return Some("Deskripsi harus 10–300 karakter.".into());
    }
    let bl = d.body.chars().count();
    if d.body.trim().chars().count() < 20 || bl > 8000 {
        return Some("Isi skill harus 20–8000 karakter.".into());
    }
    let found = scan_secrets(&format!("{}\n{}\n{}", d.name, d.description, d.body));
    if !found.is_empty() {
        return Some(format!("Ditolak: draf tampak memuat secret ({}).", found.join(", ")));
    }
    None
}

pub fn skill_markdown(e: &SkillDraft) -> String {
    let desc = regex::Regex::new(r"\s+").expect("regex").replace_all(&e.description, " ").replace('"', "\\\"");
    format!("---\nname: {}\ndescription: \"{desc}\"\n---\n\n{}\n", e.name, e.body.trim())
}

pub fn parse_skill_markdown(md: &str) -> Option<SkillDraft> {
    let re = regex::Regex::new(r"(?s)^---\r?\nname:\s*(.+?)\r?\ndescription:\s*(.+?)\r?\n---\r?\n\r?\n?(.*)$").expect("regex");
    let c = re.captures(md)?;
    let unq = |s: &str| if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') { s[1..s.len() - 1].replace("\\\"", "\"") } else { s.to_string() };
    Some(SkillDraft { name: unq(&c[1]), description: unq(&c[2]), body: c[3].trim().to_string() })
}

/// Tambah catatan ke berkas memori (format MessageToolbar.saveNote): "## YYYY-MM-DD HH:MM\n\n<teks>".
pub fn append_note(existing: &str, stamp: &str, text: &str) -> String {
    let base = existing.trim_end();
    format!("{base}{}## {stamp}\n\n{text}\n", if existing.is_empty() { "" } else { "\n\n" })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fungsi_murni() {
        assert_eq!(normalize_project_path("C:\\Code\\Zandor\\"), "c:/code/zandor");
        assert_eq!(project_key("C:\\Code\\Zandor").len(), 16);
        assert_eq!(project_key("C:\\Code\\Zandor"), project_key("c:/code/zandor/"));
        assert_eq!(pin_file("ses/a b"), "pins/ses_a_b.md");
        assert_eq!(scan_secrets("API_KEY=abcdefgh12345"), vec!["variabel rahasia"]);
        assert!(scan_secrets("halo dunia").is_empty());
        let s = parse_settings(r#"{"defaultEnabled":true,"projects":{"k":false}}"#);
        assert!(memory_enabled(&s, None, None));
        assert!(!memory_enabled(&s, Some("k"), None));
        assert_eq!(parse_settings(&serialize_settings(&s)), s);
        assert_eq!(parse_settings("rusak"), MemorySettings::default());
        let d = SkillDraft { name: "tulis-changelog".into(), description: "Menulis changelog rapi".into(), body: "Langkah-langkah menulis changelog yang baik.".into() };
        assert_eq!(validate_skill_draft(&d), None);
        assert_eq!(parse_skill_markdown(&skill_markdown(&d)), Some(d));
        assert_eq!(append_note("", "2026-01-01 10:00", "isi"), "## 2026-01-01 10:00\n\nisi\n");
        assert_eq!(append_note("lama\n\n", "s", "x"), "lama\n\n## s\n\nx\n");
    }
}

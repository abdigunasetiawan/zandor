//! Logika murni composer — port packages/core/src/chat/composer-input.ts dan permission-risk.ts:
//! token `/perintah` & `@berkas`, parsing slash, perintah bawaan, lampiran mention, mode izin & risiko.
use super::attachments::{file_url_from_path, FileAttachment};
use super::types::PermissionRequest;
use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Slash,
    Mention,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerToken {
    pub kind: TokenKind,
    pub query: String,
    /// Offset byte karakter pemicu.
    pub start: usize,
    /// Offset byte akhir token (kursor).
    pub end: usize,
}

/// Token yang sedang diketik di posisi kursor (offset byte), atau None.
pub fn active_token(text: &str, caret: usize) -> Option<ComposerToken> {
    let mut caret = caret.min(text.len());
    while !text.is_char_boundary(caret) {
        caret -= 1;
    }
    let before = &text[..caret];
    // slash hanya di awal pesan dan sebelum spasi pertama
    if let Some(q) = before.strip_prefix('/') {
        if !q.chars().any(|c| c.is_whitespace() || c == '/') {
            return Some(ComposerToken { kind: TokenKind::Slash, query: q.into(), start: 0, end: caret });
        }
    }
    let at = before.rfind('@')?;
    let q = &before[at + 1..];
    if q.chars().any(|c| c.is_whitespace() || c == '@') {
        return None;
    }
    let ok = at == 0 || before[..at].chars().last().is_some_and(char::is_whitespace);
    ok.then(|| ComposerToken { kind: TokenKind::Mention, query: q.into(), start: at, end: caret })
}

/// Ganti token dengan teks pengganti; kembalikan (teks baru, kursor).
pub fn replace_token(text: &str, token: &ComposerToken, replacement: &str) -> (String, usize) {
    let after = &text[token.end..];
    let sep = if after.starts_with(' ') { "" } else { " " };
    (format!("{}{replacement}{sep}{after}", &text[..token.start]), token.start + replacement.len() + 1)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedSlash {
    pub name: String,
    pub args: String,
}

static SLASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/([A-Za-z0-9][\w:.-]*)(?:\s+([\s\S]*))?$").unwrap());

/// `/nama argumen…` → ParsedSlash; bukan perintah (atau `//`) → None.
pub fn parse_slash(text: &str) -> Option<ParsedSlash> {
    let c = SLASH.captures(text.trim())?;
    Some(ParsedSlash { name: c[1].to_string(), args: c.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default() })
}

pub const BUILTIN_COMMANDS: [&str; 9] = ["btw", "new", "compact", "undo", "redo", "fork", "copy", "mode", "stop"];
pub const BUILTIN_NEEDS_ARGS: [&str; 2] = ["btw", "mode"];
pub fn is_builtin(name: &str) -> bool {
    BUILTIN_COMMANDS.contains(&name)
}

/// Saring: awalan dulu, lalu yang mengandung; urutan asli dipertahankan.
pub fn rank_by_query<T: Clone>(items: &[T], query: &str, key: impl Fn(&T) -> String) -> Vec<T> {
    let q = query.to_lowercase();
    if q.is_empty() {
        return items.to_vec();
    }
    let (mut pre, mut con) = (vec![], vec![]);
    for it in items {
        let k = key(it).to_lowercase();
        if k.starts_with(&q) {
            pre.push(it.clone());
        } else if k.contains(&q) {
            con.push(it.clone());
        }
    }
    pre.extend(con);
    pre
}

/// Lampiran untuk berkas project yang di-mention (`rel` relatif terhadap `root`).
pub fn mention_attachment(root: &str, rel: &str) -> FileAttachment {
    let sep = if root.contains('\\') { "\\" } else { "/" };
    let abs = format!("{}{sep}{}", root.trim_end_matches(['\\', '/']), rel.replace(['\\', '/'], sep));
    let ext = rel.rsplit('.').next().unwrap_or("").to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "text/plain",
    };
    FileAttachment { mime: mime.into(), url: file_url_from_path(&abs), filename: Some(rel.replace('\\', "/")) }
}

// ---------- mode izin ----------

pub const PERMISSION_MODES: [&str; 4] = ["ask", "auto-edit", "auto-safe", "auto-all"];
pub const DEFAULT_PERMISSION_MODE: &str = "auto-safe";

static SECRET_PATH: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)\.(pem|key|p12|pfx|jks|keystore|kdbx)$",
        r"(?i)(^|[\\/])id_(rsa|dsa|ecdsa|ed25519)(\.pub)?$",
        r"(?i)(^|[\\/])\.ssh([\\/]|$)",
        r"(?i)(^|[\\/])\.git[\\/]",
        r"(?i)(^|[\\/])(\.npmrc|\.pypirc|\.netrc|\.git-credentials|credentials(\.json)?|secrets?\.(json|ya?ml|toml))$",
    ]
    .iter()
    .map(|r| Regex::new(r).unwrap())
    .collect()
});
static ENV_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(^|[\\/])\.env(\.([^\\/]*))?$").unwrap());
static ENV_BASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)(^|[\s"'=/\\])\.env(\.(\w+))?(\s|$|["'])"#).unwrap());
const ENV_SAFE: [&str; 3] = ["example", "sample", "template"];

fn is_secret_path(p: &str) -> bool {
    let p = p.trim();
    if let Some(c) = ENV_PATH.captures(p) {
        match c.get(3) {
            None => return true,
            Some(s) if !ENV_SAFE.contains(&s.as_str().to_lowercase().as_str()) => return true,
            _ => {}
        }
    }
    SECRET_PATH.iter().any(|r| r.is_match(p))
}

static BASH_RULES: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("secret", r"(?i)[/\\]\.ssh\b|\bid_(rsa|ed25519|ecdsa)\b"),
        ("delete", r"(?i)\brm\s+(-\w*[rRf]|--recursive|--force)|\brm\s+[^|;&]*\*|\brmdir\b|\brd\s+/s|\bdel\s+(/[sfq]\s*)+|\bRemove-Item\b[^|;&]*-(Recurse|Force)|\bfind\b[^|;&]*-delete\b|\bshred\b"),
        ("git-history", r"(?i)\bgit\s+(reset\s+--hard|clean\s+-\w*f|checkout\s+(--\s|\.)|restore\b|branch\s+-D|rebase\b|filter-branch|filter-repo|stash\s+(drop|clear)|reflog\s+expire|update-ref\s+-d)"),
        ("publish", r"(?i)\bgit\s+push\b|\b(npm|pnpm|yarn|bun)\s+publish\b|\bcargo\s+publish\b|\btwine\s+upload\b|\bdocker\s+push\b|\bgh\s+(release\s+(create|delete)|repo\s+(delete|create)|pr\s+merge)\b|\bvercel\b[^|;&]*--prod|\bnetlify\s+deploy\b[^|;&]*--prod"),
        ("remote-script", r"(?i)\b(curl|wget)\b[^|]*\|\s*(sudo\s+)?(ba|z)?sh\b|\b(iwr|irm|Invoke-WebRequest|Invoke-RestMethod)\b[^|]*\|\s*(iex|Invoke-Expression)\b|\b(iex|Invoke-Expression)\b"),
        ("system", r"(?i)(^|[;&|(]\s*)(sudo|su|doas|runas|shutdown|reboot|halt|poweroff)(\s|$)|\bmkfs\b|\bformat\s+[a-z]:|\bdd\s+[^|;&]*of=|\bchmod\s+(-R\s+)?(777|-R)|\bchown\s+-R\b|\b(reg|regedit)\s+(add|delete|import)\b|\bsetx\b|\bSet-ExecutionPolicy\b|\b(sc|systemctl|service)\s+(stop|delete|disable|config)\b|\b(kill\s+-9|killall|pkill|taskkill\b[^|;&]*/f)\b|(^|\s)>\s*/dev/sd|\bdiskpart\b"),
        ("global-install", r"(?i)\b(npm|pnpm)\s+(i|install|add)\b[^|;&]*\s(-g|--global)\b|\byarn\s+global\s+add\b|\bbun\s+(add|install)\b[^|;&]*\s(-g|--global)\b|\bcargo\s+install\b|\bgo\s+install\b|\b(apt|apt-get|yum|dnf|pacman|brew|choco|winget|scoop)\s+(install|remove|uninstall)\b"),
    ]
    .into_iter()
    .map(|(k, r)| (k, Regex::new(r).unwrap()))
    .collect()
});

fn env_in_bash(c: &str) -> bool {
    ENV_BASH.captures_iter(c).any(|m| match m.get(3) {
        None => true,
        Some(s) => !ENV_SAFE.contains(&s.as_str().to_lowercase().as_str()),
    })
}

fn texts(req: &PermissionRequest) -> Vec<String> {
    let mut v = req.patterns.clone();
    for k in ["command", "filepath", "filePath", "path"] {
        match req.metadata.get(k) {
            Some(serde_json::Value::String(s)) => v.push(s.clone()),
            Some(serde_json::Value::Array(a)) => v.extend(a.iter().filter_map(|x| x.as_str().map(String::from))),
            _ => {}
        }
    }
    v
}

/// Alasan risiko (kunci `chat.risk.<alasan>`) atau None bila aman disetujui otomatis di "auto-safe".
pub fn permission_risk(req: &PermissionRequest) -> Option<&'static str> {
    match req.permission.as_str() {
        "external_directory" => Some("outside"),
        "doom_loop" => Some("loop"),
        "edit" | "write" | "read" | "list" | "glob" | "grep" => texts(req).iter().any(|p| is_secret_path(p)).then_some("secret"),
        "webfetch" | "websearch" | "codesearch" | "todowrite" | "todoread" | "task" | "skill" | "lsp" => None,
        "bash" => {
            let cmds = texts(req);
            if cmds.iter().any(|c| env_in_bash(c)) {
                return Some("secret");
            }
            BASH_RULES.iter().find(|(_, re)| cmds.iter().any(|c| re.is_match(c))).map(|(k, _)| *k)
        }
        _ => Some("unknown"),
    }
}

/// Apakah permintaan dijawab otomatis menurut mode.
pub fn auto_allowed(mode: &str, req: &PermissionRequest) -> bool {
    match mode {
        "auto-all" => true,
        "auto-edit" => req.permission == "edit",
        "auto-safe" => permission_risk(req).is_none(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(perm: &str, pats: &[&str]) -> PermissionRequest {
        PermissionRequest {
            id: "p".into(),
            session_id: "s".into(),
            permission: perm.into(),
            patterns: pats.iter().map(|s| s.to_string()).collect(),
            metadata: Default::default(),
            always: vec![],
            tool: None,
        }
    }

    #[test]
    fn tokens() {
        assert_eq!(active_token("/com", 4).unwrap().kind, TokenKind::Slash);
        assert!(active_token("/com x", 6).is_none());
        let m = active_token("lihat @src/ma", 13).unwrap();
        assert_eq!((m.kind, m.query.as_str(), m.start), (TokenKind::Mention, "src/ma", 6));
        assert!(active_token("a@b", 3).is_none());
        let (t, c) = replace_token("lihat @sr", &m_tok(), "@src/main.rs");
        assert_eq!(t, "lihat @src/main.rs ");
        assert_eq!(c, t.len());
    }
    fn m_tok() -> ComposerToken {
        active_token("lihat @sr", 9).unwrap()
    }

    #[test]
    fn slash() {
        assert_eq!(parse_slash("/mode auto-all").unwrap(), ParsedSlash { name: "mode".into(), args: "auto-all".into() });
        assert!(parse_slash("//x").is_none());
        assert!(parse_slash("halo").is_none());
        let r = rank_by_query(&["new", "undo", "fork"], "n", |s| s.to_string());
        assert_eq!(r, vec!["new", "undo"]);
    }

    #[test]
    fn mention() {
        let a = mention_attachment("C:\\p", "src/a.png");
        assert_eq!(a.mime, "image/png");
        assert_eq!(a.url, "file:///C:/p/src/a.png");
    }

    #[test]
    fn risk() {
        assert_eq!(permission_risk(&req("read", &[".env"])), Some("secret"));
        assert_eq!(permission_risk(&req("read", &[".env.example"])), None);
        assert_eq!(permission_risk(&req("bash", &["git push origin"])), Some("publish"));
        assert_eq!(permission_risk(&req("bash", &["rm -rf build"])), Some("delete"));
        assert_eq!(permission_risk(&req("bash", &["cat .env.local"])), Some("secret"));
        assert_eq!(permission_risk(&req("bash", &["cp .env.example .env.sample"])), None);
        assert_eq!(permission_risk(&req("bash", &["cargo test"])), None);
        assert_eq!(permission_risk(&req("weird", &[])), Some("unknown"));
        assert!(auto_allowed("auto-safe", &req("edit", &["a.rs"])));
        assert!(!auto_allowed("ask", &req("edit", &["a.rs"])));
    }
}

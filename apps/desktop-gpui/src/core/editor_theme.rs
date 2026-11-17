//! Tema editor — port `code-colors.ts` + `vscode-theme.ts`: palet bawaan Monaco (vs, vs-dark, hc-black, hc-light),
//! konversi tema VS Code (JSON/JSONC, termasuk `include` di dalam paket .vsix), dan penurunan warna sintaks
//! (`code_*`) dari `tokenColors`. Tidak ada kode ekstensi yang dijalankan: hanya berkas JSON tema yang dibaca.
use crate::core::appearance::{is_color, parse_color};
use crate::theme::Colors;
use serde_json::{json, Map, Value};
use std::io::Read;

pub const BUILTIN_EDITOR_THEMES: [&str; 4] = ["vs", "vs-dark", "hc-black", "hc-light"];
pub const MAX_THEME_BYTES: u64 = 400_000;
const MAX_VSIX_BYTES: u64 = 20_000_000;
const MAX_COLORS: usize = 1000;
const MAX_RULES: usize = 2000;
pub const MAX_EDITOR_THEMES: usize = 30;

pub const CODE_COLOR_KEYS: [&str; 11] = ["keyword", "string", "number", "comment", "function", "type", "attr", "add", "del", "bg", "fg"];

const SCOPES: [(&str, &[&str]); 9] = [
    ("keyword", &["keyword.control", "keyword", "storage.type", "storage"]),
    ("string", &["string.quoted", "string"]),
    ("number", &["constant.numeric", "constant.language", "constant"]),
    ("comment", &["comment.line", "comment"]),
    ("function", &["entity.name.function", "support.function"]),
    ("type", &["entity.name.type", "entity.name.class", "support.type", "support.class"]),
    ("attr", &["entity.other.attribute-name", "support.type.property-name", "variable.other.property", "variable"]),
    ("add", &["markup.inserted"]),
    ("del", &["markup.deleted"]),
];

fn builtin(id: &str) -> Option<[&'static str; 11]> {
    Some(match id {
        "vs-dark" => ["#569cd6", "#ce9178", "#b5cea8", "#6a9955", "#dcdcaa", "#4ec9b0", "#9cdcfe", "#b5cea8", "#ce9178", "#1e1e1e", "#d4d4d4"],
        "vs" => ["#0000ff", "#a31515", "#098658", "#008000", "#795e26", "#267f99", "#e50000", "#098658", "#a31515", "#fffffe", "#000000"],
        "hc-black" => ["#569cd6", "#ce9178", "#b5cea8", "#7ca668", "#dcdcaa", "#4ec9b0", "#9cdcfe", "#b5cea8", "#ce9178", "#000000", "#ffffff"],
        "hc-light" => ["#0f4a85", "#264f78", "#096d48", "#515151", "#5e2cbc", "#185e73", "#264f78", "#096d48", "#b5200d", "#ffffff", "#292929"],
        _ => return None,
    })
}

/// Tema editor terpasang (bentuk tersimpan sama dengan aplikasi TS).
#[derive(Clone, Debug, PartialEq)]
pub struct EditorTheme {
    pub id: String,
    pub name: String,
    pub base: String,
    pub colors: Map<String, Value>,
    pub rules: Vec<Value>,
    /// `[{ scope?: [..], settings: { foreground?, background?, fontStyle? } }]`
    pub token_colors: Vec<Value>,
    pub source: String,
}

impl EditorTheme {
    pub fn to_json(&self) -> Value {
        json!({ "id": self.id, "name": self.name, "base": self.base, "colors": self.colors, "rules": self.rules, "tokenColors": self.token_colors, "source": self.source })
    }
}

/// Port stripJsonc: buang komentar dan koma gantung, string dipertahankan.
pub fn strip_jsonc(text: &str) -> String {
    let b: Vec<char> = text.chars().collect();
    let n = b.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < n {
        let c = b[i];
        if c == '"' {
            let mut j = i + 1;
            while j < n && b[j] != '"' {
                j += if b[j] == '\\' { 2 } else { 1 };
            }
            out.extend(&b[i..(j + 1).min(n)]);
            i = j + 1;
        } else if c == '/' && b.get(i + 1) == Some(&'/') {
            while i < n && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && b.get(i + 1) == Some(&'*') {
            let mut j = i + 2;
            while j + 1 < n && !(b[j] == '*' && b[j + 1] == '/') {
                j += 1;
            }
            i = if j + 1 < n { j + 2 } else { n };
        } else {
            out.push(c);
            i += 1;
        }
    }
    // ,(\s*[}\]]) → $1 (di luar string; cukup aman karena string sudah utuh dan jarang memuat pola ini)
    let re = regex::Regex::new(r",(\s*[}\]])").expect("regex");
    re.replace_all(&out, "$1").into_owned()
}

fn parse_object(text: &str) -> Result<Map<String, Value>, String> {
    let text = text.trim_start_matches('\u{feff}');
    match serde_json::from_str::<Value>(&strip_jsonc(text)) {
        Ok(Value::Object(o)) => Ok(o),
        Ok(_) => Err("tema harus berupa objek JSON".into()),
        Err(e) => Err(format!("JSON tidak valid: {e}")),
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(40).collect()
}

fn hex_no_hash(v: Option<&Value>) -> Option<String> {
    let s = v?.as_str()?;
    is_color(s).then(|| s.trim_start_matches('#').to_lowercase())
}

fn base_for(ty: Option<&Value>, ui: Option<&str>) -> String {
    let t = ty.and_then(Value::as_str).unwrap_or("");
    match (ui.unwrap_or(""), t) {
        ("hc-black", _) | (_, "hc") | (_, "hcDark") => "hc-black",
        ("hc-light", _) | (_, "hcLight") => "hc-light",
        ("vs", _) | (_, "light") => "vs",
        _ => "vs-dark",
    }
    .into()
}

fn with_includes(o: &Map<String, Value>, resolve: &dyn Fn(&str) -> Option<String>, depth: u32, errors: &mut Vec<String>) -> (Map<String, Value>, Vec<Value>) {
    let mut colors = Map::new();
    let mut tc = vec![];
    if let Some(inc) = o.get("include").and_then(Value::as_str) {
        let parent = if depth < 5 { resolve(inc).and_then(|t| parse_object(&t).ok()) } else { None };
        match parent {
            Some(p) => {
                let (c, t) = with_includes(&p, resolve, depth + 1, errors);
                colors = c;
                tc = t;
            }
            None => errors.push(format!("include \"{}\" tidak bisa dibaca", inc.chars().take(80).collect::<String>())),
        }
    }
    if let Some(c) = o.get("colors").and_then(Value::as_object) {
        colors.extend(c.clone());
    }
    match o.get("tokenColors") {
        Some(Value::Array(a)) => tc.extend(a.iter().cloned()),
        Some(Value::String(_)) => errors.push("tokenColors berformat .tmTheme belum didukung".into()),
        _ => {}
    }
    (colors, tc)
}

fn scope_ok(s: &str) -> bool {
    let n = s.chars().count();
    (1..=300).contains(&n) && !s.chars().any(|c| (c as u32) < 0x20)
}

fn scope_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(String::from).collect(),
        Some(Value::String(s)) => s.split(',').map(String::from).collect(),
        _ => vec![],
    }
}

/// Port convertVsCodeTheme.
pub fn convert_vscode_theme(text: &str, name: Option<&str>, ui_theme: Option<&str>, source: &str, resolve: &dyn Fn(&str) -> Option<String>) -> (Option<EditorTheme>, Vec<String>) {
    let o = match parse_object(text) {
        Ok(o) => o,
        Err(e) => return (None, vec![e]),
    };
    let mut errors = vec![];
    let (mcolors, mtc) = with_includes(&o, resolve, 0, &mut errors);
    let name: String = name.map(String::from).or_else(|| o.get("name").and_then(Value::as_str).map(String::from)).unwrap_or_default().trim().chars().take(40).collect();
    if name.is_empty() {
        return (None, vec!["tema tidak punya nama".into()]);
    }
    let id = slug(&name);
    if id.is_empty() {
        return (None, vec!["nama tema tidak valid".into()]);
    }
    let key_re = regex::Regex::new(r"^[\w.-]{1,80}$").expect("regex");
    let tok_re = regex::Regex::new(r"^[\w.+-]{1,100}$").expect("regex");
    let fs_re = regex::Regex::new(r"^[a-z ]{0,40}$").expect("regex");
    let mut colors = Map::new();
    for (k, v) in &mcolors {
        if colors.len() >= MAX_COLORS {
            break;
        }
        if key_re.is_match(k) && v.as_str().is_some_and(is_color) {
            colors.insert(k.clone(), v.clone());
        }
    }
    let mut rules = vec![];
    let mut tm = vec![];
    for tc in &mtc {
        let st = tc.get("settings").cloned().unwrap_or(json!({}));
        let fg = hex_no_hash(st.get("foreground"));
        let bg = hex_no_hash(st.get("background"));
        let fs = st.get("fontStyle").and_then(Value::as_str).filter(|s| fs_re.is_match(s)).map(String::from);
        if fg.is_none() && bg.is_none() && fs.is_none() {
            continue;
        }
        let mut settings = Map::new();
        if let Some(f) = &fg {
            settings.insert("foreground".into(), json!(format!("#{f}")));
        }
        if let Some(b) = &bg {
            settings.insert("background".into(), json!(format!("#{b}")));
        }
        if let Some(f) = &fs {
            settings.insert("fontStyle".into(), json!(f));
        }
        let sc = tc.get("scope");
        let has_scope = !matches!(sc, None | Some(Value::Null)) && sc != Some(&json!(""));
        if tm.len() < MAX_RULES {
            let list: Vec<String> = scope_list(sc).into_iter().map(|x| x.trim().to_string()).filter(|x| scope_ok(x)).take(200).collect();
            if !has_scope {
                tm.push(json!({ "settings": settings }));
            } else if !list.is_empty() {
                tm.push(json!({ "scope": list, "settings": settings }));
            }
        }
        let scopes = if has_scope { scope_list(sc) } else { vec![String::new()] };
        for s in scopes {
            if rules.len() >= MAX_RULES {
                break;
            }
            let token = s.split_whitespace().last().unwrap_or("").to_string();
            if !token.is_empty() && !tok_re.is_match(&token) {
                continue;
            }
            let mut r = Map::new();
            r.insert("token".into(), json!(token));
            if let Some(f) = &fg {
                r.insert("foreground".into(), json!(f));
            }
            if let Some(b) = &bg {
                r.insert("background".into(), json!(b));
            }
            if let Some(f) = &fs {
                r.insert("fontStyle".into(), json!(f));
            }
            rules.push(Value::Object(r));
        }
    }
    if colors.is_empty() && rules.is_empty() {
        let mut e = vec!["tema tidak memuat warna atau aturan token yang valid".to_string()];
        e.extend(errors);
        return (None, e);
    }
    let theme = EditorTheme { id, name, base: base_for(o.get("type"), ui_theme), colors, rules, token_colors: tm, source: source.chars().take(200).collect() };
    (Some(theme), errors)
}

/// Validasi ulang tema tersimpan (data di disk tidak dipercaya).
pub fn sanitize_editor_theme(v: &Value) -> Option<EditorTheme> {
    let o = v.as_object()?;
    let id = o.get("id")?.as_str()?;
    let name = o.get("name")?.as_str()?;
    if slug(id) != id || id.is_empty() || name.trim().is_empty() || name.chars().count() > 40 {
        return None;
    }
    let base = o.get("base").and_then(Value::as_str).filter(|b| BUILTIN_EDITOR_THEMES.contains(b))?;
    let colors: Map<String, Value> = o
        .get("colors")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter(|(_, v)| v.as_str().is_some_and(is_color)).take(MAX_COLORS).map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let rules: Vec<Value> = o.get("rules").and_then(Value::as_array).map(|a| a.iter().filter(|r| r.get("token").is_some_and(Value::is_string)).take(MAX_RULES).cloned().collect()).unwrap_or_default();
    let tc: Vec<Value> = o
        .get("tokenColors")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|r| r.get("settings").is_some_and(Value::is_object)).take(MAX_RULES).cloned().collect())
        .unwrap_or_default();
    Some(EditorTheme {
        id: id.into(),
        name: name.trim().into(),
        base: base.into(),
        colors,
        rules,
        token_colors: tc,
        source: o.get("source").and_then(Value::as_str).unwrap_or("berkas").chars().take(200).collect(),
    })
}

/// Port scopeColor: warna dari aturan paling spesifik (selektor sederhana; aturan belakangan menang).
pub fn scope_color(rules: &[Value], scope: &str) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for r in rules {
        let Some(fg) = r.pointer("/settings/foreground").and_then(Value::as_str).filter(|f| is_color(f)) else { continue };
        let Some(sc) = r.get("scope").and_then(Value::as_array) else { continue };
        for s in sc.iter().filter_map(Value::as_str) {
            if s.contains(' ') {
                continue;
            }
            if scope == s || scope.starts_with(&format!("{s}.")) {
                if best.as_ref().is_none_or(|(l, _)| s.len() >= *l) {
                    best = Some((s.len(), fg.to_string()));
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// Port codeColorsFromTheme.
pub fn code_colors_from_theme(t: &EditorTheme) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = vec![];
    for (key, scopes) in SCOPES {
        if let Some(c) = scopes.iter().find_map(|s| scope_color(&t.token_colors, s)) {
            out.push((key, c));
        }
    }
    let col = |k: &str| t.colors.get(k).and_then(Value::as_str).map(String::from);
    for (key, ck) in [("add", "gitDecoration.addedResourceForeground"), ("del", "gitDecoration.deletedResourceForeground")] {
        if !out.iter().any(|(k, _)| *k == key) {
            if let Some(c) = col(ck) {
                out.push((key, c));
            }
        }
    }
    if let Some(bg) = col("editor.background") {
        out.push(("bg", bg));
    }
    let fg = col("editor.foreground").or_else(|| t.token_colors.iter().find(|r| r.get("scope").is_none()).and_then(|r| r.pointer("/settings/foreground")).and_then(Value::as_str).map(String::from));
    if let Some(fg) = fg {
        out.push(("fg", fg));
    }
    out.retain(|(_, v)| is_color(v));
    out
}

/// Port codeColorsFor: "auto"/tak dikenal → kosong (token desain).
pub fn code_colors_for(selected: &str, installed: &[EditorTheme]) -> Vec<(&'static str, String)> {
    if let Some(p) = builtin(selected) {
        return CODE_COLOR_KEYS.iter().zip(p).map(|(k, v)| (*k, v.to_string())).collect();
    }
    installed.iter().find(|t| t.id == selected).map(code_colors_from_theme).unwrap_or_default()
}

/// Terapkan warna kode ke palet (termasuk latar/teks blok kode).
pub fn apply_code_colors(c: &mut Colors, list: &[(&'static str, String)]) {
    for (k, v) in list {
        let Some(col) = parse_color(v) else { continue };
        match *k {
            "bg" => {
                c.code_bg = Some(col);
                continue;
            }
            "fg" => {
                c.code_fg = Some(col);
                continue;
            }
            _ => {}
        }
        let slot = match *k {
            "keyword" => &mut c.code_keyword,
            "string" => &mut c.code_string,
            "number" => &mut c.code_number,
            "comment" => &mut c.code_comment,
            "function" => &mut c.code_function,
            "type" => &mut c.code_type,
            "attr" => &mut c.code_attr,
            "add" => &mut c.code_add,
            "del" => &mut c.code_del,
            _ => continue,
        };
        *slot = col;
    }
}

fn read_limited(r: impl Read, max: u64) -> Option<String> {
    let mut s = String::new();
    r.take(max + 1).read_to_string(&mut s).ok()?;
    (s.len() as u64 <= max).then_some(s)
}

fn norm(path: &str) -> String {
    let mut parts: Vec<&str> = vec![];
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            x => parts.push(x),
        }
    }
    parts.join("/")
}

/// Port themesFromVsix: baca `extension/package.json` → `contributes.themes`, konversi tiap berkas tema.
pub fn themes_from_vsix(bytes: &[u8], source: &str) -> (Vec<EditorTheme>, Vec<String>) {
    let mut errors = vec![];
    if bytes.len() as u64 > MAX_VSIX_BYTES {
        return (vec![], vec!["paket terlalu besar".into()]);
    }
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else { return (vec![], vec!["bukan paket .vsix yang valid".into()]) };
    let mut read = |p: &str| -> Option<String> {
        let f = zip.by_name(&format!("extension/{}", norm(p))).ok()?;
        read_limited(f, MAX_THEME_BYTES)
    };
    let Some(pkg) = read("package.json").and_then(|t| parse_object(&t).ok()) else { return (vec![], vec!["package.json tidak ditemukan".into()]) };
    let list = pkg.get("contributes").and_then(|c| c.get("themes")).and_then(Value::as_array).cloned().unwrap_or_default();
    if list.is_empty() {
        return (vec![], vec!["paket tidak berisi tema".into()]);
    }
    // Baca semua berkas yang mungkin di-include lebih dulu agar resolver tidak meminjam zip.
    let mut files: std::collections::HashMap<String, Option<String>> = Default::default();
    let mut out = vec![];
    for th in list.iter().take(20) {
        let Some(path) = th.get("path").and_then(Value::as_str) else { continue };
        let p = norm(path);
        let Some(text) = read(&p) else {
            errors.push(format!("{p}: tidak bisa dibaca"));
            continue;
        };
        let dir = p.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        // kumpulkan rantai include (maks 5)
        let mut cur = text.clone();
        let mut cdir = dir.clone();
        for _ in 0..5 {
            let Some(inc) = parse_object(&cur).ok().and_then(|o| o.get("include").and_then(Value::as_str).map(String::from)) else { break };
            let ip = norm(&format!("{cdir}/{inc}"));
            let t = read(&ip);
            files.insert(inc.clone(), t.clone());
            let Some(t) = t else { break };
            cur = t;
            cdir = ip.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        }
        let resolver = |inc: &str| files.get(inc).cloned().flatten();
        let (t, e) = convert_vscode_theme(&text, th.get("label").and_then(Value::as_str), th.get("uiTheme").and_then(Value::as_str), source, &resolver);
        errors.extend(e.into_iter().map(|e| format!("{p}: {e}")));
        out.extend(t);
    }
    (out, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonc() {
        assert_eq!(strip_jsonc("{\"a\": \"//x\", // c\n /* b */ \"b\": [1,],}"), "{\"a\": \"//x\", \n  \"b\": [1]}");
    }

    #[test]
    fn convert_and_colors() {
        let src = r##"{ "name": "Uji Gelap", "type": "dark", "colors": {"editor.background": "#101010"},
          "tokenColors": [ {"settings": {"foreground": "#eeeeee"}},
            {"scope": "keyword", "settings": {"foreground": "#ff0000"}},
            {"scope": ["keyword.control", "meta.tag string"], "settings": {"foreground": "#00ff00"}},
            {"scope": "string", "settings": {"foreground": "#0000ff"}} ] }"##;
        let (t, e) = convert_vscode_theme(src, None, None, "berkas", &|_| None);
        assert!(e.is_empty(), "{e:?}");
        let t = t.unwrap();
        assert_eq!(t.id, "uji-gelap");
        assert_eq!(t.base, "vs-dark");
        let c = code_colors_from_theme(&t);
        assert!(c.contains(&("keyword", "#00ff00".into())));
        assert!(c.contains(&("string", "#0000ff".into())));
        assert!(c.contains(&("bg", "#101010".into())));
        assert!(c.contains(&("fg", "#eeeeee".into())));
        assert_eq!(sanitize_editor_theme(&t.to_json()), Some(t));
    }

    #[test]
    fn builtin_and_auto() {
        assert_eq!(code_colors_for("vs-dark", &[])[0], ("keyword", "#569cd6".into()));
        assert!(code_colors_for("auto", &[]).is_empty());
    }

    #[test]
    fn include() {
        let parent = r##"{"tokenColors": [{"scope": "comment", "settings": {"foreground": "#123456"}}]}"##.to_string();
        let (t, _) = convert_vscode_theme(r#"{"name": "A", "include": "./p.json"}"#, None, Some("vs"), "x", &|_| Some(parent.clone()));
        let t = t.unwrap();
        assert_eq!(t.base, "vs");
        assert!(code_colors_from_theme(&t).contains(&("comment", "#123456".into())));
    }
}

//! Pewarnaan sintaks ringan untuk blok kode chat dan penampil kode Panel 3 — pengganti lowlight/highlight.js versi React.
//! Lexer per baris (status komentar blok/string multi-baris dibawa antar baris) per keluarga bahasa: C-like, skrip (#),
//! SQL, markup (HTML/XML/SVG), JSON, CSS. Warna dari token tema `code_*`. Perbedaan jujur: bukan parser penuh
//! (tidak ada grammar per bahasa seperti highlight.js), jadi kasus tepi bisa berbeda warna.
use crate::theme::Theme;
use gpui::{HighlightStyle, Hsla};
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Family {
    CLike,
    Script,
    Sql,
    Markup,
    Json,
    Css,
    Plain,
}

fn family(lang: &str) -> Family {
    match lang.to_lowercase().as_str() {
        "js" | "javascript" | "jsx" | "ts" | "typescript" | "tsx" | "mjs" | "cjs" | "rs" | "rust" | "go" | "java" | "kotlin" | "kt" | "c" | "h" | "cpp" | "c++" | "cc" | "hpp" | "cs" | "csharp" | "php" | "swift" | "dart" | "scala" | "zig" => Family::CLike,
        "py" | "python" | "sh" | "bash" | "shell" | "zsh" | "ps1" | "powershell" | "rb" | "ruby" | "yaml" | "yml" | "toml" | "dockerfile" | "makefile" | "r" | "pl" | "perl" | "ini" | "conf" | "env" => Family::Script,
        "sql" => Family::Sql,
        "html" | "htm" | "xml" | "svg" | "vue" | "xaml" | "plist" => Family::Markup,
        "json" | "jsonc" | "json5" => Family::Json,
        "css" | "scss" | "less" => Family::Css,
        _ => Family::Plain,
    }
}

const KW_CLIKE: &[&str] = &[
    "abstract", "as", "async", "await", "break", "case", "catch", "class", "const", "continue", "crate", "default", "defer", "delete", "do", "dyn", "else", "enum", "export", "extends", "extern", "false", "final", "finally", "fn", "for", "from", "func", "function", "go", "goto", "if", "impl", "implements", "import", "in", "instanceof", "interface", "let", "loop", "match", "mod", "move", "mut", "namespace", "new", "nil", "null", "override", "package", "private", "protected", "pub", "public", "readonly", "ref", "return", "self", "Self", "static", "struct", "super", "switch", "this", "throw", "throws", "trait", "true", "try", "type", "typeof", "undefined", "unsafe", "use", "using", "val", "var", "void", "where", "while", "yield", "chan", "select", "range", "map", "virtual", "template", "typename", "sizeof", "inline", "include", "define", "echo",
];
const KW_SCRIPT: &[&str] = &[
    "and", "as", "assert", "async", "await", "begin", "break", "case", "class", "continue", "def", "del", "do", "done", "elif", "else", "elsif", "end", "esac", "except", "export", "false", "fi", "finally", "for", "foreach", "from", "function", "global", "if", "import", "in", "is", "lambda", "local", "module", "next", "nil", "None", "nonlocal", "not", "or", "param", "pass", "raise", "require", "rescue", "return", "self", "then", "True", "False", "true", "try", "unless", "until", "when", "while", "with", "yield", "FROM", "RUN", "CMD", "COPY", "ADD", "ENV", "ARG", "WORKDIR", "EXPOSE", "ENTRYPOINT", "USER", "VOLUME", "LABEL", "null",
];
const KW_SQL: &[&str] = &[
    "select", "from", "where", "and", "or", "not", "insert", "into", "values", "update", "set", "delete", "create", "table", "drop", "alter", "add", "index", "primary", "key", "foreign", "references", "join", "left", "right", "inner", "outer", "on", "as", "group", "by", "order", "having", "limit", "offset", "distinct", "union", "all", "null", "is", "in", "like", "between", "case", "when", "then", "else", "end", "exists", "default", "unique", "view", "with", "returning", "asc", "desc", "count", "sum", "avg", "min", "max", "true", "false",
];

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Carry {
    #[default]
    None,
    /// Di dalam komentar blok (/* */ atau <!-- -->).
    Block,
    /// Di dalam string multi-baris (``` ` ``` atau """).
    Str(char, bool),
}

fn style(c: Hsla) -> HighlightStyle {
    HighlightStyle { color: Some(c), ..Default::default() }
}

/// Akhiran scope TextMate per bahasa (`keyword.control.rust`, `string.quoted.double.python`, …).
fn lang_scope(lang: &str) -> String {
    match lang.to_lowercase().as_str() {
        "js" | "javascript" | "mjs" | "cjs" | "jsx" => "js".into(),
        "ts" | "typescript" => "ts".into(),
        "tsx" => "tsx".into(),
        "rs" | "rust" => "rust".into(),
        "py" | "python" => "python".into(),
        "sh" | "bash" | "shell" | "zsh" => "shell".into(),
        "ps1" | "powershell" => "powershell".into(),
        "kt" | "kotlin" => "kotlin".into(),
        "c" | "h" => "c".into(),
        "cpp" | "c++" | "cc" | "hpp" => "cpp".into(),
        "cs" | "csharp" => "cs".into(),
        "rb" | "ruby" => "ruby".into(),
        "yaml" | "yml" => "yaml".into(),
        "html" | "htm" | "vue" | "xaml" => "html".into(),
        "xml" | "svg" | "plist" => "xml".into(),
        "json" | "jsonc" | "json5" => "json".into(),
        "" => "txt".into(),
        other => other.to_string(),
    }
}

/// Pemberi warna: penyelesai scope tema editor (bila aktif) atau palet `code_*` lama.
struct Pen {
    res: Option<std::sync::Arc<crate::core::editor_theme::Resolver>>,
    sfx: String,
}

impl Pen {
    fn paint(&self, scope: &str, legacy: Option<Hsla>) -> Option<HighlightStyle> {
        match &self.res {
            Some(r) => {
                let st = r.style(&[format!("source.{}", self.sfx), format!("{scope}.{}", self.sfx)])?;
                Some(HighlightStyle {
                    color: st.color,
                    font_style: st.italic.then_some(gpui::FontStyle::Italic),
                    font_weight: st.bold.then_some(gpui::FontWeight::BOLD),
                    underline: st.underline.then(|| gpui::UnderlineStyle { thickness: gpui::px(1.), color: None, wavy: false }),
                    ..Default::default()
                })
            }
            None => legacy.map(style),
        }
    }
    fn put(&self, out: &mut Vec<(Range<usize>, HighlightStyle)>, r: Range<usize>, scope: &str, legacy: Option<Hsla>) {
        if r.start < r.end {
            if let Some(s) = self.paint(scope, legacy) {
                out.push((r, s));
            }
        }
    }
}

/// Scope kata kunci menurut jenisnya (kontrol alur, penyimpanan, konstanta bahasa, dst.).
fn kw_scope(w: &str, fam: Family) -> &'static str {
    match (fam, w) {
        (Family::Sql, _) => "keyword.other",
        (_, "true" | "false" | "null" | "nil" | "None" | "True" | "False" | "undefined") => "constant.language",
        (_, "this" | "self" | "Self" | "super") => "variable.language",
        (_, "fn" | "function" | "def" | "func") => "storage.type.function",
        (_, "let" | "const" | "var" | "val" | "class" | "struct" | "enum" | "trait" | "interface" | "impl" | "type" | "namespace" | "module") => "storage.type",
        (_, "mut" | "static" | "pub" | "private" | "public" | "protected" | "abstract" | "readonly" | "final" | "extern" | "async" | "unsafe" | "override" | "virtual" | "inline" | "ref" | "dyn" | "move") => "storage.modifier",
        (_, "new") => "keyword.operator.new",
        (_, "delete") => "keyword.operator.delete",
        (_, "typeof") => "keyword.operator.expression.typeof",
        (_, "instanceof") => "keyword.operator.expression.instanceof",
        (_, "in" | "of") => "keyword.operator.expression.in",
        _ => "keyword.control",
    }
}

fn op_scope(run: &str) -> &'static str {
    match run {
        "=>" | "->" => "storage.type.function.arrow",
        "==" | "===" | "!=" | "!==" | "<" | ">" | "<=" | ">=" => "keyword.operator.comparison",
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "|=" | "&=" | "^=" | "<<=" | ">>=" => "keyword.operator.assignment",
        r if r.contains('&') || r.contains('|') || r == "!" => "keyword.operator.logical",
        "?" => "keyword.operator.ternary",
        _ => "keyword.operator.arithmetic",
    }
}

fn is_config(lang: &str) -> bool {
    lang.starts_with('y') || matches!(lang, "toml" | "ini" | "conf" | "env" | "dockerfile" | "makefile")
}

/// Warnai satu baris; `carry` = status dari baris sebelumnya (diperbarui untuk baris berikutnya).
pub fn highlight_line(lang: &str, line: &str, carry: &mut Carry, t: &Theme) -> Vec<(Range<usize>, HighlightStyle)> {
    highlight_line_with(crate::core::editor_theme::active(), lang, line, carry, t)
}

fn highlight_line_with(res: Option<std::sync::Arc<crate::core::editor_theme::Resolver>>, lang: &str, line: &str, carry: &mut Carry, t: &Theme) -> Vec<(Range<usize>, HighlightStyle)> {
    let fam = family(lang);
    if fam == Family::Plain {
        return vec![];
    }
    let c = &t.colors;
    let pen = Pen { res, sfx: lang_scope(lang) };
    let lang_l = lang.to_lowercase();
    let mut out: Vec<(Range<usize>, HighlightStyle)> = vec![];
    let b = line.as_bytes();
    let n = b.len();
    let mut i = 0;
    let (block_open, block_close): (&str, &str) = if fam == Family::Markup { ("<!--", "-->") } else { ("/*", "*/") };
    // lanjutan dari baris sebelumnya
    match *carry {
        Carry::Block => match line.find(block_close) {
            Some(k) => {
                pen.put(&mut out, 0..k + block_close.len(), "comment.block", Some(c.code_comment));
                i = k + block_close.len();
                *carry = Carry::None;
            }
            None => {
                pen.put(&mut out, 0..n, "comment.block", Some(c.code_comment));
                return out;
            }
        },
        Carry::Str(q, triple) => {
            let close = if triple { format!("{q}{q}{q}") } else { q.to_string() };
            match line.find(&close) {
                Some(k) => {
                    pen.put(&mut out, 0..k + close.len(), "string.quoted.double", Some(c.code_string));
                    i = k + close.len();
                    *carry = Carry::None;
                }
                None => {
                    pen.put(&mut out, 0..n, "string.quoted.double", Some(c.code_string));
                    return out;
                }
            }
        }
        Carry::None => {}
    }
    let line_comment: &[&str] = match fam {
        Family::CLike => &["//"],
        Family::Script => &["#"],
        Family::Sql => &["--"],
        Family::Json => &["//"],
        _ => &[],
    };
    let mut in_tag = false;
    // CSS: apakah posisi sekarang di sisi nilai deklarasi (setelah `prop:`)
    let selector_line = line.trim_end().ends_with('{');
    let mut css_value = false;
    while i < n {
        let rest = &line[i..];
        let ch = rest.chars().next().unwrap_or(' ');
        // komentar baris
        if line_comment.iter().any(|p| rest.starts_with(p)) && !(fam == Family::Script && lang.eq_ignore_ascii_case("css")) {
            pen.put(&mut out, i..n, "comment.line", Some(c.code_comment));
            break;
        }
        // komentar blok
        if (fam != Family::Script && fam != Family::Sql && rest.starts_with(block_open)) || (fam == Family::Sql && rest.starts_with("/*")) {
            match rest[block_open.len()..].find(block_close) {
                Some(k) => {
                    let end = i + block_open.len() + k + block_close.len();
                    pen.put(&mut out, i..end, "comment.block", Some(c.code_comment));
                    i = end;
                    continue;
                }
                None => {
                    pen.put(&mut out, i..n, "comment.block", Some(c.code_comment));
                    *carry = Carry::Block;
                    break;
                }
            }
        }
        // markup: tag dan atribut
        if fam == Family::Markup {
            if ch == '<' {
                let mut j = i + 1;
                if j < n && (b[j] == b'/' || b[j] == b'!' || b[j] == b'?') {
                    j += 1;
                }
                let name_start = j;
                while j < n && (b[j].is_ascii_alphanumeric() || b"-_:.".contains(&b[j])) {
                    j += 1;
                }
                pen.put(&mut out, i..name_start, "punctuation.definition.tag", Some(c.code_keyword));
                pen.put(&mut out, name_start..j, "entity.name.tag", Some(c.code_keyword));
                in_tag = true;
                i = j;
                continue;
            }
            if in_tag && (ch == '>' || rest.starts_with("/>")) {
                let l = if ch == '>' { 1 } else { 2 };
                pen.put(&mut out, i..i + l, "punctuation.definition.tag", Some(c.code_keyword));
                in_tag = false;
                i += l;
                continue;
            }
            if in_tag && (ch.is_ascii_alphabetic() || ch == '_' || ch == '@' || ch == ':') {
                let mut j = i;
                while j < n && (b[j].is_ascii_alphanumeric() || b"-_:.@".contains(&b[j])) {
                    j += 1;
                }
                pen.put(&mut out, i..j, "entity.other.attribute-name", Some(c.code_attr));
                i = j;
                continue;
            }
            if !in_tag && ch != '"' && ch != '\'' {
                i += ch.len_utf8();
                continue;
            }
        }
        // CSS: batas sisi nilai deklarasi
        if fam == Family::Css {
            if ch == ':' && !rest.starts_with("::") && !selector_line && !line[..i].contains('{') {
                css_value = true;
            } else if ch == ';' || ch == '}' {
                css_value = false;
            }
        }
        // string
        if ch == '"' || ch == '\'' || (ch == '`' && fam == Family::CLike) {
            let triple = fam == Family::Script && rest.starts_with(&format!("{ch}{ch}{ch}"));
            let open_len = if triple { 3 } else { 1 };
            let mut j = i + open_len;
            let mut closed = false;
            while j < n {
                if b[j] == b'\\' && fam != Family::Markup {
                    j += 2;
                    continue;
                }
                if triple {
                    if line[j..].starts_with(&format!("{ch}{ch}{ch}")) {
                        j += 3;
                        closed = true;
                        break;
                    }
                } else if line[j..].starts_with(ch) {
                    j += 1;
                    closed = true;
                    break;
                }
                j += line[j..].chars().next().map(|x| x.len_utf8()).unwrap_or(1);
            }
            let j = j.min(n);
            // kunci JSON/YAML (string diikuti ':') = nama properti
            let is_key = (fam == Family::Json || fam == Family::Script && lang.starts_with("y")) && line[j..].trim_start().starts_with(':');
            if is_key {
                let key = if fam == Family::Json { "support.type.property-name" } else { "entity.name.tag" };
                pen.put(&mut out, i..j, key, Some(c.code_attr));
            } else {
                let sc = match ch {
                    '`' => "string.template",
                    '\'' => "string.quoted.single",
                    _ => "string.quoted.double",
                };
                pen.put(&mut out, i..j, sc, Some(c.code_string));
            }
            if !closed && (triple || ch == '`') {
                *carry = Carry::Str(ch, triple);
            }
            i = j;
            continue;
        }
        // angka
        if ch.is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            let mut j = i;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'.' || b[j] == b'_') {
                j += 1;
            }
            pen.put(&mut out, i..j, "constant.numeric", Some(c.code_number));
            i = j;
            continue;
        }
        // kata
        if ch.is_alphabetic() || ch == '_' || ch == '$' || (ch == '@' && fam != Family::Css) {
            let mut j = i + ch.len_utf8();
            while j < n {
                let x = line[j..].chars().next().unwrap_or(' ');
                if x.is_alphanumeric() || x == '_' || x == '$' || (fam == Family::Css && x == '-') {
                    j += x.len_utf8();
                } else {
                    break;
                }
            }
            let w = &line[i..j];
            let next = line[j..].trim_start();
            let after_dot = i > 0 && b[i - 1] == b'.';
            let is_kw = match fam {
                Family::CLike => KW_CLIKE.contains(&w),
                Family::Script => KW_SCRIPT.contains(&w),
                Family::Sql => KW_SQL.contains(&w.to_lowercase().as_str()),
                Family::Json => ["true", "false", "null"].contains(&w),
                _ => false,
            };
            let caps = w.len() > 1 && w.chars().all(|x| x.is_ascii_uppercase() || x.is_ascii_digit() || x == '_');
            let (scope, legacy): (Option<&str>, Option<Hsla>) = if is_kw && !after_dot {
                (Some(if fam == Family::Json { "constant.language" } else { kw_scope(w, fam) }), Some(c.code_keyword))
            } else {
                match fam {
                    Family::Css => {
                        if css_value {
                            (Some("support.constant.property-value"), None)
                        } else if next.starts_with(':') && !next.starts_with("::") && !selector_line {
                            (Some("support.type.property-name"), Some(c.code_attr))
                        } else if i > 0 && b[i - 1] == b'.' {
                            (Some("entity.other.attribute-name.class"), None)
                        } else if i > 0 && b[i - 1] == b'#' {
                            (Some("entity.other.attribute-name.id"), None)
                        } else {
                            (Some("entity.name.tag"), None)
                        }
                    }
                    Family::Json | Family::Markup | Family::Plain => (None, None),
                    _ if next.starts_with('(') => (Some("entity.name.function"), Some(c.code_function)),
                    Family::CLike if w.chars().next().is_some_and(|x| x.is_uppercase()) => (Some(if caps { "variable.other.constant" } else { "entity.name.type" }), Some(c.code_type)),
                    Family::Script if is_config(&lang_l) && next.starts_with(['=', ':']) => (Some(if lang_l.starts_with('y') { "entity.name.tag" } else { "support.type.property-name" }), Some(c.code_attr)),
                    Family::Sql => (None, None),
                    _ if after_dot => (Some("variable.other.property"), None),
                    _ if caps => (Some("variable.other.constant"), None),
                    _ => (Some("variable.other.readwrite"), None),
                }
            };
            if let Some(sc) = scope {
                pen.put(&mut out, i..j, sc, legacy);
            }
            i = j;
            continue;
        }
        // operator (hanya bahasa pemrograman; konfigurasi/markup/JSON/CSS tidak)
        if matches!(fam, Family::CLike | Family::Script | Family::Sql) && !is_config(&lang_l) && "=+-*/%<>!&|^~?".contains(ch) {
            let mut j = i;
            while j < n && j - i < 3 && "=+-*/%<>!&|^~?".as_bytes().contains(&b[j]) {
                j += 1;
            }
            pen.put(&mut out, i..j, op_scope(&line[i..j]), None);
            i = j;
            continue;
        }
        // tanda baca dan spasi: warna dasar
        i += ch.len_utf8();
    }
    out
}

/// Warnai seluruh teks (baris dipisah '\n'): rentang byte terhadap teks utuh.
pub fn highlight_text(lang: &str, text: &str, t: &Theme) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut carry = Carry::None;
    let mut out = vec![];
    let mut off = 0;
    for line in text.split('\n') {
        for (r, s) in highlight_line(lang, line, &mut carry, t) {
            out.push((r.start + off..r.end + off, s));
        }
        off += line.len() + 1;
    }
    out
}

/// Bahasa dari nama berkas (untuk penampil kode Panel 3).
pub fn lang_of_path(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_lowercase();
    if name == "dockerfile" || name.starts_with("dockerfile.") {
        return "dockerfile".into();
    }
    if name == "makefile" {
        return "makefile".into();
    }
    name.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spans(lang: &str, s: &str) -> Vec<String> {
        let t = Theme::new(crate::theme::ThemeMode::Dark, true, 1.0);
        highlight_text(lang, s, &t).into_iter().map(|(r, _)| s[r].to_string()).collect()
    }
    #[test]
    fn warna_dasar() {
        assert_eq!(spans("rs", "fn main() { let x = \"a\"; // k"), vec!["fn", "main", "let", "\"a\"", "// k"]);
        assert_eq!(spans("py", "def f(x): return 42  # c"), vec!["def", "f", "return", "42", "# c"]);
        assert_eq!(spans("json", "{\"k\": true, \"v\": 1}"), vec!["\"k\"", "true", "\"v\"", "1"]);
        assert_eq!(spans("html", "<a href=\"x\">t</a>"), vec!["<", "a", "href", "\"x\"", ">", "</", "a", ">"]);
        assert_eq!(spans("js", "/* a\nb */ x"), vec!["/* a", "b */"]);
        assert_eq!(spans("sql", "SELECT * FROM t"), vec!["SELECT", "FROM"]);
        assert!(spans("txt", "fn x").is_empty());
        assert_eq!(lang_of_path("C:\\a\\b.tsx"), "tsx");
        // rentang selalu di batas karakter (UTF-8)
        let _ = spans("rs", "let é = \"ñ\"; // ü");
    }

    /// Pemetaan warna sama dengan VS Code: scope → aturan tokenColors tema bawaan (Theme Darker).
    #[test]
    fn tema_bawaan_seperti_vscode() {
        use crate::core::editor_theme::{bundled_theme, Resolver};
        let r = std::sync::Arc::new(Resolver::from_theme(bundled_theme()));
        let t = Theme::new(crate::theme::ThemeMode::Dark, true, 1.0);
        let col = |lang: &str, src: &str, word: &str| {
            let mut carry = Carry::None;
            let spans = highlight_line_with(Some(r.clone()), lang, src, &mut carry, &t);
            let k = src.find(word).unwrap();
            spans.into_iter().find(|(rg, _)| rg.start <= k && k < rg.end).and_then(|(_, s)| s.color)
        };
        let hex = |h: &str| crate::core::appearance::parse_color(h);
        assert_eq!(col("rs", "fn main() {}", "fn"), hex("#d55fde"), "storage.type.function");
        assert_eq!(col("rs", "fn main() {}", "main"), hex("#61afef"), "entity.name.function");
        assert_eq!(col("rs", "let x = \"a\";", "\"a\""), hex("#89ca78"), "string");
        assert_eq!(col("py", "# catatan", "#"), hex("#7f848e"), "comment");
    }
}

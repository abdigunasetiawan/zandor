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

/// Warnai satu baris; `carry` = status dari baris sebelumnya (diperbarui untuk baris berikutnya).
pub fn highlight_line(lang: &str, line: &str, carry: &mut Carry, t: &Theme) -> Vec<(Range<usize>, HighlightStyle)> {
    let fam = family(lang);
    if fam == Family::Plain {
        return vec![];
    }
    let c = &t.colors;
    let mut out: Vec<(Range<usize>, HighlightStyle)> = vec![];
    let b = line.as_bytes();
    let n = b.len();
    let mut i = 0;
    let (block_open, block_close): (&str, &str) = if fam == Family::Markup { ("<!--", "-->") } else { ("/*", "*/") };
    // lanjutan dari baris sebelumnya
    match *carry {
        Carry::Block => match line.find(block_close) {
            Some(k) => {
                out.push((0..k + block_close.len(), style(c.code_comment)));
                i = k + block_close.len();
                *carry = Carry::None;
            }
            None => {
                out.push((0..n, style(c.code_comment)));
                return out;
            }
        },
        Carry::Str(q, triple) => {
            let close = if triple { format!("{q}{q}{q}") } else { q.to_string() };
            match line.find(&close) {
                Some(k) => {
                    out.push((0..k + close.len(), style(c.code_string)));
                    i = k + close.len();
                    *carry = Carry::None;
                }
                None => {
                    out.push((0..n, style(c.code_string)));
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
    while i < n {
        let rest = &line[i..];
        let ch = rest.chars().next().unwrap_or(' ');
        // komentar baris
        if line_comment.iter().any(|p| rest.starts_with(p)) && !(fam == Family::Script && lang.eq_ignore_ascii_case("css")) {
            out.push((i..n, style(c.code_comment)));
            break;
        }
        // komentar blok
        if (fam != Family::Script && fam != Family::Sql && rest.starts_with(block_open)) || (fam == Family::Sql && rest.starts_with("/*")) {
            match rest[block_open.len()..].find(block_close) {
                Some(k) => {
                    let end = i + block_open.len() + k + block_close.len();
                    out.push((i..end, style(c.code_comment)));
                    i = end;
                    continue;
                }
                None => {
                    out.push((i..n, style(c.code_comment)));
                    *carry = Carry::Block;
                    break;
                }
            }
        }
        // markup: tag dan atribut
        if fam == Family::Markup {
            if ch == '<' {
                let start = i;
                let mut j = i + 1;
                if j < n && (b[j] == b'/' || b[j] == b'!' || b[j] == b'?') {
                    j += 1;
                }
                while j < n && (b[j].is_ascii_alphanumeric() || b"-_:.".contains(&b[j])) {
                    j += 1;
                }
                out.push((start..j, style(c.code_keyword)));
                in_tag = true;
                i = j;
                continue;
            }
            if in_tag && (ch == '>' || rest.starts_with("/>")) {
                let l = if ch == '>' { 1 } else { 2 };
                out.push((i..i + l, style(c.code_keyword)));
                in_tag = false;
                i += l;
                continue;
            }
            if in_tag && (ch.is_ascii_alphabetic() || ch == '_' || ch == '@' || ch == ':') {
                let mut j = i;
                while j < n && (b[j].is_ascii_alphanumeric() || b"-_:.@".contains(&b[j])) {
                    j += 1;
                }
                out.push((i..j, style(c.code_attr)));
                i = j;
                continue;
            }
            if !in_tag && ch != '"' && ch != '\'' {
                i += ch.len_utf8();
                continue;
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
            // kunci JSON (string diikuti ':') = atribut
            let is_key = (fam == Family::Json || fam == Family::Script && lang.starts_with("y")) && line[j..].trim_start().starts_with(':');
            out.push((i..j, style(if is_key { c.code_attr } else { c.code_string })));
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
            out.push((i..j, style(c.code_number)));
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
            let color = match fam {
                Family::CLike if KW_CLIKE.contains(&w) => Some(c.code_keyword),
                Family::Script if KW_SCRIPT.contains(&w) => Some(c.code_keyword),
                Family::Sql if KW_SQL.contains(&w.to_lowercase().as_str()) => Some(c.code_keyword),
                Family::Json if ["true", "false", "null"].contains(&w) => Some(c.code_keyword),
                Family::Css if next.starts_with(':') && !next.starts_with("::") => Some(c.code_attr),
                Family::Css => None,
                _ if next.starts_with('(') => Some(c.code_function),
                Family::CLike if w.chars().next().is_some_and(|x| x.is_uppercase()) => Some(c.code_type),
                Family::Script if (lang.starts_with('y') || lang == "toml" || lang == "ini") && next.starts_with(['=', ':']) => Some(c.code_attr),
                _ => None,
            };
            if let Some(col) = color {
                out.push((i..j, style(col)));
            }
            i = j;
            continue;
        }
        // variabel shell/PowerShell $nama
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
        assert_eq!(spans("html", "<a href=\"x\">t</a>"), vec!["<a", "href", "\"x\"", ">", "</a", ">"]);
        assert_eq!(spans("js", "/* a\nb */ x"), vec!["/* a", "b */"]);
        assert_eq!(spans("sql", "SELECT * FROM t"), vec!["SELECT", "FROM"]);
        assert!(spans("txt", "fn x").is_empty());
        assert_eq!(lang_of_path("C:\\a\\b.tsx"), "tsx");
        // rentang selalu di batas karakter (UTF-8)
        let _ = spans("rs", "let é = \"ñ\"; // ü");
    }
}

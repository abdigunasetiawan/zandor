//! Jenis dokumen Panel 3 — port packages/core/src/docs/kinds.ts (jenis dari nama/isi, bahasa, MIME, dekode teks, path)
//! dan notebook.ts (pembaca .ipynb: hanya teks, gambar png/jpeg, galat; HTML/JS tidak dirender).
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocKind {
    Code,
    Markdown,
    Svg,
    Image,
    Pdf,
    Csv,
    Tsv,
    Json,
    Html,
    Audio,
    Video,
    Docx,
    Xlsx,
    Pptx,
    Notebook,
    Binary,
}

pub fn file_name(path: &str) -> &str {
    path.split(['\\', '/']).filter(|s| !s.is_empty()).next_back().unwrap_or(path)
}

pub fn ext_of(path: &str) -> String {
    let n = file_name(path);
    match n.rfind('.') {
        Some(d) if d > 0 => n[d + 1..].to_lowercase(),
        _ => String::new(),
    }
}

fn ext_kind(ext: &str) -> Option<DocKind> {
    use DocKind::*;
    Some(match ext {
        "md" | "markdown" | "mdx" => Markdown,
        "svg" => Svg,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "avif" => Image,
        "pdf" => Pdf,
        "csv" => Csv,
        "tsv" => Tsv,
        "json" | "jsonc" | "geojson" => Json,
        "html" | "htm" => Html,
        "mp3" | "wav" | "ogg" | "oga" | "m4a" | "flac" => Audio,
        "mp4" | "webm" | "mov" | "m4v" | "ogv" => Video,
        "docx" => Docx,
        "xlsx" => Xlsx,
        "pptx" => Pptx,
        "ipynb" => Notebook,
        "doc" | "xls" | "ppt" | "zip" | "gz" | "tar" | "7z" | "exe" | "dll" | "wasm" | "bin" | "sqlite" | "db" => Binary,
        _ => return None,
    })
}

pub fn ext_lang(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "json" | "jsonc" => "json",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "less" => "less",
        "xml" | "svg" => "xml",
        "yaml" | "yml" => "yaml",
        "toml" | "ini" => "ini",
        "py" => "python",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "kt" => "kotlin",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "sh" | "bash" | "zsh" => "shell",
        "ps1" => "powershell",
        "bat" | "cmd" => "bat",
        "sql" => "sql",
        "swift" => "swift",
        "lua" => "lua",
        "r" => "r",
        "dockerfile" => "dockerfile",
        "graphql" | "gql" => "graphql",
        "csv" | "tsv" | "txt" | "log" => "plaintext",
        _ => return None,
    })
}
fn name_lang(name: &str) -> Option<&'static str> {
    Some(match name {
        "dockerfile" => "dockerfile",
        "makefile" | ".gitignore" => "plaintext",
        ".env" => "ini",
        _ => return None,
    })
}

pub fn kind_from_name(path: &str) -> Option<DocKind> {
    let ext = ext_of(path);
    if let Some(k) = ext_kind(&ext) {
        return Some(k);
    }
    (ext_lang(&ext).is_some() || name_lang(&file_name(path).to_lowercase()).is_some()).then_some(DocKind::Code)
}

pub fn sniff_kind(bytes: &[u8]) -> DocKind {
    if bytes.iter().take(8192).any(|b| *b == 0) { DocKind::Binary } else { DocKind::Code }
}

pub fn classify(path: &str, bytes: Option<&[u8]>) -> DocKind {
    kind_from_name(path).unwrap_or_else(|| bytes.map(sniff_kind).unwrap_or(DocKind::Code))
}

pub fn language_for(path: &str) -> &'static str {
    if let Some(l) = name_lang(&file_name(path).to_lowercase()) {
        return l;
    }
    ext_lang(&ext_of(path)).unwrap_or("plaintext")
}

pub fn mime_for(path: &str) -> &'static str {
    match ext_of(path).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        "html" | "htm" => "text/html",
        _ => "application/octet-stream",
    }
}

/// Kind yang punya dua tampilan (render + mentah).
pub fn has_raw_mode(k: DocKind) -> bool {
    matches!(k, DocKind::Markdown | DocKind::Svg | DocKind::Csv | DocKind::Tsv | DocKind::Json | DocKind::Notebook)
}
/// Jenis yang selalu tampil sebagai kode (tanpa tab render): kode biasa dan HTML (pratinjau lewat "Buka di browser").
pub fn always_raw(k: DocKind) -> bool {
    matches!(k, DocKind::Code | DocKind::Html)
}
pub fn is_text_kind(k: DocKind) -> bool {
    always_raw(k) || has_raw_mode(k)
}
/// true = tampilan render; false = mentah.
pub fn default_render(k: DocKind) -> bool {
    !always_raw(k)
}

pub const MAX_TEXT_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedText {
    pub text: String,
    pub encoding: &'static str,
    pub bom: bool,
    pub truncated: bool,
}

/// Dekode teks dengan mengenali BOM UTF-8/16; memotong di `max` byte.
pub fn decode_text(bytes: &[u8], max: usize) -> DecodedText {
    let truncated = bytes.len() > max;
    let mut s = if truncated { &bytes[..max] } else { bytes };
    let (mut enc, mut bom) = ("utf-8", false);
    if s.starts_with(&[0xef, 0xbb, 0xbf]) {
        s = &s[3..];
        bom = true;
    } else if s.starts_with(&[0xff, 0xfe]) {
        s = &s[2..];
        enc = "utf-16le";
        bom = true;
    } else if s.starts_with(&[0xfe, 0xff]) {
        s = &s[2..];
        enc = "utf-16be";
        bom = true;
    }
    let text = match enc {
        "utf-16le" => String::from_utf16_lossy(&s.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()),
        "utf-16be" => String::from_utf16_lossy(&s.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>()),
        _ => String::from_utf8_lossy(s).into_owned(),
    };
    DecodedText { text, encoding: enc, bom, truncated }
}

// ---------- notebook ----------

#[derive(Clone, Debug, PartialEq)]
pub enum NbOutput {
    Text { text: String, stderr: bool },
    Image { src: String },
    Error { name: String, message: String, traceback: String },
    Skipped { reason: String },
}
#[derive(Clone, Debug, PartialEq)]
pub struct NbCell {
    pub kind: &'static str,
    pub source: String,
    pub execution_count: Option<i64>,
    pub outputs: Vec<NbOutput>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Notebook {
    pub language: String,
    pub cells: Vec<NbCell>,
    pub truncated: bool,
}

const MAX_CELLS: usize = 1000;
const MAX_OUTPUT_CHARS: usize = 20_000;
const MAX_IMAGE_B64: usize = 3_000_000;

fn join(v: &Value) -> String {
    match v {
        Value::Array(a) => a.iter().map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).collect(),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}
fn clip(s: String) -> String {
    if s.chars().count() > MAX_OUTPUT_CHARS { format!("{}\n…(dipotong)", s.chars().take(MAX_OUTPUT_CHARS).collect::<String>()) } else { s }
}

fn output_of(o: &Value) -> Option<NbOutput> {
    match o["output_type"].as_str()? {
        "stream" => Some(NbOutput::Text { text: clip(join(&o["text"])), stderr: o["name"] == "stderr" }),
        "error" => {
            let ansi = regex::Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").expect("regex");
            Some(NbOutput::Error {
                name: o["ename"].as_str().unwrap_or("Error").to_string(),
                message: o["evalue"].as_str().unwrap_or("").to_string(),
                traceback: clip(ansi.replace_all(&join(&o["traceback"]), "").into_owned()),
            })
        }
        "execute_result" | "display_data" => {
            let data = &o["data"];
            for mime in ["image/png", "image/jpeg"] {
                let b64: String = join(&data[mime]).chars().filter(|c| !c.is_whitespace()).collect();
                if !b64.is_empty() {
                    let ok = b64.len() <= MAX_IMAGE_B64 && b64.chars().all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c));
                    return Some(if ok { NbOutput::Image { src: format!("data:{mime};base64,{b64}") } } else { NbOutput::Skipped { reason: "gambar terlalu besar atau tidak valid".into() } });
                }
            }
            let plain = join(&data["text/plain"]);
            if !plain.is_empty() {
                return Some(NbOutput::Text { text: clip(plain), stderr: false });
            }
            if !data["text/html"].is_null() || !data["application/javascript"].is_null() || !data["application/vnd.jupyter.widget-view+json"].is_null() {
                return Some(NbOutput::Skipped { reason: "output interaktif/HTML tidak ditampilkan".into() });
            }
            None
        }
        _ => None,
    }
}

pub fn parse_notebook(text: &str) -> Result<Notebook, String> {
    let raw: Value = serde_json::from_str(text).map_err(|_| "bukan notebook yang valid (JSON rusak)".to_string())?;
    let cells = raw["cells"].as_array().ok_or("bukan notebook yang valid (tidak ada `cells`)")?;
    let out = cells
        .iter()
        .take(MAX_CELLS)
        .map(|c| {
            let kind = match c["cell_type"].as_str() {
                Some("markdown") => "markdown",
                Some("code") => "code",
                _ => "raw",
            };
            let outputs = if kind == "code" { c["outputs"].as_array().map(|a| a.iter().filter_map(output_of).collect()).unwrap_or_default() } else { vec![] };
            NbCell { kind, source: join(&c["source"]), execution_count: c["execution_count"].as_i64(), outputs }
        })
        .collect();
    let language = raw["metadata"]["language_info"]["name"].as_str().or_else(|| raw["metadata"]["kernelspec"]["language"].as_str()).unwrap_or("python").to_string();
    Ok(Notebook { language, cells: out, truncated: cells.len() > MAX_CELLS })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jenis() {
        assert_eq!(kind_from_name("a/b/README.md"), Some(DocKind::Markdown));
        assert_eq!(kind_from_name("x.rs"), Some(DocKind::Code));
        assert_eq!(kind_from_name("Dockerfile"), Some(DocKind::Code));
        assert_eq!(kind_from_name("x.unknown"), None);
        assert_eq!(classify("x.unknown", Some(&[1, 0, 2])), DocKind::Binary);
        assert_eq!(language_for("a.tsx"), "typescript");
        assert_eq!(ext_of(".env"), "");
        assert!(has_raw_mode(DocKind::Json) && !has_raw_mode(DocKind::Code));
        let d = decode_text(&[0xef, 0xbb, 0xbf, b'h', b'i'], 100);
        assert_eq!((d.text.as_str(), d.bom), ("hi", true));
        let d = decode_text(&[0xff, 0xfe, b'h', 0, b'i', 0], 100);
        assert_eq!((d.text.as_str(), d.encoding), ("hi", "utf-16le"));
    }
    #[test]
    fn notebook() {
        let nb = parse_notebook(r##"{"cells":[{"cell_type":"code","source":["print(1)"],"execution_count":3,"outputs":[{"output_type":"stream","name":"stdout","text":["1\n"]},{"output_type":"error","ename":"E","evalue":"x","traceback":["\u001b[31mT\u001b[0m"]}]},{"cell_type":"markdown","source":"# H"}],"metadata":{"kernelspec":{"language":"r"}}}"##).unwrap();
        assert_eq!(nb.language, "r");
        assert_eq!(nb.cells[0].execution_count, Some(3));
        assert_eq!(nb.cells[0].outputs[1], NbOutput::Error { name: "E".into(), message: "x".into(), traceback: "T".into() });
        assert!(parse_notebook("{}").is_err());
    }
}

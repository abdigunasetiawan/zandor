//! Lampiran — port packages/core/src/attachments.ts.
#![allow(dead_code)]
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct FileAttachment {
    pub mime: String,
    pub url: String,
    pub filename: Option<String>,
}

pub const DIRECTORY_MIME: &str = "application/x-directory";

pub fn mime_for_name(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "txt" | "ts" | "tsx" | "rs" | "toml" => "text/plain",
        "md" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        "html" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "py" => "text/x-python",
        "yaml" | "yml" => "text/yaml",
        "xml" => "application/xml",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "doc" => "application/msword",
        "xls" => "application/vnd.ms-excel",
        "ppt" => "application/vnd.ms-powerpoint",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        "ipynb" => "application/x-ipynb+json",
        "zip" => "application/zip",
        _ => "text/plain",
    }
}

fn encode_seg(seg: &str) -> String {
    let mut out = String::new();
    for b in seg.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `C:\a b\c.txt` → `file:///C:/a%20b/c.txt`; awalan verbatim `\\?\` dibuang.
pub fn file_url_from_path(path: &str) -> String {
    let raw = path.strip_prefix(r"\\?\").unwrap_or(path);
    let slashed = raw.replace('\\', "/");
    let encoded: Vec<String> = slashed
        .split('/')
        .enumerate()
        .map(|(i, seg)| {
            let drive = seg.len() == 2 && seg.as_bytes()[0].is_ascii_alphabetic() && seg.as_bytes()[1] == b':';
            if i == 0 && drive { seg.to_string() } else { encode_seg(seg) }
        })
        .collect();
    let joined = encoded.join("/");
    if joined.starts_with('/') { format!("file://{joined}") } else { format!("file:///{joined}") }
}

pub fn base_name_of(path: &str) -> &str {
    path.split(['\\', '/']).filter(|s| !s.is_empty()).last().unwrap_or(path)
}

pub fn attachment_from_path(path: &str, is_dir: bool) -> FileAttachment {
    let filename = base_name_of(path).to_string();
    FileAttachment {
        mime: if is_dir { DIRECTORY_MIME.into() } else { mime_for_name(&filename).into() },
        url: file_url_from_path(path),
        filename: Some(filename),
    }
}

/// Model bisa menerima lampiran ini langsung sebagai part `file`.
pub fn model_readable(mime: &str) -> bool {
    let m = mime.to_lowercase();
    let m = m.split(';').next().unwrap_or("").trim();
    m.starts_with("image/")
        || m.starts_with("text/")
        || m == "application/pdf"
        || m == "application/json"
        || m == "application/xml"
        || m == DIRECTORY_MIME
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

/// `file:///C:/a%20b/c.txt` → `C:/a b/c.txt`.
pub fn path_from_file_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let b = rest.as_bytes();
    let rest = if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' { &rest[1..] } else { rest };
    let mut out = Vec::new();
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// Lampiran → part prompt. Yang tak terbaca model dikirim sebagai catatan path sintetis.
pub fn attachment_parts(files: &[FileAttachment]) -> Vec<Value> {
    let mut parts = Vec::new();
    let mut notes = Vec::new();
    for f in files {
        if model_readable(&f.mime) {
            let mut p = json!({ "type": "file", "mime": f.mime, "url": f.url });
            if let Some(n) = &f.filename {
                p["filename"] = json!(n);
            }
            parts.push(p);
            continue;
        }
        let path = path_from_file_url(&f.url);
        let name = f
            .filename
            .clone()
            .unwrap_or_else(|| path.as_deref().map(|p| base_name_of(p).to_string()).unwrap_or_else(|| "lampiran".into()));
        notes.push(match path {
            Some(p) => format!("- {name}: {p}"),
            None => format!("- {name}: (tidak tersimpan di disk; minta pengguna menyimpannya ke folder project)"),
        });
    }
    if !notes.is_empty() {
        let mut lines = vec![
            "<zandor-attachments>".to_string(),
            "Pengguna melampirkan berkas yang tidak bisa dikirim langsung ke model. Baca dari disk dengan tool/skill yang sesuai (DOCX/XLSX/PPTX/PDF: skill zandor-docx/zandor-xlsx/zandor-pptx/zandor-pdf):".to_string(),
        ];
        lines.extend(notes);
        lines.push("</zandor-attachments>".into());
        parts.push(json!({ "type": "text", "synthetic": true, "text": lines.join("\n") }));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn url_bolak_balik() {
        let u = file_url_from_path(r"C:\a b\c.txt");
        assert_eq!(u, "file:///C:/a%20b/c.txt");
        assert_eq!(path_from_file_url(&u).as_deref(), Some("C:/a b/c.txt"));
    }
    #[test]
    fn docx_jadi_catatan() {
        let p = attachment_parts(&[attachment_from_path(r"C:\x\laporan.docx", false)]);
        assert_eq!(p.len(), 1);
        assert!(p[0]["text"].as_str().unwrap().contains("- laporan.docx: C:/x/laporan.docx"));
    }
}

/// Port attachmentsFromNote: lampiran yang dikirim sebagai path (catatan sintetis `<zandor-attachments>`).
pub fn attachments_from_note(text: &str) -> Vec<(String, Option<String>)> {
    if !text.starts_with("<zandor-attachments>") {
        return vec![];
    }
    text.split('\n')
        .filter_map(|l| l.strip_prefix("- "))
        .map(|body| match body.find(": ") {
            None => (body.to_string(), None),
            Some(at) => {
                let rest = &body[at + 2..];
                (body[..at].to_string(), (!rest.starts_with('(')).then(|| rest.to_string()))
            }
        })
        .collect()
}

#[cfg(test)]
mod note_tests {
    #[test]
    fn note() {
        let n = super::attachments_from_note("<zandor-attachments>\n- a.docx: C:/x/a.docx\n- b.pdf: (terlalu besar)\n- c");
        assert_eq!(n, vec![("a.docx".into(), Some("C:/x/a.docx".into())), ("b.pdf".into(), None), ("c".into(), None)]);
        assert!(super::attachments_from_note("biasa").is_empty());
    }
}

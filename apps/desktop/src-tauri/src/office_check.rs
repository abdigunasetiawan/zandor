//! Pemeriksa struktur dokumen bawaan (ADR-074), pengganti `office_check.py` agar skill dokumen bisa membaca dan
//! memverifikasi berkas tanpa Python. Dipanggil agent lewat `"<Zandor.exe>" office-check <berkas> [--expect "teks"]...
//! [--text]`; mencetak satu laporan JSON, kode keluar 0 bila valid dan semua `--expect` ditemukan, 1 bila tidak,
//! 2 untuk pemakaian salah.
//!
//! Memeriksa DOCX/XLSX/PPTX (ZIP + XML + relasi internal) dan PDF (header, trailer, perkiraan halaman).
//! BUKAN pemeriksa tampilan visual dan tidak menghitung ulang rumus.
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

const NS_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
/// Batas ukuran satu anggota ZIP yang dibaca (lindungi dari zip bomb).
const MAX_PART: u64 = 64 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 20_000;

pub const USAGE: &str = "Pemeriksa struktur dokumen Zandor.\n\nPemakaian:\n    Zandor office-check <berkas> [--expect \"teks\"]... [--text]\n\nMemeriksa DOCX/XLSX/PPTX (ZIP + XML + relasi internal) dan PDF (header, trailer, perkiraan halaman).\nMencetak satu laporan JSON. Kode keluar 0 bila valid dan semua --expect ditemukan, 1 bila tidak.\nIni BUKAN pemeriksa tampilan visual dan tidak menghitung ulang rumus.";

fn main_part(kind: &str) -> Option<&'static str> {
    match kind {
        "docx" => Some("word/document.xml"),
        "xlsx" => Some("xl/workbook.xml"),
        "pptx" => Some("ppt/presentation.xml"),
        _ => None,
    }
}

struct Report {
    errors: Vec<String>,
    warnings: Vec<String>,
    info: Map<String, Value>,
}

/// Teks semua elemen bernama `name` (nama lokal) di bawah `node`, termasuk dirinya.
fn texts<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Vec<&'a str> {
    node.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == name)
        .map(|n| n.text().unwrap_or(""))
        .collect()
}

fn count(node: roxmltree::Node, name: &str) -> usize {
    node.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == name)
        .count()
}

/// Gabung path ala POSIX lalu normalkan `.`/`..` (setara posixpath.normpath(join(base, t))).
fn resolve(base: &str, target: &str) -> String {
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_string();
    }
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn dirname(p: &str) -> &str {
    p.rfind('/').map_or("", |i| &p[..i])
}

/// Nomor terakhir dalam nama (mis. `ppt/slides/slide12.xml` → 12).
fn last_number(name: &str) -> Option<u32> {
    let digits: String = name
        .rsplit(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())?
        .to_string();
    digits.parse().ok()
}

/// `prefix` + angka + `.xml`, mis. `is_numbered("xl/worksheets/sheet3.xml", "xl/worksheets/sheet")`.
fn is_numbered(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|r| r.strip_suffix(".xml"))
        .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

fn check_ooxml(path: &str, kind: &str, rep: &mut Report) -> String {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            rep.errors.push(format!("gagal membaca berkas: {e}"));
            return String::new();
        }
    };
    let mut z = match zip::ZipArchive::new(file) {
        Ok(z) => z,
        Err(_) => {
            rep.errors
                .push("bukan paket ZIP (berkas rusak atau terenkripsi)".into());
            return String::new();
        }
    };
    let names: BTreeSet<String> = z.file_names().map(str::to_string).collect();
    // Baca semua bagian XML sekaligus; galat dekompresi/CRC = anggota rusak (setara ZipFile.testzip()).
    let mut raw: BTreeMap<String, String> = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = match z.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                rep.errors.push(format!("anggota ZIP rusak: #{i} ({e})"));
                return String::new();
            }
        };
        let name = f.name().to_string();
        let mut buf = Vec::new();
        if let Err(e) = (&mut f).take(MAX_PART + 1).read_to_end(&mut buf) {
            rep.errors.push(format!("anggota ZIP rusak: {name} ({e})"));
            return String::new();
        }
        if buf.len() as u64 > MAX_PART {
            rep.errors
                .push(format!("anggota ZIP terlalu besar untuk diperiksa: {name}"));
            return String::new();
        }
        if name.ends_with(".xml") || name.ends_with(".rels") {
            match String::from_utf8(buf) {
                Ok(s) => {
                    raw.insert(name, s);
                }
                Err(_) => rep
                    .errors
                    .push(format!("XML tidak valid di {name}: bukan UTF-8")),
            }
        }
    }
    if !names.contains("[Content_Types].xml") {
        rep.errors.push("[Content_Types].xml tidak ada".into());
    }
    let main = main_part(kind).unwrap_or_default();
    if !names.contains(main) {
        rep.errors.push(format!("bagian utama {main} tidak ada"));
    }
    let opts = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..Default::default()
    };
    let mut parsed: BTreeMap<&str, roxmltree::Document> = BTreeMap::new();
    for (n, s) in &raw {
        let s = s.strip_prefix('\u{feff}').unwrap_or(s);
        match roxmltree::Document::parse_with_options(s, opts) {
            Ok(d) => {
                parsed.insert(n.as_str(), d);
            }
            Err(e) => rep.errors.push(format!("XML tidak valid di {n}: {e}")),
        }
    }
    // Relasi internal harus menunjuk bagian yang ada.
    for (n, doc) in &parsed {
        if !n.ends_with(".rels") {
            continue;
        }
        let base = dirname(dirname(n));
        for rel in doc
            .descendants()
            .filter(|e| e.has_tag_name((NS_REL, "Relationship")))
        {
            if rel.attribute("TargetMode") == Some("External") {
                continue;
            }
            let t = rel.attribute("Target").unwrap_or("");
            if !names.contains(&resolve(base, t)) {
                rep.errors.push(format!("relasi rusak di {n}: {t}"));
            }
        }
    }

    let mut text: Vec<String> = Vec::new();
    match kind {
        "docx" => {
            if let Some(doc) = parsed.get("word/document.xml") {
                let root = doc.root();
                let paras: Vec<_> = root
                    .descendants()
                    .filter(|e| e.is_element() && e.tag_name().name() == "p")
                    .collect();
                rep.info.insert("paragraphs".into(), json!(paras.len()));
                rep.info.insert("tables".into(), json!(count(root, "tbl")));
                rep.info
                    .insert("sections".into(), json!(count(root, "sectPr")));
                for p in paras {
                    text.push(texts(p, "t").concat());
                }
                for (n, d) in &parsed {
                    let extra = ["header", "footer", "footnotes", "endnotes"]
                        .iter()
                        .any(|k| {
                            n.strip_prefix("word/")
                                .and_then(|r| r.strip_prefix(k))
                                .and_then(|r| r.strip_suffix(".xml"))
                                .is_some_and(|d| d.bytes().all(|b| b.is_ascii_digit()))
                        });
                    if extra {
                        text.extend(texts(d.root(), "t").into_iter().map(str::to_string));
                    }
                }
            }
        }
        "xlsx" => {
            if let Some(wb) = parsed.get("xl/workbook.xml") {
                let sheets: Vec<&str> = wb
                    .descendants()
                    .filter(|e| e.is_element() && e.tag_name().name() == "sheet")
                    .filter_map(|e| e.attribute("name"))
                    .collect();
                rep.info.insert("sheets".into(), json!(sheets));
                let shared: Vec<String> = parsed
                    .get("xl/sharedStrings.xml")
                    .map(|d| {
                        d.root_element()
                            .children()
                            .filter(|c| c.is_element())
                            .map(|si| texts(si, "t").concat())
                            .collect()
                    })
                    .unwrap_or_default();
                let (mut cells, mut formulas) = (0usize, 0usize);
                for (n, d) in &parsed {
                    if !is_numbered(n, "xl/worksheets/sheet") {
                        continue;
                    }
                    for c in d
                        .descendants()
                        .filter(|e| e.is_element() && e.tag_name().name() == "c")
                    {
                        cells += 1;
                        let kids: Vec<_> = c.children().filter(|x| x.is_element()).collect();
                        let v = kids
                            .iter()
                            .find(|x| x.tag_name().name() == "v")
                            .map(|x| x.text().unwrap_or(""));
                        if kids.iter().any(|x| x.tag_name().name() == "f") {
                            formulas += 1;
                        }
                        match (c.attribute("t"), v) {
                            (Some("s"), Some(v)) => {
                                if let Some(s) = v.parse::<usize>().ok().and_then(|i| shared.get(i))
                                {
                                    text.push(s.clone());
                                }
                            }
                            (Some("inlineStr"), _) => text.push(texts(c, "t").concat()),
                            (_, Some(v)) => text.push(v.to_string()),
                            _ => {}
                        }
                    }
                }
                rep.info.insert("cells".into(), json!(cells));
                rep.info.insert("formulas".into(), json!(formulas));
                if formulas > 0 {
                    rep.warnings.push("rumus tidak dihitung ulang oleh pemeriksa; nilai cache bisa usang sampai dibuka di Excel/LibreOffice".into());
                }
            }
        }
        "pptx" => {
            let mut slides: Vec<&str> = parsed
                .keys()
                .copied()
                .filter(|n| is_numbered(n, "ppt/slides/slide"))
                .collect();
            slides.sort_by_key(|s| last_number(s));
            rep.info.insert("slides".into(), json!(slides.len()));
            for n in slides {
                text.extend(texts(parsed[n].root(), "t").into_iter().map(str::to_string));
            }
            let notes = parsed
                .keys()
                .filter(|n| is_numbered(n, "ppt/notesSlides/notesSlide"))
                .count();
            rep.info.insert("notes".into(), json!(notes));
        }
        _ => {}
    }
    text.retain(|t| !t.is_empty());
    text.join("\n")
}

fn find(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Hitung `/Type /Page` (spasi opsional) yang tidak diikuti huruf (bukan `/Pages`).
fn count_pages(data: &[u8]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while let Some(off) = data[i..].windows(5).position(|w| w == b"/Type") {
        let mut j = i + off + 5;
        while j < data.len() && data[j].is_ascii_whitespace() {
            j += 1;
        }
        if data[j..].starts_with(b"/Page") {
            let k = j + 5;
            if k >= data.len() || !data[k].is_ascii_alphabetic() {
                n += 1;
            }
        }
        i += off + 5;
    }
    n
}

fn check_pdf(path: &str, rep: &mut Report) -> String {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            rep.errors.push(format!("gagal membaca berkas: {e}"));
            return String::new();
        }
    };
    if !data.starts_with(b"%PDF-") {
        rep.errors.push("header %PDF- tidak ada".into());
        return String::new();
    }
    let ver: String = data[5..data.len().min(8)]
        .iter()
        .map(|&b| b as char)
        .collect();
    rep.info.insert("version".into(), json!(ver));
    let tail = &data[data.len().saturating_sub(2048)..];
    if !find(tail, b"%%EOF") {
        rep.errors
            .push("penanda %%EOF tidak ada di akhir berkas (mungkin terpotong)".into());
    }
    rep.info
        .insert("pages_estimate".into(), json!(count_pages(&data)));
    if find(&data, b"/Encrypt") {
        rep.warnings.push("PDF terenkripsi".into());
    }
    rep.warnings.push(
        "teks PDF tidak diekstrak oleh pemeriksa bawaan; pakai pypdf/pdfplumber untuk isi".into(),
    );
    String::new()
}

/// Jalankan pemeriksaan untuk argumen setelah `office-check`. Mengembalikan (keluaran stdout, kode keluar).
pub fn run(args: &[String]) -> (String, i32) {
    let Some(path) = args.first().filter(|a| *a != "-h" && *a != "--help") else {
        return (USAGE.to_string(), 2);
    };
    let (mut expects, mut show_text) = (Vec::new(), false);
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--expect" if i + 1 < args.len() => {
                expects.push(args[i + 1].clone());
                i += 2;
            }
            "--text" => {
                show_text = true;
                i += 1;
            }
            other => return (format!("argumen tidak dikenal: {other}"), 2),
        }
    }
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e).to_lowercase();
    let kind = match ext.as_str() {
        "docx" | "docm" => "docx",
        "xlsx" | "xlsm" => "xlsx",
        "pptx" | "pptm" => "pptx",
        e => e,
    }
    .to_string();
    let mut rep = Report {
        errors: vec![],
        warnings: vec![],
        info: Map::new(),
    };
    let text = if main_part(&kind).is_some() {
        check_ooxml(path, &kind, &mut rep)
    } else if kind == "pdf" {
        check_pdf(path, &mut rep)
    } else {
        rep.errors.push(format!("jenis tidak didukung: .{ext}"));
        String::new()
    };
    let missing: Vec<&String> = expects
        .iter()
        .filter(|e| !text.contains(e.as_str()))
        .collect();
    let valid = rep.errors.is_empty();
    let mut out = json!({
        "file": path, "kind": kind, "valid": valid,
        "errors": rep.errors, "warnings": rep.warnings, "info": rep.info,
    });
    if !expects.is_empty() {
        out["expect_missing"] = json!(missing);
    }
    if show_text {
        out["text"] = json!(text.chars().take(MAX_TEXT_CHARS).collect::<String>());
    }
    let code = if valid && missing.is_empty() { 0 } else { 1 };
    (serde_json::to_string_pretty(&out).unwrap_or_default(), code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("zandor-oc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name).to_string_lossy().into_owned()
    }

    fn write_zip(path: &str, parts: &[(&str, &str)]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (n, c) in parts {
            w.start_file(*n, o).unwrap();
            w.write_all(c.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    fn check(args: &[&str]) -> (Value, i32) {
        let (s, code) = run(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        (serde_json::from_str(&s).unwrap(), code)
    }

    const CT: &str = r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#;
    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    #[test]
    fn docx_text_tables_header_and_expect() {
        let p = tmp("a.docx");
        let doc = format!(
            r#"<w:document xmlns:w="{W}"><w:body><w:p><w:r><w:t>Halo </w:t></w:r><w:r><w:t>BARU</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>sel</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:sectPr/></w:body></w:document>"#
        );
        let hdr = format!(r#"<w:hdr xmlns:w="{W}"><w:p><w:r><w:t>Kop</w:t></w:r></w:p></w:hdr>"#);
        let rels = format!(
            r#"<Relationships xmlns="{NS_REL}"><Relationship Id="r1" Target="header1.xml"/><Relationship Id="r2" TargetMode="External" Target="https://x"/></Relationships>"#
        );
        write_zip(
            &p,
            &[
                ("[Content_Types].xml", CT),
                ("word/document.xml", &doc),
                ("word/header1.xml", &hdr),
                ("word/_rels/document.xml.rels", &rels),
            ],
        );
        let (r, code) = check(&[
            &p,
            "--expect",
            "Halo BARU",
            "--expect",
            "Kop",
            "--expect",
            "sel",
            "--text",
        ]);
        assert_eq!(code, 0, "{r}");
        assert_eq!(r["valid"], true);
        assert_eq!(r["info"]["paragraphs"], 2);
        assert_eq!(r["info"]["tables"], 1);
        assert_eq!(r["expect_missing"], json!([]));
        let (r, code) = check(&[&p, "--expect", "TIDAK ADA"]);
        assert_eq!(code, 1);
        assert_eq!(r["expect_missing"], json!(["TIDAK ADA"]));
    }

    #[test]
    fn broken_relation_and_bad_xml_are_errors() {
        let p = tmp("b.docx");
        let rels = format!(
            r#"<Relationships xmlns="{NS_REL}"><Relationship Id="r1" Target="../media/hilang.png"/></Relationships>"#
        );
        write_zip(
            &p,
            &[
                ("[Content_Types].xml", CT),
                ("word/document.xml", "<w:document"),
                ("word/_rels/document.xml.rels", &rels),
            ],
        );
        let (r, code) = check(&[&p]);
        assert_eq!(code, 1);
        let errs = r["errors"].to_string();
        assert!(
            errs.contains("XML tidak valid di word/document.xml"),
            "{errs}"
        );
        assert!(
            errs.contains("relasi rusak") && errs.contains("hilang.png"),
            "{errs}"
        );
    }

    #[test]
    fn xlsx_shared_strings_formulas_and_pptx_slides() {
        let s = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        let p = tmp("c.xlsx");
        write_zip(
            &p,
            &[
                ("[Content_Types].xml", CT),
                (
                    "xl/workbook.xml",
                    &format!(
                        r#"<workbook xmlns="{s}"><sheets><sheet name="Data"/></sheets></workbook>"#
                    ),
                ),
                (
                    "xl/sharedStrings.xml",
                    &format!(r#"<sst xmlns="{s}"><si><t>Baru</t></si></sst>"#),
                ),
                (
                    "xl/worksheets/sheet1.xml",
                    &format!(
                        r#"<worksheet xmlns="{s}"><sheetData><row><c t="s"><v>0</v></c><c><f>1+1</f><v>2</v></c><c t="inlineStr"><is><t>sebaris</t></is></c></row></sheetData></worksheet>"#
                    ),
                ),
            ],
        );
        let (r, code) = check(&[&p, "--expect", "Baru", "--expect", "sebaris"]);
        assert_eq!(code, 0, "{r}");
        assert_eq!(r["info"]["sheets"], json!(["Data"]));
        assert_eq!(r["info"]["cells"], 3);
        assert_eq!(r["info"]["formulas"], 1);
        assert_eq!(r["warnings"].as_array().unwrap().len(), 1);

        let a = "http://schemas.openxmlformats.org/drawingml/2006/main";
        let p = tmp("d.pptx");
        let slide = |t: &str| format!(r#"<p:sld xmlns:p="x" xmlns:a="{a}"><a:t>{t}</a:t></p:sld>"#);
        write_zip(
            &p,
            &[
                ("[Content_Types].xml", CT),
                ("ppt/presentation.xml", r#"<p:presentation xmlns:p="x"/>"#),
                ("ppt/slides/slide10.xml", &slide("Akhir")),
                ("ppt/slides/slide2.xml", &slide("Ringkasan")),
                ("ppt/notesSlides/notesSlide1.xml", "<n/>"),
            ],
        );
        let (r, code) = check(&[&p, "--expect", "Ringkasan", "--text"]);
        assert_eq!(code, 0, "{r}");
        assert_eq!(r["info"]["slides"], 2);
        assert_eq!(r["info"]["notes"], 1);
        assert_eq!(r["text"], "Ringkasan\nAkhir");
    }

    #[test]
    fn pdf_and_misc() {
        let p = tmp("e.pdf");
        std::fs::write(&p, b"%PDF-1.7\n1 0 obj<</Type /Pages>>endobj 2 0 obj<</Type/Page>>endobj 3 0 obj<</Type /Page /X 1>>endobj\n%%EOF\n").unwrap();
        let (r, code) = check(&[&p]);
        assert_eq!(code, 0, "{r}");
        assert_eq!(r["info"]["version"], "1.7");
        assert_eq!(r["info"]["pages_estimate"], 2);

        let p = tmp("f.pdf");
        std::fs::write(&p, b"%PDF-1.4 terpotong").unwrap();
        assert_eq!(check(&[&p]).1, 1);
        let p = tmp("g.docx");
        std::fs::write(&p, b"bukan zip").unwrap();
        let (r, code) = check(&[&p]);
        assert_eq!(code, 1);
        assert!(r["errors"].to_string().contains("bukan paket ZIP"));
        assert_eq!(check(&[&tmp("h.txt")]).1, 1);
        assert_eq!(run(&[]).1, 2);
        assert_eq!(run(&["x.docx".into(), "--aneh".into()]).1, 2);
        assert_eq!(resolve("word", "../media/a.png"), "media/a.png");
        assert_eq!(resolve("", "/word/document.xml"), "word/document.xml");
    }
}

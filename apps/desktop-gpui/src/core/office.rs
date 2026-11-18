//! Pembaca DOCX / XLSX / PPTX — port packages/core/src/docs/office.ts: best-effort, hanya membaca (tanpa makro/skrip),
//! hasilnya data polos untuk dirender. zip + roxmltree; batas keras mencegah berkas besar membekukan UI.
use roxmltree::{Document, Node};
use std::collections::{HashMap, HashSet};
use std::io::Read;

const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_IMAGES: usize = 30;

type Files = HashMap<String, Vec<u8>>;

fn open(bytes: &[u8], want: impl Fn(&str) -> bool) -> Result<Files, String> {
    let bad = || "berkas bukan paket Office yang valid (zip rusak)".to_string();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| bad())?;
    let mut out = Files::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|_| bad())?;
        let name = f.name().to_string();
        if f.size() > 40 * 1024 * 1024 || !want(&name) {
            continue;
        }
        let mut buf = vec![];
        f.read_to_end(&mut buf).map_err(|_| bad())?;
        out.insert(name, buf);
    }
    Ok(out)
}

fn text_of(files: &Files, name: &str) -> Option<String> {
    files.get(name).map(|b| String::from_utf8_lossy(b).into_owned())
}

fn local<'a>(n: &Node<'a, '_>) -> &'a str {
    n.tag_name().name()
}
fn all<'a, 'i>(root: Node<'a, 'i>, name: &str) -> Vec<Node<'a, 'i>> {
    root.descendants().filter(|n| n.is_element() && local(n) == name).collect()
}
fn kids<'a, 'i>(root: Node<'a, 'i>, name: &str) -> Vec<Node<'a, 'i>> {
    root.children().filter(|n| n.is_element() && local(n) == name).collect()
}
fn kid<'a, 'i>(root: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    root.children().find(|n| n.is_element() && local(n) == name)
}
fn attr<'a>(el: Option<Node<'a, '_>>, name: &str) -> Option<&'a str> {
    el?.attributes().find(|a| a.name() == name).map(|a| a.value())
}
fn attr_ns<'a>(el: Node<'a, '_>, ns: &str, name: &str) -> Option<&'a str> {
    el.attribute((ns, name))
}
fn node_text(n: Node) -> String {
    n.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect()
}

struct Rel {
    target: String,
    external: bool,
}
fn rels_of(files: &Files, path: &str) -> HashMap<String, Rel> {
    let mut out = HashMap::new();
    let Some(t) = text_of(files, path) else { return out };
    let Ok(doc) = Document::parse(&t) else { return out };
    for r in all(doc.root(), "Relationship") {
        out.insert(attr(Some(r), "Id").unwrap_or("").to_string(), Rel { target: attr(Some(r), "Target").unwrap_or("").to_string(), external: attr(Some(r), "TargetMode") == Some("External") });
    }
    out
}

fn resolve(base: &str, target: &str) -> String {
    let joined = if let Some(t) = target.strip_prefix('/') { t.to_string() } else { format!("{base}{target}") };
    let mut out: Vec<&str> = vec![];
    for p in joined.split('/') {
        match p {
            ".." => {
                out.pop();
            }
            "." | "" => {}
            p => out.push(p),
        }
    }
    out.join("/")
}

fn image_url(files: &Files, path: &str, budget: &mut usize) -> Option<String> {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };
    let data = files.get(path)?;
    if data.len() > MAX_IMAGE_BYTES || *budget >= MAX_IMAGES {
        return None;
    }
    *budget += 1;
    Some(format!("data:{mime};base64,{}", super::gradient::b64_encode(data)))
}

// ====================== DOCX ======================

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocxRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub href: Option<String>,
    pub image: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum DocxBlock {
    P { style: Option<String>, list: Option<(u32, bool)>, runs: Vec<DocxRun> },
    Table { rows: Vec<Vec<Vec<DocxBlock>>> },
}
#[derive(Clone, Debug, PartialEq)]
pub struct DocxResult {
    pub blocks: Vec<DocxBlock>,
    pub truncated: bool,
}
const MAX_BLOCKS: usize = 5000;

fn heading_of(style: Option<&str>) -> Option<String> {
    let s: String = style?.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if s == "title" {
        return Some("title".into());
    }
    let re = regex::Regex::new(r"^(?:heading|titre|judul)(\d)$").expect("regex");
    let n: u32 = re.captures(&s)?[1].parse().ok()?;
    (1..=6).contains(&n).then(|| format!("h{n}"))
}

pub fn parse_docx(bytes: &[u8]) -> Result<DocxResult, String> {
    let re = regex::Regex::new(r"^word/(document\.xml|numbering\.xml|_rels/document\.xml\.rels|media/[^/]+)$").expect("regex");
    let files = open(bytes, |n| re.is_match(n))?;
    let doc_t = text_of(&files, "word/document.xml").ok_or("word/document.xml tidak ditemukan atau rusak")?;
    let doc = Document::parse(&doc_t).map_err(|_| "word/document.xml tidak ditemukan atau rusak".to_string())?;
    let rels = rels_of(&files, "word/_rels/document.xml.rels");
    let mut budget = 0usize;
    let mut ordered: HashMap<String, bool> = HashMap::new();
    if let Some(nt) = text_of(&files, "word/numbering.xml") {
        if let Ok(nd) = Document::parse(&nt) {
            let mut abs: HashMap<String, bool> = HashMap::new();
            for a in all(nd.root(), "abstractNum") {
                let lvl = kids(a, "lvl").into_iter().next().unwrap_or(a);
                abs.insert(attr(Some(a), "abstractNumId").unwrap_or("").into(), attr(kid(lvl, "numFmt"), "val") != Some("bullet"));
            }
            for n in all(nd.root(), "num") {
                let v = abs.get(attr(kid(n, "abstractNumId"), "val").unwrap_or("")).copied().unwrap_or(true);
                ordered.insert(attr(Some(n), "numId").unwrap_or("").into(), v);
            }
        }
    }
    let mut count = 0usize;
    let mut truncated = false;

    let run_of = |r: Node, href: Option<String>, budget: &mut usize| -> Option<DocxRun> {
        let rpr = kid(r, "rPr");
        let flag = |name: &str| match rpr.and_then(|p| kid(p, name)) {
            None => false,
            Some(e) => attr(Some(e), "val").is_none_or(|v| !["0", "false", "none"].contains(&v)),
        };
        let mut text = String::new();
        let mut image = None;
        for c in r.children().filter(|c| c.is_element()) {
            match local(&c) {
                "t" => text.push_str(&node_text(c)),
                "tab" => text.push('\t'),
                "br" | "cr" => text.push('\n'),
                "drawing" | "pict" => {
                    let blip = all(c, "blip").into_iter().next();
                    if let Some(rel) = blip.and_then(|b| b.attributes().find(|a| a.name() == "embed")).and_then(|a| rels.get(a.value())) {
                        if !rel.external {
                            image = image_url(&files, &resolve("word/", &rel.target), budget);
                        }
                    }
                }
                _ => {}
            }
        }
        if text.is_empty() && image.is_none() {
            return None;
        }
        Some(DocxRun { text, bold: flag("b"), italic: flag("i"), underline: flag("u"), strike: flag("strike"), href, image })
    };

    let para = |p: Node, budget: &mut usize| -> DocxBlock {
        let ppr = kid(p, "pPr");
        let style = attr(ppr.and_then(|x| kid(x, "pStyle")), "val");
        let mut list = None;
        if let Some(num) = ppr.and_then(|x| kid(x, "numPr")) {
            let id = attr(kid(num, "numId"), "val").unwrap_or("");
            if id != "0" {
                list = Some((attr(kid(num, "ilvl"), "val").and_then(|v| v.parse().ok()).unwrap_or(0), ordered.get(id).copied().unwrap_or(false)));
            }
        } else if let Some(s) = style {
            let low = s.to_lowercase();
            if low.starts_with("listbullet") || low.starts_with("list bullet") {
                list = Some((0, false));
            } else if low.starts_with("listnumber") || low.starts_with("list number") {
                list = Some((0, true));
            }
        }
        let mut runs = vec![];
        for c in p.children().filter(|c| c.is_element()) {
            match local(&c) {
                "r" => runs.extend(run_of(c, None, budget)),
                "hyperlink" => {
                    let rel = c.attributes().find(|a| a.name() == "id").and_then(|a| rels.get(a.value()));
                    let href = rel.filter(|r| r.external && (r.target.starts_with("http://") || r.target.starts_with("https://"))).map(|r| r.target.clone());
                    for r in kids(c, "r") {
                        runs.extend(run_of(r, href.clone(), budget));
                    }
                }
                _ => {}
            }
        }
        DocxBlock::P { style: heading_of(style), list, runs }
    };

    fn body<'a, 'i>(
        container: Node<'a, 'i>,
        depth: usize,
        count: &mut usize,
        truncated: &mut bool,
        budget: &mut usize,
        para: &dyn Fn(Node, &mut usize) -> DocxBlock,
    ) -> Vec<DocxBlock> {
        let mut out = vec![];
        for c in container.children().filter(|c| c.is_element()) {
            if *count >= MAX_BLOCKS {
                *truncated = true;
                break;
            }
            match local(&c) {
                "p" => {
                    out.push(para(c, budget));
                    *count += 1;
                }
                "tbl" if depth < 3 => {
                    let rows = kids(c, "tr").into_iter().map(|tr| kids(tr, "tc").into_iter().map(|tc| body(tc, depth + 1, count, truncated, budget, para)).collect()).collect();
                    out.push(DocxBlock::Table { rows });
                    *count += 1;
                }
                _ => {}
            }
        }
        out
    }
    let b = all(doc.root(), "body").into_iter().next().ok_or("isi dokumen tidak ditemukan")?;
    let blocks = body(b, 0, &mut count, &mut truncated, &mut budget, &para);
    Ok(DocxResult { blocks, truncated })
}

// ====================== XLSX ======================

#[derive(Clone, Debug, PartialEq)]
pub struct XlsxSheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}
const MAX_ROWS: usize = 2000;
const MAX_COLS: usize = 100;
const MAX_SHEETS: usize = 20;
const BUILTIN_DATE_FMT: [i64; 12] = [14, 15, 16, 17, 18, 19, 20, 21, 22, 45, 46, 47];

pub fn col_index(r: &str) -> i64 {
    let letters: String = r.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    if letters.is_empty() {
        return -1;
    }
    letters.to_uppercase().chars().fold(0i64, |n, ch| n * 26 + (ch as i64 - 64)) - 1
}

pub fn serial_to_text(serial: f64) -> String {
    let ms = ((serial - 25569.) * 86400. * 1000.).round() as i64;
    let Some(d) = chrono::DateTime::from_timestamp_millis(ms) else { return serial.to_string() };
    if serial.fract() == 0. { d.format("%Y-%m-%d").to_string() } else { d.format("%Y-%m-%d %H:%M:%S").to_string() }
}

pub fn parse_xlsx(bytes: &[u8]) -> Result<Vec<XlsxSheet>, String> {
    let re = regex::Regex::new(r"^xl/(workbook\.xml|styles\.xml|sharedStrings\.xml|_rels/workbook\.xml\.rels|worksheets/[^/]+\.xml)$").expect("regex");
    let files = open(bytes, |n| re.is_match(n))?;
    let wt = text_of(&files, "xl/workbook.xml").ok_or("xl/workbook.xml tidak ditemukan atau rusak")?;
    let wb = Document::parse(&wt).map_err(|_| "xl/workbook.xml tidak ditemukan atau rusak".to_string())?;
    let rels = rels_of(&files, "xl/_rels/workbook.xml.rels");
    let mut shared: Vec<String> = vec![];
    if let Some(st) = text_of(&files, "xl/sharedStrings.xml") {
        if let Ok(d) = Document::parse(&st) {
            for si in all(d.root(), "si") {
                shared.push(all(si, "t").into_iter().map(node_text).collect());
            }
        }
    }
    let mut date_xf: HashSet<usize> = HashSet::new();
    if let Some(st) = text_of(&files, "xl/styles.xml") {
        if let Ok(d) = Document::parse(&st) {
            let strip = regex::Regex::new(r#""[^"]*"|\[[^\]]*\]|\\."#).expect("regex");
            let dm = regex::Regex::new(r"(?i)[dmyhs]").expect("regex");
            let mut custom: HashSet<i64> = HashSet::new();
            for nf in all(d.root(), "numFmt") {
                let code = strip.replace_all(attr(Some(nf), "formatCode").unwrap_or(""), "").into_owned();
                if dm.is_match(&code) {
                    if let Some(id) = attr(Some(nf), "numFmtId").and_then(|v| v.parse().ok()) {
                        custom.insert(id);
                    }
                }
            }
            if let Some(xfs) = all(d.root(), "cellXfs").into_iter().next() {
                for (i, xf) in kids(xfs, "xf").into_iter().enumerate() {
                    let id: i64 = attr(Some(xf), "numFmtId").and_then(|v| v.parse().ok()).unwrap_or(-1);
                    if BUILTIN_DATE_FMT.contains(&id) || custom.contains(&id) {
                        date_xf.insert(i);
                    }
                }
            }
        }
    }
    let mut sheets = vec![];
    for sh in all(wb.root(), "sheet").into_iter().take(MAX_SHEETS) {
        if matches!(attr(Some(sh), "state"), Some("hidden") | Some("veryHidden")) {
            continue;
        }
        let name = attr(Some(sh), "name").unwrap_or("Sheet").to_string();
        let rel = attr_ns(sh, NS_R, "id").and_then(|id| rels.get(id));
        let text = rel.and_then(|r| text_of(&files, &resolve("xl/", &r.target)));
        let Some(doc) = text.as_deref().and_then(|t| Document::parse(t).ok()) else {
            sheets.push(XlsxSheet { name, rows: vec![], truncated: false });
            continue;
        };
        let mut rows: Vec<Vec<String>> = vec![];
        let mut truncated = false;
        let mut max_col = 0usize;
        for row in all(doc.root(), "row") {
            let Some(r) = attr(Some(row), "r").and_then(|v| v.parse::<usize>().ok()).filter(|r| *r >= 1).map(|r| r - 1) else { continue };
            if r >= MAX_ROWS {
                truncated = true;
                break;
            }
            if rows.len() <= r {
                rows.resize(r + 1, vec![]);
            }
            for c in kids(row, "c") {
                let ci = col_index(attr(Some(c), "r").unwrap_or(""));
                if ci < 0 {
                    continue;
                }
                let ci = ci as usize;
                if ci >= MAX_COLS {
                    truncated = true;
                    continue;
                }
                let t = attr(Some(c), "t");
                let v = kid(c, "v").map(node_text).unwrap_or_default();
                let f = kid(c, "f").map(node_text);
                let text = match t {
                    Some("s") => v.parse::<usize>().ok().and_then(|n| shared.get(n).cloned()).unwrap_or_default(),
                    Some("inlineStr") => all(c, "t").into_iter().map(node_text).collect(),
                    Some("b") => match v.as_str() {
                        "1" => "TRUE".into(),
                        "0" => "FALSE".into(),
                        _ => String::new(),
                    },
                    Some("str") | Some("e") => v,
                    _ if !v.is_empty() => {
                        let s: Option<usize> = attr(Some(c), "s").and_then(|x| x.parse().ok());
                        match v.parse::<f64>() {
                            Ok(n) if n.is_finite() && s.is_some_and(|s| date_xf.contains(&s)) => serial_to_text(n),
                            _ => v,
                        }
                    }
                    _ => f.map(|f| format!("={f}")).unwrap_or_default(),
                };
                let cells = &mut rows[r];
                if cells.len() <= ci {
                    cells.resize(ci + 1, String::new());
                }
                cells[ci] = text;
                max_col = max_col.max(ci + 1);
            }
        }
        for row in rows.iter_mut() {
            row.resize(max_col, String::new());
        }
        sheets.push(XlsxSheet { name, rows, truncated });
    }
    Ok(sheets)
}

// ====================== PPTX ======================

#[derive(Clone, Debug, PartialEq)]
pub struct PptxSlide {
    pub index: usize,
    pub title: String,
    pub paragraphs: Vec<String>,
    pub images: Vec<String>,
    pub tables: Vec<Vec<Vec<String>>>,
}
const MAX_SLIDES: usize = 200;

pub fn parse_pptx(bytes: &[u8]) -> Result<(Vec<PptxSlide>, bool), String> {
    let re = regex::Regex::new(r"^ppt/(presentation\.xml|_rels/presentation\.xml\.rels|slides/[^/]+\.xml|slides/_rels/[^/]+\.rels|media/[^/]+)$").expect("regex");
    let files = open(bytes, |n| re.is_match(n))?;
    let pt = text_of(&files, "ppt/presentation.xml").ok_or("ppt/presentation.xml tidak ditemukan atau rusak")?;
    let pres = Document::parse(&pt).map_err(|_| "ppt/presentation.xml tidak ditemukan atau rusak".to_string())?;
    let rels = rels_of(&files, "ppt/_rels/presentation.xml.rels");
    let mut budget = 0usize;
    let mut slides = vec![];
    let mut truncated = false;
    let text_of_p = |p: Node| -> String {
        let mut s = String::new();
        for c in p.children().filter(|c| c.is_element()) {
            match local(&c) {
                "r" | "fld" => s.push_str(&all(c, "t").into_iter().map(node_text).collect::<String>()),
                "br" => s.push('\n'),
                _ => {}
            }
        }
        s
    };
    let rids: Vec<String> = all(pres.root(), "sldId").into_iter().map(|s| attr_ns(s, NS_R, "id").unwrap_or("").to_string()).collect();
    for rid in rids {
        if slides.len() >= MAX_SLIDES {
            truncated = true;
            break;
        }
        let index = slides.len() + 1;
        let path = rels.get(&rid).map(|r| resolve("ppt/", &r.target)).unwrap_or_default();
        let text = if path.is_empty() { None } else { text_of(&files, &path) };
        let Some(doc) = text.as_deref().and_then(|t| Document::parse(t).ok()) else {
            slides.push(PptxSlide { index, title: String::new(), paragraphs: vec![], images: vec![], tables: vec![] });
            continue;
        };
        let dir = path.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
        let file = path.rsplit('/').next().unwrap_or("");
        let srels = rels_of(&files, &format!("{dir}_rels/{file}.rels"));
        let mut title = String::new();
        let mut paragraphs = vec![];
        for sp in all(doc.root(), "sp") {
            let ty = attr(all(sp, "ph").into_iter().next(), "type");
            let Some(tx) = kid(sp, "txBody") else { continue };
            let lines: Vec<String> = kids(tx, "p").into_iter().map(text_of_p).filter(|l| !l.trim().is_empty()).collect();
            if matches!(ty, Some("title") | Some("ctrTitle")) {
                if title.is_empty() {
                    title = lines.join(" ");
                }
            } else {
                paragraphs.extend(lines);
            }
        }
        let tables = all(doc.root(), "tbl")
            .into_iter()
            .map(|tbl| kids(tbl, "tr").into_iter().map(|tr| kids(tr, "tc").into_iter().map(|tc| all(tc, "p").into_iter().map(text_of_p).collect::<Vec<_>>().join(" ").trim().to_string()).collect()).collect())
            .collect();
        let mut images = vec![];
        for blip in all(doc.root(), "blip") {
            let Some(r) = blip.attributes().find(|a| a.name() == "embed").and_then(|a| srels.get(a.value())) else { continue };
            if r.external {
                continue;
            }
            if let Some(u) = image_url(&files, &resolve(&dir, &r.target), &mut budget) {
                images.push(u);
            }
        }
        slides.push(PptxSlide { index, title, paragraphs, images, tables });
    }
    Ok((slides, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(vec![]);
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            for (n, c) in entries {
                z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(c.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }
    #[test]
    fn docx() {
        let w = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Judul</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#;
        let r = parse_docx(&zip_of(&[("word/document.xml", w)])).unwrap();
        match &r.blocks[0] {
            DocxBlock::P { style, runs, .. } => {
                assert_eq!(style.as_deref(), Some("h1"));
                assert!(runs[0].bold);
                assert_eq!(runs[0].text, "Judul");
            }
            _ => panic!(),
        }
        assert!(matches!(&r.blocks[1], DocxBlock::Table { rows } if rows.len() == 1));
        assert!(parse_docx(b"bukan zip").is_err());
    }
    #[test]
    fn xlsx() {
        let wb = r#"<workbook xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="S1" r:id="rId1"/></sheets></workbook>"#;
        let rels = r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#;
        let ss = r#"<sst><si><t>halo</t></si></sst>"#;
        let sh = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1"><v>5</v></c></row><row r="2"><c r="B2" t="b"><v>1</v></c></row></sheetData></worksheet>"#;
        let s = parse_xlsx(&zip_of(&[("xl/workbook.xml", wb), ("xl/_rels/workbook.xml.rels", rels), ("xl/sharedStrings.xml", ss), ("xl/worksheets/sheet1.xml", sh)])).unwrap();
        assert_eq!(s[0].name, "S1");
        assert_eq!(s[0].rows, vec![vec!["halo".to_string(), "".into(), "5".into()], vec!["".into(), "TRUE".into(), "".into()]]);
        assert_eq!(col_index("AA3"), 26);
        assert_eq!(serial_to_text(45658.), "2025-01-01");
    }
    #[test]
    fn pptx() {
        let pres = r#"<p:presentation xmlns:p="p" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst></p:presentation>"#;
        let rels = r#"<Relationships><Relationship Id="rId2" Target="slides/slide1.xml"/></Relationships>"#;
        let sl = r#"<p:sld xmlns:p="p" xmlns:a="a"><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Judul</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:txBody><a:p><a:r><a:t>Isi</a:t></a:r></a:p></p:txBody></p:sp></p:sld>"#;
        let (s, _) = parse_pptx(&zip_of(&[("ppt/presentation.xml", pres), ("ppt/_rels/presentation.xml.rels", rels), ("ppt/slides/slide1.xml", sl)])).unwrap();
        assert_eq!(s[0].title, "Judul");
        assert_eq!(s[0].paragraphs, vec!["Isi"]);
    }
}

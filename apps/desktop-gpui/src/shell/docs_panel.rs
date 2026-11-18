//! Panel Dokumen — port DocPanel.tsx + simple-viewers.tsx + office-viewers.tsx (bagian yang bisa dirender GPUI): tab
//! berkas terbuka, toolbar (render/mentah, bungkus baris, salin isi/path, muat ulang, buka di aplikasi bawaan, tampilkan
//! di Explorer), dan penampil per jenis: Markdown, kode bernomor, gambar/SVG, CSV/TSV (tabel), JSON (diformat), HTML
//! (sumber + buka di peramban), DOCX/XLSX/PPTX/ipynb (pembaca best-effort), PDF/media/biner (info + buka di aplikasi
//! lain — GPUI tidak punya mesin PDF/media).
use crate::core::doc_kinds::{classify, decode_text, default_render, file_name, has_raw_mode, is_text_kind, parse_notebook, DocKind, NbOutput, MAX_TEXT_BYTES};
use crate::core::office::{parse_docx, parse_pptx, parse_xlsx, DocxBlock};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, markdown, ButtonVariant, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, AppContext, ClipboardItem, Context, Entity, FontWeight, Global, IntoElement, SharedString, Window};
use std::collections::{HashMap, HashSet};

const MAX_BYTES: u64 = 40 * 1024 * 1024;
const MAX_TABLE_ROWS: usize = 2000;

pub struct DocsState {
    pub open: Vec<String>,
    pub active: Option<String>,
}
struct Handle(Entity<DocsState>);
impl Global for Handle {}

impl DocsState {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let e = cx.new(|_| Self { open: vec![], active: None });
        cx.set_global(Handle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<Handle>().0.clone()
    }
    pub fn open_file(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.open.contains(&path) {
            self.open.push(path.clone());
        }
        self.active = Some(path);
        cx.notify();
    }
    fn close(&mut self, path: &str, cx: &mut Context<Self>) {
        self.open.retain(|p| p != path);
        if self.active.as_deref() == Some(path) {
            self.active = self.open.last().cloned();
        }
        cx.notify();
    }
}

struct Loaded {
    kind: DocKind,
    bytes: Result<std::sync::Arc<Vec<u8>>, String>,
    text: Option<(String, bool)>,
}

pub struct DocsPanel {
    state: Entity<DocsState>,
    cache: HashMap<String, Loaded>,
    raw: HashSet<String>,
    nowrap: HashSet<String>,
    sheet: HashMap<String, usize>,
}

impl DocsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = DocsState::global(cx);
        cx.observe(&state, |s, st, cx| {
            let open = st.read(cx).open.clone();
            s.cache.retain(|k, _| open.contains(k));
            cx.notify();
        })
        .detach();
        Self { state, cache: HashMap::new(), raw: HashSet::new(), nowrap: HashSet::new(), sheet: HashMap::new() }
    }

    fn load(path: &str) -> Loaded {
        let bytes = crate::files::read_limited(std::path::Path::new(path), MAX_BYTES).map(std::sync::Arc::new);
        let kind = classify(path, bytes.as_ref().ok().map(|b| b.as_slice()));
        let text = match &bytes {
            Ok(b) if is_text_kind(kind) => {
                let d = decode_text(b, MAX_TEXT_BYTES);
                Some((d.text, d.truncated))
            }
            _ => None,
        };
        Loaded { kind, bytes, text }
    }
}

fn note(t: &Theme, s: String) -> AnyElement {
    div().p(t.space(3)).small(t).text_color(t.colors.fg_muted).child(s).into_any_element()
}

fn code_view(t: &Theme, text: &str, wrap: bool) -> AnyElement {
    let lines: Vec<&str> = text.lines().collect();
    let w = lines.len().to_string().len().max(2);
    div()
        .flex()
        .flex_col()
        .p(t.space(3))
        .font_family(t.font_code.clone())
        .code(t)
        .children(lines.into_iter().take(50_000).enumerate().map(|(n, l)| {
            let line = if l.is_empty() { " ".to_string() } else { l.replace('\t', "    ") };
            div()
                .flex()
                .gap(t.space(3))
                .child(div().flex_none().text_color(t.colors.fg_subtle).child(format!("{:>w$}", n + 1, w = w)))
                .child(if wrap { div().flex_1().min_w_0().child(line) } else { div().whitespace_nowrap().child(line) })
        }))
        .into_any_element()
}

/// Parse CSV/TSV sederhana (tanda kutip ganda, pemisah koma/tab).
pub fn parse_delimited(text: &str, sep: char) -> Vec<Vec<String>> {
    let mut rows = vec![];
    let mut row = vec![];
    let mut cell = String::new();
    let mut quoted = false;
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if quoted {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    cell.push('"');
                    it.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() => quoted = true,
            c if c == sep => row.push(std::mem::take(&mut cell)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            c => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn table(t: &Theme, rows: &[Vec<String>], header: bool) -> AnyElement {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut grid = div().flex().flex_col().border_1().border_color(t.colors.border).rounded(t.rem(rem::RADIUS_MD)).small(t);
    for (ri, r) in rows.iter().take(MAX_TABLE_ROWS).enumerate() {
        let mut line = div().flex().when(ri > 0, |d| d.border_t_1().border_color(t.colors.border)).when(header && ri == 0, |d| d.bg(t.colors.segment).font_weight(FontWeight::SEMIBOLD));
        for ci in 0..cols {
            line = line.child(div().w(t.rem(9.)).flex_none().px(t.space(2)).py(t.space(1)).when(ci > 0, |d| d.border_l_1().border_color(t.colors.border)).truncate().child(r.get(ci).cloned().unwrap_or_default()));
        }
        grid = grid.child(line);
    }
    div().p(t.space(3)).child(grid).into_any_element()
}

fn data_img(url: &str, t: &Theme) -> Option<AnyElement> {
    crate::core::gradient::image_of(url).map(|img| gpui::img(img).max_w_full().max_h(t.rem(30.)).into_any_element())
}

fn docx_view(t: &Theme, blocks: &[DocxBlock]) -> Vec<AnyElement> {
    blocks
        .iter()
        .map(|b| match b {
            DocxBlock::P { style, list, runs } => {
                let mut p = div().flex().flex_wrap().items_baseline();
                p = match style.as_deref() {
                    Some("title") => p.typo(t, crate::theme::H1),
                    Some("h1") => p.typo(t, crate::theme::H2),
                    Some("h2") => p.typo(t, crate::theme::H3),
                    Some(_) => p.typo(t, crate::theme::H4),
                    None => p,
                };
                if let Some((lvl, _)) = list {
                    p = p.pl(t.space(4) * (*lvl as f32 + 1.)).child(div().mr(t.space(1)).child("•"));
                }
                for r in runs {
                    if let Some(img) = r.image.as_deref().and_then(|u| data_img(u, t)) {
                        p = p.child(img);
                    }
                    if !r.text.is_empty() {
                        let mut s = div().child(r.text.clone());
                        if r.bold {
                            s = s.font_weight(FontWeight::BOLD);
                        }
                        if r.italic {
                            s = s.italic();
                        }
                        if r.underline || r.href.is_some() {
                            s = s.underline();
                        }
                        if r.strike {
                            s = s.line_through();
                        }
                        if r.href.is_some() {
                            s = s.text_color(t.colors.accent);
                        }
                        p = p.child(s);
                    }
                }
                p.min_h(t.rem(1.)).into_any_element()
            }
            DocxBlock::Table { rows } => {
                let mut g = div().flex().flex_col().border_1().border_color(t.colors.border);
                for r in rows {
                    let mut line = div().flex().border_b_1().border_color(t.colors.border);
                    for c in r {
                        line = line.child(div().flex_1().min_w_0().p(t.space(1)).border_r_1().border_color(t.colors.border).flex().flex_col().children(docx_view(t, c)));
                    }
                    g = g.child(line);
                }
                g.into_any_element()
            }
        })
        .collect()
}

fn human(n: usize) -> String {
    if n >= 1 << 20 {
        format!("{:.1} MB", n as f64 / (1 << 20) as f64)
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.)
    } else {
        format!("{n} B")
    }
}

impl DocsPanel {
    fn body(&mut self, t: &Theme, i: &I18n, path: &str, cx: &mut Context<Self>) -> AnyElement {
        let raw = self.raw.contains(path);
        let wrap = !self.nowrap.contains(path);
        let l = &self.cache[path];
        let bytes = match &l.bytes {
            Err(e) => return note(t, i.tp("doc.error", &[("error", e)])),
            Ok(b) => b.clone(),
        };
        let kind = l.kind;
        let truncated = l.text.as_ref().is_some_and(|x| x.1);
        let text = l.text.as_ref().map(|x| x.0.clone()).unwrap_or_default();
        let trunc = truncated.then(|| note(t, i.t("doc.truncated")));
        let el: AnyElement = if (raw || kind == DocKind::Code) && is_text_kind(kind) {
            code_view(t, &text, wrap)
        } else {
            match kind {
                DocKind::Markdown => div().p(t.space(4)).child(markdown(&text, t)).into_any_element(),
                DocKind::Image | DocKind::Svg => div()
                    .p(t.space(3))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(t.space(2))
                    .child(gpui::img(std::path::PathBuf::from(path)).max_w_full().max_h(t.rem(40.)))
                    .child(div().small(t).text_color(t.colors.fg_subtle).child(i.tp("doc.size", &[("size", &human(bytes.len()))])))
                    .into_any_element(),
                DocKind::Csv | DocKind::Tsv => {
                    let rows = parse_delimited(&text, if kind == DocKind::Tsv { '\t' } else { ',' });
                    if rows.is_empty() {
                        note(t, i.t("doc.tableEmpty"))
                    } else {
                        let info = i.tp("doc.tableInfo", &[("rows", &rows.len().to_string()), ("cols", &rows.iter().map(|r| r.len()).max().unwrap_or(0).to_string())]);
                        let clipped = rows.len() > MAX_TABLE_ROWS;
                        div().flex().flex_col().child(note(t, info)).child(table(t, &rows, true)).when(clipped, |d| d.child(note(t, i.tp("doc.tableClipped", &[("max", &MAX_TABLE_ROWS.to_string())])))).into_any_element()
                    }
                }
                DocKind::Json => match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(v) => code_view(t, &serde_json::to_string_pretty(&v).unwrap_or_default(), wrap),
                    Err(e) => div().child(note(t, i.tp("doc.jsonInvalid", &[("error", &e.to_string())]))).child(code_view(t, &text, wrap)).into_any_element(),
                },
                DocKind::Html => {
                    let p = path.to_string();
                    div()
                        .flex()
                        .flex_col()
                        .child(div().p(t.space(3)).child(button("doc-html-open").size(ControlSize::Sm).label(i.t("doc.openInBrowser")).on_click(move |_, _, cx| cx.open_url(&format!("file:///{}", p.replace('\\', "/"))))))
                        .child(code_view(t, &text, wrap))
                        .into_any_element()
                }
                DocKind::Notebook => match parse_notebook(&text) {
                    Err(e) => note(t, e),
                    Ok(nb) => {
                        let mut col = div().flex().flex_col().gap(t.space(3)).p(t.space(3)).child(div().small(t).text_color(t.colors.fg_muted).child(i.t("doc.nbNote")));
                        for c in nb.cells {
                            let mut cell = div().flex().flex_col().gap(t.space(1));
                            cell = match c.kind {
                                "markdown" => cell.child(markdown(&c.source, t)),
                                _ => cell
                                    .child(div().small(t).text_color(t.colors.fg_subtle).child(format!("[{}]", c.execution_count.map(|n| n.to_string()).unwrap_or_else(|| " ".into()))))
                                    .child(div().rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.segment).child(code_view(t, &c.source, true))),
                            };
                            for o in c.outputs {
                                cell = cell.child(match o {
                                    NbOutput::Text { text, stderr } => div().code(t).font_family(t.font_code.clone()).when(stderr, |d| d.text_color(t.colors.danger)).child(text).into_any_element(),
                                    NbOutput::Image { src } => data_img(&src, t).unwrap_or_else(|| div().into_any_element()),
                                    NbOutput::Error { name, message, traceback } => div().code(t).font_family(t.font_code.clone()).text_color(t.colors.danger).child(format!("{name}: {message}\n{traceback}")).into_any_element(),
                                    NbOutput::Skipped { reason } => note(t, reason),
                                });
                            }
                            col = col.child(cell);
                        }
                        if nb.truncated {
                            col = col.child(note(t, i.t("doc.truncated")));
                        }
                        col.into_any_element()
                    }
                },
                DocKind::Docx => match parse_docx(&bytes) {
                    Err(e) => note(t, i.tp("doc.officeFailed", &[("name", file_name(path)), ("error", &e)])),
                    Ok(r) => div()
                        .flex()
                        .flex_col()
                        .gap(t.space(2))
                        .p(t.space(4))
                        .child(div().small(t).text_color(t.colors.fg_muted).child(i.t("doc.officeBestEffort")))
                        .children(docx_view(t, &r.blocks))
                        .when(r.truncated, |d| d.child(note(t, i.t("doc.officeTruncated"))))
                        .into_any_element(),
                },
                DocKind::Xlsx => match parse_xlsx(&bytes) {
                    Err(e) => note(t, i.tp("doc.officeFailed", &[("name", file_name(path)), ("error", &e)])),
                    Ok(sheets) => {
                        let cur = self.sheet.get(path).copied().unwrap_or(0).min(sheets.len().saturating_sub(1));
                        let mut tabs = div().flex().flex_wrap().gap(t.space(1)).p(t.space(2));
                        for (n, s) in sheets.iter().enumerate() {
                            let p = path.to_string();
                            tabs = tabs.child(button(SharedString::from(format!("sheet-{n}"))).size(ControlSize::Sm).pressed(n == cur).label(s.name.clone()).on_click(cx.listener(move |me, _, _, cx| {
                                me.sheet.insert(p.clone(), n);
                                cx.notify();
                            })));
                        }
                        let body = match sheets.get(cur) {
                            Some(s) if !s.rows.is_empty() => div().child(table(t, &s.rows, false)).when(s.truncated, |d| d.child(note(t, i.t("doc.sheetClipped")))).into_any_element(),
                            _ => note(t, i.t("doc.sheetEmpty")),
                        };
                        div().flex().flex_col().child(note(t, i.t("doc.xlsxNote"))).child(tabs).child(body).into_any_element()
                    }
                },
                DocKind::Pptx => match parse_pptx(&bytes) {
                    Err(e) => note(t, i.tp("doc.officeFailed", &[("name", file_name(path)), ("error", &e)])),
                    Ok((slides, trunc)) => {
                        let mut col = div().flex().flex_col().gap(t.space(3)).p(t.space(3)).child(div().small(t).text_color(t.colors.fg_muted).child(i.t("doc.pptxNote")));
                        for s in slides {
                            let mut card = div().flex().flex_col().gap(t.space(1)).p(t.space(3)).border_1().border_color(t.colors.border).rounded(t.rem(rem::RADIUS_LG));
                            card = card.child(div().small(t).text_color(t.colors.fg_subtle).child(i.tp("doc.slideLabel", &[("n", &s.index.to_string())])));
                            if !s.title.is_empty() {
                                card = card.child(div().typo(t, crate::theme::H4).child(s.title));
                            }
                            for p in s.paragraphs {
                                card = card.child(div().child(p));
                            }
                            for tb in s.tables {
                                card = card.child(table(t, &tb, false));
                            }
                            for u in s.images {
                                if let Some(img) = data_img(&u, t) {
                                    card = card.child(img);
                                }
                            }
                            col = col.child(card);
                        }
                        if trunc {
                            col = col.child(note(t, i.t("doc.officeTruncated")));
                        }
                        col.into_any_element()
                    }
                },
                _ => {
                    let p = path.to_string();
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(t.space(2))
                        .p(t.space(5))
                        .child(icon("file").size(IconSize::Lg))
                        .child(div().text_color(t.colors.fg_muted).child(i.tp("doc.binary", &[("name", file_name(path))])))
                        .child(button("doc-open-ext").size(ControlSize::Sm).label(i.t("doc.openExternal")).on_click(move |_, _, cx| cx.open_with_system(std::path::Path::new(&p))))
                        .into_any_element()
                }
            }
        };
        div().flex().flex_col().children(trunc).child(el).into_any_element()
    }
}

impl Render for DocsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let st = self.state.read(cx);
        let (open, active) = (st.open.clone(), st.active.clone());
        let Some(active) = active else {
            return div()
                .flex()
                .flex_col()
                .size_full()
                .items_center()
                .justify_center()
                .gap(t.space(1))
                .p(t.space(4))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("empty.p3.title")))
                .child(div().text_color(t.colors.fg_muted).child(i.t("empty.p3.body")))
                .into_any_element();
        };
        if !self.cache.contains_key(&active) {
            let l = Self::load(&active);
            if !default_render(l.kind) {
                self.raw.remove(&active);
            }
            self.cache.insert(active.clone(), l);
        }
        let kind = self.cache[&active].kind;

        let mut tabs = div().id("doc-tabs").flex().flex_none().items_center().gap(t.space(1)).px(t.space(2)).h(t.rem(rem::TITLEBAR_HEIGHT)).border_b_1().border_color(t.colors.border).overflow_x_scroll();
        for p in &open {
            let name = file_name(p).to_string();
            let cur = *p == active;
            let (p1, p2) = (p.clone(), p.clone());
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("doc-{p}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(t.space(1))
                    .pl(t.space(2))
                    .h(t.rem(rem::CONTROL_SM))
                    .rounded(t.rem(rem::RADIUS_SM))
                    .cursor_pointer()
                    .text_color(if cur { t.colors.fg } else { t.colors.fg_muted })
                    .when(cur, |d| d.bg(t.colors.active))
                    .hover(|s| s.bg(t.colors.hover))
                    .tooltip(crate::ui::tooltip::text_tooltip(p.clone().into()))
                    .on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.open_file(p1.clone(), cx))))
                    .child(crate::shell::file_icons::file_type_icon(&t, &name, false, false, rem::ICON_SM))
                    .child(div().max_w(t.rem(10.)).truncate().small(&t).child(name.clone()))
                    .child(
                        icon_button(SharedString::from(format!("docx-{p}")), "close", i.tp("doc.closeTab", &[("name", &name)]))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.close(&p2, cx)))),
                    ),
            );
        }

        let raw = self.raw.contains(&active);
        let wrap = !self.nowrap.contains(&active);
        let (a1, a2, a3, a4, a5, a6, a7) = (active.clone(), active.clone(), active.clone(), active.clone(), active.clone(), active.clone(), active.clone());
        let mut bar = div().flex().flex_none().items_center().gap(t.space(1)).px(t.space(2)).py(t.space(1)).border_b_1().border_color(t.colors.border);
        if has_raw_mode(kind) {
            bar = bar
                .child(button("doc-render").size(ControlSize::Sm).variant(ButtonVariant::Ghost).pressed(!raw).label(i.t("doc.render")).on_click(cx.listener(move |s, _, _, cx| {
                    s.raw.remove(&a1);
                    cx.notify();
                })))
                .child(button("doc-raw").size(ControlSize::Sm).variant(ButtonVariant::Ghost).pressed(raw).label(i.t("doc.raw")).on_click(cx.listener(move |s, _, _, cx| {
                    s.raw.insert(a2.clone());
                    cx.notify();
                })));
        }
        if is_text_kind(kind) {
            bar = bar
                .child(icon_button("doc-wrap", "wrap", i.t("doc.wrap")).size(ControlSize::Sm).pressed(wrap).on_click(cx.listener(move |s, _, _, cx| {
                    if !s.nowrap.remove(&a3) {
                        s.nowrap.insert(a3.clone());
                    }
                    cx.notify();
                })))
                .child(icon_button("doc-copy", "copy", i.t("doc.copy")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| {
                    if let Some((txt, _)) = s.cache.get(&a4).and_then(|l| l.text.clone()) {
                        cx.write_to_clipboard(ClipboardItem::new_string(txt));
                    }
                })));
        }
        bar = bar
            .child(div().flex_1())
            .child(icon_button("doc-reload", "refresh", i.t("doc.reload")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| {
                s.cache.remove(&a5);
                cx.notify();
            })))
            .child(icon_button("doc-copypath", "link", i.t("doc.copyPath")).size(ControlSize::Sm).on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(a6.clone()))))
            .child(icon_button("doc-reveal", "folder_open", i.t("doc.openWith.reveal")).size(ControlSize::Sm).on_click({
                let p = active.clone();
                move |_, _, cx| cx.reveal_path(std::path::Path::new(&p))
            }))
            .child(icon_button("doc-ext", "external_link", i.t("doc.openExternal")).size(ControlSize::Sm).on_click(move |_, _, cx| cx.open_with_system(std::path::Path::new(&a7))));

        let body = self.body(&t, &i, &active, cx);
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(tabs)
            .child(bar)
            .child(div().id("doc-body").flex_1().min_h_0().overflow_scroll().child(body))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn csv() {
        let r = super::parse_delimited("a,\"b,c\",\"d\"\"e\"\r\n1,2\n", ',');
        assert_eq!(r, vec![vec!["a", "b,c", "d\"e"], vec!["1", "2"]]);
        assert_eq!(super::human(2048), "2.0 KB");
    }
}

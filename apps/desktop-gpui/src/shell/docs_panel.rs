//! Panel Dokumen — port DocPanel.tsx + simple-viewers.tsx + office-viewers.tsx (bagian yang bisa dirender GPUI): tab
//! berkas terbuka, toolbar (render/mentah, bungkus baris, salin isi/path, muat ulang, buka di aplikasi bawaan, tampilkan
//! di Explorer), dan penampil per jenis: Markdown, kode bernomor, gambar/SVG, CSV/TSV (tabel), JSON (diformat), HTML
//! (sumber + buka di peramban), DOCX/XLSX/PPTX/ipynb (pembaca best-effort), PDF/media/biner (info + buka di aplikasi
//! lain — GPUI tidak punya mesin PDF/media).
use crate::core::doc_kinds::{classify, decode_text, default_render, file_name, has_raw_mode, is_text_kind, parse_notebook, DocKind, NbOutput, MAX_TEXT_BYTES};
use crate::core::patch::{drop_hunk, format_hunks, parse_patch, reverse_apply, revert_hunk, Hunk};
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
    /// Diff tinjauan per berkas (ADR-075): ada → panel menampilkan DiffViewer.
    pub diffs: HashMap<String, String>,
}
struct Handle(Entity<DocsState>);
impl Global for Handle {}

impl DocsState {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let e = cx.new(|_| Self { open: vec![], active: None, diffs: HashMap::new() });
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
    pub fn open_diff(&mut self, path: String, patch: String, cx: &mut Context<Self>) {
        self.diffs.insert(path.clone(), patch);
        self.open_file(path, cx);
    }
    pub fn set_diff(&mut self, path: &str, patch: Option<String>, cx: &mut Context<Self>) {
        match patch {
            Some(p) => self.diffs.insert(path.to_string(), p),
            None => self.diffs.remove(path),
        };
        cx.notify();
    }
    fn close(&mut self, path: &str, cx: &mut Context<Self>) {
        self.diffs.remove(path);
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
    diff_at: HashMap<String, usize>,
    confirm_all: bool,
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
        Self { state, cache: HashMap::new(), raw: HashSet::new(), nowrap: HashSet::new(), sheet: HashMap::new(), diff_at: HashMap::new(), confirm_all: false }
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

        let diff = self.state.read(cx).diffs.get(&active).cloned();
        let (bar, body) = match diff {
            Some(d) => {
                let (b, body) = self.diff_view(&t, &i, &active, &d, cx);
                (b.into_any_element(), body)
            }
            None => (bar.into_any_element(), self.body(&t, &i, &active, cx)),
        };
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

// ---- DiffViewer (port DiffViewer.tsx): tinjau perubahan agent per bagian. "Terima" hanya menandai selesai;
// "Tolak" mengembalikan bagian itu ke isi semula dan langsung menyimpan berkas (bisa diurungkan dari toast). Sisi
// "sebelum" dihitung dengan membalik diff terhadap isi berkas saat ini. GPUI tidak punya editor diff Monaco, jadi
// tiap bagian ditampilkan sebagai blok unified (bagian aktif ditandai) — isinya sama, tata letaknya tidak berdampingan.
impl DocsPanel {
    /// Isi berkas saat ini bila aman ditulis ulang (UTF-8 utuh, tidak terpotong).
    fn writable_text(&self, path: &str) -> Option<String> {
        let l = self.cache.get(path)?;
        let (txt, trunc) = l.text.clone()?;
        let bytes = l.bytes.as_ref().ok()?;
        (!trunc && std::str::from_utf8(bytes).is_ok()).then_some(txt)
    }

    fn write_file(&mut self, path: &str, text: &str, cx: &mut Context<Self>) -> bool {
        let i = cx.global::<I18n>().clone();
        // konflik: berkas berubah di disk sejak dimuat
        let disk = std::fs::read(path).ok();
        let loaded = self.cache.get(path).and_then(|l| l.bytes.as_ref().ok().cloned());
        if disk.as_deref() != loaded.as_deref().map(|b| b.as_slice()) {
            crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(i.t("diff.conflict")) }, cx);
            return false;
        }
        match std::fs::write(path, text.as_bytes()) {
            Ok(()) => {
                self.cache.insert(path.to_string(), Self::load(path));
                true
            }
            Err(e) => {
                crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(i.tp("diff.saveError", &[("error", &e.to_string())])) }, cx);
                false
            }
        }
    }

    fn finish(&mut self, path: &str, rest: Vec<Hunk>, cx: &mut Context<Self>) {
        let p = path.to_string();
        if rest.is_empty() {
            self.state.update(cx, |d, cx| d.set_diff(&p, None, cx));
            let title = cx.global::<I18n>().t("diff.done");
            crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Success, ..crate::ui::ToastInput::new(title) }, cx);
        } else {
            let f = format_hunks(&rest);
            self.state.update(cx, |d, cx| d.set_diff(&p, Some(f), cx));
        }
    }

    fn hunks_of(&self, path: &str, cx: &App) -> Vec<Hunk> {
        self.state.read(cx).diffs.get(path).map(|d| parse_patch(d)).unwrap_or_default()
    }

    fn accept_hunk(&mut self, path: &str, cx: &mut Context<Self>) {
        let hunks = self.hunks_of(path, cx);
        let at = self.diff_at.get(path).copied().unwrap_or(0).min(hunks.len().saturating_sub(1));
        if hunks.is_empty() {
            return;
        }
        self.finish(path, drop_hunk(&hunks, at, false), cx);
    }

    fn reject_hunk(&mut self, path: &str, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        let hunks = self.hunks_of(path, cx);
        let at = self.diff_at.get(path).copied().unwrap_or(0).min(hunks.len().saturating_sub(1));
        let (Some(h), Some(cur)) = (hunks.get(at), self.writable_text(path)) else { return };
        let Some(next) = revert_hunk(&cur, h) else {
            crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Warning, ..crate::ui::ToastInput::new(i.t("diff.stale")) }, cx);
            return;
        };
        let prev_patch = self.state.read(cx).diffs.get(path).cloned();
        if !self.write_file(path, &next, cx) {
            return;
        }
        self.finish(path, drop_hunk(&hunks, at, true), cx);
        let me = cx.entity().downgrade();
        let p = path.to_string();
        crate::ui::Toasts::show(
            crate::ui::ToastInput {
                action: Some(crate::ui::ToastAction {
                    label: i.t("diff.undo").into(),
                    on_click: std::rc::Rc::new(move |_, cx| {
                        let (p, cur, pp) = (p.clone(), cur.clone(), prev_patch.clone());
                        me.update(cx, |s, cx| {
                            if s.write_file(&p, &cur, cx) {
                                s.state.update(cx, |d, cx| d.set_diff(&p, pp, cx));
                            }
                        })
                        .ok();
                    }),
                }),
                ..crate::ui::ToastInput::new(i.t("diff.rejected"))
            },
            cx,
        );
    }

    fn reject_all(&mut self, path: &str, cx: &mut Context<Self>) {
        self.confirm_all = false;
        let hunks = self.hunks_of(path, cx);
        let Some(before) = self.writable_text(path).and_then(|c| reverse_apply(&c, &hunks)) else { return };
        if self.write_file(path, &before, cx) {
            self.state.update(cx, |d, cx| d.set_diff(path, None, cx));
            let title = cx.global::<I18n>().t("diff.rejectedAll");
            crate::ui::Toasts::show(crate::ui::ToastInput::new(title), cx);
        }
        cx.notify();
    }

    fn diff_view(&mut self, t: &Theme, i: &I18n, path: &str, patch: &str, cx: &mut Context<Self>) -> (gpui::Div, AnyElement) {
        let hunks = parse_patch(patch);
        let current = self.cache.get(path).and_then(|l| l.text.as_ref().map(|x| x.0.clone())).unwrap_or_default();
        let before = reverse_apply(&current, &hunks);
        let at = self.diff_at.get(path).copied().unwrap_or(0).min(hunks.len().saturating_sub(1));
        let writable = self.writable_text(path).is_some();
        let reject_reason = (!writable).then(|| i.t("diff.cantWrite"));
        let can_reject = before.is_some() && writable;
        let name = file_name(path).to_string();
        let p = |s: &str| -> SharedString { SharedString::from(format!("{s}-{path}")) };
        let (p1, p2, p3, p4, p5, p6, p7) = (path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string());
        let n = hunks.len();
        let bar = div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(t.space(1))
            .px(t.space(2))
            .py(t.space(1))
            .border_b_1()
            .border_color(t.colors.border)
            .child(div().small(t).text_color(t.colors.fg_muted).child(if n > 0 { i.tp("diff.pos", &[("i", &(at + 1).to_string()), ("n", &n.to_string())]) } else { i.t("diff.none") }))
            .child(icon_button(p("dprev"), "chevron_up", i.t("diff.prev")).size(ControlSize::Sm).disabled(at == 0).on_click(cx.listener(move |s, _, _, cx| {
                let e = s.diff_at.entry(p1.clone()).or_insert(0);
                *e = e.saturating_sub(1);
                cx.notify();
            })))
            .child(icon_button(p("dnext"), "chevron_down", i.t("diff.next")).size(ControlSize::Sm).disabled(at + 1 >= n).on_click(cx.listener(move |s, _, _, cx| {
                s.diff_at.insert(p2.clone(), at + 1);
                cx.notify();
            })))
            .child(div().flex_1())
            .child(button(p("dacc")).size(ControlSize::Sm).variant(ButtonVariant::Solid).disabled(n == 0).child(icon("check").size(IconSize::Sm)).label(i.t("diff.accept")).on_click(cx.listener(move |s, _, _, cx| s.accept_hunk(&p3, cx))))
            .child(button(p("drej")).size(ControlSize::Sm).disabled(n == 0 || !can_reject).child(icon("undo").size(IconSize::Sm)).label(i.t("diff.reject")).on_click(cx.listener(move |s, _, _, cx| s.reject_hunk(&p4, cx))))
            .child(button(p("daccall")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("diff.acceptAll")).on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.set_diff(&p5, None, cx)))))
            .child(button(p("drejall")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).disabled(!can_reject).label(i.t("diff.rejectAll")).on_click(cx.listener(|s, _, _, cx| {
                s.confirm_all = true;
                cx.notify();
            })))
            .child(icon_button(p("dclose"), "close", i.t("diff.close")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.set_diff(&p6, None, cx)))));

        let mut body = div().flex().flex_col().gap(t.space(2)).p(t.space(3));
        if let (Some(r), true) = (&reject_reason, before.is_some()) {
            body = body.child(note(t, r.clone()));
        }
        if before.is_none() {
            body = body.child(note(t, i.t("diff.mismatch")));
        } else {
            for (k, h) in hunks.iter().enumerate() {
                let rows = crate::core::tool_view::parse_diff(&format_hunks(std::slice::from_ref(h)));
                let pp = path.to_string();
                body = body.child(
                    div()
                        .id(SharedString::from(format!("dh-{k}-{path}")))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .border_1()
                        .border_color(if k == at { t.colors.accent } else { t.colors.border_soft })
                        .cursor_pointer()
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.diff_at.insert(pp.clone(), k);
                            cx.notify();
                        }))
                        .child(super::chat_panel::chat_parts::diff_block(t, i, SharedString::from(format!("dhb-{k}-{path}")), &rows)),
                );
            }
        }
        if self.confirm_all {
            body = body.child(
                crate::ui::dialog(i.t("diff.rejectAll.title"))
                    .description(i.tp("diff.rejectAll.body", &[("n", &n.to_string()), ("name", &name)]))
                    .on_dismiss(cx.listener(|s, _, _, cx| {
                        s.confirm_all = false;
                        cx.notify();
                    }))
                    .footer(
                        div()
                            .flex()
                            .justify_end()
                            .gap(t.space(2))
                            .child(button("drejall-no").label(i.t("ui.cancel")).on_click(cx.listener(|s, _, _, cx| {
                                s.confirm_all = false;
                                cx.notify();
                            })))
                            .child(button("drejall-yes").variant(ButtonVariant::Danger).label(i.t("diff.rejectAll")).on_click(cx.listener(move |s, _, _, cx| s.reject_all(&p7, cx)))),
                    ),
            );
        }
        (bar, body.into_any_element())
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

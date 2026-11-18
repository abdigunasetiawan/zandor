//! Markdown — port shell/Markdown.tsx (subset GFM): judul, paragraf, tebal/miring/kode/tautan inline, blok kode,
//! daftar (berurut/tidak, tugas), kutipan, garis, tabel sederhana.
use crate::theme::{rem, Theme, FONT_CODE, H1, H2, H3, H4};
use crate::ui::TypeExt;
use gpui::{div, prelude::*, AnyElement, FontStyle, FontWeight, HighlightStyle, IntoElement, StyledText, UnderlineStyle};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Default)]
struct Inline {
    text: String,
    hl: Vec<(Range<usize>, HighlightStyle)>,
    bold: usize,
    italic: usize,
    strike: usize,
    link: usize,
    /// URL tautan yang sedang terbuka (hanya http/https yang bisa diklik).
    href: Option<String>,
    links: Vec<(Range<usize>, String)>,
    /// Penghitung blok untuk id elemen yang stabil antar frame.
    seq: usize,
}

/// Tautan yang diklik di respons model: menu kecil (buka di Browser Zandor / eksternal / salin) di posisi mouse.
/// Port ChatLink.tsx; digambar oleh Root.
#[derive(Clone, Default)]
pub struct LinkMenu(pub Option<(String, gpui::Point<gpui::Pixels>)>);
impl gpui::Global for LinkMenu {}

/// Port isWebLink / SAFE_LINK: hanya http(s).
/// Pecah teks menjadi potongan (teks, apakah URL http/https polos).
pub fn split_autolinks(s: &str) -> Vec<(&str, bool)> {
    let mut out = vec![];
    let mut rest = s;
    while let Some(k) = rest.find("http").filter(|_| !rest.is_empty()) {
        let cand = &rest[k..];
        let lower = cand.get(..8).unwrap_or(cand).to_lowercase();
        let ok_start = k == 0 || !rest[..k].chars().last().is_some_and(|c| c.is_alphanumeric());
        if ok_start && (lower.starts_with("https://") || lower.starts_with("http://")) {
            let end = cand.find(|c: char| c.is_whitespace() || c == '<' || c == '>' || c == '"').unwrap_or(cand.len());
            if k > 0 {
                out.push((&rest[..k], false));
            }
            out.push((&cand[..end], true));
            rest = &cand[end..];
        } else {
            let step = k + 4;
            out.push((&rest[..step], false));
            rest = &rest[step..];
        }
    }
    if !rest.is_empty() {
        out.push((rest, false));
    }
    out
}

pub fn is_web_link(href: &str) -> bool {
    let h = href.trim().to_lowercase();
    h.starts_with("http://") || h.starts_with("https://")
}

impl Inline {
    fn push(&mut self, s: &str, t: &Theme, code: bool) {
        let start = self.text.len();
        self.text.push_str(s);
        let r = start..self.text.len();
        if r.is_empty() {
            return;
        }
        let mut h = HighlightStyle::default();
        let mut any = false;
        if self.bold > 0 {
            h.font_weight = Some(FontWeight::SEMIBOLD);
            any = true;
        }
        if self.italic > 0 {
            h.font_style = Some(FontStyle::Italic);
            any = true;
        }
        if self.strike > 0 {
            h.strikethrough = Some(gpui::StrikethroughStyle { thickness: gpui::px(1.), color: None });
            any = true;
        }
        if let Some(u) = self.href.clone().filter(|_| self.link > 0) {
            self.links.push((r.clone(), u));
        }
        if self.link > 0 {
            h.color = Some(t.colors.accent);
            h.underline = Some(UnderlineStyle { thickness: gpui::px(1.), color: Some(t.colors.accent), wavy: false });
            any = true;
        }
        if code {
            h.background_color = Some(t.colors.raised);
            h.color = Some(t.colors.code_string);
            any = true;
        }
        if any {
            self.hl.push((r, h));
        }
    }
    fn take(&mut self) -> Option<AnyElement> {
        if self.text.trim().is_empty() {
            self.text.clear();
            self.hl.clear();
            self.links.clear();
            return None;
        }
        let text = std::mem::take(&mut self.text);
        let hl = std::mem::take(&mut self.hl);
        let links = std::mem::take(&mut self.links);
        self.seq += 1;
        if links.is_empty() {
            return Some(StyledText::new(text).with_highlights(hl).into_any_element());
        }
        use std::hash::{Hash, Hasher};
        let mut hs = std::collections::hash_map::DefaultHasher::new();
        (self.seq, &text).hash(&mut hs);
        let id = gpui::ElementId::Name(format!("md-link-{:x}", hs.finish()).into());
        let (ranges, urls): (Vec<_>, Vec<_>) = links.into_iter().unzip();
        Some(
            gpui::InteractiveText::new(id, StyledText::new(text).with_highlights(hl))
                .on_click(ranges, move |ix, window, cx| {
                    if let Some(u) = urls.get(ix) {
                        cx.set_global(LinkMenu(Some((u.clone(), window.mouse_position()))));
                        cx.refresh_windows();
                    }
                })
                .into_any_element(),
        )
    }
}

enum Block {
    Container { kind: Container, children: Vec<AnyElement> },
}
enum Container {
    Root,
    Quote,
    List(Option<u64>),
    Item(String),
}

pub fn markdown(src: &str, t: &Theme, i: &crate::i18n::I18n) -> AnyElement {
    let opts = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES | Options::ENABLE_MATH;
    let mut stack: Vec<Block> = vec![Block::Container { kind: Container::Root, children: vec![] }];
    let mut inl = Inline::default();
    let mut heading: Option<HeadingLevel> = None;
    let mut code: Option<(String, String)> = None;
    let mut table_row: Vec<AnyElement> = vec![];
    let mut table_rows: Vec<(bool, Vec<AnyElement>)> = vec![];
    // teks polos sel untuk salin/simpan tabel
    let mut text_row: Vec<String> = vec![];
    let mut text_rows: Vec<Vec<String>> = vec![];
    let mut in_head = false;

    fn push_child(stack: &mut [Block], el: AnyElement) {
        if let Some(Block::Container { children, .. }) = stack.last_mut() {
            children.push(el);
        }
    }
    let flush = |inl: &mut Inline, stack: &mut Vec<Block>, heading: Option<HeadingLevel>| {
        if let Some(st) = inl.take() {
            let mut d = div().w_full();
            d = match heading {
                Some(HeadingLevel::H1) => d.typo(t, H1),
                Some(HeadingLevel::H2) => d.typo(t, H2),
                Some(HeadingLevel::H3) => d.typo(t, H3),
                Some(_) => d.typo(t, H4),
                None => d,
            };
            push_child(stack, d.child(st).into_any_element());
        }
    };

    for ev in Parser::new_ext(src, opts) {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    flush(&mut inl, &mut stack, None);
                    heading = Some(level);
                }
                Tag::Paragraph => flush(&mut inl, &mut stack, None),
                Tag::Strong => inl.bold += 1,
                Tag::Emphasis => inl.italic += 1,
                Tag::Strikethrough => inl.strike += 1,
                Tag::Link { dest_url, .. } => {
                    inl.link += 1;
                    inl.href = is_web_link(&dest_url).then(|| dest_url.to_string());
                }
                Tag::CodeBlock(k) => {
                    flush(&mut inl, &mut stack, None);
                    let lang = match k {
                        CodeBlockKind::Fenced(l) => l.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    code = Some((lang, String::new()));
                }
                Tag::BlockQuote(_) => {
                    flush(&mut inl, &mut stack, None);
                    stack.push(Block::Container { kind: Container::Quote, children: vec![] });
                }
                Tag::List(start) => {
                    flush(&mut inl, &mut stack, None);
                    stack.push(Block::Container { kind: Container::List(start), children: vec![] });
                }
                Tag::Item => {
                    flush(&mut inl, &mut stack, None);
                    let n = match stack.last() {
                        Some(Block::Container { kind: Container::List(Some(s)), children }) => format!("{}.", s + children.len() as u64),
                        _ => "•".into(),
                    };
                    stack.push(Block::Container { kind: Container::Item(n), children: vec![] });
                }
                Tag::TableHead => in_head = true,
                Tag::TableRow | Tag::Table(_) => {}
                Tag::TableCell => {}
                _ => {}
            },
            Event::End(end) => match end {
                TagEnd::Heading(_) => {
                    flush(&mut inl, &mut stack, heading);
                    heading = None;
                }
                TagEnd::Paragraph => flush(&mut inl, &mut stack, None),
                TagEnd::Strong => inl.bold = inl.bold.saturating_sub(1),
                TagEnd::Emphasis => inl.italic = inl.italic.saturating_sub(1),
                TagEnd::Strikethrough => inl.strike = inl.strike.saturating_sub(1),
                TagEnd::Link => {
                    inl.link = inl.link.saturating_sub(1);
                    inl.href = None;
                }
                TagEnd::CodeBlock => {
                    if let Some((lang, body)) = code.take() {
                        let raw = body.trim_end_matches('\n').to_string();
                        let el = if lang.trim().eq_ignore_ascii_case("mermaid") { super::mermaid::block(t, i, &raw) } else { super::rich::code_block(t, i, &lang, &raw) };
                        push_child(&mut stack, el);
                    }
                }
                TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item => {
                    flush(&mut inl, &mut stack, None);
                    if let Some(Block::Container { kind, children }) = stack.pop() {
                        let body = div().flex().flex_col().gap(t.space(2)).min_w_0().flex_1().children(children);
                        let el = match kind {
                            Container::Quote => div()
                                .pl(t.space(3))
                                .border_l_2()
                                .border_color(t.colors.border_strong)
                                .text_color(t.colors.fg_muted)
                                .child(body)
                                .into_any_element(),
                            Container::List(_) => div().flex().flex_col().gap(t.space(1)).pl(t.space(1)).child(body).into_any_element(),
                            Container::Item(n) => div()
                                .flex()
                                .gap(t.space(2))
                                .child(div().flex_none().min_w(t.rem(1.)).text_color(t.colors.fg_subtle).child(n))
                                .child(body)
                                .into_any_element(),
                            Container::Root => body.into_any_element(),
                        };
                        push_child(&mut stack, el);
                    }
                }
                TagEnd::TableCell => {
                    text_row.push(inl.text.trim().to_string());
                    table_row.push(inl.take().unwrap_or_else(|| div().into_any_element()));
                }
                TagEnd::TableHead => {
                    table_rows.push((true, std::mem::take(&mut table_row)));
                    text_rows.push(std::mem::take(&mut text_row));
                    in_head = false;
                }
                TagEnd::TableRow => {
                    table_rows.push((in_head, std::mem::take(&mut table_row)));
                    text_rows.push(std::mem::take(&mut text_row));
                }
                TagEnd::Table => {
                    let mut tbl = div().flex().flex_col().w_full().border_1().border_color(t.colors.border_soft).rounded(t.rem(rem::RADIUS_MD));
                    for (head, cells) in table_rows.drain(..) {
                        let row = div()
                            .flex()
                            .border_b_1()
                            .border_color(t.colors.border_soft)
                            .when(head, |d| d.bg(t.colors.raised).font_weight(FontWeight::SEMIBOLD))
                            .children(cells.into_iter().map(|c| div().flex_1().min_w_0().px(t.space(2)).py(t.space(1)).child(c)));
                        tbl = tbl.child(row);
                    }
                    let bar = super::rich::table_bar(t, i, std::mem::take(&mut text_rows));
                    push_child(&mut stack, div().flex().flex_col().gap(t.space(1)).w_full().child(bar).child(tbl).into_any_element());
                }
                _ => {}
            },
            Event::Text(s) => {
                if let Some((_, body)) = code.as_mut() {
                    body.push_str(&s);
                } else if inl.link > 0 {
                    inl.push(&s, t, false);
                } else {
                    // tautan polos (autolink GFM) di teks biasa
                    for (piece, url) in split_autolinks(&s) {
                        if url {
                            inl.link += 1;
                            inl.href = Some(piece.trim_end_matches(['.', ',', ')', ';', ':', '!', '?']).to_string());
                            let core = inl.href.clone().unwrap_or_default();
                            inl.push(&core, t, false);
                            inl.link -= 1;
                            inl.href = None;
                            inl.push(&piece[core.len()..], t, false);
                        } else {
                            inl.push(piece, t, false);
                        }
                    }
                }
            }
            Event::Code(s) => inl.push(&s, t, true),
            Event::InlineMath(s) => {
                inl.italic += 1;
                inl.push(&super::math::inline_text(&s), t, false);
                inl.italic -= 1;
            }
            Event::DisplayMath(s) => {
                flush(&mut inl, &mut stack, None);
                let bar = super::rich::math_bar(t, i, &s);
                push_child(&mut stack, div().flex().flex_col().w_full().child(bar).child(super::math::render_block(&s, t)).into_any_element());
            }
            Event::SoftBreak => inl.push(" ", t, false),
            Event::HardBreak => inl.push("\n", t, false),
            Event::Rule => {
                flush(&mut inl, &mut stack, None);
                push_child(&mut stack, div().h(gpui::px(1.)).w_full().bg(t.colors.border).into_any_element());
            }
            Event::TaskListMarker(done) => inl.push(if done { "☑ " } else { "☐ " }, t, false),
            Event::Html(s) | Event::InlineHtml(s) => inl.push(&s, t, false),
            _ => {}
        }
    }
    flush(&mut inl, &mut stack, None);
    while stack.len() > 1 {
        if let Some(Block::Container { children, .. }) = stack.pop() {
            for c in children {
                push_child(&mut stack, c);
            }
        }
    }
    let Some(Block::Container { children, .. }) = stack.pop() else { return div().into_any_element() };
    div().flex().flex_col().gap(t.space(2)).w_full().min_w_0().children(children).into_any_element()
}

#[cfg(test)]
mod link_tests {
    #[test]
    fn autolink_dan_web_link() {
        let v = super::split_autolinks("lihat https://a.id/x. dan httpx lalu http://b.c");
        assert_eq!(v, vec![("lihat ", false), ("https://a.id/x.", true), (" dan http", false), ("x lalu ", false), ("http://b.c", true)]);
        assert!(super::is_web_link("HTTPS://x") && !super::is_web_link("javascript:alert(1)") && !super::is_web_link("file:///c"));
    }
}

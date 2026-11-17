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
    link: usize,
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
    fn take(&mut self) -> Option<StyledText> {
        if self.text.trim().is_empty() {
            self.text.clear();
            self.hl.clear();
            return None;
        }
        let text = std::mem::take(&mut self.text);
        let hl = std::mem::take(&mut self.hl);
        Some(StyledText::new(text).with_highlights(hl))
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

pub fn markdown(src: &str, t: &Theme) -> AnyElement {
    let opts = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES;
    let mut stack: Vec<Block> = vec![Block::Container { kind: Container::Root, children: vec![] }];
    let mut inl = Inline::default();
    let mut heading: Option<HeadingLevel> = None;
    let mut code: Option<(String, String)> = None;
    let mut table_row: Vec<StyledText> = vec![];
    let mut table_rows: Vec<(bool, Vec<StyledText>)> = vec![];
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
                Tag::Link { .. } => inl.link += 1,
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
                TagEnd::Link => inl.link = inl.link.saturating_sub(1),
                TagEnd::CodeBlock => {
                    if let Some((lang, body)) = code.take() {
                        let el = div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .rounded(t.rem(rem::RADIUS_MD))
                            .border_1()
                            .border_color(t.colors.border_soft)
                            .bg(t.colors.raised)
                            .overflow_hidden()
                            .when(!lang.is_empty(), |d| {
                                d.child(
                                    div()
                                        .px(t.space(3))
                                        .py(t.space(1))
                                        .border_b_1()
                                        .border_color(t.colors.border_soft)
                                        .small(t)
                                        .text_color(t.colors.fg_subtle)
                                        .child(lang),
                                )
                            })
                            .child(
                                div()
                                    .px(t.space(3))
                                    .py(t.space(2))
                                    .font_family(FONT_CODE)
                                    .code(t)
                                    .child(body.trim_end_matches('\n').to_string()),
                            );
                        push_child(&mut stack, el.into_any_element());
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
                    table_row.push(inl.take().unwrap_or_else(|| StyledText::new("")));
                }
                TagEnd::TableHead => {
                    table_rows.push((true, std::mem::take(&mut table_row)));
                    in_head = false;
                }
                TagEnd::TableRow => table_rows.push((in_head, std::mem::take(&mut table_row))),
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
                    push_child(&mut stack, tbl.into_any_element());
                }
                _ => {}
            },
            Event::Text(s) => {
                if let Some((_, body)) = code.as_mut() {
                    body.push_str(&s);
                } else {
                    inl.push(&s, t, false);
                }
            }
            Event::Code(s) => inl.push(&s, t, true),
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

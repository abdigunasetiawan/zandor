//! Panel Dokumen — pratinjau berkas yang dibuka dari pohon Berkas: Markdown dirender, berkas teks lain ditampilkan
//! sebagai kode dengan nomor baris. Berkas biner/terlalu besar diberi catatan. Tab per berkas terbuka.
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{icon, icon_button, markdown, ButtonVariant, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, App, AppContext, Context, Entity, Global, IntoElement, SharedString, Window};

const MAX_BYTES: u64 = 2 * 1024 * 1024;

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

pub struct DocsPanel {
    state: Entity<DocsState>,
    cache: Option<(String, Result<String, String>)>,
}

impl DocsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = DocsState::global(cx);
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, cache: None }
    }
}

fn load(path: &str) -> Result<String, String> {
    let bytes = crate::files::read_limited(std::path::Path::new(path), MAX_BYTES)?;
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return Err("Berkas biner tidak bisa dipratinjau.".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(i.t("empty.p3.title")))
                .child(div().text_color(t.colors.fg_muted).child(i.t("empty.p3.body")))
                .into_any_element();
        };
        if self.cache.as_ref().is_none_or(|(p, _)| *p != active) {
            self.cache = Some((active.clone(), load(&active)));
        }
        let content = self.cache.as_ref().map(|(_, c)| c.clone()).unwrap_or_else(|| Ok(String::new()));

        let mut tabs = div().flex().flex_none().items_center().gap(t.space(1)).px(t.space(2)).h(t.rem(rem::TITLEBAR_HEIGHT)).border_b_1().border_color(t.colors.border).overflow_hidden();
        for p in &open {
            let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.clone());
            let cur = *p == active;
            let (p1, p2) = (p.clone(), p.clone());
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("doc-{p}")))
                    .flex()
                    .items_center()
                    .gap(t.space(1))
                    .pl(t.space(2))
                    .h(t.rem(rem::CONTROL_SM))
                    .rounded(t.rem(rem::RADIUS_SM))
                    .cursor_pointer()
                    .text_color(if cur { t.colors.fg } else { t.colors.fg_muted })
                    .when(cur, |d| d.bg(t.colors.active))
                    .hover(|s| s.bg(t.colors.hover))
                    .on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.open_file(p1.clone(), cx))))
                    .child(icon("file").size(IconSize::Sm))
                    .child(div().max_w(t.rem(10.)).truncate().small(&t).child(name.clone()))
                    .child(
                        icon_button(SharedString::from(format!("docx-{p}")), "close", i.tp("chat.closeTab", &[("title", &name)]))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.close(&p2, cx)))),
                    ),
            );
        }

        let body: gpui::AnyElement = match content {
            Err(e) => div().p(t.space(3)).text_color(t.colors.fg_muted).child(e).into_any_element(),
            Ok(text) => {
                let lower = active.to_lowercase();
                if lower.ends_with(".md") || lower.ends_with(".markdown") {
                    div().p(t.space(4)).child(markdown(&text, &t)).into_any_element()
                } else {
                    let lines: Vec<&str> = text.lines().collect();
                    let w = lines.len().to_string().len().max(2);
                    div()
                        .flex()
                        .flex_col()
                        .p(t.space(3))
                        .font_family(t.font_code.clone())
                        .code(&t)
                        .children(lines.into_iter().take(20_000).enumerate().map(|(n, l)| {
                            div()
                                .flex()
                                .gap(t.space(3))
                                .child(div().flex_none().text_color(t.colors.fg_subtle).child(format!("{:>w$}", n + 1, w = w)))
                                .child(div().whitespace_nowrap().child(if l.is_empty() { " ".to_string() } else { l.replace('\t', "    ") }))
                        }))
                        .into_any_element()
                }
            }
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(tabs)
            .child(div().id("doc-body").flex_1().min_h_0().overflow_scroll().child(body))
            .into_any_element()
    }
}

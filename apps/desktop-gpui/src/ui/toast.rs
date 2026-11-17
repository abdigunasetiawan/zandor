//! Toast — port molecules/Toast.tsx + .z-toast (kanan-bawah, monokrom, warna status hanya di ikon).
use super::icon::{icon, IconSize};
use crate::theme::{rem, Theme, H4, LABEL, SMALL};
use gpui::{
    div, prelude::*, App, Context, Entity, FontWeight, Global, IntoElement, SharedString, Window,
};
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastTone {
    #[default]
    Neutral,
    Success,
    Danger,
    Warning,
}

#[derive(Clone)]
pub struct ToastAction {
    pub label: SharedString,
    pub on_click: Rc<dyn Fn(&mut Window, &mut App)>,
}

#[derive(Clone)]
pub struct ToastInput {
    pub title: SharedString,
    pub description: Option<SharedString>,
    pub tone: ToastTone,
    pub timeout_ms: Option<u64>,
    pub action: Option<ToastAction>,
}
impl ToastInput {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self { title: title.into(), description: None, tone: ToastTone::Neutral, timeout_ms: None, action: None }
    }
}

struct Item {
    id: u64,
    input: ToastInput,
}

pub struct Toasts {
    next: u64,
    items: Vec<Item>,
}

struct ToastsHandle(Entity<Toasts>);
impl Global for ToastsHandle {}

/// Bawaan base-ui: 5 detik.
const DEFAULT_TIMEOUT_MS: u64 = 5000;

impl Toasts {
    pub fn init(cx: &mut App) -> Entity<Toasts> {
        let e = cx.new(|_| Toasts { next: 0, items: vec![] });
        cx.set_global(ToastsHandle(e.clone()));
        e
    }
    /// Tampilkan toast; kembalikan id-nya.
    pub fn show(input: ToastInput, cx: &mut App) -> u64 {
        if matches!(input.tone, ToastTone::Danger | ToastTone::Warning) {
            let lvl = if matches!(input.tone, ToastTone::Danger) { "error" } else { "warn" };
            crate::core::activity::log(cx, "app", lvl, input.title.to_string());
        }
        let e = cx.global::<ToastsHandle>().0.clone();
        e.update(cx, |this, cx| {
            let id = this.next;
            this.next += 1;
            let timeout = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
            this.items.push(Item { id, input });
            cx.notify();
            if timeout > 0 {
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(timeout)).await;
                    this.update(cx, |t, cx| t.close(id, cx)).ok();
                })
                .detach();
            }
            id
        })
    }
    pub fn close(&mut self, id: u64, cx: &mut Context<Self>) {
        self.items.retain(|i| i.id != id);
        cx.notify();
    }
}

impl Render for Toasts {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = t.colors.clone();
        let mut vp = div()
            .absolute()
            .right(t.space(4))
            .bottom(t.space(4))
            .flex()
            .flex_col()
            .gap(t.space(2))
            .w(t.rem(rem::SPACE[7] * 7.));
        for item in &self.items {
            let id = item.id;
            let inp = &item.input;
            let (glyph, tone_fg) = match inp.tone {
                ToastTone::Neutral => ("info", c.fg_muted),
                ToastTone::Success => ("badge_check", c.success),
                ToastTone::Danger => ("octagon_x", c.danger),
                ToastTone::Warning => ("triangle_alert", c.warning),
            };
            let mut content = div()
                .flex()
                .flex_1()
                .flex_col()
                .items_start()
                .gap(t.space(1))
                .min_w_0()
                .child(
                    div()
                        .text_size(t.rem(t.ty(crate::theme::LABEL).size))
                        .font_weight(FontWeight(t.ty(crate::theme::H4).weight))
                        .line_height(gpui::relative(t.ty(crate::theme::LABEL).lh))
                        .child(inp.title.clone()),
                );
            if let Some(d) = &inp.description {
                content = content.child(
                    div()
                        .text_size(t.rem(t.ty(crate::theme::SMALL).size))
                        .line_height(gpui::relative(t.ty(crate::theme::SMALL).lh))
                        .text_color(c.fg_muted)
                        .child(d.clone()),
                );
            }
            if let Some(a) = inp.action.clone() {
                let (hover, active) = (c.hover, c.active);
                content = content.child(
                    div()
                        .id(("toast-action", id))
                        .mt(t.space(1))
                        .h(t.rem(rem::CONTROL_SM))
                        .px(t.space(3))
                        .flex()
                        .items_center()
                        .rounded(t.rem(rem::RADIUS_MD))
                        .bg(hover)
                        .text_size(t.rem(t.ty(crate::theme::SMALL).size))
                        .font_weight(FontWeight(t.ty(crate::theme::LABEL).weight))
                        .cursor_pointer()
                        .hover(move |s| s.bg(active))
                        .on_click(cx.listener(move |this, _, w, cx| {
                            (a.on_click)(w, cx);
                            this.close(id, cx);
                        }))
                        .child(a.label),
                );
            }
            let (subtle, fg, hover) = (c.fg_subtle, c.fg, c.hover);
            vp = vp.child(
                div()
                    .id(("toast", id))
                    .occlude()
                    .flex()
                    .items_start()
                    .gap(t.space(3))
                    .pt(t.space(3))
                    .pb(t.space(3))
                    .pr(t.space(3))
                    .pl(t.space(4))
                    .border_1()
                    .border_color(c.border_soft)
                    .rounded(t.rem(rem::RADIUS_LG))
                    .bg(c.composer)
                    .text_color(c.fg)
                    .shadow(t.shadow_panel())
                    .child(div().mt(t.rem(0.0625)).child(icon(glyph).color(tone_fg)))
                    .child(content)
                    .child(
                        div()
                            .id(("toast-close", id))
                            .flex_none()
                            .p(t.space(1))
                            .rounded(t.rem(rem::RADIUS_SM))
                            .text_color(subtle)
                            .cursor_pointer()
                            .hover(move |s| s.bg(hover).text_color(fg))
                            .on_click(cx.listener(move |this, _, _, cx| this.close(id, cx)))
                            .child(icon("close").size(IconSize::Sm).color(subtle)),
                    ),
            );
        }
        vp
    }
}

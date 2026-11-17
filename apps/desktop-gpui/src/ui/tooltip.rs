//! Tooltip — port .z-tooltip (latar accent, teks on-accent, small, radius md, shadow-2).
use crate::theme::{Theme, SMALL};
use gpui::{div, prelude::*, AnyView, App, Context, IntoElement, SharedString, Window};

pub struct TextTooltip {
    text: SharedString,
}

impl Render for TextTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>();
        div()
            .px(t.space(2))
            .py(t.space(1))
            .rounded(t.rem(crate::theme::rem::RADIUS_MD))
            .bg(t.colors.accent)
            .text_color(t.colors.on_accent)
            .text_size(t.rem(t.ty(crate::theme::SMALL).size))
            .line_height(gpui::relative(t.ty(crate::theme::SMALL).lh))
            .shadow(t.shadow_2())
            .child(self.text.clone())
    }
}

pub fn text_tooltip(text: SharedString) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_, cx| cx.new(|_| TextTooltip { text: text.clone() }).into()
}

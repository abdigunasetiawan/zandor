//! Spinner — port .z-spinner (cincin border-strong, sisi atas fg, berputar 700ms linear).
use super::icon::IconSize;
use crate::theme::Theme;
use gpui::{
    div, percentage, prelude::*, svg, Animation, AnimationExt, App, IntoElement, Transformation,
    Window,
};
use std::time::Duration;

#[derive(IntoElement)]
pub struct Spinner {
    size: IconSize,
}

pub fn spinner(size: IconSize) -> Spinner {
    Spinner { size }
}

impl RenderOnce for Spinner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>();
        let s = t.rem(self.size.rem());
        div()
            .relative()
            .flex_none()
            .size(s)
            .child(svg().path("icons/spinner_ring.svg").absolute().size(s).text_color(t.colors.border_strong))
            .child(
                svg()
                    .path("icons/spinner_arc.svg")
                    .absolute()
                    .size(s)
                    .text_color(t.colors.fg)
                    .with_animation(
                        "z-spin",
                        Animation::new(Duration::from_millis(700)).repeat(),
                        |el, delta| el.with_transformation(Transformation::rotate(percentage(delta))),
                    ),
            )
    }
}

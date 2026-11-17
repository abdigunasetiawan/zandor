//! Zandor — kerangka awal rewrite ke Rust + GPUI.
use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, SharedString, Window,
    WindowBounds, WindowOptions,
};

struct ZandorShell {
    title: SharedString,
}

impl Render for ZandorShell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xffffff))
            .text_xl()
            .child(self.title.clone())
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| ZandorShell { title: "Zandor (GPUI)".into() }),
        )
        .unwrap();
        cx.activate(true);
    });
}

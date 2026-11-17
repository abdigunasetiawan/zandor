//! Zandor — rewrite ke Rust + GPUI.
mod assets;
mod i18n;
mod theme;
mod ui;

use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, Window, WindowBounds, WindowOptions,
};
use i18n::{detect_locale, I18n};
use theme::{Theme, ThemeMode};
use ui::{button, icon_button, ButtonVariant, ControlSize};

struct ZandorShell;

impl Render for ZandorShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>();
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap(t.space(3))
            .p(t.space(5))
            .bg(t.colors.bg)
            .text_color(t.colors.fg)
            .font_family(theme::FONT_UI)
            .child(i.t("app.name"))
            .child(
                div()
                    .flex()
                    .gap(t.space(2))
                    .items_center()
                    .child(button("solid").variant(ButtonVariant::Solid).label(i.t("ui.confirm")))
                    .child(button("outline").label(i.t("ui.cancel")))
                    .child(button("ghost").variant(ButtonVariant::Ghost).label(i.t("ui.close")))
                    .child(button("danger").variant(ButtonVariant::Danger).label(i.t("ui.clear")))
                    .child(button("loading").loading(true).label(i.t("ui.loading")))
                    .child(button("sm").size(ControlSize::Sm).label("Sm"))
                    .child(button("lg").size(ControlSize::Lg).label("Lg"))
                    .child(icon_button("set", "settings", i.t("settings.title"))),
            )
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            cx.set_global(Theme::new(ThemeMode::Dark, true, 1.0));
            cx.set_global(I18n::new(detect_locale(std::env::var("LANG").ok().as_deref())));
            let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| ZandorShell),
            )
            .expect("buka jendela");
            cx.activate(true);
        });
}

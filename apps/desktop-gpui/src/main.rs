//! Zandor — rewrite ke Rust + GPUI.
mod assets;
mod core;
mod i18n;
mod shell;
mod theme;
mod ui;

use gpui::{px, size, App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions};
use i18n::{detect_locale, I18n};
use shell::root::Root;
use shell::state::AppState;
use theme::{Theme, ThemeMode};

fn main() {
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            assets::load_fonts(cx);
            ui::text_input::bind_keys(cx);
            cx.set_global(Theme::new(ThemeMode::Dark, true, 1.0));
            cx.set_global(I18n::new(detect_locale(std::env::var("LANG").ok().as_deref())));
            AppState::init("/", cx);
            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // Tanpa bingkai (sama dengan tauri.conf.json `decorations: false`).
                    titlebar: Some(TitlebarOptions {
                        title: Some("Zandor".into()),
                        appears_transparent: true,
                        traffic_light_position: None,
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    window_min_size: Some(size(px(720.), px(480.))),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Root::new(window, cx)),
            )
            .expect("buka jendela");
            cx.activate(true);
        });
}

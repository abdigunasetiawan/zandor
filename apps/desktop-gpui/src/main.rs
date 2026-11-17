//! Zandor — rewrite ke Rust + GPUI.
mod assets;
mod core;
mod i18n;
mod shell;
mod theme;
mod ui;

// ---- Backend bersama dengan Tauri: sumber tunggal di apps/desktop/src-tauri/src ----
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/bundled_skills.rs"]
mod bundled_skills;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/files.rs"]
mod files;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/git.rs"]
mod git;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/jobs.rs"]
mod jobs;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/memory.rs"]
mod memory;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/providers.rs"]
mod providers;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/registry.rs"]
mod registry;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/secrets.rs"]
mod secrets;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/shells.rs"]
mod shells;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/syscheck.rs"]
mod syscheck;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/fonts.rs"]
mod fonts;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/images.rs"]
mod images;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/office_check.rs"]
mod office_check;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/openvsx.rs"]
mod openvsx;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/openwith.rs"]
mod openwith;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/control.rs"]
mod control;
#[allow(dead_code, unused_imports)]
#[path = "../../desktop/src-tauri/src/netprobe.rs"]
mod netprobe;
#[allow(dead_code)]
#[path = "host/engine.rs"]
mod engine;

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
            shell::command_center::bind_keys(cx);
            cx.set_global(Theme::new(ThemeMode::Dark, true, 1.0));
            cx.set_global(I18n::new(detect_locale(std::env::var("LANG").ok().as_deref())));
            AppState::init("/", cx);
            core::services::init(cx);
            cx.on_app_quit(|cx| {
                core::services::shutdown(cx);
                async {}
            })
            .detach();
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

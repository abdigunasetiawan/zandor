//! Aset tertanam (ikon SVG, font) — dibundel ke dalam binary.
use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*"]
#[include = "fonts/**/*"]
pub struct Assets;

/// Daftarkan font bundel (Plus Jakarta Sans & JetBrains Mono, OFL) ke sistem teks GPUI.
pub fn load_fonts(cx: &mut gpui::App) {
    let fonts: Vec<Cow<'static, [u8]>> = Assets::iter()
        .filter(|p| p.starts_with("fonts/") && p.ends_with(".ttf"))
        .filter_map(|p| Assets::get(&p).map(|f| f.data))
        .collect();
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        eprintln!("gagal memuat font bundel: {e}");
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|f| f.data))
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter(|p| p.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

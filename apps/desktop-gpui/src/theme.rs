//! Token desain Zandor — port 1:1 dari packages/ui/src/styles/tokens.css.
//! Ukuran dalam rem (1rem = 16px × ui_scale), warna sama persis dengan CSS.
#![allow(dead_code)]
use gpui::{SharedString, px, rgb, rgba, Global, Hsla, Pixels};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
    System,
}

/// Langkah skala yang didukung (ADR-025): 75-150%.
pub const UI_SCALES: [f32; 7] = [0.75, 0.875, 1.0, 1.125, 1.25, 1.375, 1.5];

/// Skala berikutnya/sebelumnya dalam daftar; berhenti di ujung.
pub fn step_scale(current: f32, dir: i32) -> f32 {
    let i = UI_SCALES
        .iter()
        .position(|s| (*s - current).abs() < 1e-4)
        .unwrap_or(2) as i32;
    UI_SCALES[(i + dir).clamp(0, UI_SCALES.len() as i32 - 1) as usize]
}

#[derive(Clone, Debug)]
pub struct Colors {
    pub frame: Hsla,
    pub bg: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub border: Hsla,
    pub segment: Hsla,
    pub segment_active: Hsla,
    pub border_soft: Hsla,
    pub composer: Hsla,
    pub bubble: Hsla,
    pub scrollbar: Hsla,
    pub scrollbar_hover: Hsla,
    pub border_strong: Hsla,
    pub fg: Hsla,
    pub fg_muted: Hsla,
    pub fg_subtle: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub focus: Hsla,
    pub backdrop: Hsla,
    pub danger: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub shadow: Hsla,
    pub code_keyword: Hsla,
    pub code_string: Hsla,
    pub code_number: Hsla,
    pub code_comment: Hsla,
    pub code_function: Hsla,
    pub code_type: Hsla,
    pub code_attr: Hsla,
    pub code_add: Hsla,
    pub code_del: Hsla,
    /// Latar/teks blok kode dari tema editor (None = token permukaan).
    pub code_bg: Option<Hsla>,
    pub code_fg: Option<Hsla>,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}
fn a(hex: u32, alpha: f32) -> Hsla {
    rgba((hex << 8) | (alpha * 255.0).round() as u32).into()
}

/// Setara `color-mix(in srgb, x p, y)`.
pub fn mix(x: Hsla, p: f32, y: Hsla) -> Hsla {
    let (xa, ya) = (x.to_rgb(), y.to_rgb());
    gpui::Rgba {
        r: xa.r * p + ya.r * (1. - p),
        g: xa.g * p + ya.g * (1. - p),
        b: xa.b * p + ya.b * (1. - p),
        a: xa.a * p + ya.a * (1. - p),
    }
    .into()
}

impl Colors {
    pub fn dark() -> Self {
        let bg = c(0x151515);
        let fg = c(0xececec);
        Self {
            frame: mix(bg, 0.5, c(0)),
            bg,
            surface: c(0x1a1a1a),
            raised: c(0x212121),
            hover: c(0x2a2a2a),
            active: c(0x333333),
            border: a(0xffffff, 0.11),
            segment: c(0x202020),
            segment_active: c(0x333333),
            border_soft: a(0xffffff, 0.07),
            composer: c(0x262626),
            bubble: mix(fg, 0.08, bg),
            scrollbar: a(0xffffff, 0.12),
            scrollbar_hover: a(0xffffff, 0.22),
            border_strong: c(0x666666),
            fg,
            fg_muted: c(0xa3a3a3),
            fg_subtle: c(0x8a8a8a),
            accent: c(0xececec),
            on_accent: c(0x151515),
            focus: c(0xffffff),
            backdrop: a(0, 0.5),
            danger: c(0xff6b6b),
            success: c(0x5ccf8b),
            warning: c(0xe0b341),
            shadow: a(0, 0.38),
            code_keyword: c(0x79b8ff),
            code_string: c(0x9ecb7a),
            code_number: c(0xe6a95e),
            code_comment: c(0x8a8a8a),
            code_function: c(0xd2a8ff),
            code_type: c(0xffab70),
            code_attr: c(0x85e89d),
            code_add: c(0x9ecb7a),
            code_del: c(0xff7b72),
            code_bg: None,
            code_fg: None,
        }
    }

    pub fn light() -> Self {
        let bg = c(0xfdfdfd);
        let fg = c(0x161616);
        Self {
            frame: mix(bg, 0.9, c(0)),
            bg,
            surface: c(0xf6f6f6),
            raised: c(0xffffff),
            hover: c(0xededed),
            active: c(0xe3e3e3),
            border: a(0, 0.09),
            segment: c(0xececec),
            segment_active: c(0xffffff),
            border_soft: a(0, 0.07),
            composer: c(0xffffff),
            bubble: mix(fg, 0.08, bg),
            scrollbar: a(0, 0.16),
            scrollbar_hover: a(0, 0.28),
            border_strong: c(0x8a8a8a),
            fg,
            fg_muted: c(0x5c5c5c),
            fg_subtle: c(0x6b6b6b),
            accent: c(0x161616),
            on_accent: c(0xfdfdfd),
            focus: c(0),
            backdrop: a(0, 0.35),
            danger: c(0xc62828),
            success: c(0x1b7a43),
            warning: c(0x8a6100),
            shadow: a(0, 0.16),
            code_keyword: c(0x0b5cad),
            code_string: c(0x1a7f37),
            code_number: c(0xa35200),
            code_comment: c(0x6b6b6b),
            code_function: c(0x7f3fbf),
            code_type: c(0x9a4a00),
            code_attr: c(0x116329),
            code_add: c(0x1a7f37),
            code_del: c(0xc62828),
            code_bg: None,
            code_fg: None,
        }
    }
}

/// Satu variabel tipografi: ukuran (rem) / ketebalan / tinggi baris / jarak huruf (em).
#[derive(Clone, Copy, Debug)]
pub struct TypeStyle {
    /// Indeks di TYPE_VARS (untuk override dari Tampilan).
    pub id: usize,
    pub size: f32,
    pub weight: f32,
    pub lh: f32,
    pub tracking: f32,
}
const fn ts(id: usize, size: f32, weight: f32, lh: f32, tracking: f32) -> TypeStyle {
    TypeStyle { id, size, weight, lh, tracking }
}
pub const TYPE_VARS: [&str; 10] = ["h1", "h2", "h3", "h4", "p", "small", "link", "button", "label", "code"];
pub const H1: TypeStyle = ts(0, 1.75, 700., 1.25, -0.02);
pub const H2: TypeStyle = ts(1, 1.375, 700., 1.3, -0.015);
pub const H3: TypeStyle = ts(2, 1.125, 600., 1.35, -0.01);
pub const H4: TypeStyle = ts(3, 1.0, 600., 1.4, 0.);
pub const P: TypeStyle = ts(4, 0.875, 400., 1.55, 0.);
pub const SMALL: TypeStyle = ts(5, 0.75, 400., 1.4, 0.);
pub const LINK: TypeStyle = ts(6, 0.875, 500., 1.55, 0.);
pub const BUTTON: TypeStyle = ts(7, 0.8125, 500., 1.2, 0.);
pub const LABEL: TypeStyle = ts(8, 0.8125, 500., 1.3, 0.);
pub const CODE: TypeStyle = ts(9, 0.8125, 400., 1.5, 0.);

pub const FONT_UI: &str = "Plus Jakarta Sans";
pub const FONT_CODE: &str = "JetBrains Mono";

/// Nilai rem dari tokens.css.
pub mod rem {
    pub const SPACE: [f32; 8] = [0., 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0];
    pub const RADIUS_SM: f32 = 0.25;
    pub const RADIUS_MD: f32 = 0.5;
    pub const RADIUS_LG: f32 = 0.75;
    pub const RADIUS_FULL: f32 = 62.;
    pub const CONTROL_SM: f32 = 1.5;
    pub const CONTROL_MD: f32 = 2.0;
    pub const CONTROL_LG: f32 = 2.5;
    pub const ICON_SM: f32 = 0.875;
    pub const ICON_MD: f32 = 1.0;
    pub const ICON_LG: f32 = 1.25;
    pub const TITLEBAR_HEIGHT: f32 = 2.5;
    pub const RAIL_WIDTH: f32 = 3.0;
    pub const SEPARATOR: f32 = 0.375;
    pub const CHAT_WIDTH: f32 = 46.;
    pub const CHAT_COMPOSER_RATIO: f32 = 1.2;
    pub const DOC_WIDTH: f32 = 48.;
}
pub const DURATION_FAST_MS: u64 = 90;
pub const DURATION_BASE_MS: u64 = 150;

#[derive(Clone)]
pub struct Theme {
    pub mode: ThemeMode,
    pub dark: bool,
    pub scale: f32,
    pub colors: Colors,
    /// Tipografi efektif (default + override Tampilan), urut TYPE_VARS.
    pub types: [TypeStyle; 10],
    pub font_ui: SharedString,
    pub font_code: SharedString,
    pub font_terminal: SharedString,
    /// Gaya panel chat dari Tampilan (None = bawaan).
    pub chat: ChatVars,
}

/// Port variabel --z-chat-* (tokens.css).
#[derive(Clone, Debug, Default)]
pub struct ChatVars {
    pub opacity: Option<f32>,
    pub weight: Option<f32>,
    pub size: Option<f32>,
    pub gap: Option<f32>,
    pub font: Option<SharedString>,
    pub width: Option<f32>,
    pub line: Option<f32>,
    pub tracking: Option<f32>,
    pub code_size: Option<f32>,
    pub composer_ratio: Option<f32>,
}

impl Global for Theme {}

impl Theme {
    pub fn new(mode: ThemeMode, system_dark: bool, scale: f32) -> Self {
        let dark = match mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => system_dark,
        };
        Self {
            mode,
            dark,
            scale,
            colors: if dark { Colors::dark() } else { Colors::light() },
            types: [H1, H2, H3, H4, P, SMALL, LINK, BUTTON, LABEL, CODE],
            font_ui: FONT_UI.into(),
            font_code: FONT_CODE.into(),
            font_terminal: FONT_CODE.into(),
            chat: ChatVars::default(),
        }
    }
    /// Gaya tipografi efektif untuk variabel ini.
    pub fn ty(&self, s: TypeStyle) -> TypeStyle {
        self.types[s.id]
    }
    /// rem → piksel sesuai skala UI.
    pub fn rem(&self, r: f32) -> Pixels {
        px(r * 16. * self.scale)
    }
    pub fn space(&self, i: usize) -> Pixels {
        self.rem(rem::SPACE[i])
    }
    fn shadow(&self, y: f32, blur: f32) -> gpui::BoxShadow {
        gpui::BoxShadow {
            color: self.colors.shadow,
            offset: gpui::point(px(0.), self.rem(y)),
            blur_radius: self.rem(blur),
            spread_radius: px(0.),
            inset: false,
        }
    }
    /// --z-shadow-1: 0 0.0625rem 0.125rem
    pub fn shadow_1(&self) -> Vec<gpui::BoxShadow> {
        vec![self.shadow(0.0625, 0.125)]
    }
    /// --z-shadow-2: 0 0.25rem 0.75rem
    pub fn shadow_2(&self) -> Vec<gpui::BoxShadow> {
        vec![self.shadow(0.25, 0.75)]
    }
    /// --z-shadow-3: 0 0.75rem 2rem
    pub fn shadow_3(&self) -> Vec<gpui::BoxShadow> {
        vec![self.shadow(0.75, 2.0)]
    }
    /// --z-shadow-panel
    pub fn shadow_panel(&self) -> Vec<gpui::BoxShadow> {
        vec![self.shadow(0.0625, 0.125), self.shadow(0.375, 1.25)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skala_berhenti_di_ujung() {
        assert_eq!(step_scale(1.5, 1), 1.5);
        assert_eq!(step_scale(0.75, -1), 0.75);
        assert_eq!(step_scale(1.0, 1), 1.125);
        assert_eq!(step_scale(0.9, 1), 1.125);
    }
}

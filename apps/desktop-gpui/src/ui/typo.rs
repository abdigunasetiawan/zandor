//! Tipografi — port kelas .z-h1….z-code di base.css sebagai ekstensi Styled.
use crate::theme::{Theme, TypeStyle, FONT_CODE};
use gpui::{FontWeight, Styled};

pub trait TypeExt: Styled + Sized {
    /// Terapkan ukuran/ketebalan/tinggi baris dari satu variabel tipografi.
    fn typo(self, t: &Theme, s: TypeStyle) -> Self {
        let s = t.ty(s);
        self.text_size(t.rem(s.size))
            .font_weight(FontWeight(s.weight))
            .line_height(gpui::relative(s.lh))
    }
    /// .z-small: small + fg-muted.
    fn small(self, t: &Theme) -> Self {
        self.typo(t, crate::theme::SMALL).text_color(t.colors.fg_muted)
    }
    /// .z-code
    fn code(self, t: &Theme) -> Self {
        self.typo(t, crate::theme::CODE).font_family(t.font_code.clone())
    }
}
impl<T: Styled> TypeExt for T {}

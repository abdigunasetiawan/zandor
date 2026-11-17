//! Ikon Lucide (stroke 1.5) — port dari packages/ui/src/icons.tsx.
//! SVG diekspor oleh packages/ui/scripts/export-icons-gpui.tsx ke assets/icons/<nama>.svg.
use crate::theme::{rem, Theme};
use gpui::{prelude::*, svg, App, Hsla, IntoElement, SharedString, Window};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl IconSize {
    pub fn rem(self) -> f32 {
        match self {
            IconSize::Sm => rem::ICON_SM,
            IconSize::Md => rem::ICON_MD,
            IconSize::Lg => rem::ICON_LG,
        }
    }
}

#[derive(IntoElement)]
pub struct Icon {
    name: SharedString,
    size: IconSize,
    color: Option<Hsla>,
}

/// `name` = nama berkas tanpa ekstensi, mis. "chevron_down".
pub fn icon(name: impl Into<SharedString>) -> Icon {
    Icon { name: name.into(), size: IconSize::Md, color: None }
}

impl Icon {
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let s = theme.rem(self.size.rem());
        svg()
            .path(SharedString::from(format!("icons/{}.svg", self.name)))
            .flex_none()
            .size(s)
            .text_color(self.color.unwrap_or(theme.colors.fg))
    }
}

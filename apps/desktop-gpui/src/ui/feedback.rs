//! Badge, Skeleton, Kbd, Avatar, Working — port Feedback.tsx, Kbd.tsx, Avatar.tsx + atoms.css.
use super::button::ControlSize;
use crate::theme::{rem, Theme, FONT_CODE, LABEL, SMALL};
use gpui::{
    div, prelude::*, Animation, AnimationExt, App, FontWeight, Hsla, IntoElement, SharedString,
    Window,
};
use std::time::Duration;

// ---------- Badge ----------
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeTone {
    #[default]
    Neutral,
    Outline,
    Danger,
    Success,
    Warning,
}

#[derive(IntoElement)]
pub struct Badge {
    tone: BadgeTone,
    text: SharedString,
}
pub fn badge(text: impl Into<SharedString>) -> Badge {
    Badge { tone: BadgeTone::Neutral, text: text.into() }
}
impl Badge {
    pub fn tone(mut self, t: BadgeTone) -> Self {
        self.tone = t;
        self
    }
}
impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>();
        let c = &t.colors;
        let clear = gpui::transparent_black();
        let (bg, border, fg) = match self.tone {
            BadgeTone::Neutral => (c.raised, c.border, c.fg),
            BadgeTone::Outline => (clear, c.border_strong, c.fg_muted),
            BadgeTone::Danger => (clear, c.danger, c.danger),
            BadgeTone::Success => (clear, c.success, c.success),
            BadgeTone::Warning => (clear, c.warning, c.warning),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .px(t.space(2))
            .h(t.space(5))
            .border_1()
            .border_color(border)
            .rounded(t.rem(rem::RADIUS_FULL))
            .bg(bg)
            .text_color(fg)
            .text_size(t.rem(t.ty(crate::theme::SMALL).size))
            .font_weight(FontWeight(t.ty(crate::theme::LABEL).weight))
            .line_height(gpui::relative(1.))
            .whitespace_nowrap()
            .child(self.text)
    }
}

// ---------- Skeleton ----------
/// Placeholder memuat: kilau raised→hover→raised 1.4s ease-in-out.
#[derive(IntoElement)]
pub struct Skeleton {
    id: SharedString,
}
pub fn skeleton(id: impl Into<SharedString>) -> Skeleton {
    Skeleton { id: id.into() }
}
impl RenderOnce for Skeleton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>();
        let (raised, hover) = (t.colors.raised, t.colors.hover);
        div()
            .min_h(t.space(4))
            .w_full()
            .rounded(t.rem(rem::RADIUS_MD))
            .bg(raised)
            .with_animation(
                self.id,
                Animation::new(Duration::from_millis(1400)).repeat().with_easing(gpui::ease_in_out),
                move |el, d| {
                    let k = 1. - (2. * d - 1.).abs();
                    el.bg(crate::theme::mix(hover, k, raised))
                },
            )
    }
}

// ---------- Kbd ----------
/// `Ctrl+Shift+K` → [Ctrl] + [Shift] + [K].
#[derive(IntoElement)]
pub struct Kbd {
    keys: SharedString,
}
pub fn kbd(keys: impl Into<SharedString>) -> Kbd {
    Kbd { keys: keys.into() }
}
impl RenderOnce for Kbd {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>();
        let parts: Vec<String> = self.keys.split('+').filter(|s| !s.is_empty()).map(String::from).collect();
        let n = parts.len();
        let mut row = div().flex().items_center().gap(t.space(1));
        for (i, p) in parts.into_iter().enumerate() {
            row = row.child(
                div()
                    .min_w(t.space(4))
                    .px(t.space(1))
                    .border_1()
                    .border_color(t.colors.border)
                    .rounded(t.rem(rem::RADIUS_SM))
                    .bg(t.colors.raised)
                    .text_color(t.colors.fg_muted)
                    .font_family(t.font_code.clone())
                    .text_size(t.rem(t.ty(crate::theme::SMALL).size))
                    .line_height(gpui::relative(1.3))
                    .text_center()
                    .child(p),
            );
            if i + 1 < n {
                row = row.child(div().mx(t.space(1)).text_color(t.colors.fg_subtle).child("+"));
            }
        }
        row
    }
}

// ---------- Avatar ----------
/// Inisial: huruf pertama kata pertama + kata terakhir.
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.is_empty() {
        return "?".into();
    }
    let first = words[0].chars().next().map(String::from).unwrap_or_default();
    let last = if words.len() > 1 {
        words[words.len() - 1].chars().next().map(String::from).unwrap_or_default()
    } else {
        String::new()
    };
    first + &last
}

#[derive(IntoElement)]
pub struct Avatar {
    name: SharedString,
    src: Option<gpui::ImageSource>,
    size: ControlSize,
}
pub fn avatar(name: impl Into<SharedString>) -> Avatar {
    Avatar { name: name.into(), src: None, size: ControlSize::Md }
}
impl Avatar {
    pub fn src(mut self, src: impl Into<gpui::ImageSource>) -> Self {
        self.src = Some(src.into());
        self
    }
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }
}
impl RenderOnce for Avatar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>();
        let s = t.rem(self.size.height_rem());
        let base = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(s)
            .overflow_hidden()
            .rounded(t.rem(rem::RADIUS_FULL))
            .bg(t.colors.raised)
            .border_1()
            .border_color(t.colors.border)
            .text_color(t.colors.fg)
            .text_size(t.rem(t.ty(crate::theme::SMALL).size))
            .font_weight(FontWeight(t.ty(crate::theme::LABEL).weight));
        match self.src {
            // GPUI memotong `overflow_hidden` secara persegi: gambar sendiri harus ikut membulat.
            Some(src) => base.child(
                gpui::img(src)
                    .size_full()
                    .rounded(t.rem(rem::RADIUS_FULL))
                    .object_fit(gpui::ObjectFit::Cover),
            ),
            None => base.child(initials(&self.name).to_uppercase()),
        }
    }
}

// ---------- Working ----------
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkingVariant {
    #[default]
    Wave,
    Ripple,
    Orbit,
    Grid,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkingSize {
    Dot,
    Sm,
    #[default]
    Md,
}

#[derive(IntoElement)]
pub struct Working {
    id: SharedString,
    variant: WorkingVariant,
    size: WorkingSize,
    color: Option<Hsla>,
}
pub fn working(id: impl Into<SharedString>) -> Working {
    Working { id: id.into(), variant: WorkingVariant::Wave, size: WorkingSize::Md, color: None }
}
impl Working {
    pub fn variant(mut self, v: WorkingVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: WorkingSize) -> Self {
        self.size = s;
        self
    }
    pub fn color(mut self, c: Hsla) -> Self {
        self.color = Some(c);
        self
    }
}

/// Kurva keyframe 0→1→0 dengan puncak di `peak` (0..1).
fn bump(phase: f32, peak: f32) -> f32 {
    if phase < peak { phase / peak } else { (1. - phase) / (1. - peak) }
}

impl RenderOnce for Working {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>();
        let side = match self.size {
            WorkingSize::Dot => t.space(3),
            WorkingSize::Sm => t.rem(rem::ICON_SM),
            WorkingSize::Md => t.rem(rem::ICON_MD),
        };
        let color = self.color.unwrap_or(t.colors.fg);
        let id = self.id;
        let root = div().relative().flex().flex_none().items_center().justify_center().size(side);
        match self.variant {
            WorkingVariant::Wave => {
                let bar_w = t.rem(0.125);
                let mut row = root.gap(t.rem(0.0625));
                for i in 0..5u64 {
                    let off = (i * 120) as f32 / 1100.;
                    row = row.child(
                        div().w(bar_w).h(side).flex().items_center().child(
                            div().w(bar_w).rounded(t.rem(rem::RADIUS_FULL)).bg(color).with_animation(
                                SharedString::from(format!("{id}-w{i}")),
                                Animation::new(Duration::from_millis(1100)).repeat(),
                                move |el, d| {
                                    let k = bump((d + 1. - off) % 1., 0.45);
                                    el.h(side * (0.25 + 0.75 * k)).opacity(0.45 + 0.55 * k)
                                },
                            ),
                        ),
                    );
                }
                row
            }
            WorkingVariant::Grid => {
                // 3 baris × 3 kolom eksplisit (bukan flex-wrap): pembungkusan bergantung pada muatnya 3 sel + 2 celah
                // di kotak sekecil 12px, dan pembulatan/skala UI membuat sel turun baris (kolom jadi < 3).
                let (cell, gap) = (side * 0.26, side * 0.11);
                let delays = [0u64, 120, 240, 120, 240, 360, 240, 360, 480];
                let mut grid = root.flex_col().gap(gap);
                for r in 0..3usize {
                    let mut row = div().flex().flex_none().gap(gap);
                    for c in 0..3usize {
                        let i = r * 3 + c;
                        let off = delays[i] as f32 / 1400.;
                        row = row.child(
                            div().flex_none().size(cell).rounded(cell * 0.3).bg(color).with_animation(
                                SharedString::from(format!("{id}-g{i}")),
                                Animation::new(Duration::from_millis(1400)).repeat().with_easing(gpui::ease_in_out),
                                move |el, d| el.opacity(0.2 + 0.8 * bump((d + 1. - off) % 1., 0.5)),
                            ),
                        );
                    }
                    grid = grid.child(row);
                }
                grid
            }
            WorkingVariant::Ripple | WorkingVariant::Orbit => root
                .child(div().absolute().size(side).rounded(side).bg(color).opacity(0.08))
                .child(
                    div().size(side * 0.34).rounded(side).bg(color).with_animation(
                        SharedString::from(format!("{id}-r")),
                        Animation::new(Duration::from_millis(1800)).repeat().with_easing(gpui::ease_in_out),
                        move |el, d| el.opacity(0.7 + 0.3 * bump(d, 0.5)),
                    ),
                ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::initials;
    #[test]
    fn inisial() {
        assert_eq!(initials("Abdi Guna Setiawan"), "AS");
        assert_eq!(initials("  zandor "), "z");
        assert_eq!(initials(""), "?");
    }
}

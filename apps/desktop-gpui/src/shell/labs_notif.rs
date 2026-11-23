//! Labs → Notifikasi: 30 alternatif desain toast untuk dibandingkan sebelum dipasang ke `ui/toast.rs`.
//! Tiap kartu menampilkan tiga toast (sukses + aksi, peringatan, galat judul saja) dengan warna dari tema aktif.
//! Hanya pratinjau statis: tidak mengirim notifikasi, tidak mengubah pengaturan.
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H2, H3, H4, LABEL, SMALL};
use crate::ui::{icon, IconSize, ToastTone, TypeExt};
use gpui::{div, prelude::*, AnyElement, FontWeight, Hsla, IntoElement};

#[derive(Clone, Copy)]
enum Ic {
    None,
    Plain,
    Square,
    Circle,
    Dot,
    Big,
    Ring,
    Solid,
}

#[derive(Clone, Copy)]
enum Surf {
    Composer,
    Raised,
    Hover,
    Bg,
    Tint,
    Invert,
}

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Fill,
    Text,
    Outline,
    Inline,
}

#[derive(Clone, Copy)]
struct Look {
    surf: Surf,
    ic: Ic,
    radius: f32,
    /// 0 = tanpa border, 1 = border soft, 2 = border biasa.
    bw: u8,
    /// 0 = tanpa bayangan, 1 = shadow-1, 2 = panel, 3 = shadow-3.
    shadow: u8,
    pad: usize,
    w: f32,
    header: bool,
    act: Act,
    progress: bool,
    close: bool,
    center: bool,
    title_only: bool,
    big: bool,
    stripe: bool,
    /// Set glyph: 0 = bawaan, 1 = sederhana, 2 = tegas, 3 = ekspresif.
    gs: u8,
    /// Warna ikon: 0 = warna status, 1 = redup, 2 = teks utama.
    icc: u8,
}

fn base() -> Look {
    Look { surf: Surf::Composer, ic: Ic::Plain, radius: rem::RADIUS_LG, bw: 1, shadow: 2, pad: 3, w: 22., header: false, act: Act::Fill, progress: false, close: true, center: false, title_only: false, big: false, stripe: false, gs: 0, icc: 0 }
}

fn looks() -> [Look; 30] {
    let b = Look { shadow: 0, ..base() };
    let s1 = Look { shadow: 1, ..b };
    let line = Look { bw: 2, ..b };
    [
        b,
        Look { gs: 1, ..b },
        Look { gs: 2, ..b },
        Look { gs: 3, ..b },
        Look { icc: 1, ..b },
        Look { icc: 2, gs: 1, ..b },
        Look { ic: Ic::Square, ..b },
        Look { ic: Ic::Circle, ..b },
        Look { ic: Ic::Ring, ..b },
        Look { ic: Ic::Solid, gs: 1, ..b },
        Look { ic: Ic::Dot, ..b },
        Look { ic: Ic::None, ..b },
        s1,
        Look { gs: 1, ..s1 },
        Look { ic: Ic::Circle, gs: 1, ..s1 },
        Look { ic: Ic::Solid, gs: 1, ..s1 },
        line,
        Look { ic: Ic::Ring, ..line },
        Look { icc: 1, ..line },
        Look { ic: Ic::Square, gs: 2, ..line },
        Look { icc: 1, gs: 3, ..s1 },
        Look { ic: Ic::Solid, gs: 2, ..b },
        Look { ic: Ic::Ring, gs: 3, ..b },
        Look { ic: Ic::Square, icc: 1, ..b },
        Look { ic: Ic::Big, ..b },
        Look { ic: Ic::Big, gs: 1, ..s1 },
        Look { ic: Ic::Circle, icc: 1, gs: 1, ..b },
        Look { ic: Ic::Dot, progress: true, ..b },
        Look { ic: Ic::Circle, header: true, ..b },
        Look { ic: Ic::Ring, icc: 1, gs: 1, ..line },
    ]
}

fn preview(t: &Theme, l: &Look, tone: ToastTone, title: String, desc: Option<String>, action: Option<String>, brand_now: &str) -> AnyElement {
    let c = &t.colors;
    let tone_c: Hsla = match tone {
        ToastTone::Neutral | ToastTone::Info => c.fg_muted,
        ToastTone::Success => c.success,
        ToastTone::Danger => c.danger,
        ToastTone::Warning => c.warning,
    };
    let kind = match tone {
        ToastTone::Neutral | ToastTone::Info => 0,
        ToastTone::Success => 1,
        ToastTone::Danger => 2,
        ToastTone::Warning => 3,
    };
    let glyph: &str = [
        ["info", "badge_check", "octagon_x", "triangle_alert"],
        ["info", "check", "close", "triangle_alert"],
        ["info", "check", "stop", "info"],
        ["bell", "sparkles", "shield", "ping"],
    ][l.gs as usize][kind];
    let icon_c = match l.icc {
        0 => tone_c,
        1 => c.fg_muted,
        _ => c.fg,
    };
    let chip = if l.icc == 0 { tone_c.opacity(0.16) } else { c.hover };
    let (bg, fg, muted, subtle) = match l.surf {
        Surf::Composer | Surf::Tint => (c.composer, c.fg, c.fg_muted, c.fg_subtle),
        Surf::Raised => (c.raised, c.fg, c.fg_muted, c.fg_subtle),
        Surf::Hover => (c.hover, c.fg, c.fg_muted, c.fg_subtle),
        Surf::Bg => (c.bg, c.fg, c.fg_muted, c.fg_subtle),
        Surf::Invert => (c.fg, c.bg, c.bg.opacity(0.7), c.bg.opacity(0.5)),
    };
    let tint = matches!(l.surf, Surf::Tint).then(|| tone_c.opacity(0.14));
    let size = |s: crate::theme::TypeStyle| t.rem(t.ty(s).size);
    let ic_el: Option<AnyElement> = match l.ic {
        Ic::None => None,
        Ic::Plain => Some(div().mt(t.rem(0.0625)).child(icon(glyph).color(icon_c)).into_any_element()),
        Ic::Square => Some(div().size(t.rem(2.)).flex_none().flex().items_center().justify_center().rounded(t.rem(rem::RADIUS_MD)).bg(chip).child(icon(glyph).color(icon_c)).into_any_element()),
        Ic::Circle => Some(div().size(t.rem(2.)).flex_none().flex().items_center().justify_center().rounded_full().bg(chip).child(icon(glyph).color(icon_c)).into_any_element()),
        Ic::Ring => Some(div().size(t.rem(2.)).flex_none().flex().items_center().justify_center().rounded_full().border_1().border_color(if l.icc == 0 { tone_c.opacity(0.5) } else { c.border }).child(icon(glyph).color(icon_c)).into_any_element()),
        Ic::Solid => Some(div().mt(t.rem(0.0625)).size(t.rem(1.5)).flex_none().flex().items_center().justify_center().rounded_full().bg(icon_c).child(icon(glyph).size(IconSize::Sm).color(c.bg)).into_any_element()),
        Ic::Dot => Some(div().mt(t.rem(0.375)).size(t.rem(0.5)).flex_none().rounded_full().bg(tone_c).into_any_element()),
        Ic::Big => Some(div().size(t.rem(2.75)).flex_none().flex().items_center().justify_center().rounded(t.rem(rem::RADIUS_MD)).bg(icon_c).child(icon(glyph).color(c.bg)).into_any_element()),
    };
    let title_el = div().text_size(size(if l.big { H4 } else { LABEL })).font_weight(FontWeight(t.ty(H4).weight)).child(title);
    let act_el = (!l.title_only).then_some(action).flatten().map(|label| {
        let d = div().flex().items_center().h(t.rem(rem::CONTROL_SM)).text_size(size(SMALL)).font_weight(FontWeight(t.ty(LABEL).weight));
        match l.act {
            Act::Fill | Act::Inline => d.px(t.space(3)).rounded(t.rem(rem::RADIUS_MD)).bg(if matches!(l.surf, Surf::Invert) { c.bg.opacity(0.16) } else { c.hover }).child(label),
            Act::Text => d.text_color(fg).underline().child(label),
            Act::Outline => d.px(t.space(3)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(if matches!(l.surf, Surf::Invert) { c.bg.opacity(0.4) } else { c.border }).child(label),
        }
    });
    let inline = l.act == Act::Inline;
    let mut content = div().flex().flex_1().flex_col().gap(t.space(1)).min_w_0().when(l.center, |d| d.items_center().text_center()).when(!l.center, |d| d.items_start()).child(title_el);
    if let (Some(d), false) = (desc, l.title_only) {
        content = content.child(div().text_size(size(SMALL)).text_color(muted).child(d));
    }
    let mut act_below = None;
    let mut act_inline = None;
    if let Some(a) = act_el {
        if inline {
            act_inline = Some(a);
        } else {
            act_below = Some(a);
        }
    }
    if let Some(a) = act_below {
        content = content.child(div().mt(t.space(1)).child(a));
    }
    let mut row = div().flex().items_start().gap(t.space(3)).p(t.space(l.pad)).when_some(tint, |d, c| d.bg(c));
    if l.title_only {
        row = row.items_center();
    }
    if let Some(i) = ic_el {
        row = row.child(i);
    }
    row = row.child(content);
    if let Some(a) = act_inline {
        row = row.child(div().flex_none().self_center().child(a));
    }
    if l.close {
        row = row.child(div().flex_none().p(t.space(1)).child(icon("close").size(IconSize::Sm).color(subtle)));
    }
    let mut root = div().flex().flex_col().w(t.rem(l.w)).flex_none().overflow_hidden().rounded(t.rem(l.radius)).bg(bg).text_color(fg);
    root = match l.bw {
        0 => root,
        1 => root.border_1().border_color(c.border_soft),
        _ => root.border_1().border_color(c.border),
    };
    root = match l.shadow {
        0 => root,
        1 => root.shadow(t.shadow_1()),
        2 => root.shadow(t.shadow_panel()),
        _ => root.shadow(t.shadow_3()),
    };
    if l.stripe {
        root = root.child(div().h(t.rem(0.1875)).w_full().bg(tone_c));
    }
    if l.header {
        root = root.child(
            div()
                .flex()
                .items_center()
                .gap(t.space(2))
                .px(t.space(l.pad))
                .pt(t.space(2))
                .text_size(size(SMALL))
                .text_color(muted)
                .child(icon("bell").size(IconSize::Sm).color(muted))
                .child(div().flex_1().child("Zandor"))
                .child(div().child(brand_now.to_string())),
        );
    }
    root = root.child(row);
    if l.progress {
        root = root.child(div().h(t.rem(0.125)).w(gpui::relative(0.6)).bg(tone_c));
    }
    root.into_any_element()
}

pub fn view(t: &Theme, i: &I18n) -> AnyElement {
    let sample = i.t("labs.notification.sample");
    let now = i.t("labs.notification.now");
    let cards = looks().into_iter().enumerate().map(|(n, l)| {
        div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .p(t.space(3))
            .rounded(t.rem(rem::RADIUS_LG))
            .border_1()
            .border_color(t.colors.border_soft)
            .child(div().typo(t, LABEL).child(format!("{}. {}", n + 1, i.t(&format!("labs.notification.v{}", n + 1)))))
            .child(preview(t, &l, ToastTone::Success, i.t("notify.done"), Some(sample.clone()), Some(i.t("notify.open")), &now))
            .child(preview(t, &l, ToastTone::Warning, i.t("notify.permission"), Some(sample.clone()), None, &now))
            .child(preview(t, &l, ToastTone::Danger, i.t("notify.failed"), None, None, &now))
    });
    div()
        .flex()
        .flex_col()
        .gap(t.space(4))
        .child(div().typo(t, H2).child(i.t("labs.page.notification")))
        .child(div().small(t).text_color(t.colors.fg_muted).child(i.t("labs.notification.desc")))
        .child(div().typo(t, H3).child(i.t("labs.notification.variants")))
        .child(div().flex().flex_wrap().items_start().gap(t.space(3)).children(cards))
        .into_any_element()
}

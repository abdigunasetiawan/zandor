//! Color picker standar: kotak saturasi (kiri→kanan) × kecerahan (bawah→atas) dan bilah hue pelangi di bawahnya.
//! Dipakai dialog kustomisasi project. Memancarkan `ColorPicked("#rrggbb")` saat pengguna memilih;
//! `set_hex` menyelaraskan pemilih dengan kolom hex tanpa memancarkan peristiwa.
use crate::theme::{rem, Theme};
use gpui::{canvas, div, point, prelude::*, px, size, Bounds, Context, EventEmitter, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Rgba, Window};

pub struct ColorPicked(pub String);

const CELLS: usize = 28;
const HUE_STEPS: usize = 72;

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    None,
    Square,
    Hue,
}

pub struct ColorPicker {
    hue: f32,
    sat: f32,
    val: f32,
    square: Option<Bounds<Pixels>>,
    bar: Option<Bounds<Pixels>>,
    drag: Drag,
}
impl EventEmitter<ColorPicked> for ColorPicker {}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let hp = (h % 360. + 360.) % 360. / 60.;
    let x = c * (1. - (hp % 2. - 1.).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = v - c;
    (r + m, g + m, b + m)
}

pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let d = max - r.min(g).min(b);
    let h = if d == 0. {
        0.
    } else if max == r {
        60. * (((g - b) / d) % 6.)
    } else if max == g {
        60. * ((b - r) / d + 2.)
    } else {
        60. * ((r - g) / d + 4.)
    };
    (if h < 0. { h + 360. } else { h }, if max == 0. { 0. } else { d / max }, max)
}

fn rgba(h: f32, s: f32, v: f32) -> Rgba {
    let (r, g, b) = hsv_to_rgb(h, s, v);
    Rgba { r, g, b, a: 1. }
}

fn to_hex(h: f32, s: f32, v: f32) -> String {
    let c = rgba(h, s, v);
    let f = |x: f32| (x.clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", f(c.r), f(c.g), f(c.b))
}

impl ColorPicker {
    pub fn new(hex: &str, cx: &mut Context<Self>) -> Self {
        let mut me = Self { hue: 0., sat: 0., val: 1., square: None, bar: None, drag: Drag::None };
        me.set_hex(hex, cx);
        me
    }

    /// Selaraskan dengan nilai hex (abaikan bila bukan warna valid). Tidak memancarkan peristiwa.
    pub fn set_hex(&mut self, hex: &str, cx: &mut Context<Self>) {
        let Some(c) = crate::core::appearance::parse_color(hex) else { return };
        let c: Rgba = c.into();
        let (h, s, v) = rgb_to_hsv(c.r, c.g, c.b);
        if to_hex(self.hue, self.sat, self.val) == to_hex(h, s, v) {
            return;
        }
        // Abu-abu/hitam tidak punya hue bermakna: pertahankan hue yang ada.
        if s > 0.001 && v > 0.001 {
            self.hue = h;
        }
        self.sat = s;
        self.val = v;
        cx.notify();
    }

    fn emit_color(&mut self, cx: &mut Context<Self>) {
        cx.emit(ColorPicked(to_hex(self.hue, self.sat, self.val)));
        cx.notify();
    }

    fn pick(&mut self, pos: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        match self.drag {
            Drag::Square => {
                let Some(b) = self.square else { return };
                self.sat = ((pos.x - b.left()) / b.size.width).clamp(0., 1.);
                self.val = 1. - ((pos.y - b.top()) / b.size.height).clamp(0., 1.);
            }
            Drag::Hue => {
                let Some(b) = self.bar else { return };
                self.hue = ((pos.x - b.left()) / b.size.width).clamp(0., 1.) * 360.;
            }
            Drag::None => return,
        }
        self.emit_color(cx);
    }
}

impl Render for ColorPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let (sq_entity, bar_entity) = (cx.entity(), cx.entity());
        let (hue, sat, val) = (self.hue, self.sat, self.val);
        let width = t.rem(14.);
        let marker = t.space(4);
        let current = crate::core::appearance::parse_color(&to_hex(hue, sat, val));

        let square = div()
            .id("cp-square")
            .relative()
            .w(width)
            .h(t.rem(9.))
            .rounded(t.rem(rem::RADIUS_SM))
            .overflow_hidden()
            .cursor_crosshair()
            .child(
                canvas(
                    move |b, _, cx| sq_entity.update(cx, |s, _| s.square = Some(b)),
                    move |b, _, w, _| {
                        let (cw, ch) = (f32::from(b.size.width) / CELLS as f32, f32::from(b.size.height) / CELLS as f32);
                        for y in 0..CELLS {
                            for x in 0..CELLS {
                                let c = rgba(hue, (x as f32 + 0.5) / CELLS as f32, 1. - (y as f32 + 0.5) / CELLS as f32);
                                // +1px agar tidak ada celah antar-sel.
                                let cell = Bounds::new(point(b.origin.x + px(x as f32 * cw), b.origin.y + px(y as f32 * ch)), size(px(cw + 1.), px(ch + 1.)));
                                w.paint_quad(gpui::fill(cell, c));
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .left(gpui::relative(sat))
                    .top(gpui::relative(1. - val))
                    .ml(-marker / 2.)
                    .mt(-marker / 2.)
                    .size(marker)
                    .rounded(t.rem(rem::RADIUS_FULL))
                    .border_2()
                    .border_color(gpui::white())
                    .shadow(t.shadow_1())
                    .when_some(current, |d, c| d.bg(c)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, e: &MouseDownEvent, _, cx| {
                    s.drag = Drag::Square;
                    s.pick(e.position, cx);
                }),
            );

        let bar = div()
            .id("cp-hue")
            .relative()
            .w(width)
            .h(t.space(4))
            .rounded(t.rem(rem::RADIUS_FULL))
            .cursor_pointer()
            .child(
                canvas(
                    move |b, _, cx| bar_entity.update(cx, |s, _| s.bar = Some(b)),
                    move |b, _, w, _| {
                        let sw = f32::from(b.size.width) / HUE_STEPS as f32;
                        for k in 0..HUE_STEPS {
                            let cell = Bounds::new(point(b.origin.x + px(k as f32 * sw), b.origin.y), size(px(sw + 1.), b.size.height));
                            w.paint_quad(gpui::fill(cell, rgba((k as f32 + 0.5) / HUE_STEPS as f32 * 360., 1., 1.)));
                        }
                    },
                )
                .absolute()
                .size_full()
                .rounded(t.rem(rem::RADIUS_FULL)),
            )
            .child(
                div()
                    .absolute()
                    .left(gpui::relative(hue / 360.))
                    .top_0()
                    .ml(-t.space(2))
                    .size(t.space(4))
                    .rounded(t.rem(rem::RADIUS_FULL))
                    .border_2()
                    .border_color(gpui::white())
                    .shadow(t.shadow_1())
                    .bg(rgba(hue, 1., 1.)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, e: &MouseDownEvent, _, cx| {
                    s.drag = Drag::Hue;
                    s.pick(e.position, cx);
                }),
            );

        div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .child(square)
            .child(bar)
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                if s.drag != Drag::None && e.dragging() {
                    s.pick(e.position, cx);
                } else {
                    s.drag = Drag::None;
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|s, _, _, _| s.drag = Drag::None))
    }
}

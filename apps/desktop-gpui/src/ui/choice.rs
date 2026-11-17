//! Switch, Checkbox, Slider — port Fields.tsx + .z-switch/.z-checkbox/.z-slider di atoms.css.
use super::icon::{icon, IconSize};
use crate::theme::{rem, Theme, LABEL};
use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Context, ElementId, EventEmitter, FontWeight,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, SharedString, Window,
};

type OnToggle = Box<dyn Fn(bool, &mut Window, &mut App) + 'static>;

fn choice_label(t: &Theme, label: SharedString) -> impl IntoElement {
    div()
        .text_size(t.rem(t.ty(crate::theme::LABEL).size))
        .font_weight(FontWeight(t.ty(crate::theme::LABEL).weight))
        .line_height(gpui::relative(t.ty(crate::theme::LABEL).lh))
        .text_color(t.colors.fg)
        .child(label)
}

// ---------- Switch ----------
#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    label: SharedString,
    checked: bool,
    disabled: bool,
    on_change: Option<OnToggle>,
}
pub fn switch(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Switch {
    Switch { id: id.into(), label: label.into(), checked, disabled: false, on_change: None }
}
impl Switch {
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
impl RenderOnce for Switch {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = &t.colors;
        // --sw-h = space-5, --sw-w = 1.9×, --sw-gap = space-1/2
        let h = rem::SPACE[5];
        let w = h * 1.9;
        let gap = rem::SPACE[1] / 2.;
        let thumb = t.rem(h - 2. * gap) - px(2.);
        let (bg, border, thumb_bg) = if self.checked {
            (c.accent, c.accent, c.on_accent)
        } else {
            (c.raised, c.border_strong, c.fg_muted)
        };
        let checked = self.checked;
        let mut track = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .w(t.rem(w))
            .h(t.rem(h))
            .p(t.rem(gap))
            .border_1()
            .border_color(border)
            .rounded(t.rem(rem::RADIUS_FULL))
            .bg(bg)
            .child(
                div()
                    .flex_none()
                    .size(thumb)
                    .rounded(t.rem(rem::RADIUS_FULL))
                    .bg(thumb_bg)
                    .when(checked, |d| d.ml(t.rem(w - h))),
            );
        if self.disabled {
            track = track.opacity(0.5).cursor_not_allowed();
        } else {
            track = track.cursor_pointer();
            if let Some(f) = self.on_change {
                track = track.on_click(move |_, w, cx| f(!checked, w, cx));
            }
        }
        div().flex().items_center().gap(t.space(2)).child(track).child(choice_label(&t, self.label))
    }
}

// ---------- Checkbox ----------
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    label: SharedString,
    checked: bool,
    indeterminate: bool,
    disabled: bool,
    on_change: Option<OnToggle>,
}
pub fn checkbox(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Checkbox {
    Checkbox { id: id.into(), label: label.into(), checked, indeterminate: false, disabled: false, on_change: None }
}
impl Checkbox {
    pub fn indeterminate(mut self, v: bool) -> Self {
        self.indeterminate = v;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = &t.colors;
        let on = self.checked || self.indeterminate;
        let checked = self.checked;
        let mut b = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(t.space(4))
            .border_1()
            .border_color(if on { c.accent } else { c.border_strong })
            .rounded(t.rem(rem::RADIUS_SM))
            .bg(if on { c.accent } else { c.surface });
        if self.indeterminate {
            b = b.child(div().w(t.space(2)).h(px(2.)).bg(c.on_accent));
        } else if self.checked {
            b = b.child(icon("check").size(IconSize::Sm).color(c.on_accent));
        }
        if self.disabled {
            b = b.opacity(0.5).cursor_not_allowed();
        } else {
            b = b.cursor_pointer();
            if let Some(f) = self.on_change {
                b = b.on_click(move |_, w, cx| f(!checked, w, cx));
            }
        }
        div().flex().items_center().gap(t.space(2)).child(b).child(choice_label(&t, self.label))
    }
}

// ---------- Slider ----------
pub struct SliderChanged(pub f32);

/// Slider berkeadaan (butuh bounds trek untuk seret). Pancarkan `SliderChanged`.
pub struct Slider {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub label: SharedString,
    pub disabled: bool,
    track: Option<Bounds<Pixels>>,
    dragging: bool,
}
impl EventEmitter<SliderChanged> for Slider {}

impl Slider {
    pub fn new(value: f32, min: f32, max: f32, step: f32, label: impl Into<SharedString>) -> Self {
        Self { value, min, max, step, label: label.into(), disabled: false, track: None, dragging: false }
    }
    fn set_from_x(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let Some(b) = self.track else { return };
        let ratio = ((x - b.left()) / b.size.width).clamp(0., 1.);
        let raw = self.min + ratio * (self.max - self.min);
        let stepped = (((raw - self.min) / self.step).round() * self.step + self.min).clamp(self.min, self.max);
        if (stepped - self.value).abs() > f32::EPSILON {
            self.value = stepped;
            cx.emit(SliderChanged(stepped));
            cx.notify();
        }
    }
}

impl Render for Slider {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = &t.colors;
        let span = (self.max - self.min).max(f32::EPSILON);
        let ratio = ((self.value - self.min) / span).clamp(0., 1.);
        let thumb = t.space(4);
        let this = cx.entity();
        let mut root = div()
            .id("slider")
            .w_full()
            .min_w(t.space(7))
            .h(t.rem(rem::CONTROL_MD))
            .flex()
            .items_center()
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(t.space(1))
                    .rounded(t.rem(rem::RADIUS_FULL))
                    .bg(c.border_strong)
                    .child(
                        canvas(
                            move |b, _, cx| this.update(cx, |s, _| s.track = Some(b)),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(div().h_full().w(gpui::relative(ratio)).rounded(t.rem(rem::RADIUS_FULL)).bg(c.accent))
                    .child(
                        div()
                            .absolute()
                            .top(-(thumb - t.space(1)) / 2.)
                            .left(gpui::relative(ratio))
                            .ml(-thumb / 2.)
                            .size(thumb)
                            .rounded(t.rem(rem::RADIUS_FULL))
                            .bg(c.accent)
                            .border_2()
                            .border_color(c.bg)
                            .shadow(t.shadow_1()),
                    ),
            );
        if self.disabled {
            root = root.opacity(0.5);
        } else {
            root = root
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|s, e: &MouseDownEvent, _, cx| {
                        s.dragging = true;
                        s.set_from_x(e.position.x, cx);
                    }),
                )
                .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                    if s.dragging && e.dragging() {
                        s.set_from_x(e.position.x, cx);
                    } else {
                        s.dragging = false;
                    }
                }))
                .on_mouse_up(MouseButton::Left, cx.listener(|s, _, _, _| s.dragging = false));
        }
        root
    }
}

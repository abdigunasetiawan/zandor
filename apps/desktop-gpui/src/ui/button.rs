//! Button & IconButton — port 1:1 dari packages/ui/src/atoms/Button.tsx + .z-button di atoms.css.
use super::icon::{icon, IconSize};
use super::spinner::spinner;
use crate::theme::{rem, Theme, BUTTON};
use gpui::{
    div, prelude::*, AnyElement, App, ClickEvent, ElementId, FontWeight, IntoElement,
    SharedString, Window,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    Solid,
    #[default]
    Outline,
    Ghost,
    Danger,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl ControlSize {
    pub fn height_rem(self) -> f32 {
        match self {
            ControlSize::Sm => rem::CONTROL_SM,
            ControlSize::Md => rem::CONTROL_MD,
            ControlSize::Lg => rem::CONTROL_LG,
        }
    }
    fn pad_space(self) -> usize {
        match self {
            ControlSize::Sm => 2,
            ControlSize::Md => 3,
            ControlSize::Lg => 4,
        }
    }
}

type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    variant: ButtonVariant,
    size: ControlSize,
    loading: bool,
    disabled: bool,
    square: bool,
    pressed: bool,
    tooltip: Option<SharedString>,
    children: Vec<AnyElement>,
    on_click: Option<OnClick>,
}

pub fn button(id: impl Into<ElementId>) -> Button {
    Button {
        id: id.into(),
        variant: ButtonVariant::Outline,
        size: ControlSize::Md,
        loading: false,
        disabled: false,
        square: false,
        pressed: false,
        tooltip: None,
        children: Vec::new(),
        on_click: None,
    }
}

/// Tombol ikon: varian default ghost, persegi, label wajib (tooltip).
pub fn icon_button(id: impl Into<ElementId>, icon_name: &'static str, label: impl Into<SharedString>) -> Button {
    let mut b = button(id).variant(ButtonVariant::Ghost);
    b.square = true;
    b.tooltip = Some(label.into());
    b.child(icon(icon_name))
}

impl Button {
    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }
    /// aria-pressed: tombol toggle aktif berlatar `active`.
    pub fn pressed(mut self, p: bool) -> Self {
        self.pressed = p;
        self
    }
    pub fn loading(mut self, l: bool) -> Self {
        self.loading = l;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn label(self, text: impl Into<SharedString>) -> Self {
        self.child(text.into())
    }
    pub fn child(mut self, c: impl IntoElement) -> Self {
        self.children.push(c.into_any_element());
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = &t.colors;
        let inert = self.disabled || self.loading;
        let h = t.rem(self.size.height_rem());

        let (bg, border, fg) = match self.variant {
            ButtonVariant::Solid => (c.accent, c.accent, c.on_accent),
            ButtonVariant::Outline => (c.hover, gpui::transparent_black(), c.fg),
            ButtonVariant::Ghost => (gpui::transparent_black(), gpui::transparent_black(), c.fg),
            ButtonVariant::Danger => (c.hover, gpui::transparent_black(), c.danger),
        };
        let (hover_bg, active_bg) = match self.variant {
            ButtonVariant::Solid => (c.accent, c.accent),
            ButtonVariant::Ghost => (c.hover, c.active),
            _ => (c.active, c.border_strong),
        };
        let solid = self.variant == ButtonVariant::Solid;
        let bg = if self.pressed { c.active } else { bg };

        let mut el = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(t.space(2))
            .h(h)
            .border_1()
            .border_color(border)
            .rounded(t.rem(rem::RADIUS_MD))
            .bg(bg)
            .text_color(fg)
            .text_size(t.rem(BUTTON.size))
            .font_weight(FontWeight(BUTTON.weight))
            .line_height(gpui::relative(BUTTON.lh))
            .whitespace_nowrap();
        el = if self.square { el.w(h) } else { el.px(t.space(self.size.pad_space())) };

        if inert {
            el = el.opacity(0.5).cursor_not_allowed();
        } else {
            el = el
                .cursor_pointer()
                .hover(move |s| if solid { s.opacity(0.88) } else { s.bg(hover_bg) })
                .active(move |s| if solid { s.opacity(0.76) } else { s.bg(active_bg) });
            if let Some(f) = self.on_click {
                el = el.on_click(f);
            }
        }
        if let Some(tip) = self.tooltip {
            el = el.tooltip(super::tooltip::text_tooltip(tip));
        }
        if self.loading {
            el = el.child(spinner(IconSize::Sm));
        }
        el.children(self.children)
    }
}

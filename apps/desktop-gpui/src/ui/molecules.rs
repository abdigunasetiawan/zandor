//! Molekul: Popup, MenuItem, Separator, Tabs, Field, Dialog — port molecules.css + atoms.css (.z-popup/.z-item).
use super::icon::{icon, IconSize};
use super::typo::TypeExt;
use crate::theme::{rem, Theme, BUTTON, H3, H4, LABEL};
use gpui::{
    deferred, div, prelude::*, AnimationExt, AnyElement, App, ClickEvent, Div, ElementId, FontWeight,
    IntoElement, SharedString, Window,
};

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Permukaan .z-popup (menu, select, popover).
pub fn popup(t: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w(t.space(7))
        .p(t.space(1))
        .border_1()
        .border_color(t.colors.border_soft)
        .rounded(t.rem(rem::RADIUS_LG))
        .bg(t.colors.raised)
        .text_color(t.colors.fg)
        .shadow(t.shadow_3())
}

/// .z-separator
pub fn separator(t: &Theme) -> Div {
    div().h(gpui::px(1.)).my(t.space(1)).bg(t.colors.border)
}

// ---------- MenuItem (.z-item) ----------
#[derive(IntoElement)]
pub struct MenuItem {
    id: ElementId,
    label: SharedString,
    description: Option<SharedString>,
    icon: Option<&'static str>,
    lead: Option<AnyElement>,
    checked: bool,
    danger: bool,
    disabled: bool,
    highlighted: bool,
    shortcut: Option<SharedString>,
    on_click: Option<Handler>,
}
pub fn menu_item(id: impl Into<ElementId>, label: impl Into<SharedString>) -> MenuItem {
    MenuItem {
        id: id.into(),
        label: label.into(),
        description: None,
        icon: None,
        lead: None,
        checked: false,
        danger: false,
        disabled: false,
        highlighted: false,
        shortcut: None,
        on_click: None,
    }
}
impl MenuItem {
    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.description = Some(d.into());
        self
    }
    pub fn icon(mut self, name: &'static str) -> Self {
        self.icon = Some(name);
        self
    }
    /// Elemen pembuka bebas (mis. ikon gambar aplikasi) menggantikan ikon nama.
    pub fn lead(mut self, el: AnyElement) -> Self {
        self.lead = Some(el);
        self
    }
    pub fn checked(mut self, v: bool) -> Self {
        self.checked = v;
        self
    }
    pub fn danger(mut self, v: bool) -> Self {
        self.danger = v;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn highlighted(mut self, v: bool) -> Self {
        self.highlighted = v;
        self
    }
    pub fn shortcut(mut self, s: impl Into<SharedString>) -> Self {
        self.shortcut = Some(s.into());
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}
impl RenderOnce for MenuItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = &t.colors;
        let fg = if self.danger { c.danger } else { c.fg };
        let icon_fg = if self.danger { c.danger } else { c.fg_muted };
        let hover = c.hover;
        let described = self.description.is_some();
        let mut el = div()
            .id(self.id)
            .flex()
            .gap(t.space(2))
            .min_h(t.rem(rem::CONTROL_MD))
            .px(t.space(2))
            .rounded(t.rem(rem::RADIUS_MD))
            .text_size(t.rem(t.ty(crate::theme::BUTTON).size))
            .text_color(fg)
            .when(described, |d| d.items_start().py(t.space(2)))
            .when(!described, |d| d.items_center())
            .when(self.highlighted, |d| d.bg(hover));
        if let Some(l) = self.lead {
            el = el.child(l);
        } else if let Some(name) = self.icon {
            el = el.child(icon(name).color(icon_fg));
        }
        el = match self.description {
            Some(desc) => el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .min_w_0()
                    .child(div().font_weight(FontWeight(t.ty(crate::theme::H4).weight)).child(self.label))
                    .child(div().max_w(t.rem(rem::SPACE[7] * 6.)).small(&t).child(desc)),
            ),
            None => el.child(self.label),
        };
        if let Some(s) = self.shortcut {
            el = el.child(div().ml_auto().child(super::feedback::kbd(s)));
        }
        if self.checked {
            el = el.child(div().ml_auto().child(icon("check").size(IconSize::Sm)));
        }
        if self.disabled {
            el.opacity(0.5)
        } else {
            el = el.hover(move |s| s.bg(hover));
            if let Some(f) = self.on_click {
                el = el.on_click(f);
            }
            el
        }
    }
}

// ---------- Tabs ----------
pub struct TabItem {
    pub value: SharedString,
    pub label: SharedString,
    pub icon: Option<&'static str>,
    pub disabled: bool,
}

#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    items: Vec<TabItem>,
    value: SharedString,
    vertical: bool,
    panel: Option<AnyElement>,
    on_change: Option<Box<dyn Fn(SharedString, &mut Window, &mut App) + 'static>>,
}
pub fn tabs(id: impl Into<ElementId>, items: Vec<TabItem>, value: impl Into<SharedString>) -> Tabs {
    Tabs { id: id.into(), items, value: value.into(), vertical: false, panel: None, on_change: None }
}
impl Tabs {
    pub fn vertical(mut self, v: bool) -> Self {
        self.vertical = v;
        self
    }
    /// Isi panel tab aktif.
    pub fn panel(mut self, el: impl IntoElement) -> Self {
        self.panel = Some(el.into_any_element());
        self
    }
    pub fn on_change(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
impl RenderOnce for Tabs {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = &t.colors;
        let on_change = self.on_change.map(std::rc::Rc::new);
        let mut list = div()
            .flex()
            .flex_none()
            .gap(t.space(1))
            .p(t.space(1))
            .rounded(t.rem(rem::RADIUS_LG))
            .bg(c.segment)
            .when(self.vertical, |d| d.flex_col());
        for (i, it) in self.items.into_iter().enumerate() {
            let active = it.value == self.value;
            let fg = c.fg;
            let mut tab = div()
                .id(("tab", i))
                .flex()
                .flex_none()
                .items_center()
                .when(!self.vertical, |d| d.justify_center())
                .gap(t.space(2))
                .whitespace_nowrap()
                .h(t.rem(rem::CONTROL_MD))
                .px(t.space(3))
                .border_1()
                .border_color(gpui::transparent_black())
                .rounded(t.rem(rem::RADIUS_MD))
                .text_size(t.rem(t.ty(crate::theme::BUTTON).size))
                .font_weight(FontWeight(t.ty(crate::theme::BUTTON).weight))
                .text_color(if active { c.fg } else { c.fg_muted })
                .when(active, |d| d.bg(c.segment_active).shadow(t.shadow_1()));
            if let Some(name) = it.icon {
                tab = tab.child(icon(name).color(if active { c.fg } else { c.fg_muted }));
            }
            tab = tab.child(it.label);
            if it.disabled {
                tab = tab.opacity(0.5).cursor_not_allowed();
            } else {
                tab = tab.cursor_pointer().hover(move |s| s.text_color(fg));
                if let Some(f) = on_change.clone() {
                    let v = it.value.clone();
                    tab = tab.on_click(move |_, w, cx| f(v.clone(), w, cx));
                }
            }
            list = list.child(tab);
        }
        div()
            .id(self.id)
            .flex()
            .gap(t.space(3))
            .when(self.vertical, |d| d.flex_row())
            .when(!self.vertical, |d| d.flex_col())
            .child(list)
            .children(self.panel.map(|p| div().flex_1().min_w_0().child(p)))
    }
}

// ---------- Field ----------
/// Label + kontrol + deskripsi + galat (.z-field).
pub fn field(
    t: &Theme,
    label: impl Into<SharedString>,
    control: impl IntoElement,
    description: Option<SharedString>,
    error: Option<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(t.space(1))
        .child(div().typo(t, LABEL).text_color(t.colors.fg).child(label.into()))
        .child(control)
        .children(description.map(|d| div().small(t).child(d)))
        .children(error.map(|e| div().small(t).text_color(t.colors.danger).child(e)))
}

// ---------- Dialog ----------
/// Dialog modal di tengah dengan backdrop (.z-backdrop + .z-dialog).
#[derive(IntoElement)]
pub struct Dialog {
    title: SharedString,
    description: Option<SharedString>,
    body: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    width_rem: f32,
    on_dismiss: Option<Handler>,
}
pub fn dialog(title: impl Into<SharedString>) -> Dialog {
    Dialog { title: title.into(), description: None, body: vec![], footer: vec![], width_rem: rem::SPACE[7] * 9., on_dismiss: None }
}
impl Dialog {
    pub fn description(mut self, d: impl Into<SharedString>) -> Self {
        self.description = Some(d.into());
        self
    }
    pub fn width_rem(mut self, w: f32) -> Self {
        self.width_rem = w;
        self
    }
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.body.push(el.into_any_element());
        self
    }
    pub fn footer(mut self, el: impl IntoElement) -> Self {
        self.footer.push(el.into_any_element());
        self
    }
    pub fn on_dismiss(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }
}
impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = &t.colors;
        let vp = window.viewport_size();
        let mut backdrop = div()
            .id("z-backdrop")
            .absolute()
            .inset_0()
            .bg(c.backdrop)
            .occlude();
        if let Some(f) = self.on_dismiss {
            backdrop = backdrop.on_click(f);
        }
        let panel = div()
            .id("z-dialog")
            .occlude()
            .on_click(|_, _, cx| cx.stop_propagation())
            .flex()
            .flex_col()
            .gap(t.space(3))
            .w(t.rem(self.width_rem))
            .max_w(vp.width - t.space(6))
            .max_h(vp.height - t.space(6))
            .p(t.space(5))
            .border_1()
            .border_color(c.border)
            .rounded(t.rem(rem::RADIUS_LG))
            .bg(c.raised)
            .text_color(c.fg)
            .shadow(t.shadow_3())
            .child(div().typo(&t, H3).child(self.title))
            .children(self.description.map(|d| div().text_color(c.fg_muted).child(d)))
            .child(div().id("z-dialog-body").min_h_0().overflow_y_scroll().flex().flex_col().gap(t.space(3)).children(self.body))
            .when(!self.footer.is_empty(), |d| {
                d.child(div().flex().justify_end().gap(t.space(2)).mt(t.space(2)).children(self.footer))
            });
        crate::ui::overlay_deferred(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(backdrop.with_animation(
                    "z-backdrop-anim",
                    gpui::Animation::new(std::time::Duration::from_millis(crate::theme::DURATION_BASE_MS)),
                    |el, d| el.opacity(d),
                ))
                .child(panel.with_animation(
                    "z-dialog-anim",
                    gpui::Animation::new(std::time::Duration::from_millis(crate::theme::DURATION_BASE_MS)).with_easing(gpui::ease_out_quint()),
                    |el, d| el.opacity(d),
                )),
        )
        .with_priority(60)
    }
}

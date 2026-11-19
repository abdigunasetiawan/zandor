//! Select — port @zandor/ui Select (base-ui): pemicu .z-select + popup daftar opsi dengan centang pada nilai terpilih.
use super::menu::{entry, menu_popup};
use super::{icon, ControlSize, IconSize};
use crate::theme::{rem, Theme, BUTTON};
use gpui::{div, prelude::*, Anchor, App, ElementId, IntoElement, SharedString, Window};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    items: Vec<(SharedString, SharedString)>,
    value: SharedString,
    size: ControlSize,
    disabled: bool,
    on_change: Option<Rc<dyn Fn(SharedString, &mut Window, &mut App)>>,
    /// Ikon per nilai opsi (dipakai di pemicu dan daftar).
    icons: Option<Rc<dyn Fn(&str) -> gpui::AnyElement>>,
    /// Pil ringan selebar label (port `.composer__mode`): bulat penuh, border soft, transparan.
    pill: bool,
    /// Ikon tetap di depan label pemicu (prop `icon` Select React).
    lead: Option<&'static str>,
    /// Deskripsi per nilai opsi (item Select React dengan `description`).
    descs: Option<Rc<dyn Fn(&str) -> Option<SharedString>>>,
    /// Popup dibuka ke atas (pemicu di dekat tepi bawah, mis. composer).
    up: bool,
    /// Menyatu dengan elemen di kirinya: sudut kiri atas/bawah lurus (hover tidak membulat di sisi sambungan).
    flat_left: bool,
}

/// `items`: (nilai, label).
pub fn select(id: impl Into<ElementId>, items: Vec<(SharedString, SharedString)>, value: impl Into<SharedString>) -> Select {
    Select { id: id.into(), items, value: value.into(), size: ControlSize::Sm, disabled: false, on_change: None, icons: None, pill: false, lead: None, descs: None, up: false, flat_left: false }
}

impl Select {
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn icons(mut self, f: impl Fn(&str) -> gpui::AnyElement + 'static) -> Self {
        self.icons = Some(Rc::new(f));
        self
    }
    pub fn pill(mut self, v: bool) -> Self {
        self.pill = v;
        self
    }
    pub fn descriptions(mut self, f: impl Fn(&str) -> Option<SharedString> + 'static) -> Self {
        self.descs = Some(Rc::new(f));
        self
    }
    pub fn flat_left(mut self, v: bool) -> Self {
        self.flat_left = v;
        self
    }
    pub fn up(mut self, v: bool) -> Self {
        self.up = v;
        self
    }
    pub fn lead_icon(mut self, name: &'static str) -> Self {
        self.lead = Some(name);
        self
    }
    pub fn on_change(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Select {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let open = window.use_keyed_state(self.id.clone(), cx, |_, _| false);
        let is_open = *open.read(cx);
        let label = self.items.iter().find(|(v, _)| *v == self.value).map(|(_, l)| l.clone()).unwrap_or_else(|| self.value.clone());
        let o2 = open.clone();
        let mut el = div()
            .relative()
            .child(
                div()
                    .id(self.id.clone())
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .h(t.rem(self.size.height_rem()))
                    .when(!self.pill, |d| d.min_w(t.rem(rem::SPACE[7] * 2.5)).px(t.space(3)).border_color(t.colors.border).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.surface))
                    .when(self.pill, |d| d.flex_none().gap(t.space(1)).px(t.space(2)).border_color(t.colors.border_soft).rounded_full())
                    .when(self.flat_left, |d| d.rounded_tl_none().rounded_bl_none().rounded_tr_full().rounded_br_full().border_color(gpui::transparent_black()))
                    .border_1()
                    .text_size(t.rem(t.ty(crate::theme::BUTTON).size))
                    .when(is_open && !self.pill, |d| d.border_color(t.colors.focus))
                    .when(self.disabled, |d| d.opacity(0.5).cursor_not_allowed())
                    .when(!self.disabled, |d| {
                        d.cursor_pointer().hover(|s| s.bg(t.colors.hover)).on_click(move |_, _, cx| {
                            o2.update(cx, |v, cx| {
                                *v = !*v;
                                cx.notify();
                            })
                        })
                    })
                    .children(self.lead.map(|n| icon(n).size(IconSize::Sm).color(t.colors.fg_muted)))
                    .children(self.icons.as_ref().map(|f| f(&self.value)))
                    .child(if self.pill { div().whitespace_nowrap().child(label) } else { div().flex_1().min_w_0().truncate().child(label) })
                    .child(icon("chevron_down").size(IconSize::Sm).color(t.colors.fg_muted)),
            );
        if is_open {
            let entries = self
                .items
                .iter()
                .map(|(v, l)| {
                    let (v2, cb) = (v.clone(), self.on_change.clone());
                    let e = entry(l.clone(), move |w, cx| {
                        if let Some(f) = &cb {
                            f(v2.clone(), w, cx);
                        }
                    })
                    .checked(*v == self.value);
                    let e = match self.descs.as_ref().and_then(|f| f(v)) {
                        Some(d) => e.description(d),
                        None => e,
                    };
                    match self.icons.clone() {
                        Some(f) => {
                            let v3 = v.clone();
                            e.lead(move || f(&v3))
                        }
                        None => e,
                    }
                })
                .collect();
            let o3 = open.clone();
            el = el.child(menu_popup(SharedString::from(format!("{:?}-pop", self.id)), &t, entries, if self.up { Anchor::BottomLeft } else { Anchor::TopLeft }, move |_, cx| {
                o3.update(cx, |v, cx| {
                    *v = false;
                    cx.notify();
                })
            }));
        }
        el
    }
}

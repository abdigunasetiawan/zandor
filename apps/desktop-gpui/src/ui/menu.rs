//! Menu popup (Menu/ContextMenu base-ui) — daftar entri .z-item di .z-popup, ditambatkan ke pemicu.
use super::molecules::{menu_item, popup, separator};
use crate::theme::{rem, Theme};
use gpui::{anchored, deferred, div, prelude::*, px, AnyElement, Anchor, App, IntoElement, SharedString, Window};
use std::rc::Rc;

pub type Action = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub enum MenuEntry {
    Item {
        label: SharedString,
        icon: Option<&'static str>,
        shortcut: Option<SharedString>,
        danger: bool,
        disabled: bool,
        checked: bool,
        action: Action,
    },
    Separator,
}

pub fn entry(label: impl Into<SharedString>, action: impl Fn(&mut Window, &mut App) + 'static) -> MenuEntry {
    MenuEntry::Item {
        label: label.into(),
        icon: None,
        shortcut: None,
        danger: false,
        disabled: false,
        checked: false,
        action: Rc::new(action),
    }
}

impl MenuEntry {
    pub fn icon(mut self, name: &'static str) -> Self {
        if let MenuEntry::Item { icon, .. } = &mut self {
            *icon = Some(name);
        }
        self
    }
    pub fn danger(mut self) -> Self {
        if let MenuEntry::Item { danger, .. } = &mut self {
            *danger = true;
        }
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        if let MenuEntry::Item { disabled, .. } = &mut self {
            *disabled = v;
        }
        self
    }
    pub fn checked(mut self, v: bool) -> Self {
        if let MenuEntry::Item { checked, .. } = &mut self {
            *checked = v;
        }
        self
    }
    pub fn shortcut(mut self, s: impl Into<SharedString>) -> Self {
        if let MenuEntry::Item { shortcut, .. } = &mut self {
            *shortcut = Some(s.into());
        }
        self
    }
}

/// Rapikan pemisah di ujung/ganda.
fn tidy(entries: Vec<MenuEntry>) -> Vec<MenuEntry> {
    let mut out: Vec<MenuEntry> = Vec::new();
    for e in entries {
        let sep = matches!(e, MenuEntry::Separator);
        if sep && (out.is_empty() || matches!(out.last(), Some(MenuEntry::Separator))) {
            continue;
        }
        out.push(e);
    }
    if matches!(out.last(), Some(MenuEntry::Separator)) {
        out.pop();
    }
    out
}

/// Popup menu terbuka. `on_close` dipanggil setelah memilih atau klik di luar.
pub fn menu_popup(
    id: impl Into<SharedString>,
    t: &Theme,
    entries: Vec<MenuEntry>,
    anchor: Anchor,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let id: SharedString = id.into();
    let on_close: Action = Rc::new(on_close);
    let mut list = popup(t).min_w(t.rem(rem::SPACE[7] * 4.));
    for (n, e) in tidy(entries).into_iter().enumerate() {
        match e {
            MenuEntry::Separator => list = list.child(separator(t)),
            MenuEntry::Item { label, icon, shortcut, danger, disabled, checked, action } => {
                let close = on_close.clone();
                let mut it = menu_item(SharedString::from(format!("{id}-{n}")), label)
                    .danger(danger)
                    .disabled(disabled)
                    .checked(checked)
                    .on_click(move |_, w, cx| {
                        close(w, cx);
                        action(w, cx);
                    });
                if let Some(i) = icon {
                    it = it.icon(i);
                }
                if let Some(s) = shortcut {
                    it = it.shortcut(s);
                }
                list = list.child(it);
            }
        }
    }
    let close = on_close.clone();
    let top = matches!(anchor, Anchor::TopLeft | Anchor::TopRight);
    deferred(
        anchored().anchor(anchor).snap_to_window_with_margin(px(8.)).child(
            div()
                .when(top, |d| d.mt(t.space(1)))
                .when(!top, |d| d.mb(t.space(1)))
                .child(list.occlude().on_mouse_down_out(move |_, w, cx| close(w, cx))),
        ),
    )
    .with_priority(65)
    .into_any_element()
}

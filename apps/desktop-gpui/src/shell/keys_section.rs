//! Tab "Shortcut" di Tampilan — port KeysTab (AppearancePage.tsx): daftar pintasan perintah, rekam kombinasi baru
//! (wajib Ctrl/Alt atau tombol F; Esc membatalkan), cek bentrok, kembali ke bawaan. Override disimpan di appearance.json.
use super::commands::{find, set_shortcut, shortcut_of, COMMANDS};
use super::state::format_combo;
use crate::core::appearance::AppearanceStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, kbd, ButtonVariant, ControlSize, TypeExt};
use gpui::{div, prelude::*, Context, Global, IntoElement, SharedString, Window};

/// Penanda global: true saat sedang merekam pintasan (root tidak menjalankan perintah).
pub struct KeyCapture(pub bool);
impl Global for KeyCapture {}

pub struct KeysSection {
    capturing: Option<&'static str>,
    error: Option<(&'static str, String)>,
}

impl KeysSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.set_global(KeyCapture(false));
        cx.observe(&AppearanceStore::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe_keystrokes(|this, ev, _, cx| {
            let Some(id) = this.capturing else { return };
            let k = &ev.keystroke;
            if ["control", "alt", "shift", "platform", "function"].contains(&k.key.as_str()) {
                return;
            }
            if k.key == "escape" {
                this.stop(cx);
                return;
            }
            let fkey = k.key.len() >= 2 && k.key.starts_with('f') && k.key[1..].chars().all(|c| c.is_ascii_digit());
            let m = &k.modifiers;
            if !(m.control || m.alt || m.platform || fkey) {
                this.error = Some((id, cx.global::<I18n>().t("ap.keys.needModifier")));
                cx.notify();
                return;
            }
            let combo = super::command_center::combo(k);
            match set_shortcut(id, Some(&combo), cx) {
                Ok(()) => {
                    this.error = None;
                    this.stop(cx);
                }
                Err(e) => {
                    this.error = Some((id, e));
                    cx.notify();
                }
            }
        })
        .detach();
        Self { capturing: None, error: None }
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.capturing = None;
        cx.set_global(KeyCapture(false));
        cx.notify();
    }
}

impl Render for KeysSection {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let mut list = div().flex().flex_col().gap(t.space(2));
        let mut any = false;
        for c in COMMANDS.iter().filter(|c| c.shortcut.is_some()) {
            any = true;
            let id = c.id;
            let cur = shortcut_of(id, cx).unwrap_or_default();
            let def = find(id).and_then(|c| c.shortcut).unwrap_or("");
            let changed = cur != def;
            let capturing = self.capturing == Some(id);
            list = list.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .p(t.space(3))
                    .rounded(t.rem(rem::RADIUS_LG))
                    .border_1()
                    .border_color(if capturing { t.colors.focus } else { t.colors.border_soft })
                    .bg(t.colors.raised)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .child(div().flex_1().child(i.t(c.title_key)))
                            .child(kbd(if capturing { i.t("ap.keys.press") } else { format_combo(&cur) }))
                            .child(
                                button(SharedString::from(format!("key-ch-{id}")))
                                    .label(i.t("ap.keys.change"))
                                    .size(ControlSize::Sm)
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        s.error = None;
                                        s.capturing = Some(id);
                                        cx.set_global(KeyCapture(true));
                                        cx.notify();
                                    })),
                            )
                            .when(changed, |d| {
                                d.child(
                                    button(SharedString::from(format!("key-rs-{id}")))
                                        .label(i.t("ap.keys.reset"))
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            if let Err(e) = set_shortcut(id, None, cx) {
                                                s.error = Some((id, e));
                                            } else {
                                                s.error = None;
                                            }
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .when(changed, |d| d.child(div().small(&t).text_color(t.colors.fg_muted).child(i.tp("ap.keys.default", &[("combo", &format_combo(def))]))))
                    .children(self.error.as_ref().filter(|(e, _)| *e == id).map(|(_, e)| div().small(&t).text_color(t.colors.danger).child(e.clone()))),
            );
        }
        if !any {
            return div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.keys.empty"));
        }
        div().flex().flex_col().gap(t.space(3)).child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.keys.intro"))).child(list)
    }
}

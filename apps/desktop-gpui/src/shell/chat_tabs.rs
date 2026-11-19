//! Bilah tab sesi Panel 2 — port SessionTabs (ChatPanel.tsx) + PanelTabBar + ChatCustomize.tsx: tab per sesi terbuka
//! (logo model, status, judul, tombol tutup, menu klik-kanan tutup lainnya/kanan/kiri/semua), tombol "Kustomisasi"
//! (popover kontrol gaya chat + tautan ke Pengaturan → Chat), dan tombol tutup panel.
use super::ChatPanel;
use crate::core::projects::session_title;
use crate::core::types::Role;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, entry, icon, icon_button, menu_popup, ButtonVariant, ControlSize, IconSize, MenuEntry};
use gpui::{anchored, div, prelude::*, px, AnyElement, Anchor, Context, MouseButton, Point, Pixels, SharedString};

#[derive(Default)]
pub struct TabsState {
    /// Menu klik-kanan terbuka: (session id, posisi kursor).
    pub menu: Option<(String, Point<Pixels>)>,
    pub customize: Option<gpui::Entity<super::super::chat_style_section::ChatStyleSection>>,
}

impl ChatPanel {
    pub(super) fn tabs_bar(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let (tabs, sel, sessions) = {
            let s = self.store.read(cx);
            (s.tabs.clone(), s.selection.session_id.clone(), s.sessions.clone())
        };
        let chat = self.chat.read(cx);
        let fallback = chat.model.clone();
        let title_of = |key: &str, id: &str| -> String {
            if key == crate::core::demo::DEMO_KEY {
                return crate::core::demo::demo_by_id(id).map(|d| i.t(&d.title_key)).unwrap_or_else(|| i.t("sb.untitled"));
            }
            sessions.get(key).and_then(|g| g.items.iter().find(|x| x.id == id)).and_then(|s| session_title(s).map(str::to_string)).unwrap_or_else(|| i.t("sb.untitled"))
        };
        let model_of = |id: &str| -> (Option<String>, Option<String>) {
            let last = chat.sessions.get(id).and_then(|st| st.messages.iter().rev().find(|m| m.info.role == Role::Assistant && m.info.model_id.is_some()));
            match last {
                Some(m) => (m.info.provider_id.clone(), m.info.model_id.clone()),
                None => (fallback.as_ref().map(|m| m.provider_id.clone()), fallback.as_ref().map(|m| m.model_id.clone())),
            }
        };
        let models: Vec<(Option<String>, Option<String>)> = tabs.iter().map(|tb| model_of(&tb.session_id)).collect();
        let last = tabs.len().saturating_sub(1);
        let n_tabs = tabs.len();

        let mut nav = div().id("stabs").flex().flex_1().min_w_0().h_full().items_end().gap(t.space(1)).pt(t.space(1)).pl(t.space(2)).overflow_x_scroll();
        for (n, tb) in tabs.iter().enumerate() {
            let title = title_of(&tb.key, &tb.session_id);
            let active = sel.as_deref() == Some(tb.session_id.as_str());
            let sid = tb.session_id.clone();
            let lead: AnyElement = {
                let (p, m) = &models[n];
                let ind = super::super::notification_driver::Attention::indicator_of(&sid, cx);
                // port TabStatus: titik hanya bila tidak idle; logo selebar ikon kecil (.stabs__label .z-model-logo)
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().flex_none().size(t.rem(rem::ICON_SM)).overflow_hidden().child(super::super::widgets::model_logo_sized(t, cx, p.as_deref(), m.as_deref(), "", false, &format!("tab-{sid}"), rem::ICON_SM)))
                    .when(ind != crate::core::attention::Indicator::Idle, |d| d.child(super::super::sidebar::session_dot(t, i, ind, &sid)))
                    .into_any_element()
            };
            let (key, s1, s2, s3) = (tb.key.clone(), sid.clone(), sid.clone(), sid.clone());
            nav = nav.child(
                div()
                    .id(SharedString::from(format!("stab-{sid}")))
                    .group("stab")
                    .flex()
                    .flex_none()
                    .items_center()
                    .max_w(t.rem(12.))
                    .border_1()
                    .border_b_0()
                    .border_color(gpui::transparent_black())
                    .rounded_t(t.rem(rem::RADIUS_MD))
                    .cursor_pointer()
                    .text_color(t.colors.fg)
                    .when(active, |d| d.bg(t.colors.raised).border_color(t.colors.border))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        let (k, id) = (key.clone(), s1.clone());
                        s.store.update(cx, |st, cx| st.select(Some(k), Some(id), cx));
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |s, e: &gpui::MouseDownEvent, _, cx| {
                            s.tabs_ui.menu = Some((s2.clone(), e.position));
                            cx.notify();
                        }),
                    )
                    .child(div().flex().items_center().gap(t.space(2)).min_w_0().px(t.space(2)).py(t.space(1)).child(lead).child(div().min_w_0().truncate().child(title.clone())))
                    .child(
                        div()
                            .id(SharedString::from(format!("stabx-{sid}")))
                            .flex_none()
                            .p(t.space(1))
                            .text_color(t.colors.fg_muted)
                            .hover(|h| h.text_color(t.colors.fg))
                            .tooltip(crate::ui::tooltip::text_tooltip(i.tp("chat.closeTab", &[("title", &title)]).into()))
                            .child(icon("close").size(IconSize::Sm).color(t.colors.fg_muted))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                cx.stop_propagation();
                                let id = s3.clone();
                                s.store.update(cx, |st, cx| st.close_tab(&id, cx));
                            })),
                    ),
            );
        }

        // menu klik-kanan tab
        let menu = self.tabs_ui.menu.clone().map(|(sid, pos)| {
            let idx = tabs.iter().position(|x| x.session_id == sid).unwrap_or(0);
            let store = self.store.clone();
            let by = |mode: &'static str, anchor: Option<String>| {
                let store = store.clone();
                move |_: &mut gpui::Window, cx: &mut gpui::App| store.update(cx, |st, cx| st.close_tabs_by(mode, anchor.as_deref(), cx))
            };
            let close_one = {
                let (store, sid) = (store.clone(), sid.clone());
                move |_: &mut gpui::Window, cx: &mut gpui::App| store.update(cx, |st, cx| st.close_tab(&sid, cx))
            };
            let entries = vec![
                entry(i.t("tabs.close"), close_one),
                entry(i.t("tabs.closeOthers"), by("others", Some(sid.clone()))).disabled(n_tabs < 2),
                entry(i.t("tabs.closeRight"), by("right", Some(sid.clone()))).disabled(idx == last),
                entry(i.t("tabs.closeLeft"), by("left", Some(sid.clone()))).disabled(idx == 0),
                MenuEntry::Separator,
                entry(i.t("tabs.closeAll"), by("all", None)),
            ];
            let me = cx.entity().downgrade();
            div()
                .absolute()
                .child(anchored().position(pos).child(menu_popup("stab-menu", t, entries, Anchor::TopLeft, move |_, cx| {
                    me.update(cx, |s, cx| {
                        s.tabs_ui.menu = None;
                        cx.notify();
                    })
                    .ok();
                })))
                .into_any_element()
        });

        // popover Kustomisasi
        let pop = self.tabs_ui.customize.clone().map(|sec| {
            let me = cx.entity().downgrade();
            let me2 = me.clone();
            let app = self.app.clone();
            crate::ui::overlay_deferred(
                anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(
                    crate::ui::popup(t)
                        .occlude()
                        .mt(t.space(1))
                        .w(t.rem(22.))
                        .max_h(t.rem(32.))
                        .flex()
                        .flex_col()
                        .on_mouse_down_out(move |_, _, cx| {
                            me.update(cx, |s, cx| {
                                s.tabs_ui.customize = None;
                                cx.notify();
                            })
                            .ok();
                        })
                        .child(div().px(t.space(3)).pt(t.space(3)).pb(t.space(2)).font_weight(gpui::FontWeight::SEMIBOLD).child(i.t("chatStyle.title")))
                        .child(div().id("cstyle-pop-body").flex_1().min_h_0().overflow_y_scroll().px(t.space(3)).pb(t.space(3)).child(sec))
                        .child(
                            div().flex().border_t_1().border_color(t.colors.border).p(t.space(2)).child(
                                button("cstyle-all")
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .child(icon("settings").size(IconSize::Sm))
                                    .label(i.t("chatStyle.allSettings"))
                                    .on_click(move |_, _, cx| {
                                        me2.update(cx, |s, cx| {
                                            s.tabs_ui.customize = None;
                                            cx.notify();
                                        })
                                        .ok();
                                        app.update(cx, |a, cx| a.push("/settings/chat", cx));
                                    }),
                            ),
                        ),
                ),
            )
            .with_priority(50)
        });
        let app = self.app.clone();

        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(t.space(1))
            .h(t.rem(rem::CONTROL_LG))
            .border_b_1()
            .border_color(t.colors.border)
            .child(nav)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(t.space(1))
                    .mr(t.space(2))
                    .child(
                        div().relative().child(
                            icon_button("chat-customize", "customize", i.t("chatStyle.open")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| {
                                s.tabs_ui.customize = match s.tabs_ui.customize.take() {
                                    Some(_) => None,
                                    None => Some(cx.new(super::super::chat_style_section::ChatStyleSection::new)),
                                };
                                cx.notify();
                            })),
                        )
                        .children(pop),
                    )
                    .child(
                        icon_button("chat-close-panel", "close", i.t("panel.close.chat"))
                            .size(ControlSize::Sm)
                            .on_click(move |_, _, cx| app.update(cx, |a, cx| a.toggle(super::super::state::PanelId::Chat, cx))),
                    ),
            )
            .children(menu)
            .into_any_element()
    }
}

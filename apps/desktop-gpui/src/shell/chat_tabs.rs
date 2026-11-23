//! Bilah info sesi Panel 2: menampilkan "Nama project · branch git · nama sesi" (sesi di project) atau hanya
//! "Nama sesi" (Chat tanpa project), tombol "Kustomisasi" (popover kontrol gaya chat + tautan ke Pengaturan → Chat)
//! dan tombol tutup panel. Tab per sesi sudah tidak ditampilkan; sesi dipilih dari sidebar.
use super::ChatPanel;
use crate::core::projects::{session_title, CHAT_KEY};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, ButtonVariant, ControlSize, IconSize};
use gpui::{anchored, div, prelude::*, px, AnyElement, Anchor, Context, FontWeight};

#[derive(Default)]
pub struct TabsState {
    pub customize: Option<gpui::Entity<super::super::chat_style_section::ChatStyleSection>>,
    /// Cache branch git: (folder, branch). Dimuat di latar saat folder/sesi berganti.
    branch: Option<(String, Option<String>)>,
    branch_loading: Option<String>,
}

impl ChatPanel {
    /// Branch git untuk folder project (None bila bukan repo / belum dimuat). Memuat ulang di latar bila folder berganti.
    fn project_branch(&mut self, root: &str, cx: &mut Context<Self>) -> Option<String> {
        let cached = match &self.tabs_ui.branch {
            Some((r, b)) if r == root => Some(b.clone()),
            _ => None,
        };
        if cached.is_none() && self.tabs_ui.branch_loading.as_deref() != Some(root) {
            self.tabs_ui.branch_loading = Some(root.to_string());
            let r = root.to_string();
            let task = cx.background_spawn(async move {
                let st = crate::git::status(std::path::Path::new(&r)).ok();
                let b = st.filter(|s| s.is_repo).map(|s| if s.detached { String::from("HEAD") } else { s.branch.unwrap_or_default() }).filter(|b| !b.is_empty());
                (r, b)
            });
            cx.spawn(async move |this, cx| {
                let (r, b) = task.await;
                this.update(cx, |s, cx| {
                    s.tabs_ui.branch = Some((r, b));
                    s.tabs_ui.branch_loading = None;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cached.flatten()
    }

    pub(super) fn tabs_bar(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let (key, sid, sessions, project) = {
            let s = self.store.read(cx);
            let key = s.selection.key.clone().unwrap_or_else(|| CHAT_KEY.to_string());
            let project = s.projects.iter().find(|p| p.id == key).map(|p| (p.name.clone(), p.path.clone()));
            (key, s.selection.session_id.clone(), s.sessions.clone(), project)
        };
        let title = sid
            .as_deref()
            .map(|id| {
                sessions.get(&key).and_then(|g| g.items.iter().find(|x| x.id == id)).and_then(|s| session_title(s).map(str::to_string)).unwrap_or_else(|| i.t("sb.untitled"))
            })
            .unwrap_or_else(|| i.t("sb.untitled"));
        let branch = match &project {
            Some((_, path)) => self.project_branch(path, cx),
            None => None,
        };

        let sep = || div().flex_none().text_color(t.colors.fg_subtle).child("·");
        let mut info = div().id("session-info").flex().flex_1().min_w_0().h_full().items_center().gap(t.space(2)).pl(t.space(3)).overflow_hidden();
        if let Some((name, _)) = &project {
            info = info.child(div().flex_none().max_w(t.rem(14.)).truncate().text_color(t.colors.fg_muted).child(name.clone())).child(sep());
            if let Some(b) = branch {
                info = info
                    .child(div().flex().flex_none().items_center().gap(t.space(1)).max_w(t.rem(14.)).text_color(t.colors.fg_muted).child(icon("git").size(IconSize::Sm).color(t.colors.fg_muted)).child(div().min_w_0().truncate().child(b)))
                    .child(sep());
            }
        }
        info = info.child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(t.colors.fg).child(title));

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
            .child(info)
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
            .into_any_element()
    }
}

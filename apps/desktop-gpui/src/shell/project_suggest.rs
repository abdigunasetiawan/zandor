//! Saran "buat project" di sesi Chat. Dua sumber:
//! - **Pengguna**: pesan pertama yang terdeteksi sebagai pekerjaan project (`core::project_intent`) ditahan.
//! - **Agent**: tool `zandor_suggest_project` (aksi `suggest_project`) di tengah percakapan, lewat `agent_control`.
//!
//! Kartu di atas composer menawarkan **Buat project** (folder baru + sesi baru; pesan pengguna atau ringkasan dari agent
//! menjadi pesan pertama di sana) atau **Tetap di Chat**. Enter dengan kartu terbuka = tetap di Chat. Pilihan "tetap"
//! berlaku untuk sesi itu (tidak bertanya lagi).
use crate::shell::agent_control::AgentSuggest;
use super::ChatPanel;
use crate::core::project_intent::{self, Kind};
use crate::core::projects::CHAT_KEY;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, ButtonVariant, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, FontWeight};
use std::collections::HashSet;

pub(super) struct Pending {
    /// Pesan pertama di project baru (pesan pengguna, atau ringkasan dari agent).
    text: String,
    name: String,
    /// Jenis pekerjaan (saran dari pengguna) atau None (saran dari agent: memakai `reason`).
    kind: Option<Kind>,
    reason: Option<String>,
    /// Id sesi Chat ("" = halaman sesi-baru).
    skip_key: String,
    from_agent: bool,
}

#[derive(Default)]
pub(super) struct SuggestState {
    pending: Option<Pending>,
    /// Sesi Chat (id; "" = halaman sesi-baru) yang sudah memilih tetap di Chat.
    skip: HashSet<String>,
}

impl ChatPanel {
    /// true = pesan ditahan dan kartu saran tampil (pesan belum dikirim).
    pub(super) fn maybe_suggest_project(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        if let Some(p) = self.suggest.pending.take() {
            // Enter lagi (atau pesan baru) dengan kartu terbuka = lanjut di Chat
            self.suggest.skip.insert(p.skip_key);
            return false;
        }
        if !crate::core::settings::SettingsStore::global(cx).read(cx).bool("suggestProject") {
            return false;
        }
        let (key, sid) = match self.current(cx) {
            Some((k, id, _, _)) => (k, id),
            None => (self.store.read(cx).selection.key.clone().unwrap_or_else(|| CHAT_KEY.to_string()), String::new()),
        };
        if key != CHAT_KEY || self.suggest.skip.contains(&sid) {
            return false;
        }
        // Pesan pengguna: hanya pesan pertama sesi (percakapan berjalan dijaga agent lewat zandor_suggest_project)
        if !sid.is_empty() && self.chat.read(cx).sessions.get(&sid).is_some_and(|s| !s.messages.is_empty()) {
            return false;
        }
        let Some(intent) = project_intent::detect(text, self.assist.files.len()) else { return false };
        self.suggest.pending = Some(Pending { text: text.to_string(), name: intent.name, kind: Some(intent.kind), reason: None, skip_key: sid, from_agent: false });
        cx.notify();
        true
    }

    /// Agent meminta kartu saran (aksi `suggest_project`). Diabaikan bila pengguna sudah memilih tetap di sesi itu.
    pub(super) fn suggest_from_agent(&mut self, r: AgentSuggest, cx: &mut Context<Self>) {
        if self.suggest.skip.contains(&r.session_id) {
            return;
        }
        self.suggest.pending = Some(Pending { text: r.summary, name: r.name, kind: None, reason: Some(r.reason), skip_key: r.session_id, from_agent: true });
        cx.notify();
    }

    fn suggest_stay(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.suggest.pending.take() else { return };
        self.suggest.skip.insert(p.skip_key.clone());
        if p.from_agent {
            // agent sedang menunggu: beri tahu dia (dan pengguna melihatnya di riwayat)
            let msg = cx.global::<I18n>().t("projSuggest.stayMsg");
            self.submit(msg, cx);
        } else {
            self.submit(p.text, cx);
        }
    }

    fn suggest_create(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.suggest.pending.take() else { return };
        let store = self.store.clone();
        let project = match store.update(cx, |s, cx| s.create_project_folder(&p.name, cx)) {
            Ok(pr) => pr,
            Err(e) => {
                crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(e) }, cx);
                self.suggest.pending = Some(p);
                cx.notify();
                return;
            }
        };
        if !p.from_agent {
            self.input.update(cx, |i, cx| i.set_text("", cx));
        }
        let files = std::mem::take(&mut self.assist.files);
        let text = p.text;
        let task = store.update(cx, |s, cx| s.create_session(&project.id, None, true, cx));
        let toast = cx.global::<I18n>().tp("projSuggest.created", &[("name", &project.name), ("path", &project.path)]);
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |me, cx| match r {
                Ok(s) => {
                    me.sent_from_draft.insert(s.id.clone());
                    me.sync(cx);
                    me.chat.update(cx, |c, cx| c.send_with(&s.id, text, files, cx));
                    crate::ui::Toasts::show(crate::ui::ToastInput::new(toast), cx);
                }
                Err(e) => {
                    crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(e) }, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn render_project_suggest(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.suggest.pending.as_ref()?;
        if p.from_agent && self.current(cx).map(|c| c.1).as_deref() != Some(p.skip_key.as_str()) {
            return None; // saran agent hanya tampil di sesi yang memintanya
        }
        let body = match (&p.reason, p.kind) {
            (Some(reason), _) => i.tp("projSuggest.bodyAgent", &[("reason", reason), ("name", &p.name)]),
            (None, Some(k)) => i.tp("projSuggest.body", &[("kind", &i.t(&format!("projSuggest.kind.{}", k.id()))), ("name", &p.name)]),
            (None, None) => String::new(),
        };
        Some(
            div()
                .id("project-suggest")
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .rounded(t.rem(rem::RADIUS_LG))
                .border_1()
                .border_color(t.colors.border_soft)
                .bg(t.colors.raised)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .child(icon("folder_plus").size(IconSize::Md).color(t.colors.fg_muted))
                        .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(i.t("projSuggest.title")))
                        .child(
                            icon_button("ps-close", "close", i.t("projSuggest.dismiss"))
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Ghost)
                                .on_click(cx.listener(|me, _, _, cx| {
                                    me.suggest.pending = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(div().small(t).text_color(t.colors.fg_muted).child(body))
                .child(div().small(t).text_color(t.colors.fg_subtle).child(i.t(if p.from_agent { "projSuggest.whereAgent" } else { "projSuggest.where" })))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(t.space(2))
                        .child(button("ps-create").label(i.t("projSuggest.create")).size(ControlSize::Sm).variant(ButtonVariant::Solid).on_click(cx.listener(|me, _, _, cx| me.suggest_create(cx))))
                        .child(button("ps-stay").label(i.t("projSuggest.stay")).size(ControlSize::Sm).on_click(cx.listener(|me, _, _, cx| me.suggest_stay(cx)))),
                )
                .into_any_element(),
        )
    }
}

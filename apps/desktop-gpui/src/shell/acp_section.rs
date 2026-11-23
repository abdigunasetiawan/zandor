//! Agent CLI (ACP) — Pengaturan > Model dan langkah provider di Onboarding (ADR-083).
//!
//! Menampilkan Claude Code, Codex, dan Gemini (Antigravity) dengan status jujur (CLI tidak ada, adaptor belum
//! dipasang, Node.js tidak ada, galat terakhir, siap + jumlah model), sakelar aktif, pasang adaptor npm, uji koneksi
//! (ambil daftar model), dan perintah login untuk disalin. Zandor tidak memegang token: agent memakai login CLI-nya.
//! Semua pemeriksaan (cari biner, jalankan agent, npm) berjalan di latar.
use crate::core::acp::{self, agents::Missing, host::AgentStatus};
use crate::core::services::EngineStatus;
use crate::i18n::I18n;
use crate::theme::Theme;
use crate::ui::{badge, button, icon, switch, BadgeTone, ButtonVariant, ControlSize, IconSize, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, AnyElement, ClipboardItem, Context, FontWeight, IntoElement, SharedString, Window};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
enum Busy {
    Toggle,
    Install,
    Test,
}

pub struct AcpSection {
    /// None = status sedang dibaca.
    statuses: Option<Vec<AgentStatus>>,
    busy: HashMap<&'static str, Busy>,
    /// Tampilkan judul dan penjelasan bagian (Pengaturan). Onboarding memakai judulnya sendiri.
    pub heading: bool,
}

impl AcpSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut me = Self { statuses: None, busy: HashMap::new(), heading: true };
        me.reload(cx);
        me
    }

    /// Baca ulang status semua agent di latar (mencari biner di PATH bisa lambat).
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async move { acp::agents::AGENTS.iter().filter_map(|d| acp::host().status(d.id)).collect::<Vec<_>>() });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            this.update(cx, |s, cx| {
                s.statuses = Some(list);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Jalankan `job` di latar untuk agent `id`, tampilkan hasilnya sebagai toast, lalu muat ulang status dan
    /// beri tahu pemilih model bahwa daftar model berubah.
    fn run(&mut self, id: &'static str, kind: Busy, job: impl FnOnce() -> Result<Option<usize>, String> + Send + 'static, cx: &mut Context<Self>) {
        if self.busy.contains_key(id) {
            return;
        }
        self.busy.insert(id, kind);
        cx.notify();
        let task = cx.background_spawn(async move { job() });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.busy.remove(id);
                let i = cx.global::<I18n>().clone();
                let name = acp::agents::by_id(id).map(|d| d.name).unwrap_or(id);
                let toast = match (&r, kind) {
                    (Ok(Some(n)), _) => Some((i.tp("acp.toast.ready", &[("name", name), ("n", &n.to_string())]), ToastTone::Success)),
                    (Ok(None), Busy::Install) => Some((i.tp("acp.toast.installed", &[("name", name)]), ToastTone::Success)),
                    (Ok(None), _) => None,
                    (Err(e), _) => Some((i.tp("acp.toast.failed", &[("name", name), ("error", e)]), ToastTone::Danger)),
                };
                if let Some((title, tone)) = toast {
                    Toasts::show(ToastInput { tone, ..ToastInput::new(title) }, cx);
                }
                s.reload(cx);
                EngineStatus::models_changed(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn toggle(&mut self, id: &'static str, on: bool, needs_models: bool, cx: &mut Context<Self>) {
        self.run(
            id,
            Busy::Toggle,
            move || {
                let h = acp::host();
                h.set_enabled(id, on)?;
                // Aktif pertama kali: ambil daftar model sekarang agar langsung muncul di pemilih model.
                if on && needs_models {
                    return h.refresh_models(id).map(Some);
                }
                Ok(None)
            },
            cx,
        );
    }

    fn install(&mut self, id: &'static str, cx: &mut Context<Self>) {
        self.run(
            id,
            Busy::Install,
            move || {
                let h = acp::host();
                h.install(id)?;
                if h.config(id).enabled {
                    return h.refresh_models(id).map(Some);
                }
                Ok(None)
            },
            cx,
        );
    }

    fn test(&mut self, id: &'static str, cx: &mut Context<Self>) {
        self.run(id, Busy::Test, move || acp::host().refresh_models(id).map(Some), cx);
    }

    fn row(&self, t: &Theme, i: &I18n, s: &AgentStatus, cx: &mut Context<Self>) -> AnyElement {
        let id = s.id;
        let busy = self.busy.get(id).copied();
        let def = acp::agents::by_id(id);
        let provider = def.map(|d| d.provider_id);
        let (tone, label) = match (&s.launch, &s.error) {
            (Err(Missing::Cli), _) => (BadgeTone::Danger, i.t("acp.status.noCli")),
            (Err(Missing::Adapter { .. }), _) => (BadgeTone::Warning, i.t("acp.status.noAdapter")),
            (Err(Missing::Node), _) => (BadgeTone::Danger, i.t("acp.status.noNode")),
            (Err(Missing::Command(_)), _) => (BadgeTone::Danger, i.t("acp.status.noCommand")),
            (Ok(_), Some(e)) if e.contains(s.login) => (BadgeTone::Warning, i.t("acp.status.login")),
            (Ok(_), Some(_)) => (BadgeTone::Danger, i.t("acp.status.error")),
            (Ok(_), None) if s.models > 0 => (BadgeTone::Success, i.tp("acp.status.ready", &[("n", &s.models.to_string())])),
            (Ok(_), None) => (BadgeTone::Outline, i.t("acp.status.untested")),
        };
        let launchable = s.launch.is_ok();
        let needs_models = s.models == 0;
        let head = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .child(super::widgets::model_logo(t, cx, provider, None, s.name, busy.is_some(), &format!("acp-logo-{id}")))
            .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(s.name))
            .child(badge(label).tone(tone))
            .child(
                switch(SharedString::from(format!("acp-on-{id}")), i.t("acp.enable"), s.enabled)
                    // Mematikan selalu boleh; menyalakan hanya bila agent bisa dijalankan.
                    .disabled(busy.is_some() || (!s.enabled && !launchable))
                    .on_change({
                        let me = cx.entity().downgrade();
                        move |on, _, cx| {
                            me.update(cx, |s, cx| s.toggle(id, on, needs_models, cx)).ok();
                        }
                    }),
            );

        // Penjelasan sebab dan jalan keluar.
        let hint: Option<String> = match &s.launch {
            Err(Missing::Cli) => Some(i.tp("acp.hint.noCli", &[("name", s.name), ("cli", def.map(|d| d.cli).unwrap_or(id))])),
            Err(Missing::Adapter { npm: true }) => Some(i.t("acp.hint.noAdapter")),
            Err(Missing::Adapter { npm: false }) | Err(Missing::Node) => Some(i.t("acp.hint.noNode")),
            Err(Missing::Command(c)) => Some(i.tp("acp.hint.noCommand", &[("cmd", c)])),
            Ok(_) => None,
        };
        let mut col = div().flex().flex_col().gap(t.space(2)).py(t.space(2)).border_t_1().border_color(t.colors.border_soft).child(head);
        if let Some(h) = hint {
            col = col.child(div().small(t).text_color(t.colors.fg_muted).child(h));
        }
        if let Some(e) = &s.error {
            col = col.child(div().small(t).text_color(t.colors.danger).child(e.clone()));
        }
        if let Ok(cmd) = &s.launch {
            col = col.child(div().small(t).text_color(t.colors.fg_subtle).truncate().child(i.tp("acp.launch", &[("cmd", cmd)])));
        }

        let login = s.login.to_string();
        let mut actions = div().flex().flex_wrap().items_center().gap(t.space(2));
        if matches!(s.launch, Err(Missing::Adapter { npm: true })) {
            let installing = busy == Some(Busy::Install);
            actions = actions.child(
                button(SharedString::from(format!("acp-inst-{id}")))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Solid)
                    .label(i.t(if installing { "acp.installing" } else { "acp.install" }))
                    .disabled(busy.is_some())
                    .on_click(cx.listener(move |s, _, _, cx| s.install(id, cx))),
            );
        }
        if launchable {
            let testing = matches!(busy, Some(Busy::Test | Busy::Toggle));
            actions = actions.child(
                button(SharedString::from(format!("acp-test-{id}")))
                    .size(ControlSize::Sm)
                    .label(i.t(if testing { "acp.testing" } else { "acp.test" }))
                    .disabled(busy.is_some())
                    .on_click(cx.listener(move |s, _, _, cx| s.test(id, cx))),
            );
        }
        actions = actions.child(
            button(SharedString::from(format!("acp-login-{id}")))
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .child(icon("copy").size(IconSize::Sm))
                .label(i.tp("acp.copyLogin", &[("cmd", &login)]))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(login.clone()));
                    let title = cx.global::<I18n>().t("onb.sys.copied");
                    Toasts::show(ToastInput { timeout_ms: Some(2500), ..ToastInput::new(title) }, cx);
                }),
        );
        col.child(actions).into_any_element()
    }
}

impl Render for AcpSection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let mut col = div().flex().flex_col().gap(t.space(2));
        if self.heading {
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("acp.title"))).child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("acp.body"))))
                    .child(
                        button("acp-recheck")
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .child(icon("refresh").size(IconSize::Sm))
                            .label(i.t("acp.recheck"))
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.statuses = None;
                                s.reload(cx);
                                cx.notify();
                            })),
                    ),
            );
        }
        match self.statuses.clone() {
            None => col = col.child(div().flex().items_center().gap(t.space(2)).small(&t).text_color(t.colors.fg_muted).child(crate::ui::spinner(IconSize::Sm)).child(i.t("acp.checking"))),
            Some(list) => {
                for s in &list {
                    col = col.child(self.row(&t, &i, s, cx));
                }
            }
        }
        col.child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("acp.note")))
    }
}

//! Pembaruan — Pengaturan > Umum (ADR-084). Versi terpasang, status jujur pembaruan otomatis (tidak aktif di
//! build pengembangan, memeriksa, mengunduh + progres, siap, galat + sebab), "Periksa sekarang", dan
//! "Mulai ulang sekarang" bila pembaruan sudah terunduh. Pemasangan sendiri terjadi otomatis saat Zandor ditutup.
use crate::core::update::{release_version, Disabled, Phase, Updater};
use crate::i18n::I18n;
use crate::theme::Theme;
use crate::ui::{badge, button, BadgeTone, ButtonVariant, ControlSize, TypeExt};
use gpui::{div, prelude::*, Context, Entity, FontWeight, IntoElement, Window};

pub struct UpdateSection {
    updater: Option<Entity<Updater>>,
}

impl UpdateSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let updater = Updater::global(cx);
        if let Some(u) = &updater {
            cx.observe(u, |_, _, cx| cx.notify()).detach();
        }
        Self { updater }
    }
}

impl Render for UpdateSection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let version = env!("ZANDOR_VERSION");
        let (phase, last) = match &self.updater {
            Some(u) => {
                let u = u.read(cx);
                (u.phase.clone(), u.last_check)
            }
            None => (Phase::Disabled(Disabled::DevBuild), None),
        };

        let (tone, label) = match &phase {
            Phase::Disabled(_) => (BadgeTone::Outline, i.t("update.badge.off")),
            Phase::Idle | Phase::Checking => (BadgeTone::Outline, i.t("update.badge.auto")),
            Phase::UpToDate => (BadgeTone::Success, i.t("update.badge.latest")),
            Phase::Downloading { .. } => (BadgeTone::Outline, i.t("update.badge.downloading")),
            Phase::Ready { .. } => (BadgeTone::Warning, i.t("update.badge.ready")),
            Phase::Error(_) => (BadgeTone::Danger, i.t("update.badge.error")),
        };
        let channel = if release_version().is_some() { i.t("update.channel") } else { i.t("update.devBuild") };
        let head = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .flex_col()
                    .gap(t.space(1))
                    .child(div().font_weight(FontWeight::MEDIUM).child(i.tp("update.version", &[("version", version)])))
                    .child(div().small(&t).text_color(t.colors.fg_muted).child(channel)),
            )
            .child(badge(label).tone(tone));

        let status: String = match &phase {
            Phase::Disabled(Disabled::DevBuild) => i.t("update.disabled.dev"),
            Phase::Disabled(Disabled::NotInstalled) => i.t("update.disabled.notInstalled"),
            Phase::Idle => i.t("update.idle"),
            Phase::Checking => i.t("update.checking"),
            Phase::UpToDate => i.t("update.upToDate"),
            Phase::Downloading { version, done, total } => {
                let pct = if *total > 0 { (*done as f64 / *total as f64 * 100.).floor() as u64 } else { 0 };
                i.tp("update.downloading", &[("version", version), ("pct", &pct.to_string())])
            }
            Phase::Ready { version, .. } => i.tp("update.ready", &[("version", version)]),
            Phase::Error(e) => i.tp("update.error", &[("error", e)]),
        };
        let mut col = div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .py(t.space(3))
            .child(head)
            .child(div().small(&t).text_color(if matches!(phase, Phase::Error(_)) { t.colors.danger } else { t.colors.fg_muted }).child(status));

        if let Phase::Downloading { done, total, .. } = &phase {
            let p = if *total > 0 { (*done as f32 / *total as f32).clamp(0., 1.) } else { 0. };
            col = col.child(div().h(t.space(1)).w_full().rounded_full().bg(t.colors.segment).child(div().h_full().w(gpui::relative(p)).rounded_full().bg(t.colors.fg)));
        }
        if let Phase::Ready { notes, .. } = &phase {
            if !notes.trim().is_empty() {
                col = col.child(div().small(&t).text_color(t.colors.fg_muted).child(notes.trim().to_string()));
            }
        }
        if let Some(at) = last {
            col = col.child(div().small(&t).text_color(t.colors.fg_subtle).child(i.tp("update.lastCheck", &[("time", &at.format("%H:%M").to_string())])));
        }

        if let (Some(u), false) = (self.updater.clone(), matches!(phase, Phase::Disabled(_))) {
            let busy = matches!(phase, Phase::Checking | Phase::Downloading { .. });
            let mut actions = div().flex().flex_wrap().items_center().gap(t.space(2));
            if matches!(phase, Phase::Ready { .. }) {
                let u2 = u.clone();
                actions = actions.child(
                    button("update-restart")
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Solid)
                        .label(i.t("update.restartNow"))
                        .on_click(move |_, _, cx| u2.update(cx, |u, cx| u.restart_now(cx))),
                );
            }
            actions = actions.child(
                button("update-check")
                    .size(ControlSize::Sm)
                    .label(i.t(if matches!(phase, Phase::Error(_)) { "update.retry" } else { "update.checkNow" }))
                    .disabled(busy)
                    .on_click(move |_, _, cx| u.update(cx, |u, cx| u.check(cx))),
            );
            col = col.child(actions);
        }
        col
    }
}

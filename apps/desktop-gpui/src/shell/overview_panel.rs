//! Ikhtisar (Overview) Panel 4: Lingkungan (mesin/engine, folder, cabang git), Konteks (pemakaian jendela konteks +
//! auto compact) dan Perubahan sesi (+n −m, Buat PR), lalu rincian sesi (InfoPanel) di bawahnya.
//! Hanya data nyata: engine tidak memberi rincian System tools/MCP/Skills, jadi bilah konteks hanya memuat
//! percakapan, buffer auto compact, dan sisa ruang. Tidak ada saklar "Remote Control" — fiturnya belum ada.
use super::session_tools::InfoPanel;
use crate::core::chat_store::ChatStore;
use crate::core::engine::{ModelOption, SessionFileDiff};
use crate::core::projects::ProjectStore;
use crate::core::settings::SettingsStore;
use crate::core::usage::session_usage;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, select, switch, ButtonVariant, ControlSize, IconSize, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use std::path::Path;

pub struct OverviewPanel {
    info: Entity<InfoPanel>,
    models: Vec<ModelOption>,
    /// (folder, cabang) — cabang None = bukan repo git.
    branch: Option<(String, Option<String>)>,
    /// (id sesi, diff snapshot engine).
    diff: Option<(String, Vec<SessionFileDiff>)>,
    /// Pemicu muat ulang: (id sesi, jumlah pesan, sibuk).
    trigger: Option<(String, usize, bool)>,
    pr_busy: bool,
}

struct Env {
    sid: String,
    dir: String,
    engine: Option<String>,
    messages: usize,
    busy: bool,
}

impl OverviewPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&ProjectStore::global(cx), |s, _, cx| s.sync(cx)).detach();
        cx.observe(&ChatStore::global(cx), |s, _, cx| s.sync(cx)).detach();
        cx.observe(&SettingsStore::global(cx), |_, _, cx| cx.notify()).detach();
        if let Some(api) = ProjectStore::global(cx).read(cx).api() {
            let task = cx.background_spawn(async move { api.list_models().unwrap_or_default() });
            cx.spawn(async move |this, cx| {
                let m = task.await;
                this.update(cx, |s, cx| {
                    s.models = m;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        let info = cx.new(InfoPanel::new);
        let mut me = Self { info, models: vec![], branch: None, diff: None, trigger: None, pr_busy: false };
        me.sync(cx);
        me
    }

    fn env(cx: &gpui::App) -> Option<Env> {
        let st = ProjectStore::global(cx).read(cx);
        let (_, s, p) = st.selected()?;
        let dir = if s.directory.is_empty() { p.as_ref().map(|p| p.path.clone()).unwrap_or_default() } else { s.directory.clone() };
        let engine = p.as_ref().and_then(|p| p.engine.clone()).and_then(|id| st.remote_by_id(&id).map(|r| r.name.clone()));
        let chat = ChatStore::global(cx).read(cx).sessions.get(&s.id).map(|c| (c.messages.len(), c.status != crate::core::types::SessionRunStatus::Idle));
        let (messages, busy) = chat.unwrap_or((0, false));
        Some(Env { sid: s.id, dir, engine, messages, busy })
    }

    /// Muat cabang git dan diff sesi; diff dimuat ulang tiap giliran agent selesai (sama dengan tab Perubahan).
    fn sync(&mut self, cx: &mut Context<Self>) {
        cx.notify();
        let Some(e) = Self::env(cx) else { return };
        let trig = (e.sid.clone(), e.messages, e.busy);
        if self.trigger.as_ref() == Some(&trig) {
            return;
        }
        self.trigger = Some(trig);
        if e.busy || e.dir.is_empty() {
            return;
        }
        let dir = e.dir.clone();
        let task = cx.background_spawn(async move { crate::git::status(Path::new(&dir)).ok().filter(|s| s.is_repo).map(|s| s.branch.unwrap_or_else(|| "HEAD".into())) });
        let dir = e.dir.clone();
        cx.spawn(async move |this, cx| {
            let b = task.await;
            this.update(cx, |s, cx| {
                s.branch = Some((dir, b));
                cx.notify();
            })
            .ok();
        })
        .detach();
        if let Some(api) = ProjectStore::global(cx).read(cx).api() {
            let (dir, sid) = (e.dir.clone(), e.sid.clone());
            let task = cx.background_spawn(async move { api.session_diff(&dir, &sid).unwrap_or_default() });
            cx.spawn(async move |this, cx| {
                let d = task.await;
                this.update(cx, |s, cx| {
                    s.diff = Some((e.sid, d));
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// Buka formulir PR di browser lewat GitHub CLI (`gh pr create --web --fill`); PR baru dikonfirmasi pengguna di halaman itu.
    fn create_pr(&mut self, dir: String, cx: &mut Context<Self>) {
        if self.pr_busy {
            return;
        }
        self.pr_busy = true;
        cx.notify();
        let task = cx.background_spawn(async move {
            let mut c = std::process::Command::new("gh");
            c.args(["pr", "create", "--web", "--fill"]).current_dir(dir);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                c.creation_flags(0x0800_0000);
            }
            match c.output() {
                Ok(o) if o.status.success() => Ok(()),
                Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").trim().to_string()),
                Err(e) => Err(e.to_string()),
            }
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.pr_busy = false;
                let i = cx.global::<I18n>().clone();
                let (title, tone) = match r {
                    Ok(()) => (i.t("ov.prOpened"), ToastTone::Success),
                    Err(e) => (i.tp("ov.prFailed", &[("error", &e)]), ToastTone::Danger),
                };
                Toasts::show(ToastInput { title: title.into(), description: None, tone, timeout_ms: None, action: None }, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn section_title(t: &Theme, text: String) -> AnyElement {
    div().font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg).child(text).into_any_element()
}

fn env_row(t: &Theme, ic: &'static str, text: impl Into<SharedString>) -> gpui::Div {
    div().flex().items_center().gap(t.space(2)).child(icon(ic).size(IconSize::Sm).color(t.colors.fg_muted)).child(div().flex_1().min_w_0().truncate().child(text.into()))
}

fn tokens(n: f64) -> String {
    if n >= 1_000_000. {
        format!("{:.1}M", n / 1_000_000.)
    } else if n >= 1000. {
        format!("{:.1}K", n / 1000.)
    } else {
        format!("{}", n.round() as i64)
    }
}

impl Render for OverviewPanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let Some(env) = Self::env(cx) else { return super::widgets::tool_notice(&t, "layout", i.t("info.empty")) };

        // --- Lingkungan
        let local = env.engine.clone().unwrap_or_else(|| i.t("ov.thisComputer"));
        let branch = self.branch.as_ref().filter(|(d, _)| *d == env.dir).and_then(|(_, b)| b.clone());
        let mut environment = div().flex().flex_col().gap(t.space(2)).child(section_title(&t, i.t("ov.environment"))).child(env_row(&t, "server", local));
        if !env.dir.is_empty() {
            environment = environment.child(env_row(&t, "folder", env.dir.clone()).id("ov-dir").tooltip(crate::ui::tooltip::text_tooltip(env.dir.clone().into())));
        }
        if let Some(b) = branch.clone() {
            environment = environment.child(env_row(&t, "git", b));
        }

        // --- Konteks
        let (auto, thr) = {
            let st = SettingsStore::global(cx).read(cx);
            (st.bool("autoCompact"), st.num("autoCompactAt").clamp(1., 100.))
        };
        let max = SettingsStore::global(cx).read(cx).num("contextMax").max(0.) as u64;
        let chat = ChatStore::global(cx).read(cx);
        let cur = chat.model.clone();
        let msgs = chat.sessions.get(&env.sid).map(|c| c.messages.clone()).unwrap_or_default();
        let last = msgs.iter().rev().find(|m| m.info.role == crate::core::types::Role::Assistant && m.info.model_id.is_some());
        let by_cur = self.models.iter().find(|m| cur.as_ref().is_some_and(|c| c.provider_id == m.provider_id && c.model_id == m.model_id));
        let model = by_cur.or_else(|| self.models.iter().find(|m| last.is_some_and(|l| l.info.provider_id.as_deref() == Some(m.provider_id.as_str()) && l.info.model_id.as_deref() == Some(m.model_id.as_str()))));
        let limit = crate::core::context_policy::effective_limit(model.and_then(|m| m.context_limit), max).map(|v| v as f64);
        let u = session_usage(&msgs, limit, None);

        let mut context = div().flex().flex_col().gap(t.space(2)).child(section_title(&t, i.t("ov.context")));
        match (u.context_tokens, limit) {
            (Some(used), Some(lim)) => {
                let pct = (used / lim * 100.).min(100.);
                let buffer_pct = if auto { (100. - thr).clamp(0., 100.) } else { 0. };
                let msg_w = (pct / 100.) as f32;
                let buf_w = (buffer_pct as f32 / 100.).min((1. - msg_w).max(0.));
                let free_w = (1. - msg_w - buf_w).max(0.);
                let bar = div()
                    .flex()
                    .h(t.rem(0.375))
                    .w_full()
                    .rounded_full()
                    .overflow_hidden()
                    .bg(t.colors.raised)
                    .child(div().h_full().w(gpui::relative(msg_w)).bg(if pct >= 90. { t.colors.danger } else if pct >= 70. { t.colors.warning } else { t.colors.accent }))
                    .child(div().h_full().w(gpui::relative(free_w)))
                    .child(div().h_full().w(gpui::relative(buf_w)).bg(t.colors.active));
                context = context
                    .child(div().flex().items_center().justify_between().child(div().child(i.tp("ov.used", &[("pct", &format!("{pct:.0}"))]))).child(div().small(&t).text_color(t.colors.fg_muted).child(format!("{} / {}", tokens(used), tokens(lim)))))
                    .child(bar);
                let legend = |color: gpui::Hsla, label: String, n: f64| {
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .small(&t)
                        .child(div().flex_none().size(t.rem(0.5)).rounded(t.rem(rem::RADIUS_SM)).bg(color))
                        .child(div().flex_1().child(label))
                        .child(div().flex_none().text_color(t.colors.fg_muted).child(tokens(n)))
                        .child(div().flex_none().w(t.rem(3.)).text_right().child(format!("{:.1}%", n / lim * 100.)))
                };
                context = context
                    .child(legend(t.colors.accent, i.t("ov.messages"), used))
                    .child(legend(t.colors.active, i.t("ov.buffer"), lim * buffer_pct / 100.))
                    .child(legend(t.colors.raised, i.t("ov.free"), (lim - used - lim * buffer_pct / 100.).max(0.)));
            }
            (Some(used), None) => {
                context = context.child(div().child(format!("{} ({})", tokens(used), i.t("info.contextUnknown"))));
            }
            _ => {
                context = context.child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ov.contextNone")));
            }
        }
        let items: Vec<(SharedString, SharedString)> = [70, 80, 90, 95].iter().map(|n| (SharedString::from(n.to_string()), SharedString::from(format!("{n}%")))).collect();
        let settings = SettingsStore::global(cx);
        let settings2 = settings.clone();
        context = context
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(t.space(2))
                    .child(div().child(i.t("ov.autoCompactAt")))
                    .child(select("ov-compact-at", items, format!("{}", thr as i64)).disabled(!auto).on_change(move |v, _, cx| {
                        if let Ok(n) = v.parse::<i64>() {
                            settings.update(cx, |s, cx| {
                                let _ = s.set("autoCompactAt", serde_json::Value::from(n), cx);
                            });
                        }
                    })),
            )
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ov.autoCompactHint")))
            .child(switch("ov-compact", i.t("settings.autoCompact"), auto).on_change(move |on, _, cx| {
                settings2.update(cx, |s, cx| {
                    let _ = s.set("autoCompact", serde_json::Value::from(on), cx);
                });
            }));

        // --- Perubahan
        let diff = self.diff.as_ref().filter(|(sid, _)| *sid == env.sid).map(|(_, d)| d.clone());
        let mut changes = div().flex().flex_col().gap(t.space(2)).child(section_title(&t, i.t("ov.changes")));
        match diff {
            None => changes = changes.child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ov.changesLoading"))),
            Some(d) => {
                let (add, del) = d.iter().fold((0, 0), |a, x| (a.0 + x.additions, a.1 + x.deletions));
                let dir = env.dir.clone();
                let can_pr = branch.is_some() && !d.is_empty();
                changes = changes.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .child(icon("diff").size(IconSize::Sm).color(t.colors.fg_muted))
                        .child(if d.is_empty() {
                            div().flex_1().text_color(t.colors.fg_muted).child(i.t("chg.empty"))
                        } else {
                            div()
                                .flex_1()
                                .flex()
                                .gap(t.space(2))
                                .child(div().text_color(t.colors.code_add).child(format!("+{add}")))
                                .child(div().text_color(t.colors.code_del).child(format!("\u{2212}{del}")))
                                .child(div().text_color(t.colors.fg_muted).child(i.tp("ov.files", &[("n", &d.len().to_string())])))
                        })
                        .child(button("ov-open-changes").size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("ov.openChanges")).on_click(|_, _, cx| {
                            cx.set_global(super::layout::OpenToolRequest(Some(super::layout::ToolId::Changes)));
                        }))
                        .child(button("ov-pr").size(ControlSize::Sm).loading(self.pr_busy).disabled(!can_pr).label(i.t("ov.createPr")).on_click(cx.listener(move |s, _, _, cx| s.create_pr(dir.clone(), cx)))),
                );
                if branch.is_none() {
                    changes = changes.child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("ov.prNoRepo")));
                }
            }
        }

        let divider = || div().w_full().border_t_1().border_color(t.colors.border_soft);
        div()
            .id("overview")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(3))
                    .p(t.space(3))
                    .child(environment)
                    .child(divider())
                    .child(context)
                    .child(divider())
                    .child(changes)
                    .child(divider())
                    .child(self.info.clone()),
            )
            .into_any_element()
    }
}

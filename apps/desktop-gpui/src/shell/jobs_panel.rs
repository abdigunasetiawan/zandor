//! Panel 4 — Jobs (ADR-072): port JobsPanel.tsx. Background job yang dijalankan agent (dev server, watcher): cakupan
//! sesi ini / semua, status (+ kode keluar), perintah, port (buka di Browser Zandor), log yang diperbarui saat berjalan,
//! serta Stop / Jalankan ulang / Hapus. Data dibaca dari berkas plugin engine (jobs.rs bersama), dipoll tiap 1,5 detik.
use crate::core::projects::ProjectStore;
use crate::core::services::app_data_dir;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{icon, icon_button, select, working, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, IntoElement, ScrollHandle, SharedString, Window};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

const POLL: Duration = Duration::from_millis(1500);

fn jobs_dir() -> PathBuf {
    app_data_dir().join("jobs")
}

fn is_live(s: &str) -> bool {
    s == "running" || s == "starting"
}

pub struct JobsPanel {
    jobs: Vec<Value>,
    error: Option<String>,
    scope: SharedString,
    open: Option<String>,
    /// Log job yang dibuka (None = memuat).
    log: Option<String>,
    log_scroll: ScrollHandle,
    /// Daftar pertama sudah dibaca.
    loaded: bool,
    /// Panel sedang tampil (polling hanya saat tampil).
    pub shown: bool,
}

/// Minta tampilkan Browser dan buka URL (port openInZandorBrowser).
pub struct OpenInBrowser(pub String);
impl gpui::EventEmitter<OpenInBrowser> for JobsPanel {}

impl JobsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(POLL).await;
            let alive = this
                .update(cx, |s, cx| {
                    if s.shown {
                        s.refresh(cx);
                    }
                })
                .is_ok();
            if !alive {
                break;
            }
        })
        .detach();
        let mut me = Self { jobs: vec![], error: None, scope: "session".into(), open: None, log: None, log_scroll: ScrollHandle::new(), loaded: false, shown: true };
        me.refresh(cx);
        me
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let open = self.open.clone();
        let task = cx.background_spawn(async move {
            let jobs = crate::jobs::list(&jobs_dir());
            let log = open.map(|id| crate::jobs::log(&jobs_dir(), &id, 256 * 1024).unwrap_or_default());
            (jobs, log)
        });
        cx.spawn(async move |this, cx| {
            let (jobs, log) = task.await;
            this.update(cx, |s, cx| {
                let changed = s.jobs != jobs || !s.loaded;
                s.loaded = true;
                s.jobs = jobs;
                s.error = None;
                if let Some(l) = log {
                    if s.log.as_deref() != Some(l.as_str()) {
                        // ikuti ke bawah hanya bila pengguna sudah di dekat dasar
                        let h = &s.log_scroll;
                        let at_bottom = h.max_offset().y - h.offset().y.abs() < gpui::px(24.);
                        s.log = Some(l);
                        if at_bottom {
                            s.log_scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    }
                }
                if changed {
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn act(&mut self, id: String, action: &'static str, cx: &mut Context<Self>) {
        if let Err(e) = crate::jobs::control(&jobs_dir(), &id, action) {
            self.error = Some(e);
        }
        if action == "remove" && self.open.as_deref() == Some(id.as_str()) {
            self.open = None;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(400)).await;
            this.update(cx, |s, cx| s.refresh(cx)).ok();
        })
        .detach();
        cx.notify();
    }

    fn toggle(&mut self, id: String, cx: &mut Context<Self>) {
        if self.open.as_deref() == Some(id.as_str()) {
            self.open = None;
        } else {
            self.open = Some(id);
            self.log = None;
            self.refresh(cx);
        }
        cx.notify();
    }
}

fn notice(t: &Theme, text: String, big: bool) -> AnyElement {
    if big {
        super::widgets::tool_notice(t, "server", text)
    } else {
        div().px(t.space(3)).py(t.space(2)).child(super::widgets::tool_notice_inline(t, text)).into_any_element()
    }
}

impl Render for JobsPanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let session = ProjectStore::global(cx).read(cx).selection.session_id.clone();
        let shown: Vec<Value> = if self.scope.as_ref() == "session" && session.is_some() {
            self.jobs.iter().filter(|j| j["sessionID"].as_str() == session.as_deref()).cloned().collect()
        } else {
            self.jobs.clone()
        };
        let running = self.jobs.iter().filter(|j| is_live(j["status"].as_str().unwrap_or(""))).count();
        let me = cx.entity().downgrade();
        let bar = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .px(t.space(3))
            .py(t.space(2))
            .border_b_1()
            .border_color(t.colors.border_soft)
            .child(select("jobs-scope", vec![("session".into(), i.t("jobs.scope.session").into()), ("all".into(), i.t("jobs.scope.all").into())], self.scope.clone()).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.scope = v;
                    cx.notify();
                })
                .ok();
            }))
            .child(div().flex_1().small(&t).text_color(t.colors.fg_muted).child(i.tp("jobs.count", &[("count", &shown.len().to_string()), ("running", &running.to_string())])))
            .child(icon_button("jobs-refresh", "refresh", i.t("jobs.refresh")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| s.refresh(cx))));
        let mut col = div().flex().flex_col().size_full().child(bar);
        if let Some(e) = &self.error {
            col = col.child(notice(&t, i.tp("jobs.error", &[("error", e)]), false));
        }
        if self.scope.as_ref() == "session" && session.is_none() {
            col = col.child(notice(&t, i.t("jobs.noSession"), false));
        }
        if !self.loaded {
            return col.child(notice(&t, i.t("jobs.loading"), false));
        }
        if shown.is_empty() {
            return col.child(notice(&t, i.t("jobs.empty"), true));
        }
        let mut list = div().id("jobs-list").flex().flex_col().flex_1().min_h_0().overflow_y_scroll();
        for j in shown {
            let s = |k: &str| j[k].as_str().unwrap_or("").to_string();
            let (id, name, command, cwd, status) = (s("id"), s("name"), s("command"), s("cwd"), s("status"));
            let live = is_live(&status);
            let st_label = i.t(&format!("jobs.status.{status}"));
            let status_text = match j["exitCode"].as_i64() {
                Some(code) if status != "stopped" => i.tp("jobs.statusCode", &[("status", &st_label), ("code", &code.to_string())]),
                _ => st_label,
            };
            let expanded = self.open.as_deref() == Some(id.as_str());
            let id1 = id.clone();
            let mut actions = div().flex().flex_none().items_center().gap(t.space(1));
            for p in j["ports"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
                actions = actions.child(icon_button(SharedString::from(format!("job-port-{id}-{p}")), "globe", i.tp("jobs.openPort", &[("port", &p.to_string())])).size(ControlSize::Sm).disabled(!live).on_click(cx.listener(move |_, _, _, cx| cx.emit(OpenInBrowser(format!("http://localhost:{p}"))))));
            }
            let (id2, id3) = (id.clone(), id.clone());
            actions = if live {
                actions.child(icon_button(SharedString::from(format!("job-stop-{id}")), "stop", i.t("jobs.stop")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.act(id2.clone(), "stop", cx))))
            } else {
                actions.child(icon_button(SharedString::from(format!("job-rm-{id}")), "trash", i.t("jobs.remove")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.act(id2.clone(), "remove", cx))))
            };
            if status != "lost" {
                actions = actions.child(icon_button(SharedString::from(format!("job-re-{id}")), "refresh", i.t("jobs.restart")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.act(id3.clone(), "restart", cx))));
            }
            // Latar hover dipasang pada SELURUH baris (termasuk ikon aksi), lembut (alpha), bukan hanya sisi kiri.
            let hover_bg = t.colors.hover.opacity(0.55);
            let head = div()
                .id(SharedString::from(format!("job-row-{id}")))
                .flex()
                .items_center()
                .gap(t.space(1))
                .pr(t.space(1))
                .rounded(t.rem(rem::RADIUS_MD))
                .hover(move |s| s.bg(hover_bg))
                .child(
                    div()
                        .id(SharedString::from(format!("job-{id}")))
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap(t.space(2))
                        .px(t.space(2))
                        .py(t.space(1))
                        .cursor_pointer()
                        .tooltip(crate::ui::tooltip::text_tooltip(command.clone().into()))
                        .on_click(cx.listener(move |s, _, _, cx| s.toggle(id1.clone(), cx)))
                        .child(icon(if expanded { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm).color(t.colors.fg_subtle))
                        .child(if live {
                            working(&format!("job-w-{id}")).into_any_element()
                        } else {
                            div()
                                .flex_none()
                                .size(t.rem(0.5))
                                .rounded_full()
                                .bg(match status.as_str() {
                                    "failed" | "lost" => t.colors.danger,
                                    "exited" => t.colors.success,
                                    _ => t.colors.fg_subtle,
                                })
                                .into_any_element()
                        })
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .child(div().truncate().font_weight(gpui::FontWeight::MEDIUM).child(name.clone()))
                                .child(div().flex().gap(t.space(2)).small(&t).text_color(t.colors.fg_muted).child(div().flex_none().child(status_text)).child(div().min_w_0().truncate().code(&t).child(command.clone()))),
                        ),
                )
                .child(actions);
            let mut row = div().flex().flex_col().gap(t.space(1)).px(t.space(2)).py(t.space(1)).border_b_1().border_color(t.colors.border_soft).child(head);
            if expanded {
                let text = match &self.log {
                    None => i.t("jobs.loadingLog"),
                    Some(l) if l.is_empty() => i.t("jobs.noOutput"),
                    Some(l) => l.clone(),
                };
                row = row
                    .child(div().px(t.space(2)).small(&t).text_color(t.colors.fg_muted).truncate().child(cwd))
                    .child(div().id(SharedString::from(format!("job-log-{id}"))).track_scroll(&self.log_scroll).max_h(t.rem(18.)).overflow_y_scroll().p(t.space(2)).mx(t.space(1)).rounded(t.rem(rem::RADIUS_SM)).bg(t.colors.raised).code(&t).child(text));
            }
            list = list.child(row);
        }
        col.child(list)
    }
}

//! Panel alat — host GitPanel (git_panel.rs), FilesTree (files_tree.rs), JobsPanel, ChangesPanel, SessionInfo, rencana sesi. Memakai git.rs/jobs.rs
//! bersama dan API engine; Terminal (terminal.rs) dan Browser (browser.rs, webview native) sebagai entity terpisah.
use super::layout::ToolId;
use crate::core::chat_store::ChatStore;
use crate::core::engine::SessionFileDiff;
use crate::core::projects::ProjectStore;
use crate::core::services::app_data_dir;
use crate::core::types::{PartKind, Role, TodoStatus, ToolStatus};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{badge, button, icon, icon_button, BadgeTone, ButtonVariant, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use std::path::PathBuf;

pub struct ToolsHost {
    pub tool: ToolId,
    files_tree: Option<Entity<super::files_tree::FilesTree>>,
    git_panel: Option<Entity<super::git_panel::GitPanel>>,
    changes: Option<Result<Vec<SessionFileDiff>, String>>,
    changes_for: Option<String>,
    open_change: Option<String>,
    jobs: Vec<serde_json::Value>,
    job_log: Option<(String, String)>,
    terminal: Option<Entity<super::terminal::Terminal>>,
    browser: Option<Entity<super::browser::BrowserPanel>>,
    log_filter: String,
    trajectory: Option<Entity<super::trajectory_panel::TrajectoryPanel>>,
    log_observed: bool,
}

fn jobs_dir() -> PathBuf {
    app_data_dir().join("jobs")
}

impl ToolsHost {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = ProjectStore::global(cx);
        cx.observe(&store, |s, _, cx| {
            s.sync(cx);
            cx.notify();
        })
        .detach();
        cx.observe(&ChatStore::global(cx), |_, _, cx| cx.notify()).detach();
        let mut me = Self {
            tool: ToolId::Terminal,
            files_tree: None,
            git_panel: None,
            changes: None,
            changes_for: None,
            open_change: None,
            jobs: vec![],
            job_log: None,
            terminal: None,
            browser: None,
            log_filter: "all".into(),
            trajectory: None,
            log_observed: false,
        };
        me.sync(cx);
        me
    }

    /// Direktori kerja aktif: sesi terpilih → direktorinya; atau project terpilih.
    fn dir(&self, cx: &gpui::App) -> Option<String> {
        let st = ProjectStore::global(cx).read(cx);
        if let Some((_, s, p)) = st.selected() {
            if !s.directory.is_empty() {
                return Some(s.directory);
            }
            return p.map(|p| p.path);
        }
        let key = st.selection.key.clone()?;
        st.projects.iter().find(|p| p.id == key).map(|p| p.path.clone())
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let sid = ProjectStore::global(cx).read(cx).selection.session_id.clone();
        if sid != self.changes_for {
            self.changes_for = sid;
            self.changes = None;
            self.refresh_changes(cx);
        }
        self.jobs = crate::jobs::list(&jobs_dir());
    }

    fn refresh_changes(&mut self, cx: &mut Context<Self>) {
        let (Some(sid), Some(dir)) = (self.changes_for.clone(), self.dir(cx)) else { return };
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        let task = cx.background_spawn(async move { api.session_diff(&dir, &sid).map_err(|e| e.0) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.changes = Some(r);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn note(t: &Theme, s: String) -> AnyElement {
    div().p(t.space(3)).text_color(t.colors.fg_muted).child(s).into_any_element()
}

pub(crate) fn diff_view(t: &Theme, patch: &str) -> AnyElement {
    let mut col = div().flex().flex_col().code(t).p(t.space(2)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.raised);
    for l in patch.lines().take(800) {
        let c = if l.starts_with('+') && !l.starts_with("+++") {
            t.colors.code_add
        } else if l.starts_with('-') && !l.starts_with("---") {
            t.colors.code_del
        } else if l.starts_with("@@") {
            t.colors.code_keyword
        } else {
            t.colors.fg_muted
        };
        col = col.child(div().text_color(c).child(if l.is_empty() { " ".to_string() } else { l.to_string() }));
    }
    col.into_any_element()
}

impl ToolsHost {
    fn render_git(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.git_panel.get_or_insert_with(|| cx.new(super::git_panel::GitPanel::new)).clone().into_any_element()
    }

    fn render_files(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ft = self.files_tree.get_or_insert_with(|| cx.new(super::files_tree::FilesTree::new)).clone();
        ft.into_any_element()
    }

    fn render_changes(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        if self.changes_for.is_none() {
            return note(t, i.t("info.empty"));
        }
        let dir = self.dir(cx).unwrap_or_default();
        let list = match self.changes.clone() {
            None => vec![],
            Some(Err(e)) => return note(t, i.tp("chg.error", &[("error", &e)])),
            Some(Ok(l)) => l,
        };
        let (add, del) = list.iter().fold((0, 0), |a, d| (a.0 + d.additions, a.1 + d.deletions));
        let summary: AnyElement = if self.changes.is_none() {
            crate::ui::spinner(IconSize::Sm).into_any_element()
        } else {
            div().small(t).text_color(t.colors.fg_muted).child(i.tp("chg.summary", &[("n", &list.len().to_string()), ("add", &add.to_string()), ("del", &del.to_string())])).into_any_element()
        };
        let mut col = div().flex().flex_col().gap(t.space(1)).p(t.space(2)).child(
            div()
                .flex()
                .items_center()
                .child(div().flex_1().child(summary))
                .child(icon_button("ch-refresh", "refresh", i.t("git.refresh")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| s.refresh_changes(cx)))),
        );
        if matches!(self.changes, Some(Ok(ref l)) if l.is_empty()) {
            col = col.child(div().p(t.space(2)).small(t).text_color(t.colors.fg_muted).child(i.t("chg.empty")));
        }
        for d in list {
            let open = self.open_change.as_deref() == Some(d.file.as_str());
            let f = d.file.clone();
            let rel = std::path::Path::new(&d.file).strip_prefix(&dir).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| d.file.clone());
            let (key, color) = match d.status.as_str() {
                "added" => ("chg.status.added", t.colors.code_add),
                "deleted" => ("chg.status.deleted", t.colors.code_del),
                _ => ("chg.status.modified", t.colors.warning),
            };
            let mut row = div()
                .flex()
                .items_center()
                .gap(t.space(1))
                .child(
                    div()
                        .id(SharedString::from(format!("ch-{}", d.file)))
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap(t.space(2))
                        .px(t.space(1))
                        .h(t.rem(rem::CONTROL_SM))
                        .rounded(t.rem(rem::RADIUS_SM))
                        .cursor_pointer()
                        .hover(|s| s.bg(t.colors.hover))
                        .tooltip(crate::ui::tooltip::text_tooltip(d.file.clone().into()))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.open_change = if s.open_change.as_deref() == Some(f.as_str()) { None } else { Some(f.clone()) };
                            cx.notify();
                        }))
                        .child(div().flex_none().small(t).font_weight(gpui::FontWeight::SEMIBOLD).text_color(color).child(i.t(key)))
                        .child(div().flex_1().min_w_0().truncate().child(rel)),
                )
                .child(div().flex_none().code(t).text_color(t.colors.code_add).child(format!("+{}", d.additions)))
                .child(div().flex_none().code(t).text_color(t.colors.code_del).child(format!("\u{2212}{}", d.deletions)));
            if d.status != "deleted" && !d.patch.is_empty() {
                let (f, p) = (d.file.clone(), d.patch.clone());
                row = row.child(crate::ui::button(SharedString::from(format!("chd-{}", d.file))).size(ControlSize::Sm).label(i.t("diff.open")).on_click(move |_, _, cx| {
                    super::docs_panel::DocsState::global(cx).update(cx, |s, cx| s.open_diff(f.clone(), p.clone(), cx));
                }));
            }
            if d.status != "deleted" {
                let f = d.file.clone();
                row = row.child(crate::ui::button(SharedString::from(format!("cho-{}", d.file))).size(ControlSize::Sm).label(i.t("chg.open")).on_click(move |_, _, cx| {
                    super::docs_panel::DocsState::global(cx).update(cx, |s, cx| s.open_file(f.clone(), cx));
                }));
            }
            col = col.child(row);
            if open {
                col = col.child(if d.patch.is_empty() { div().small(t).text_color(t.colors.fg_muted).child(i.t("chg.noPatch")).into_any_element() } else { diff_view(t, &d.patch) });
            }
        }
        col.into_any_element()
    }

    fn render_jobs(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div().flex().flex_col().gap(t.space(2)).p(t.space(3));
        let running = self.jobs.iter().filter(|j| j["status"] == "running").count();
        col = col.child(
            div()
                .flex()
                .items_center()
                .child(div().flex_1().small(t).text_color(t.colors.fg_muted).child(i.tp("jobs.count", &[("count", &self.jobs.len().to_string()), ("running", &running.to_string())])))
                .child(icon_button("jobs-refresh", "refresh", i.t("jobs.refresh")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| {
                    s.jobs = crate::jobs::list(&jobs_dir());
                    cx.notify();
                }))),
        );
        if self.jobs.is_empty() {
            col = col.child(div().text_color(t.colors.fg_muted).child(i.t("jobs.empty")));
        }
        for j in self.jobs.clone() {
            let id = j["id"].as_str().unwrap_or("").to_string();
            let name = j["name"].as_str().or(j["command"].as_str()).unwrap_or(&id).to_string();
            let status = j["status"].as_str().unwrap_or("lost").to_string();
            let tone = match status.as_str() {
                "running" | "starting" => BadgeTone::Warning,
                "exited" => BadgeTone::Success,
                "failed" | "lost" => BadgeTone::Danger,
                _ => BadgeTone::Neutral,
            };
            let act = |a: &'static str, ic: &'static str, label: String, id: String| {
                icon_button(SharedString::from(format!("job-{a}-{id}")), ic, label).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| {
                    if let Err(e) = crate::jobs::control(&jobs_dir(), &id, a) {
                        eprintln!("[zandor] {e}");
                    }
                    s.jobs = crate::jobs::list(&jobs_dir());
                    cx.notify();
                }))
            };
            let id2 = id.clone();
            let open_log = self.job_log.as_ref().is_some_and(|(x, _)| *x == id);
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .p(t.space(2))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .border_1()
                    .border_color(t.colors.border_soft)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(name.clone()))
                            .child(badge(i.t(&format!("jobs.status.{status}"))).tone(tone))
                            .child(
                                icon_button(SharedString::from(format!("job-log-{id}")), "file_text", i.tp("jobs.log", &[("name", &name)])).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| {
                                    s.job_log = if s.job_log.as_ref().is_some_and(|(x, _)| *x == id2) {
                                        None
                                    } else {
                                        Some((id2.clone(), crate::jobs::log(&jobs_dir(), &id2, 64 * 1024).unwrap_or_else(|e| e)))
                                    };
                                    cx.notify();
                                })),
                            )
                            .child(act("stop", "stop", i.t("jobs.stop"), id.clone()))
                            .child(act("restart", "refresh", i.t("jobs.restart"), id.clone()))
                            .child(act("remove", "trash", i.t("jobs.remove"), id.clone())),
                    )
                    .children(j["command"].as_str().map(|c| div().code(t).text_color(t.colors.fg_muted).truncate().child(c.to_string())))
                    .when(open_log, |d| {
                        let text = self.job_log.as_ref().map(|(_, l)| l.clone()).unwrap_or_default();
                        d.child(div().p(t.space(2)).rounded(t.rem(rem::RADIUS_SM)).bg(t.colors.raised).code(t).child(if text.is_empty() { i.t("jobs.noOutput") } else { text }))
                    }),
            );
        }
        col.into_any_element()
    }

    fn render_info(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>, plan: bool) -> AnyElement {
        let st = ProjectStore::global(cx).read(cx);
        let Some((_, s, p)) = st.selected() else { return note(t, i.t("info.empty")) };
        let chat = ChatStore::global(cx).read(cx).sessions.get(&s.id).cloned();
        if plan {
            let todos = chat.map(|c| c.todos).unwrap_or_default();
            if todos.is_empty() {
                return note(t, i.t("todo.heading"));
            }
            let done = todos.iter().filter(|x| x.status == TodoStatus::Completed).count();
            let mut col = div().flex().flex_col().gap(t.space(1)).p(t.space(3)).child(div().font_weight(FontWeight::SEMIBOLD).child(i.tp("todo.title", &[("done", &done.to_string()), ("total", &todos.len().to_string())])));
            for td in todos {
                let (ic, c) = match td.status {
                    TodoStatus::Completed => ("check", t.colors.success),
                    TodoStatus::InProgress => ("activity", t.colors.warning),
                    TodoStatus::Cancelled => ("close", t.colors.fg_subtle),
                    TodoStatus::Pending => ("list", t.colors.fg_muted),
                };
                col = col.child(div().flex().gap(t.space(2)).child(icon(ic).size(IconSize::Sm).color(c)).child(div().flex_1().child(td.content)));
            }
            return col.into_any_element();
        }
        let mut kv: Vec<(String, String)> = vec![
            (i.t("info.project"), p.map(|p| p.name).unwrap_or_else(|| i.t("chat.noProject"))),
            (i.t("info.dir"), s.directory.clone()),
            (i.t("info.session"), s.id.clone()),
        ];
        if let Some(c) = chat {
            let user = c.messages.iter().filter(|m| m.info.role == Role::User).count();
            let asst = c.messages.len() - user;
            kv.push((i.t("info.messages"), i.tp("info.messagesValue", &[("user", &user.to_string()), ("assistant", &asst.to_string())])));
            let (mut inp, mut out, mut cost) = (0., 0., 0.);
            let (mut ok, mut failed, mut running) = (0, 0, 0);
            let mut model = None;
            for m in &c.messages {
                if let Some(tk) = &m.info.tokens {
                    inp += tk.input;
                    out += tk.output;
                }
                cost += m.info.cost.unwrap_or(0.);
                if m.info.model_id.is_some() {
                    model = m.info.model_id.clone();
                }
                for p in &m.parts {
                    if let PartKind::Tool { state, .. } = &p.kind {
                        match state.status {
                            ToolStatus::Completed => ok += 1,
                            ToolStatus::Error => failed += 1,
                            _ => running += 1,
                        }
                    }
                }
            }
            if let Some(m) = model {
                kv.push((i.t("info.model"), m));
            }
            kv.push((i.t("info.tokens"), format!("{} · {}", inp as i64, out as i64)));
            kv.push((i.t("info.cost"), if cost > 0. { format!("${cost:.4}") } else { i.t("info.costNone") }));
            kv.push((i.t("info.tools"), i.tp("info.toolsValue", &[("ok", &ok.to_string()), ("failed", &failed.to_string()), ("running", &running.to_string())])));
        }
        div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .p(t.space(3))
            .children(kv.into_iter().map(|(k, v)| div().flex().flex_col().child(div().small(t).text_color(t.colors.fg_subtle).child(k)).child(div().truncate().child(v))))
            .into_any_element()
    }
}

impl ToolsHost {
    fn render_log(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        use crate::core::activity::{Activity, SOURCES};
        let Some(act) = Activity::global(cx) else { return note(t, i.t("act.empty")) };
        if !self.log_observed {
            self.log_observed = true;
            cx.observe(&act, |_, _, cx| cx.notify()).detach();
        }
        let filter = self.log_filter.clone();
        let all = act.read(cx).entries.clone();
        let shown: Vec<_> = all.iter().filter(|e| filter == "all" || e.source == filter).cloned().collect();
        let items: Vec<(SharedString, SharedString)> = SOURCES.iter().map(|s| (SharedString::from(*s), SharedString::from(i.t(&format!("act.src.{s}"))))).collect();
        let me = cx.entity();
        let act2 = act.clone();
        let bar = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .p(t.space(2))
            .child(crate::ui::select("act-filter", items, filter).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.log_filter = v.to_string();
                    cx.notify();
                })
            }))
            .child(div().flex_1().small(t).text_color(t.colors.fg_muted).child(i.tp("act.count", &[("count", &shown.len().to_string())])))
            .child(button("act-clear").label(i.t("act.clear")).size(ControlSize::Sm).disabled(all.is_empty()).on_click(move |_, _, cx| act2.update(cx, |a, cx| a.clear(cx))));
        let mut list = div().flex().flex_col().px(t.space(2)).pb(t.space(2));
        if shown.is_empty() {
            list = list.child(note(t, i.t("act.empty")));
        }
        for e in shown.iter() {
            let color = match e.level {
                "error" => t.colors.danger,
                "warn" => t.colors.warning,
                _ => t.colors.fg,
            };
            list = list.child(
                div()
                    .flex()
                    .gap(t.space(2))
                    .py(t.space(1))
                    .border_b_1()
                    .border_color(t.colors.border_soft)
                    .child(div().flex_none().small(t).text_color(t.colors.fg_subtle).child(crate::core::automation::fmt_time(e.at)))
                    .child(div().flex_none().small(t).text_color(t.colors.fg_muted).child(i.t(&format!("act.src.{}", e.source))))
                    .child(div().flex_1().min_w_0().text_color(color).child(e.message.clone())),
            );
        }
        div().flex().flex_col().child(bar).child(list).into_any_element()
    }

    fn render_trajectory(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let st = ProjectStore::global(cx).read(cx);
        let Some((_, s, _)) = st.selected() else { return note(t, i.t("info.empty")) };
        let Some(chat) = ChatStore::global(cx).read(cx).sessions.get(&s.id).cloned() else { return note(t, i.t("traj.empty")) };
        if chat.messages.is_empty() {
            return note(t, i.t("traj.empty"));
        }
        let mut col = div().flex().flex_col().gap(t.space(2)).p(t.space(3));
        let mut turn = 0;
        for m in &chat.messages {
            if m.info.role == Role::User {
                turn += 1;
                col = col.child(div().mt(t.space(2)).font_weight(FontWeight::SEMIBOLD).child(i.tp("traj.turn", &[("n", &turn.to_string())])));
            }
            for p in &m.parts {
                let (kind, label, color, dur): (&str, String, gpui::Hsla, Option<f64>) = match &p.kind {
                    PartKind::Text { text, synthetic } if m.info.role == Role::User => {
                        (if *synthetic { "context" } else { "user" }, text.lines().next().unwrap_or("").to_string(), t.colors.accent, None)
                    }
                    PartKind::Text { text, .. } => ("assistant", text.lines().next().unwrap_or("").to_string(), t.colors.fg, None),
                    PartKind::Reasoning { text } => ("thinking", text.lines().next().unwrap_or("").to_string(), t.colors.fg_muted, None),
                    PartKind::Tool { tool, state, .. } => (
                        if state.status == ToolStatus::Error { "error" } else { "tool" },
                        format!("{tool} {}", state.title.clone().unwrap_or_default()),
                        if state.status == ToolStatus::Error { t.colors.danger } else { t.colors.warning },
                        state.time.and_then(|(a, b)| b.map(|b| b - a)),
                    ),
                    PartKind::StepFinish { tokens, .. } => ("step", format!("{} tok", tokens.total as i64), t.colors.fg_subtle, None),
                    PartKind::File { filename, .. } => ("other", filename.clone().unwrap_or_default(), t.colors.fg_subtle, None),
                    PartKind::Other { raw_type } => ("other", raw_type.clone(), t.colors.fg_subtle, None),
                };
                col = col.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .small(t)
                        .child(div().flex_none().w(t.rem(0.25)).h(t.rem(1.)).rounded_full().bg(color))
                        .child(div().flex_none().w(t.rem(5.)).text_color(t.colors.fg_subtle).child(i.t(&format!("traj.kind.{kind}"))))
                        .child(div().flex_1().min_w_0().truncate().child(label))
                        .children(dur.map(|d| div().flex_none().code(t).text_color(t.colors.fg_subtle).child(format!("{:.1}s", d / 1000.)))),
                );
            }
        }
        col.into_any_element()
    }
}

impl Render for ToolsHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        if self.tool == ToolId::Browser {
            let b = self.browser.get_or_insert_with(|| cx.new(super::browser::BrowserPanel::new)).clone();
            return div().size_full().child(b).into_any_element();
        }
        if let Some(b) = &self.browser {
            b.update(cx, |b, _| b.hide_all());
        }
        if self.tool == ToolId::Trajectory {
            let p = self.trajectory.get_or_insert_with(|| cx.new(super::trajectory_panel::TrajectoryPanel::new)).clone();
            return div().size_full().child(p).into_any_element();
        }
        if self.tool == ToolId::Terminal {
            if self.terminal.is_none() {
                let dir = self.dir(cx);
                self.terminal = Some(cx.new(|cx| super::terminal::Terminal::new(dir, cx)));
            }
            return div().size_full().children(self.terminal.clone()).into_any_element();
        }
        let body = match self.tool {
            ToolId::Git => self.render_git(cx),
            ToolId::Files => self.render_files(cx),
            ToolId::Changes => self.render_changes(&t, &i, cx),
            ToolId::Jobs => self.render_jobs(&t, &i, cx),
            ToolId::Info => self.render_info(&t, &i, cx, false),
            ToolId::Session => self.render_info(&t, &i, cx, true),
            ToolId::Trajectory => self.render_trajectory(&t, &i, cx),
            ToolId::Log => self.render_log(&t, &i, cx),
            other => note(&t, format!("{} — belum tersedia di versi GPUI.", other.key())),
        };
        div().id("tools-body").size_full().overflow_y_scroll().child(body).into_any_element()
    }
}

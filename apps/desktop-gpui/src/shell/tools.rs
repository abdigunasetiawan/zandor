//! Panel alat — port GitPanel, FilesTree, JobsPanel, ChangesPanel, SessionInfo, rencana sesi. Memakai git.rs/jobs.rs
//! bersama dan API engine. Terminal, Browser, Trajektori, dan Log belum diport (ditampilkan apa adanya).
use super::layout::ToolId;
use crate::core::chat_store::ChatStore;
use crate::core::engine::{FileNode, SessionFileDiff};
use crate::core::projects::ProjectStore;
use crate::core::services::app_data_dir;
use crate::core::types::{PartKind, Role, TodoStatus, ToolStatus};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{badge, button, icon, icon_button, BadgeTone, ButtonVariant, ControlSize, IconSize, TextInput, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct ToolsHost {
    pub tool: ToolId,
    git: Option<Result<crate::git::GitStatus, String>>,
    git_log: Vec<crate::git::CommitInfo>,
    git_dir: Option<String>,
    diff: Option<(String, String)>,
    commit_msg: Entity<TextInput>,
    files: HashMap<String, Result<Vec<FileNode>, String>>,
    open_dirs: HashSet<String>,
    files_root: Option<String>,
    changes: Option<Result<Vec<SessionFileDiff>, String>>,
    changes_for: Option<String>,
    open_change: Option<String>,
    jobs: Vec<serde_json::Value>,
    job_log: Option<(String, String)>,
    terminal: Option<Entity<super::terminal::Terminal>>,
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
        let ph = cx.global::<I18n>().t("git.commitMsg");
        let commit_msg = cx.new(|cx| TextInput::new(cx, ph));
        let mut me = Self {
            tool: ToolId::Terminal,
            git: None,
            git_log: vec![],
            git_dir: None,
            diff: None,
            commit_msg,
            files: HashMap::new(),
            open_dirs: HashSet::new(),
            files_root: None,
            changes: None,
            changes_for: None,
            open_change: None,
            jobs: vec![],
            job_log: None,
            terminal: None,
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
        let dir = self.dir(cx);
        if dir != self.git_dir {
            self.git_dir = dir.clone();
            self.git = None;
            self.diff = None;
            self.refresh_git(cx);
        }
        if dir != self.files_root {
            self.files_root = dir.clone();
            self.files.clear();
            self.open_dirs.clear();
            if let Some(d) = dir {
                self.load_dir(d, String::new(), cx);
            }
        }
        let sid = ProjectStore::global(cx).read(cx).selection.session_id.clone();
        if sid != self.changes_for {
            self.changes_for = sid;
            self.changes = None;
            self.refresh_changes(cx);
        }
        self.jobs = crate::jobs::list(&jobs_dir());
    }

    fn refresh_git(&mut self, cx: &mut Context<Self>) {
        let Some(dir) = self.git_dir.clone() else { return };
        let task = cx.background_spawn(async move {
            let p = PathBuf::from(&dir);
            let st = crate::git::status(&p);
            let log = crate::git::log(&p, 10).unwrap_or_default();
            (st, log)
        });
        cx.spawn(async move |this, cx| {
            let (st, log) = task.await;
            this.update(cx, |s, cx| {
                s.git = Some(st);
                s.git_log = log;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn git_op(&mut self, f: impl FnOnce(&std::path::Path) -> Result<String, String> + Send + 'static, cx: &mut Context<Self>) {
        let Some(dir) = self.git_dir.clone() else { return };
        let task = cx.background_spawn(async move { f(&PathBuf::from(dir)) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                let (msg, tone) = match r {
                    Ok(m) => (m, crate::ui::ToastTone::Success),
                    Err(e) => (e, crate::ui::ToastTone::Danger),
                };
                if !msg.trim().is_empty() {
                    crate::ui::Toasts::show(crate::ui::ToastInput { title: msg.into(), description: None, tone, timeout_ms: Some(4000), action: None }, cx);
                }
                s.refresh_git(cx);
            })
            .ok();
        })
        .detach();
    }

    fn load_dir(&mut self, root: String, rel: String, cx: &mut Context<Self>) {
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        let r2 = rel.clone();
        let task = cx.background_spawn(async move { api.list_files(&root, &r2).map_err(|e| e.0) });
        cx.spawn(async move |this, cx| {
            let mut r = task.await;
            if let Ok(v) = r.as_mut() {
                v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
            }
            this.update(cx, |s, cx| {
                s.files.insert(rel, r);
                cx.notify();
            })
            .ok();
        })
        .detach();
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

fn diff_view(t: &Theme, patch: &str) -> AnyElement {
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
    fn render_git(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        if self.git_dir.is_none() {
            return note(t, i.t("git.noProject"));
        }
        let Some(st) = self.git.clone() else { return note(t, i.t("git.loading")) };
        let st = match st {
            Err(e) => return note(t, e),
            Ok(s) if !s.is_repo => return note(t, i.t("git.notRepo")),
            Ok(s) => s,
        };
        let mut col = div().flex().flex_col().gap(t.space(2)).p(t.space(3));
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(t.space(2))
                .child(icon("git").size(IconSize::Sm))
                .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(if st.detached { i.t("git.detached") } else { st.branch.clone().unwrap_or_default() }))
                .when(st.ahead > 0 || st.behind > 0, |d| d.child(div().small(t).text_color(t.colors.fg_muted).child(format!("↑{} ↓{}", st.ahead, st.behind))))
                .child(icon_button("git-pull", "download", i.t("git.pull")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| s.git_op(|d| crate::git::pull(d), cx))))
                .child(icon_button("git-push", "upload", i.t("git.push")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| s.git_op(|d| crate::git::push(d), cx))))
                .child(icon_button("git-refresh", "refresh", i.t("git.refresh")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| s.refresh_git(cx)))),
        );
        let staged: Vec<_> = st.files.iter().filter(|f| f.index != ' ' && f.index != '?').cloned().collect();
        let changes: Vec<_> = st.files.iter().filter(|f| f.worktree != ' ' || f.index == '?').cloned().collect();
        if st.files.is_empty() {
            col = col.child(div().text_color(t.colors.fg_muted).child(i.t("git.clean")));
        }
        for (title, list, is_staged) in [(i.t("git.staged"), staged, true), (i.t("git.changes"), changes, false)] {
            if list.is_empty() {
                continue;
            }
            col = col.child(div().mt(t.space(2)).small(t).font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg_subtle).child(format!("{title} · {}", list.len())));
            for f in list {
                let code = if is_staged { f.index } else if f.index == '?' { 'A' } else { f.worktree };
                let (p1, p2, p3) = (f.path.clone(), f.path.clone(), f.path.clone());
                let untracked = f.index == '?';
                col = col.child(
                    div()
                        .id(SharedString::from(format!("gf-{is_staged}-{}", f.path)))
                        .group("gf")
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .px(t.space(1))
                        .h(t.rem(rem::CONTROL_SM))
                        .rounded(t.rem(rem::RADIUS_SM))
                        .cursor_pointer()
                        .hover(|s| s.bg(t.colors.hover))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            let p = p1.clone();
                            let Some(dir) = s.git_dir.clone() else { return };
                            let text = crate::git::diff(&PathBuf::from(dir), &p, is_staged, untracked).unwrap_or_else(|e| e);
                            s.diff = Some((p, text));
                            cx.notify();
                        }))
                        .child(div().w(t.rem(1.)).code(t).text_color(match code {
                            'A' => t.colors.code_add,
                            'D' => t.colors.code_del,
                            'U' => t.colors.danger,
                            _ => t.colors.warning,
                        }).child(code.to_string()))
                        .child(div().flex_1().min_w_0().truncate().child(f.path.clone()))
                        .child(
                            div().invisible().group_hover("gf", |s| s.visible()).child(
                                icon_button(SharedString::from(format!("gs-{is_staged}-{}", f.path)), if is_staged { "undo" } else { "plus" }, i.tp(if is_staged { "git.unstage" } else { "git.stage" }, &[("name", &f.path)]))
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        let p = vec![p2.clone()];
                                        s.git_op(move |d| if is_staged { crate::git::unstage(d, &p) } else { crate::git::stage(d, &p) }.map(|_| String::new()), cx)
                                    })),
                            ),
                        )
                        .when(!is_staged, |d| {
                            d.child(
                                div().invisible().group_hover("gf", |s| s.visible()).child(
                                    icon_button(SharedString::from(format!("gd-{}", f.path)), "trash", i.tp("git.discardFile", &[("name", &f.path)]))
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            let p = vec![p3.clone()];
                                            s.git_op(move |d| crate::git::discard(d, &p).map(|_| String::new()), cx)
                                        })),
                                ),
                            )
                        }),
                );
            }
        }
        col = col.child(
            div()
                .flex()
                .gap(t.space(2))
                .mt(t.space(2))
                .child(div().flex_1().child(self.commit_msg.clone()))
                .child(button("git-commit").label(i.t("git.commit")).variant(ButtonVariant::Solid).on_click(cx.listener(|s, _, _, cx| {
                    let msg = s.commit_msg.read(cx).text().trim().to_string();
                    if msg.is_empty() {
                        return;
                    }
                    s.commit_msg.update(cx, |m, cx| m.set_text("", cx));
                    s.git_op(move |d| crate::git::commit(d, &msg), cx)
                }))),
        );
        if let Some((path, text)) = self.diff.clone() {
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .mt(t.space(2))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(div().flex_1().small(t).font_weight(FontWeight::SEMIBOLD).child(i.tp("git.diffTitle", &[("path", &path)])))
                            .child(icon_button("git-closediff", "close", i.t("git.closeDiff")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| {
                                s.diff = None;
                                cx.notify();
                            }))),
                    )
                    .child(if text.trim().is_empty() { note(t, i.t("git.diffEmpty")) } else { diff_view(t, &text) }),
            );
        }
        if !self.git_log.is_empty() {
            col = col.child(div().mt(t.space(2)).small(t).font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg_subtle).child(i.t("git.recent")));
            for c in &self.git_log {
                col = col.child(
                    div()
                        .flex()
                        .gap(t.space(2))
                        .small(t)
                        .child(div().code(t).text_color(t.colors.fg_subtle).child(c.hash.chars().take(7).collect::<String>()))
                        .child(div().flex_1().min_w_0().truncate().child(c.subject.clone())),
                );
            }
        }
        col.into_any_element()
    }

    fn file_rows(&self, t: &Theme, rel: &str, depth: usize, out: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let Some(r) = self.files.get(rel) else {
            out.push(div().pl(t.rem(0.75 * depth as f32 + 0.5)).text_color(t.colors.fg_subtle).small(t).child("…").into_any_element());
            return;
        };
        let nodes = match r {
            Err(e) => {
                out.push(div().pl(t.rem(0.75 * depth as f32 + 0.5)).text_color(t.colors.danger).small(t).child(e.clone()).into_any_element());
                return;
            }
            Ok(n) => n.clone(),
        };
        for n in nodes {
            let open = self.open_dirs.contains(&n.path);
            let p = n.path.clone();
            let abs = if n.absolute.is_empty() { format!("{}/{}", self.files_root.clone().unwrap_or_default(), n.path) } else { n.absolute.clone() };
            let is_dir = n.is_dir;
            out.push(
                div()
                    .id(SharedString::from(format!("fn-{}", n.path)))
                    .flex()
                    .items_center()
                    .gap(t.space(1))
                    .h(t.rem(rem::CONTROL_SM))
                    .pl(t.rem(0.75 * depth as f32 + 0.25))
                    .pr(t.space(2))
                    .rounded(t.rem(rem::RADIUS_SM))
                    .cursor_pointer()
                    .text_color(if n.ignored { t.colors.fg_subtle } else { t.colors.fg })
                    .hover(|s| s.bg(t.colors.hover))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if !is_dir {
                            let abs = abs.clone();
                            super::docs_panel::DocsState::global(cx).update(cx, |d, cx| d.open_file(abs, cx));
                            return;
                        }
                        if !s.open_dirs.remove(&p) {
                            s.open_dirs.insert(p.clone());
                            if !s.files.contains_key(&p) {
                                if let Some(root) = s.files_root.clone() {
                                    s.load_dir(root, p.clone(), cx);
                                }
                            }
                        }
                        cx.notify();
                    }))
                    .child(if is_dir {
                        icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm).color(t.colors.fg_subtle).into_any_element()
                    } else {
                        div().w(t.rem(rem::ICON_SM)).into_any_element()
                    })
                    .child(icon(if is_dir { if open { "folder_open" } else { "folder" } } else { "file" }).size(IconSize::Sm).color(t.colors.fg_muted))
                    .child(div().flex_1().min_w_0().truncate().child(n.name.clone()))
                    .into_any_element(),
            );
            if is_dir && open {
                self.file_rows(t, &n.path, depth + 1, out, cx);
            }
        }
    }

    fn render_files(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        if self.files_root.is_none() {
            return note(t, i.t("files.noProject"));
        }
        let mut rows = vec![];
        self.file_rows(t, "", 0, &mut rows, cx);
        if matches!(self.files.get(""), Some(Ok(v)) if v.is_empty()) {
            return note(t, i.t("files.empty"));
        }
        div().flex().flex_col().p(t.space(2)).children(rows).into_any_element()
    }

    fn render_changes(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        if self.changes_for.is_none() {
            return note(t, i.t("info.empty"));
        }
        let Some(r) = self.changes.clone() else { return note(t, i.t("mem.loading")) };
        let list = match r {
            Err(e) => return note(t, e),
            Ok(l) if l.is_empty() => return note(t, i.t("git.clean")),
            Ok(l) => l,
        };
        let mut col = div().flex().flex_col().gap(t.space(1)).p(t.space(2)).child(
            div().flex().justify_end().child(icon_button("ch-refresh", "refresh", i.t("git.refresh")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| s.refresh_changes(cx)))),
        );
        for d in list {
            let open = self.open_change.as_deref() == Some(d.file.as_str());
            let f = d.file.clone();
            col = col.child(
                div()
                    .id(SharedString::from(format!("ch-{}", d.file)))
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .px(t.space(1))
                    .h(t.rem(rem::CONTROL_SM))
                    .rounded(t.rem(rem::RADIUS_SM))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.colors.hover))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.open_change = if s.open_change.as_deref() == Some(f.as_str()) { None } else { Some(f.clone()) };
                        cx.notify();
                    }))
                    .child(icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
                    .child(div().flex_1().min_w_0().truncate().child(d.file.clone()))
                    .child(div().code(t).text_color(t.colors.code_add).child(format!("+{}", d.additions)))
                    .child(div().code(t).text_color(t.colors.code_del).child(format!("-{}", d.deletions))),
            );
            if open {
                col = col.child(diff_view(t, &d.patch));
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
    fn render_log(&mut self, t: &Theme, _i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let acts = ChatStore::global(cx).read(cx).activity.clone();
        if acts.is_empty() {
            return note(t, "—".into());
        }
        div()
            .flex()
            .flex_col()
            .p(t.space(2))
            .code(t)
            .children(acts.iter().rev().take(300).map(|(ts, s)| {
                div()
                    .flex()
                    .gap(t.space(2))
                    .child(div().flex_none().text_color(t.colors.fg_subtle).child(crate::core::automation::fmt_time(*ts)))
                    .child(div().min_w_0().truncate().child(s.clone()))
            }))
            .into_any_element()
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
        if self.tool == ToolId::Terminal {
            if self.terminal.is_none() {
                let dir = self.dir(cx);
                self.terminal = Some(cx.new(|cx| super::terminal::Terminal::new(dir, cx)));
            }
            return div().size_full().children(self.terminal.clone()).into_any_element();
        }
        let body = match self.tool {
            ToolId::Git => self.render_git(&t, &i, cx),
            ToolId::Files => self.render_files(&t, &i, cx),
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

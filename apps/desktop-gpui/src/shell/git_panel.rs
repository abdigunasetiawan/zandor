//! Panel 4 — Git: port GitPanel.tsx + GitStore (packages/core/src/tools/git.ts). Status, stage/unstage (per berkas dan
//! semua), buang perubahan (dengan konfirmasi), diff, commit, cabang (pilih/buat), pull/push dengan ↓/↑, stash/pop,
//! konflik, pilih repo di anak folder (monorepo), dan riwayat commit. Menjalankan `git` CLI lewat git.rs bersama.
use crate::core::projects::{ProjectStore, CHAT_KEY};
use crate::git::{Branch, CommitInfo, FileChange, GitStatus};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, select, ButtonVariant, ControlSize, IconSize, InputChanged, TextInput, Toasts, ToastInput, ToastTone, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Groups {
    staged: Vec<FileChange>,
    unstaged: Vec<FileChange>,
    conflicts: Vec<FileChange>,
}

/// Port groupChanges.
fn group_changes(files: &[FileChange]) -> Groups {
    let mut g = Groups::default();
    for f in files {
        let conflict = f.index == 'U' || f.worktree == 'U' || (f.index == 'A' && f.worktree == 'A') || (f.index == 'D' && f.worktree == 'D');
        if conflict {
            g.conflicts.push(f.clone());
            continue;
        }
        if f.index != ' ' && f.index != '?' {
            g.staged.push(f.clone());
        }
        if f.worktree != ' ' || f.index == '?' {
            g.unstaged.push(f.clone());
        }
    }
    g
}

fn untracked(f: &FileChange) -> bool {
    f.index == '?'
}

fn status_label(i: &I18n, f: &FileChange, staged: bool) -> String {
    if untracked(f) {
        return i.t("git.s.?");
    }
    let c = if staged { f.index } else { f.worktree };
    if "MADRCTU".contains(c) { i.t(&format!("git.s.{c}")) } else { c.to_string() }
}

struct Loaded {
    root: String,
    repos: Vec<String>,
    status: GitStatus,
    branches: Vec<Branch>,
    log: Vec<CommitInfo>,
    stashes: usize,
}

pub struct GitPanel {
    project_root: Option<String>,
    root: Option<String>,
    repos: Vec<String>,
    /// Repo pilihan pengguna per project (selama aplikasi berjalan).
    chosen: HashMap<String, String>,
    status: Option<GitStatus>,
    branches: Vec<Branch>,
    log: Vec<CommitInfo>,
    stashes: usize,
    loading: bool,
    busy: bool,
    error: Option<String>,
    seq: u64,
    /// Diff terbuka: (path, teks; None = memuat, error).
    diff: Option<(String, Option<String>, Option<String>)>,
    message: Entity<TextInput>,
    new_branch: Option<Entity<TextInput>>,
    confirm: Option<(Vec<String>, String)>,
}

fn activity(cx: &mut gpui::App, msg: String) {
    let level = if msg.starts_with("error") { "error" } else { "info" };
    crate::core::activity::log(cx, "git", level, msg);
}

/// Executable git tersedia? (dicek sekali per jalannya aplikasi)
fn git_available() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| {
        let mut c = std::process::Command::new("git");
        c.arg("--version").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000);
        }
        c.status().map(|s| s.success()).unwrap_or(false)
    })
}

impl GitPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let i = cx.global::<I18n>().clone();
        let message = cx.new(|cx| {
            let mut t = TextInput::new(cx, i.t("git.commitMsg"));
            t.multiline = true;
            t.enter_newline = true;
            t
        });
        cx.subscribe(&message, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        cx.observe(&ProjectStore::global(cx), |s, _, cx| s.sync(cx)).detach();
        let mut me = Self {
            project_root: None,
            root: None,
            repos: vec![],
            chosen: HashMap::new(),
            status: None,
            branches: vec![],
            log: vec![],
            stashes: 0,
            loading: false,
            busy: false,
            error: None,
            seq: 0,
            diff: None,
            message,
            new_branch: None,
            confirm: None,
        };
        me.sync(cx);
        me
    }

    /// Project aktif (folder Chat bila tanpa project) → set root.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let root = {
            let st = ProjectStore::global(cx).read(cx);
            let key = st.selection.key.clone().unwrap_or_else(|| CHAT_KEY.to_string());
            st.projects.iter().find(|p| p.id == key).map(|p| p.path.clone())
        };
        if root == self.project_root {
            return;
        }
        self.seq += 1;
        self.project_root = root.clone();
        self.root = root;
        self.repos.clear();
        self.status = None;
        self.branches.clear();
        self.log.clear();
        self.stashes = 0;
        self.diff = None;
        self.error = None;
        self.refresh(cx);
    }

    fn select_repo(&mut self, dir: String, cx: &mut Context<Self>) {
        let Some(pr) = self.project_root.clone() else { return };
        if !self.repos.contains(&dir) || Some(&dir) == self.root.as_ref() {
            return;
        }
        self.chosen.insert(pr, dir.clone());
        self.seq += 1;
        self.root = Some(dir);
        self.status = None;
        self.branches.clear();
        self.log.clear();
        self.diff = None;
        self.error = None;
        self.refresh(cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        self.seq += 1;
        let n = self.seq;
        self.loading = true;
        self.error = None;
        let at_project = self.project_root.as_deref() == Some(root.as_str());
        let chosen = self.chosen.get(&root).cloned();
        let prev_repos = self.repos.clone();
        let task = cx.background_spawn(async move {
            let mut status = crate::git::status(Path::new(&root))?;
            let mut active = root.clone();
            let mut repos = prev_repos;
            if !status.is_repo && at_project {
                repos = crate::git::find_repos(Path::new(&root));
                let pick = repos.iter().find(|r| Some(*r) == chosen.as_ref()).or(repos.first()).cloned();
                if let Some(p) = pick {
                    status = crate::git::status(Path::new(&p))?;
                    active = p;
                }
            }
            let (branches, log, stashes) = if status.is_repo {
                let p = PathBuf::from(&active);
                (crate::git::branches(&p).unwrap_or_default(), crate::git::log(&p, 15).unwrap_or_default(), crate::git::stash_count(&p).unwrap_or(0))
            } else {
                (vec![], vec![], 0)
            };
            Ok::<_, String>(Loaded { root: active, repos, status, branches, log, stashes })
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                if n != s.seq {
                    return;
                }
                s.loading = false;
                match r {
                    Ok(l) => {
                        // diff yang dibuka tetap ditampilkan hanya bila berkasnya masih berubah
                        if let Some((p, ..)) = &s.diff {
                            if !l.status.files.iter().any(|f| &f.path == p) {
                                s.diff = None;
                            }
                        }
                        s.root = Some(l.root);
                        s.repos = l.repos;
                        s.status = Some(l.status);
                        s.branches = l.branches;
                        s.log = l.log;
                        s.stashes = l.stashes;
                    }
                    Err(e) => s.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Jalankan operasi git di background; sukses → `ok(hasil)` lalu muat ulang; gagal → toast + catatan aktivitas.
    fn act(&mut self, f: impl FnOnce(&Path) -> Result<String, String> + Send + 'static, ok: impl FnOnce(&mut Self, String, &mut Context<Self>) + 'static, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        self.busy = true;
        self.error = None;
        let task = cx.background_spawn(async move { f(Path::new(&root)) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.busy = false;
                match r {
                    Ok(out) => ok(s, out, cx),
                    Err(e) => {
                        s.error = Some(e.clone());
                        activity(cx, format!("error: {e}"));
                        let title = cx.global::<I18n>().tp("sb.error", &[("error", &e)]);
                        Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(title) }, cx);
                    }
                }
                s.refresh(cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn show_diff(&mut self, f: &FileChange, staged: bool, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let (path, un) = (f.path.clone(), untracked(f));
        self.diff = Some((path.clone(), None, None));
        let p2 = path.clone();
        let task = cx.background_spawn(async move { crate::git::diff(Path::new(&root), &p2, staged, un) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                if s.diff.as_ref().is_some_and(|d| d.0 == path) {
                    s.diff = Some(match r {
                        Ok(t) => (path, Some(t), None),
                        Err(e) => (path, None, Some(e)),
                    });
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let staged = self.status.as_ref().map(|s| group_changes(&s.files).staged.len()).unwrap_or(0);
        if staged == 0 {
            let title = cx.global::<I18n>().t("git.commitNeedStaged");
            Toasts::show(ToastInput { tone: ToastTone::Warning, ..ToastInput::new(title) }, cx);
            return;
        }
        let msg = self.message.read(cx).text().trim().to_string();
        if msg.is_empty() {
            return;
        }
        self.act(
            move |d| crate::git::commit(d, &msg),
            |s, hash, cx| {
                s.message.update(cx, |m, cx| m.set_text("", cx));
                activity(cx, format!("commit {hash}"));
                let title = cx.global::<I18n>().tp("git.commitDone", &[("hash", &hash)]);
                Toasts::show(ToastInput { tone: ToastTone::Success, ..ToastInput::new(title) }, cx);
            },
            cx,
        );
    }

    fn net(&mut self, push: bool, cx: &mut Context<Self>) {
        self.act(
            move |d| if push { crate::git::push(d) } else { crate::git::pull(d) },
            move |_, out, cx| {
                activity(cx, format!("{}: {}", if push { "push" } else { "pull" }, if out.trim().is_empty() { "ok" } else { out.trim() }));
                let title = cx.global::<I18n>().t(if push { "git.pushed" } else { "git.pulled" });
                Toasts::show(ToastInput { tone: ToastTone::Success, ..ToastInput::new(title) }, cx);
            },
            cx,
        );
    }

    fn stage(&mut self, paths: Vec<String>, on: bool, cx: &mut Context<Self>) {
        self.act(move |d| if on { crate::git::stage(d, &paths) } else { crate::git::unstage(d, &paths) }.map(|_| String::new()), |_, _, _| {}, cx);
    }

    fn discard_now(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let n = paths.len();
        self.act(move |d| crate::git::discard(d, &paths).map(|_| String::new()), move |_, _, cx| activity(cx, format!("discard {n} berkas")), cx);
    }

    fn row(&self, t: &Theme, i: &I18n, f: &FileChange, staged: bool, cx: &mut Context<Self>) -> AnyElement {
        let kind = if staged { f.index } else if untracked(f) { '?' } else { f.worktree };
        let color = match kind {
            'A' | '?' => t.colors.code_add,
            'D' => t.colors.code_del,
            'U' => t.colors.danger,
            _ => t.colors.warning,
        };
        let label = match &f.orig_path {
            Some(o) => format!("{o} → {}", f.path),
            None => f.path.clone(),
        };
        let (fc, p1, p2, p3) = (f.clone(), f.path.clone(), f.path.clone(), f.path.clone());
        div()
            .flex()
            .items_center()
            .gap(t.space(1))
            .child(
                div()
                    .id(SharedString::from(format!("gf-{staged}-{}", f.path)))
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
                    .tooltip(crate::ui::tooltip::text_tooltip(i.tp("git.diff", &[("name", &p1)]).into()))
                    .on_click(cx.listener(move |s, _, _, cx| s.show_diff(&fc, staged, cx)))
                    .child(div().flex_none().px(t.space(1)).rounded(t.rem(rem::RADIUS_SM)).small(t).font_weight(FontWeight::SEMIBOLD).text_color(color).child(status_label(i, f, staged)))
                    .child(div().flex_1().min_w_0().truncate().child(label)),
            )
            .child(button(SharedString::from(format!("gs-{staged}-{}", f.path))).size(ControlSize::Sm).disabled(self.busy).tooltip_text(i.tp(if staged { "git.unstage" } else { "git.stage" }, &[("name", &f.path)])).label(if staged { "−" } else { "+" }).on_click(cx.listener(move |s, _, _, cx| s.stage(vec![p2.clone()], !staged, cx))))
            .when(!staged, |d| {
                d.child(icon_button(SharedString::from(format!("gd-{}", f.path)), "undo", i.tp("git.discardFile", &[("name", &f.path)])).size(ControlSize::Sm).disabled(self.busy).on_click(cx.listener(move |s, _, _, cx| {
                    s.confirm = Some((vec![p3.clone()], p3.clone()));
                    cx.notify();
                })))
            })
            .into_any_element()
    }
}

fn notice(t: &Theme, ic: &'static str, text: String) -> AnyElement {
    super::widgets::tool_notice(t, ic, text)
}

impl Render for GitPanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let wrap = |el: AnyElement| div().id("gitp").size_full().overflow_y_scroll().child(el);
        if !git_available() {
            return wrap(notice(&t, "git", i.t("git.unavailable")));
        }
        if self.project_root.is_none() {
            return wrap(notice(&t, "git", i.t("git.noProject")));
        }
        if self.loading && self.status.is_none() {
            return wrap(div().p(t.space(3)).small(&t).child(format!("… {}", i.t("git.loading"))).into_any_element());
        }
        let refresh_btn = |id: &'static str, cx: &mut Context<Self>| button(id).label(i.t("git.refresh")).on_click(cx.listener(|s, _, _, cx| s.refresh(cx)));
        if let (Some(e), None) = (&self.error, &self.status) {
            return wrap(div().flex().flex_col().gap(t.space(2)).p(t.space(3)).child(div().small(&t).text_color(t.colors.danger).child(e.clone())).child(div().child(refresh_btn("git-r1", cx))).into_any_element());
        }
        let Some(st) = self.status.clone().filter(|s| s.is_repo) else {
            return wrap(div().flex().flex_col().gap(t.space(2)).p(t.space(3)).child(div().child(i.t("git.notRepo"))).child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("git.notRepoHint"))).child(div().child(refresh_btn("git-r2", cx))).into_any_element());
        };
        let groups = group_changes(&st.files);
        let me = cx.entity().downgrade();

        // ---- bilah atas: repo, cabang, cabang baru, muat ulang ----
        let mut bar = div().flex().items_center().gap(t.space(2));
        if !self.repos.is_empty() {
            let base = self.project_root.clone().unwrap_or_default().trim_end_matches(['\\', '/']).to_string();
            let items: Vec<(SharedString, SharedString)> = self
                .repos
                .iter()
                .map(|r| {
                    let rel = if r.len() > base.len() { r[base.len() + 1..].to_string() } else { r.clone() };
                    (r.clone().into(), rel.replace('\\', "/").into())
                })
                .collect();
            let me = me.clone();
            bar = bar.child(select("git-repo", items, self.root.clone().unwrap_or_default()).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| s.select_repo(v.to_string(), cx)).ok();
            }));
        }
        if !self.branches.is_empty() && !st.detached {
            let items: Vec<(SharedString, SharedString)> = self.branches.iter().map(|b| (b.name.clone().into(), b.name.clone().into())).collect();
            let cur = self.branches.iter().find(|b| b.current).map(|b| b.name.clone()).unwrap_or_default();
            let me = me.clone();
            bar = bar.child(select("git-branch", items, cur).on_change(move |v, _, cx| {
                let name = v.to_string();
                me.update(cx, |s, cx| s.act(move |d| crate::git::switch(d, &name).map(|_| String::new()), |_, _, _| {}, cx)).ok();
            }));
        } else {
            bar = bar.child(div().font_weight(FontWeight::SEMIBOLD).child(if st.detached { i.t("git.detached") } else { st.branch.clone().unwrap_or_else(|| "?".into()) }));
        }
        bar = bar
            .child(div().flex_1())
            .child(icon_button("git-newbranch", "plus", i.t("git.newBranch")).size(ControlSize::Sm).on_click(cx.listener(|s, _, w, cx| {
                if s.new_branch.take().is_none() {
                    let ph = cx.global::<I18n>().t("git.branchName");
                    let e = cx.new(|cx| TextInput::new(cx, ph));
                    cx.subscribe(&e, |_, _, _: &InputChanged, cx| cx.notify()).detach();
                    cx.subscribe(&e, |s, _, _: &crate::ui::InputSubmit, cx| s.create_branch(cx)).detach();
                    let fh = gpui::Focusable::focus_handle(e.read(cx), cx);
                    w.focus(&fh, cx);
                    s.new_branch = Some(e);
                }
                cx.notify();
            })))
            .child(icon_button("git-refresh", "refresh", i.t("git.refresh")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| s.refresh(cx))));

        let busy = self.busy;
        let sync = div()
            .flex()
            .flex_wrap()
            .gap(t.space(1))
            .child(button("git-pull").size(ControlSize::Sm).disabled(busy || st.upstream.is_none()).child(icon("download").size(IconSize::Sm)).label(if st.behind > 0 { format!("{} ↓{}", i.t("git.pull"), st.behind) } else { i.t("git.pull") }).on_click(cx.listener(|s, _, _, cx| s.net(false, cx))))
            .child(button("git-push").size(ControlSize::Sm).disabled(busy).child(icon("upload").size(IconSize::Sm)).label(if st.ahead > 0 { format!("{} ↑{}", i.t("git.push"), st.ahead) } else { i.t("git.push") }).on_click(cx.listener(|s, _, _, cx| s.net(true, cx))))
            .child(button("git-stash").size(ControlSize::Sm).disabled(busy || st.files.is_empty()).child(icon("archive").size(IconSize::Sm)).label(i.t("git.stash")).on_click(cx.listener(|s, _, _, cx| s.act(|d| crate::git::stash_push(d, ""), |_, _, _| {}, cx))))
            .child(button("git-pop").size(ControlSize::Sm).disabled(busy || self.stashes == 0).label(if self.stashes > 0 { format!("{} ({})", i.t("git.stashPop"), self.stashes) } else { i.t("git.stashPop") }).on_click(cx.listener(|s, _, _, cx| s.act(crate::git::stash_pop, |_, _, _| {}, cx))));

        let mut col = div().flex().flex_col().gap(t.space(2)).p(t.space(3)).child(bar).child(sync);
        if let Some(nb) = self.new_branch.clone() {
            let empty = nb.read(cx).text().trim().is_empty();
            col = col.child(div().flex().gap(t.space(2)).child(div().flex_1().child(nb)).child(button("git-create").variant(ButtonVariant::Solid).disabled(empty || busy).label(i.t("git.create")).on_click(cx.listener(|s, _, _, cx| s.create_branch(cx)))));
        }
        if let Some(e) = &self.error {
            col = col.child(div().small(&t).text_color(t.colors.danger).child(e.clone()));
        }
        if st.files.is_empty() {
            col = col.child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("git.clean")));
        }
        let head = |title: String, extra: Vec<AnyElement>| div().flex().items_center().gap(t.space(2)).mt(t.space(2)).child(div().flex_1().small(&t).font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg_subtle).child(title)).children(extra);
        if !groups.conflicts.is_empty() {
            col = col.child(head(i.t("git.conflicts"), vec![]));
            for f in &groups.conflicts {
                col = col.child(self.row(&t, &i, f, false, cx));
            }
        }
        if !groups.staged.is_empty() {
            let paths: Vec<String> = groups.staged.iter().map(|f| f.path.clone()).collect();
            col = col.child(head(format!("{} ({})", i.t("git.staged"), groups.staged.len()), vec![button("git-unstage-all").size(ControlSize::Sm).disabled(busy).label(i.t("git.unstageAll")).on_click(cx.listener(move |s, _, _, cx| s.stage(paths.clone(), false, cx))).into_any_element()]));
            for f in &groups.staged {
                col = col.child(self.row(&t, &i, f, true, cx));
            }
        }
        if !groups.unstaged.is_empty() {
            let paths: Vec<String> = groups.unstaged.iter().map(|f| f.path.clone()).collect();
            let p2 = paths.clone();
            let n = groups.unstaged.len();
            col = col.child(head(
                format!("{} ({n})", i.t("git.changes")),
                vec![
                    button("git-stage-all").size(ControlSize::Sm).disabled(busy).label(i.t("git.stageAll")).on_click(cx.listener(move |s, _, _, cx| s.stage(paths.clone(), true, cx))).into_any_element(),
                    button("git-discard-all").size(ControlSize::Sm).variant(ButtonVariant::Danger).disabled(busy).label(i.t("git.discardAll")).on_click(cx.listener(move |s, _, _, cx| {
                        let label = cx.global::<I18n>().tp("git.discardAllName", &[("n", &n.to_string())]);
                        s.confirm = Some((p2.clone(), label));
                        cx.notify();
                    })).into_any_element(),
                ],
            ));
            for f in &groups.unstaged {
                col = col.child(self.row(&t, &i, f, false, cx));
            }
        }

        if let Some((path, text, err)) = self.diff.clone() {
            col = col.child(head(
                i.tp("git.diffTitle", &[("path", &path)]),
                vec![button("git-closediff").size(ControlSize::Sm).label(i.t("git.closeDiff")).on_click(cx.listener(|s, _, _, cx| {
                    s.diff = None;
                    cx.notify();
                })).into_any_element()],
            ));
            if let Some(e) = err {
                col = col.child(div().small(&t).text_color(t.colors.danger).child(e));
            } else {
                match text {
                    None => col = col.child(crate::ui::spinner(IconSize::Sm)),
                    Some(x) if x.trim().is_empty() => col = col.child(div().small(&t).child(i.t("git.diffEmpty"))),
                    Some(x) => col = col.child(super::tools::diff_view(&t, &x)),
                }
            }
        }

        let msg_empty = self.message.read(cx).text().trim().is_empty();
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .mt(t.space(2))
                .child(div().min_h(t.rem(3.)).child(self.message.clone()))
                .child(div().flex().justify_end().child(button("git-commit").variant(ButtonVariant::Solid).disabled(busy || msg_empty || groups.staged.is_empty()).label(i.t("git.commit")).on_click(cx.listener(|s, _, _, cx| s.commit(cx))))),
        );

        if !self.log.is_empty() {
            col = col.child(head(i.t("git.recent"), vec![]));
            for c in &self.log {
                col = col.child(
                    div()
                        .id(SharedString::from(format!("gl-{}", c.hash)))
                        .flex()
                        .flex_col()
                        .py(t.space(1))
                        .tooltip(crate::ui::tooltip::text_tooltip(format!("{} {}", c.hash, c.subject).into()))
                        .child(div().truncate().child(c.subject.clone()))
                        .child(
                            div()
                                .flex()
                                .gap(t.space(2))
                                .small(&t)
                                .text_color(t.colors.fg_muted)
                                .child(div().code(&t).child(c.hash.clone()))
                                .child(div().truncate().child(c.author.clone()))
                                .child(div().flex_none().child(c.date.clone())),
                        ),
                );
            }
        }

        // ---- konfirmasi buang perubahan ----
        let dlg = self.confirm.clone().map(|(paths, label)| {
            crate::ui::dialog(i.tp("git.discardTitle", &[("name", &label)]))
                .description(i.t("git.discardBody"))
                .on_dismiss(cx.listener(|s, _, _, cx| {
                    s.confirm = None;
                    cx.notify();
                }))
                .footer(
                    div()
                        .flex()
                        .gap(t.space(2))
                        .child(button("git-dc-no").label(i.t("ui.cancel")).on_click(cx.listener(|s, _, _, cx| {
                            s.confirm = None;
                            cx.notify();
                        })))
                        .child(button("git-dc-yes").variant(ButtonVariant::Danger).label(i.t("git.discard")).on_click(cx.listener(move |s, _, _, cx| {
                            s.confirm = None;
                            s.discard_now(paths.clone(), cx);
                        }))),
                )
        });
        wrap(div().child(col).children(dlg).into_any_element())
    }
}

impl GitPanel {
    fn create_branch(&mut self, cx: &mut Context<Self>) {
        let Some(nb) = self.new_branch.clone() else { return };
        let name = nb.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        self.act(
            move |d| crate::git::create_branch(d, &name).map(|_| String::new()),
            |s, _, cx| {
                s.new_branch = None;
                cx.notify();
            },
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(p: &str, i: char, w: char) -> FileChange {
        FileChange { path: p.into(), orig_path: None, index: i, worktree: w }
    }
    #[test]
    fn grup() {
        let g = group_changes(&[f("a", 'M', ' '), f("b", ' ', 'M'), f("c", '?', '?'), f("d", 'U', 'U'), f("e", 'M', 'M')]);
        assert_eq!(g.staged.iter().map(|x| x.path.as_str()).collect::<Vec<_>>(), ["a", "e"]);
        assert_eq!(g.unstaged.iter().map(|x| x.path.as_str()).collect::<Vec<_>>(), ["b", "c", "e"]);
        assert_eq!(g.conflicts.len(), 1);
    }
}

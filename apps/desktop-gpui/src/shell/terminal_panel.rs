//! Panel 4 — Terminal: port TerminalPanel.tsx. Bilah konteks (nama + folder kerja, pemilih shell, terminal baru), tab
//! terminal per konteks (project / sesi Chat / folder Chat) dengan penanda selesai dan tombol tutup, serta keadaan kosong.
//! Terminal hidup di PTY engine (WebSocket), jadi tab dan riwayatnya pulih setelah aplikasi dibuka ulang; PTY lokal
//! (portable-pty) hanya dipakai sebagai cadangan bila engine tidak bisa membuat PTY.
use super::terminal::Terminal;
use crate::core::projects::{ProjectStore, CHAT_KEY};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, select, ButtonVariant, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, Context, Entity, IntoElement, SharedString, Window};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct ShellOption {
    pub id: String,
    pub name: String,
    pub path: String,
    pub args: Vec<String>,
}

fn which(exe: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(exe)).find(|p| p.is_file())
}

/// Deteksi shell terpasang (urutan = preferensi).
pub fn detect_shells() -> Vec<ShellOption> {
    let mut out = vec![];
    let mut add = |id: &str, name: &str, p: PathBuf, args: &[&str]| {
        out.push(ShellOption { id: id.into(), name: name.into(), path: p.to_string_lossy().into_owned(), args: args.iter().map(|s| s.to_string()).collect() });
    };
    if cfg!(windows) {
        if let Some(p) = which("pwsh.exe") {
            add("pwsh", "PowerShell 7", p, &["-NoLogo"]);
        }
        if let Some(p) = which("powershell.exe") {
            add("powershell", "Windows PowerShell", p, &["-NoLogo"]);
        }
        if let Some(p) = which("cmd.exe") {
            add("cmd", "Command Prompt", p, &[]);
        }
        let git = [r"C:\Program Files\Git\bin\bash.exe", r"C:\Program Files (x86)\Git\bin\bash.exe"].iter().map(PathBuf::from).find(|p| p.is_file());
        if let Some(p) = git {
            add("gitbash", "Git Bash", p, &["--login", "-i"]);
        }
        if let Some(p) = which("wsl.exe") {
            add("wsl", "WSL", p, &[]);
        }
    } else {
        for (id, name) in [("bash", "bash"), ("zsh", "zsh"), ("fish", "fish"), ("sh", "sh")] {
            if let Some(p) = which(id) {
                add(id, name, p, &[]);
            }
        }
    }
    out
}

struct Tab {
    id: usize,
    key: String,
    title: String,
    term: Entity<Terminal>,
    /// (folder, id PTY engine) — None untuk PTY lokal.
    remote: Option<(String, String)>,
    command: String,
}

pub struct TerminalPanel {
    shells: Vec<ShellOption>,
    shell: Option<String>,
    tabs: Vec<Tab>,
    active: HashMap<String, usize>,
    next: usize,
    /// Konteks yang PTY engine-nya sudah diadopsi (setelah aplikasi dibuka ulang).
    adopted: std::collections::HashSet<String>,
    error: Option<String>,
}

/// (kunci konteks, cwd, nama tampilan).
fn context(cx: &App, i: &I18n) -> (String, Option<String>, String) {
    let st = ProjectStore::global(cx).read(cx);
    let key = st.selection.key.clone().unwrap_or_else(|| CHAT_KEY.into());
    if let Some(p) = st.projects.iter().find(|p| p.id == key) {
        return (key, Some(p.path.clone()), p.name.clone());
    }
    if key == CHAT_KEY {
        if let Some((_, s, None)) = st.selected() {
            if !s.directory.is_empty() {
                return (format!("chat:{}", s.id), Some(s.directory.clone()), i.t("sb.chatGroup"));
            }
        }
        return (CHAT_KEY.into(), st.chat_dir.clone(), i.t("sb.chatGroup"));
    }
    (key, None, i.t("sb.chatGroup"))
}

impl TerminalPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&ProjectStore::global(cx), |_, _, cx| cx.notify()).detach();
        let task = cx.background_spawn(async { detect_shells() });
        cx.spawn(async move |this, cx| {
            let s = task.await;
            this.update(cx, |me, cx| {
                me.shells = s;
                cx.notify();
            })
            .ok();
        })
        .detach();
        Self { shells: vec![], shell: None, tabs: vec![], active: HashMap::new(), next: 1, adopted: Default::default(), error: None }
    }

    fn current(&self) -> Option<&ShellOption> {
        self.shell.as_ref().and_then(|id| self.shells.iter().find(|s| &s.id == id)).or(self.shells.first())
    }

    fn push(&mut self, key: String, title: String, command: String, term: Entity<Terminal>, remote: Option<(String, String)>, cx: &mut Context<Self>) {
        cx.observe(&term, |_, _, cx| cx.notify()).detach();
        let id = self.next;
        self.next += 1;
        self.tabs.push(Tab { id, key: key.clone(), title, term, remote, command });
        self.active.insert(key, id);
        cx.notify();
    }

    /// Ambil alih PTY engine yang masih hidup untuk konteks ini.
    fn adopt(&mut self, key: String, cwd: String, cx: &mut Context<Self>) {
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        let a2 = api.clone();
        let dir = cwd.clone();
        let task = cx.background_spawn(async move { a2.pty_list(&dir).unwrap_or_default() });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            this.update(cx, |s, cx| {
                for p in list.into_iter().filter(|p| p.running) {
                    if s.tabs.iter().any(|t| t.remote.as_ref().is_some_and(|r| r.1 == p.id)) {
                        continue;
                    }
                    let (api, dir, pid) = (api.clone(), cwd.clone(), p.id.clone());
                    let term = cx.new(|cx| Terminal::remote(api, dir, pid, cx));
                    s.push(key.clone(), p.title.clone(), p.command.clone(), term, Some((cwd.clone(), p.id)), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn create(&mut self, key: String, cwd: String, cx: &mut Context<Self>) {
        let sh = self.current().cloned();
        let name = sh.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| "Shell".into());
        let n = self.tabs.iter().filter(|t| t.key == key && t.title.starts_with(&name)).count() + 1;
        let title = format!("{name} {n}");
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        let (a2, dir, sh2, t2) = (api.clone(), cwd.clone(), sh.clone(), title.clone());
        let task = cx.background_spawn(async move {
            let (cmd, args) = sh2.map(|s| (s.path, s.args)).unwrap_or_else(|| (if cfg!(windows) { "powershell.exe".into() } else { "/bin/sh".into() }, vec![]));
            a2.pty_create(&dir, &cmd, &args, Some(&t2)).map(|p| (p, cmd))
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| match r {
                Ok((p, cmd)) => {
                    s.error = None;
                    let (dir, pid) = (cwd.clone(), p.id.clone());
                    let term = cx.new(|cx| Terminal::remote(api, dir, pid, cx));
                    s.push(key, title, cmd, term, Some((cwd, p.id)), cx);
                }
                Err(e) => {
                    // cadangan: PTY lokal, agar terminal tetap bisa dipakai
                    eprintln!("PTY engine gagal, memakai PTY lokal: {e}");
                    let cmd = sh.as_ref().map(|s| s.path.clone()).unwrap_or_default();
                    let term = cx.new(|cx| Terminal::with_shell(Some(cwd), sh.map(|s| (s.path, s.args)), cx));
                    s.push(key, title, cmd, term, None, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn close(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            let tab = self.tabs.remove(pos);
            if let (Some((dir, pid)), Some(api)) = (tab.remote.clone(), ProjectStore::global(cx).read(cx).api()) {
                cx.background_spawn(async move {
                    let _ = api.pty_remove(&dir, &pid);
                })
                .detach();
            }
            let key = tab.key;
            if self.active.get(&key) == Some(&id) {
                match self.tabs.iter().rev().find(|t| t.key == key) {
                    Some(t) => self.active.insert(key, t.id),
                    None => self.active.remove(&key),
                };
            }
        }
        cx.notify();
    }
}

fn notice(t: &Theme, text: String) -> AnyElement {
    div().flex().flex_col().items_center().gap(t.space(2)).p(t.space(6)).text_color(t.colors.fg_muted).child(icon("terminal").size(IconSize::Lg)).child(div().text_center().child(text)).into_any_element()
}

impl Render for TerminalPanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        if !ProjectStore::global(cx).read(cx).engine_ready {
            return notice(&t, i.t("tm.noEngine"));
        }
        let (key, cwd, name) = context(cx, &i);
        let Some(cwd) = cwd.filter(|d| Path::new(d).is_dir()) else { return notice(&t, i.t("tm.noContext")) };
        if self.adopted.insert(key.clone()) {
            self.adopt(key.clone(), cwd.clone(), cx);
        }
        let active = self.active.get(&key).copied().or_else(|| self.tabs.iter().rev().find(|x| x.key == key).map(|x| x.id));

        let me = cx.entity().downgrade();
        let (k1, c1) = (key.clone(), cwd.clone());
        let mut head = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(t.space(2))
            .px(t.space(2))
            .py(t.space(1))
            .border_b_1()
            .border_color(t.colors.border_soft)
            .child(
                div()
                    .id("tm-cwd")
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(t.space(1))
                    .small(&t)
                    .tooltip(crate::ui::tooltip::text_tooltip(cwd.clone().into()))
                    .child(icon("folder").size(IconSize::Sm).color(t.colors.fg_subtle))
                    .child(div().flex_none().font_weight(gpui::FontWeight::MEDIUM).child(name))
                    .child(div().min_w_0().truncate().text_color(t.colors.fg_muted).child(cwd.clone())),
            );
        if self.shells.len() > 1 {
            let cur = self.current().map(|s| s.id.clone()).unwrap_or_default();
            head = head.child(select("tm-shell", self.shells.iter().map(|s| (SharedString::from(s.id.clone()), SharedString::from(s.name.clone()))).collect(), cur).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.shell = Some(v.to_string());
                    cx.notify();
                })
                .ok();
            }));
        }
        head = head.child(icon_button("tm-new", "plus", i.t("tm.new")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.create(k1.clone(), c1.clone(), cx))));

        let mut col = div().flex().flex_col().size_full().child(head);
        if let Some(e) = &self.error {
            col = col.child(div().px(t.space(3)).py(t.space(1)).small(&t).text_color(t.colors.danger).child(e.clone()));
        }
        let tabs: Vec<&Tab> = self.tabs.iter().filter(|x| x.key == key).collect();
        if !tabs.is_empty() {
            let mut nav = div().id("tm-tabs").flex().flex_none().overflow_x_scroll().gap(t.space(1)).px(t.space(2)).py(t.space(1)).border_b_1().border_color(t.colors.border_soft);
            for x in &tabs {
                let on = Some(x.id) == active;
                let exited = x.term.read(cx).exited();
                let (id, k) = (x.id, key.clone());
                nav = nav.child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(t.space(1))
                        .pl(t.space(2))
                        .pr(t.space(1))
                        .py(t.rem(0.125))
                        .rounded(t.rem(rem::RADIUS_SM))
                        .when(on, |d| d.bg(t.colors.raised))
                        .child(
                            div()
                                .id(SharedString::from(format!("tm-tab-{id}")))
                                .flex()
                                .items_center()
                                .gap(t.space(1))
                                .cursor_pointer()
                                .small(&t)
                                .text_color(if on { t.colors.fg } else { t.colors.fg_muted })
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.active.insert(k.clone(), id);
                                    cx.notify();
                                }))
                                .child(icon("terminal").size(IconSize::Sm))
                                .child(x.title.clone())
                                .when(!x.command.is_empty(), |d| d)
                                .when(exited, |d| d.child(div().id(SharedString::from(format!("tm-x-{id}"))).text_color(t.colors.fg_subtle).tooltip(crate::ui::tooltip::text_tooltip(i.t("tm.exited").into())).child("✕"))),
                        )
                        .child(icon_button(SharedString::from(format!("tm-close-{id}")), "close", i.tp("tm.close", &[("title", &x.title)])).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.close(id, cx)))),
                );
            }
            col = col.child(nav);
        }
        let mut views = div().flex().flex_col().flex_1().min_h_0();
        if tabs.is_empty() {
            let (k2, c2) = (key.clone(), cwd.clone());
            views = views.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(t.space(3))
                    .size_full()
                    .text_color(t.colors.fg_muted)
                    .child(icon("terminal").size(IconSize::Lg))
                    .child(i.t("tm.empty"))
                    .child(button("tm-empty-new").variant(ButtonVariant::Solid).label(i.t("tm.new")).on_click(cx.listener(move |s, _, _, cx| s.create(k2.clone(), c2.clone(), cx)))),
            );
        }
        if let Some(x) = tabs.iter().find(|x| Some(x.id) == active) {
            let term = x.term.read(cx);
            if let Some(e) = term.error() {
                views = views.child(div().px(t.space(3)).py(t.space(2)).small(&t).text_color(t.colors.danger).child(i.tp("tm.error", &[("error", e)])));
            } else {
                let exited = term.exited();
                match term.conn_state() {
                    1 => views = views.child(div().flex().items_center().gap(t.space(1)).px(t.space(3)).py(t.space(1)).small(&t).text_color(t.colors.fg_muted).child(crate::ui::working("tm-conn")).child(i.t("tm.connecting"))),
                    2 => views = views.child(div().px(t.space(3)).py(t.space(1)).small(&t).text_color(t.colors.fg_muted).child(i.t("tm.reconnecting"))),
                    _ => {}
                }
                views = views.child(div().flex_1().min_h_0().child(x.term.clone()));
                if exited {
                    views = views.child(div().px(t.space(3)).py(t.space(1)).small(&t).text_color(t.colors.fg_subtle).child(i.t("tm.exited")));
                }
            }
        }
        col.child(views).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn deteksi_shell_tidak_panik_dan_id_unik() {
        let s = super::detect_shells();
        let mut ids: Vec<_> = s.iter().map(|x| x.id.clone()).collect();
        ids.dedup();
        assert_eq!(ids.len(), s.len());
    }
}

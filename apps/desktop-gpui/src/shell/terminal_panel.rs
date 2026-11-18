//! Panel 4 — Terminal: port TerminalPanel.tsx. Bilah konteks (nama + folder kerja, pemilih shell, terminal baru), tab
//! terminal per konteks (project / sesi Chat / folder Chat) dengan penanda selesai dan tombol tutup, serta keadaan kosong.
//! Perbedaan jujur: PTY di sini lokal (portable-pty), bukan PTY engine — tab tidak pulih setelah aplikasi ditutup.
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
}

pub struct TerminalPanel {
    shells: Vec<ShellOption>,
    shell: Option<String>,
    tabs: Vec<Tab>,
    active: HashMap<String, usize>,
    next: usize,
    /// Konteks yang sudah dibuatkan terminal otomatis (perilaku lama: terminal pertama langsung terbuka).
    seeded: std::collections::HashSet<String>,
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
        Self { shells: vec![], shell: None, tabs: vec![], active: HashMap::new(), next: 1, seeded: Default::default() }
    }

    fn current(&self) -> Option<&ShellOption> {
        self.shell.as_ref().and_then(|id| self.shells.iter().find(|s| &s.id == id)).or(self.shells.first())
    }

    fn create(&mut self, key: String, cwd: String, cx: &mut Context<Self>) {
        let sh = self.current().cloned();
        let name = sh.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| "Shell".into());
        let n = self.tabs.iter().filter(|t| t.key == key && t.title.starts_with(&name)).count() + 1;
        let term = cx.new(|cx| Terminal::with_shell(Some(cwd), sh.map(|s| (s.path, s.args)), cx));
        cx.observe(&term, |_, _, cx| cx.notify()).detach();
        let id = self.next;
        self.next += 1;
        self.tabs.push(Tab { id, key: key.clone(), title: format!("{name} {n}"), term });
        self.active.insert(key, id);
        cx.notify();
    }

    fn close(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            let key = self.tabs.remove(pos).key;
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
        // terminal pertama dibuka otomatis untuk konteks baru (versi GPUI sebelumnya juga begitu)
        if self.seeded.insert(key.clone()) && !self.tabs.iter().any(|x| x.key == key) {
            self.create(key.clone(), cwd.clone(), cx);
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

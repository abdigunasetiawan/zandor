//! Asisten composer — port shell/composer-assist.tsx + ComposerSuggest.tsx: saran `/perintah` dan `@berkas`,
//! navigasi keyboard (↑↓ Enter/Tab Esc lewat `InputNav`), eksekusi perintah slash bawaan & engine, dan lampiran.
use super::ChatPanel;
use crate::core::attachments::{attachment_from_path, FileAttachment};
use crate::core::chat::message_text;
use crate::core::composer::*;
use crate::core::engine::EngineCommand;
use crate::core::projects::CHAT_KEY;
use crate::core::engine::SessionInfo;
use crate::core::types::Role;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{icon, IconSize, InputNav, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, ClipboardItem, Context, Focusable, FontWeight, SharedString};
use std::time::Duration;

#[derive(Clone)]
pub(super) enum SugPick {
    Cmd(String),
    File(String),
}

#[derive(Clone)]
pub(super) struct SugEntry {
    label: String,
    hint: Option<String>,
    desc: Option<String>,
    icon: &'static str,
    group: Option<String>,
    badge: Option<String>,
    pick: SugPick,
}

pub(super) struct Suggest {
    #[allow(dead_code)] // label aksesibilitas (aria-label di TS)
    title: String,
    pub entries: Vec<SugEntry>,
    pub status: Option<String>,
    token: ComposerToken,
}

/// State asisten di ChatPanel.
#[derive(Default)]
pub(super) struct AssistState {
    pub files: Vec<FileAttachment>,
    commands: Option<(String, Vec<EngineCommand>)>,
    commands_req: Option<String>,
    found: Option<(String, String, Result<Vec<String>, String>)>,
    find_req: Option<(String, String)>,
    pub active: usize,
    dismissed: Option<String>,
}

fn builtin_icon(name: &str) -> &'static str {
    match name {
        "btw" => "sparkles",
        "new" => "plus",
        "compact" | "redo" => "refresh",
        "undo" => "undo",
        "fork" => "git",
        "copy" => "copy",
        "mode" => "shield",
        _ => "stop",
    }
}

pub(super) fn note(cx: &mut App, title: String, tone: ToastTone) {
    Toasts::show(ToastInput { title: title.into(), description: None, tone, timeout_ms: None, action: None }, cx);
}

impl ChatPanel {
    fn root_dir(&self, cx: &App) -> Option<String> {
        self.current(cx).map(|c| c.2).filter(|d| !d.is_empty())
    }

    fn load_commands(&mut self, root: &str, cx: &mut Context<Self>) {
        if self.assist.commands.as_ref().is_some_and(|(r, _)| r == root) || self.assist.commands_req.as_deref() == Some(root) {
            return;
        }
        self.assist.commands_req = Some(root.to_string());
        let task = self.chat.read(cx).list_commands(root, cx);
        let r = root.to_string();
        cx.spawn(async move |this, cx| {
            let list = task.await;
            this.update(cx, |s, cx| {
                if s.assist.commands_req.as_deref() == Some(&r) {
                    s.assist.commands = Some((r, list));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Popup saran untuk token di kursor; memicu pemuatan perintah/pencarian berkas bila perlu.
    pub(super) fn suggest(&mut self, cx: &mut Context<Self>) -> Option<Suggest> {
        let (text, caret) = {
            let inp = self.input.read(cx);
            (inp.text().to_string(), inp.caret())
        };
        let token = active_token(&text, caret)?;
        let key = format!("{:?}:{}", token.kind, token.start);
        if self.assist.dismissed.as_deref() == Some(&key) {
            return None;
        }
        let i = cx.global::<I18n>().clone();
        let root = self.root_dir(cx);
        match token.kind {
            TokenKind::Slash => {
                if let Some(r) = &root {
                    self.load_commands(r, cx);
                }
                let builtin: Vec<SugEntry> = BUILTIN_COMMANDS
                    .iter()
                    .map(|n| SugEntry {
                        label: format!("/{n}"),
                        hint: BUILTIN_NEEDS_ARGS.contains(n).then(|| i.t(&format!("slash.{n}.args"))),
                        desc: Some(i.t(&format!("slash.{n}.desc"))),
                        icon: builtin_icon(n),
                        group: Some(i.t("slash.title")),
                        badge: None,
                        pick: SugPick::Cmd(n.to_string()),
                    })
                    .collect();
                let engine: Vec<SugEntry> = self
                    .assist
                    .commands
                    .as_ref()
                    .filter(|(r, _)| Some(r) == root.as_ref())
                    .map(|(_, l)| l.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|c| !is_builtin(&c.name))
                    .map(|c| SugEntry {
                        label: format!("/{}", c.name),
                        hint: (!c.hints.is_empty()).then(|| c.hints.join(" ")),
                        desc: c.description.clone(),
                        icon: "wrench",
                        group: Some(i.t("slash.engine")),
                        badge: c.source.as_deref().filter(|s| *s != "command").map(|s| i.t(&format!("slash.source.{s}"))),
                        pick: SugPick::Cmd(c.name.clone()),
                    })
                    .collect();
                let name = |e: &SugEntry| e.label.trim_start_matches('/').to_string();
                let mut entries = rank_by_query(&builtin, &token.query, name);
                entries.extend(rank_by_query(&engine, &token.query, name));
                Some(Suggest { title: i.t("slash.title"), entries, status: None, token })
            }
            TokenKind::Mention => {
                let title = i.t("mention.title");
                let Some(root) = root else {
                    return Some(Suggest { title, entries: vec![], status: Some(i.t("mention.noRoot")), token });
                };
                let q = token.query.clone();
                let hit = self.assist.found.as_ref().filter(|(r, fq, _)| *r == root && *fq == q).map(|(_, _, res)| res.clone());
                match hit {
                    None => {
                        let want = (root.clone(), q.clone());
                        if self.assist.find_req.as_ref() != Some(&want) {
                            self.assist.find_req = Some(want);
                            let (r2, q2) = (root.clone(), q.clone());
                            cx.spawn(async move |this, cx| {
                                cx.background_executor().timer(Duration::from_millis(120)).await;
                                let Ok(task) = this.update(cx, |s, cx| {
                                    (s.assist.find_req.as_ref() == Some(&(r2.clone(), q2.clone())))
                                        .then(|| s.chat.read(cx).find_files(&r2, &q2, cx))
                                }) else {
                                    return;
                                };
                                let Some(task) = task else { return };
                                let res = task.await;
                                this.update(cx, |s, cx| {
                                    s.assist.found = Some((r2, q2, res));
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        }
                        Some(Suggest { title, entries: vec![], status: Some(i.t("mention.loading")), token })
                    }
                    Some(Err(e)) => Some(Suggest { title, entries: vec![], status: Some(i.tp("mention.error", &[("error", &e)])), token }),
                    Some(Ok(paths)) => {
                        let entries: Vec<SugEntry> = paths
                            .iter()
                            .map(|p| {
                                let clean = p.replace('\\', "/").trim_end_matches('/').to_string();
                                let (dir, base) = match clean.rfind('/') {
                                    Some(c) => (Some(clean[..c].to_string()), clean[c + 1..].to_string()),
                                    None => (None, clean.clone()),
                                };
                                SugEntry { label: base, hint: dir, desc: None, icon: "file", group: None, badge: None, pick: SugPick::File(clean) }
                            })
                            .collect();
                        let status = entries.is_empty().then(|| i.t("mention.empty"));
                        Some(Suggest { title, entries, status, token })
                    }
                }
            }
        }
    }

    pub(super) fn pick(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(sug) = self.suggest(cx) else { return };
        let Some(e) = sug.entries.get(idx).cloned() else { return };
        let text = self.input.read(cx).text().to_string();
        let rep = match &e.pick {
            SugPick::Cmd(n) => format!("/{n}"),
            SugPick::File(p) => format!("@{p}"),
        };
        let (next, caret) = replace_token(&text, &sug.token, &rep);
        self.input.update(cx, |inp, cx| inp.set_text_at(next, caret, cx));
        if let (SugPick::File(p), Some(root)) = (&e.pick, self.root_dir(cx)) {
            let att = mention_attachment(&root, p);
            if !self.assist.files.iter().any(|f| f.url == att.url) {
                self.assist.files.push(att);
            }
        }
        self.assist.active = 0;
        cx.notify();
    }

    pub(super) fn on_nav(&mut self, nav: InputNav, cx: &mut Context<Self>) {
        let Some(sug) = self.suggest(cx) else { return };
        let n = sug.entries.len();
        match nav {
            InputNav::Escape => {
                self.assist.dismissed = Some(format!("{:?}:{}", sug.token.kind, sug.token.start));
            }
            InputNav::Up | InputNav::Down if n > 0 => {
                let cur = self.assist.active.min(n - 1);
                self.assist.active = if nav == InputNav::Down { (cur + 1) % n } else { (cur + n - 1) % n };
            }
            InputNav::Enter | InputNav::Tab if n > 0 => {
                let idx = self.assist.active.min(n - 1);
                self.pick(idx, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    /// Elemen popup saran (di atas kolom input).
    pub(super) fn render_suggest(&self, sug: &Suggest, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let active = if sug.entries.is_empty() { 0 } else { self.assist.active.min(sug.entries.len() - 1) };
        let mut list = div().id("csug-list").flex().flex_col().max_h(t.rem(18.)).overflow_y_scroll().p(t.space(1));
        let mut prev_group: Option<String> = None;
        for (n, e) in sug.entries.iter().enumerate() {
            if e.group.is_some() && e.group != prev_group {
                list = list.child(
                    div().px(t.space(2)).pt(t.space(2)).pb(t.space(1)).small(&t).font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg_subtle).child(e.group.clone().unwrap_or_default()),
                );
            }
            prev_group = e.group.clone();
            let cur = n == active;
            list = list.child(
                div()
                    .id(SharedString::from(format!("csug-{n}")))
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .px(t.space(2))
                    .py(t.space(1))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .cursor_pointer()
                    .when(cur, |d| d.bg(t.colors.active))
                    .on_mouse_move(cx.listener(move |s, _, _, cx| {
                        if s.assist.active != n {
                            s.assist.active = n;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |s, _, w, cx| {
                        s.pick(n, cx);
                        let fh = s.input.read(cx).focus_handle(cx);
                        fh.focus(w, cx);
                    }))
                    .child(div().flex_none().text_color(t.colors.fg_muted).child(icon(e.icon).size(IconSize::Sm)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .gap(t.space(2))
                                    .min_w_0()
                                    .child(div().flex_none().child(e.label.clone()))
                                    .children(e.hint.clone().map(|h| div().truncate().text_color(t.colors.fg_subtle).child(h))),
                            )
                            .children(e.desc.clone().map(|d| div().truncate().small(&t).text_color(t.colors.fg_muted).child(d))),
                    )
                    .children(e.badge.clone().map(|b| div().flex_none().small(&t).text_color(t.colors.fg_subtle).child(b))),
            );
        }
        div()
            .flex()
            .flex_col()
            .w_full()
            .border_1()
            .border_color(t.colors.border)
            .rounded(t.rem(rem::RADIUS_LG))
            .bg(t.colors.raised)
            .shadow(t.shadow_1())
            .overflow_hidden()
            .when(!sug.entries.is_empty(), |d| d.child(list))
            .when(sug.entries.is_empty(), |d| d.children(sug.status.clone().map(|s| div().px(t.space(3)).py(t.space(2)).small(&t).text_color(t.colors.fg_muted).child(s))))
            .child(div().px(t.space(3)).py(t.space(1)).border_t_1().border_color(t.colors.border_soft).small(&t).text_color(t.colors.fg_subtle).child(i.t("composer.suggestHint")))
            .into_any_element()
    }

    pub(super) fn attach(&mut self, folder: bool, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: !folder, directories: folder, multiple: true, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                this.update(cx, |s, cx| {
                    for p in paths {
                        let a = attachment_from_path(&p.to_string_lossy(), folder);
                        if !s.assist.files.iter().any(|f| f.url == a.url) {
                            s.assist.files.push(a);
                        }
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Berkas/folder yang diseret dari OS ke composer → lampiran (folder terdeteksi dari sistem berkas).
    pub(super) fn attach_dropped(&mut self, paths: &[std::path::PathBuf], cx: &mut Context<Self>) {
        for p in paths {
            let a = attachment_from_path(&p.to_string_lossy(), p.is_dir());
            if !self.assist.files.iter().any(|f| f.url == a.url) {
                self.assist.files.push(a);
            }
        }
        cx.notify();
    }

    fn clear_composer(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |i, cx| i.set_text("", cx));
        self.assist.files.clear();
    }

    fn adopt(&mut self, s: SessionInfo, cx: &mut Context<Self>) {
        let key = self.store.read(cx).selected().map(|(k, _, _)| k).unwrap_or_else(|| CHAT_KEY.to_string());
        self.store.update(cx, |st, cx| st.adopt_session(&key, s, cx));
    }

    /// Jalankan perintah slash. Selalu menangani (tidak dikirim sebagai pesan biasa).
    pub(super) fn run_slash(&mut self, ps: ParsedSlash, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        let name = ps.name.clone();
        let args = ps.args.clone();
        let warn = |cx: &mut Context<Self>, key: &str, extra: &[(&str, &str)]| {
            let mut v: Vec<(&str, &str)> = vec![("name", name.as_str())];
            v.extend_from_slice(extra);
            note(cx, i.tp(key, &v), ToastTone::Warning)
        };
        let cur = self.current(cx);
        let sid = cur.as_ref().map(|c| c.1.clone());
        let state = sid.as_ref().and_then(|id| self.chat.read(cx).sessions.get(id).cloned());
        let running = state.as_ref().is_some_and(|s| s.busy());
        let files = self.assist.files.clone();

        if is_builtin(&name) {
            if BUILTIN_NEEDS_ARGS.contains(&name.as_str()) && args.is_empty() {
                let a = i.t(&format!("slash.{name}.args"));
                return warn(cx, "slash.needsArgs", &[("args", &a)]);
            }
            match name.as_str() {
                "new" => {
                    self.clear_composer(cx);
                    let key = cur.as_ref().map(|c| c.0.clone()).unwrap_or_else(|| CHAT_KEY.to_string());
                    let task = self.store.update(cx, |s, cx| s.create_session(&key, None, true, cx));
                    cx.spawn(async move |_, cx| {
                        if let Err(e) = task.await {
                            cx.update(|cx| note(cx, e, ToastTone::Danger));
                        }
                    })
                    .detach();
                    return;
                }
                "mode" => {
                    if !PERMISSION_MODES.contains(&args.as_str()) {
                        return warn(cx, "slash.badMode", &[("mode", &args)]);
                    }
                    self.chat.update(cx, |c, cx| c.set_permission_mode(&args, cx));
                    self.clear_composer(cx);
                    let m = i.t(&format!("chat.mode.{args}"));
                    return note(cx, i.tp("slash.modeSet", &[("mode", &m)]), ToastTone::Success);
                }
                _ => {}
            }
            let Some(sid) = sid else { return warn(cx, "slash.needsSession", &[]) };
            match name.as_str() {
                "btw" => {
                    self.clear_composer(cx);
                    let task = { let s2 = sid.clone(); self.chat.read(cx).call(&sid, cx, move |api, dir| api.fork(dir, &s2, None)) };
                    let ok = i.t("slash.btwStarted");
                    cx.spawn(async move |this, cx| {
                        let r = task.await;
                        this.update(cx, |me, cx| match r {
                            Ok(branch) => {
                                let dir = if branch.directory.is_empty() { me.current(cx).map(|c| c.2).unwrap_or_default() } else { branch.directory.clone() };
                                let bid = branch.id.clone();
                                me.adopt(branch, cx);
                                me.chat.update(cx, |c, cx| {
                                    c.open(&dir, &bid, cx);
                                    c.send_with(&bid, args, files, cx);
                                });
                                note(cx, ok, ToastTone::Success);
                            }
                            Err(e) => note(cx, e, ToastTone::Danger),
                        })
                        .ok();
                    })
                    .detach();
                }
                "stop" => {
                    self.clear_composer(cx);
                    self.chat.update(cx, |c, cx| c.abort(&sid, cx));
                }
                "fork" => {
                    self.clear_composer(cx);
                    let task = { let s2 = sid.clone(); self.chat.read(cx).call(&sid, cx, move |api, dir| api.fork(dir, &s2, None)) };
                    cx.spawn(async move |this, cx| {
                        let r = task.await;
                        this.update(cx, |me, cx| match r {
                            Ok(b) => me.adopt(b, cx),
                            Err(e) => note(cx, e, ToastTone::Danger),
                        })
                        .ok();
                    })
                    .detach();
                }
                "copy" => {
                    let ans = state
                        .as_ref()
                        .and_then(|s| s.messages.iter().rev().find(|m| m.info.role == Role::Assistant))
                        .map(message_text)
                        .unwrap_or_default();
                    if ans.is_empty() {
                        return warn(cx, "slash.nothing", &[]);
                    }
                    cx.write_to_clipboard(ClipboardItem::new_string(ans));
                    self.clear_composer(cx);
                    note(cx, i.t("slash.copied"), ToastTone::Success);
                }
                _ if running => warn(cx, "slash.busy", &[]),
                "compact" => {
                    if state.as_ref().is_none_or(|s| s.messages.is_empty()) {
                        return warn(cx, "slash.nothing", &[]);
                    }
                    self.clear_composer(cx);
                    self.chat.update(cx, |c, cx| c.compact(&sid, cx));
                    note(cx, i.t("slash.compacted"), ToastTone::Success);
                }
                "undo" => {
                    let Some(st) = state else { return warn(cx, "slash.nothing", &[]) };
                    let last = st
                        .messages
                        .iter()
                        .filter(|m| st.revert.as_ref().is_none_or(|cut| m.info.id < *cut))
                        .rev()
                        .find(|m| m.info.role == Role::User)
                        .cloned();
                    let Some(last) = last else { return warn(cx, "slash.nothing", &[]) };
                    self.chat.update(cx, |c, cx| c.revert_to(&sid, &last.info.id, cx));
                    let prev = message_text(&last);
                    let n = prev.len();
                    self.assist.files.clear();
                    self.input.update(cx, |inp, cx| inp.set_text_at(prev, n, cx));
                }
                _ => {
                    // redo
                    if state.as_ref().is_none_or(|s| s.revert.is_none()) {
                        return warn(cx, "slash.nothing", &[]);
                    }
                    self.clear_composer(cx);
                    self.chat.update(cx, |c, cx| c.unrevert(&sid, cx));
                }
            }
            cx.notify();
            return;
        }

        // perintah engine
        let Some(root) = self.root_dir(cx) else { return warn(cx, "slash.unknown", &[]) };
        if running {
            return warn(cx, "slash.busy", &[]);
        }
        let list_task = self.chat.read(cx).list_commands(&root, cx);
        let unknown = i.tp("slash.unknown", &[("name", &name)]);
        cx.spawn(async move |this, cx| {
            let list = list_task.await;
            let _ = this.update(cx, |me, cx| {
                me.assist.commands = Some((root.clone(), list.clone()));
                if !list.iter().any(|c| c.name == name) {
                    return note(cx, unknown, ToastTone::Warning);
                }
                me.clear_composer(cx);
                match sid {
                    Some(id) => me.chat.update(cx, |c, cx| c.run_command(&id, name, args, files, cx)),
                    None => {
                        let task = me.store.update(cx, |s, cx| s.create_session(CHAT_KEY, None, true, cx));
                        cx.spawn(async move |this, cx| match task.await {
                            Ok(s) => {
                                this.update(cx, |me, cx| {
                                    me.sync(cx);
                                    me.chat.update(cx, |c, cx| c.run_command(&s.id, name, args, files, cx));
                                })
                                .ok();
                            }
                            Err(e) => {
                                cx.update(|cx| note(cx, e, ToastTone::Danger));
                            }
                        })
                        .detach();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}


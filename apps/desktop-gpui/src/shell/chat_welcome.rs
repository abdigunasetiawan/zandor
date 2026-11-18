//! Halaman sesi baru Panel 2 — port DraftChat + Greeting + SessionScope + saran prompt (ChatPanel.tsx): sapaan menurut
//! bagian hari, pil lingkup [Chat/project ▾ · meta redup / cabang git ▾], composer di tengah, chip saran di bawahnya.
use super::ChatPanel;
use crate::core::projects::CHAT_KEY;
use crate::core::suggestions::{suggest_prompts, SuggestionContext};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{entry, icon, menu_popup, IconSize, TypeExt};
use gpui::{div, prelude::*, AnyElement, Anchor, App, Context, SharedString};

/// Konteks halaman sambutan per folder project: saran + status git (dibaca di thread latar).
#[derive(Clone, Debug, Default)]
pub struct WelcomeCtx {
    pub ctx: SuggestionContext,
    pub is_repo: bool,
    pub branch: Option<String>,
    pub detached: bool,
    pub branches: Vec<(String, bool)>,
}

/// Port dayPart (use-os-user.ts).
pub fn day_part(hour: u32) -> &'static str {
    match hour {
        4..=10 => "morning",
        11..=14 => "afternoon",
        15..=18 => "evening",
        _ => "night",
    }
}

/// Sapaan: bagian hari + nama profil.
pub(super) fn greeting(t: &Theme, i: &I18n, engine_ready: bool, cx: &App) -> AnyElement {
    use chrono::Timelike;
    let part = day_part(chrono::Local::now().hour());
    let name = crate::core::profile::Profile::global(cx).read(cx).display_name();
    let name = if name.trim().is_empty() { String::new() } else { format!(", {name}") };
    div()
        .flex()
        .flex_col()
        .items_center()
        .text_center()
        .child(div().typo(t, crate::theme::H2).child(i.tp(&format!("chat.greet.{part}"), &[("name", &name)])))
        .child(div().text_color(t.colors.fg_muted).child(i.t(if engine_ready { "chat.greet.sub" } else { "chat.engineDown" })))
        .into_any_element()
}

fn load_ctx(root: &str) -> WelcomeCtx {
    let path = std::path::Path::new(root);
    let root_files = std::fs::read_dir(path).ok().map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect::<Vec<_>>());
    let st = crate::git::status(path).ok();
    let is_repo = st.as_ref().is_some_and(|s| s.is_repo);
    let changed: Vec<String> = st.as_ref().map(|s| s.files.iter().map(|f| f.path.clone()).collect()).unwrap_or_default();
    let staged = st.as_ref().map(|s| s.files.iter().filter(|f| f.index != ' ' && f.index != '?').count()).unwrap_or(0);
    let branches = if is_repo { crate::git::branches(path).unwrap_or_default().into_iter().map(|b| (b.name, b.current)).collect() } else { vec![] };
    WelcomeCtx {
        ctx: SuggestionContext { has_project: true, root_files, changed, staged, active_doc: None },
        is_repo,
        branch: st.as_ref().and_then(|s| s.branch.clone()),
        detached: st.as_ref().is_some_and(|s| s.detached),
        branches,
    }
}

impl ChatPanel {
    /// Path project terpilih (None = Chat tanpa project).
    fn welcome_root(&self, cx: &App) -> Option<(String, String)> {
        let s = self.store.read(cx);
        let key = s.selection.key.clone().unwrap_or_else(|| CHAT_KEY.to_string());
        if key == CHAT_KEY {
            return None;
        }
        s.projects.iter().find(|p| p.id == key).map(|p| (p.name.clone(), p.path.clone()))
    }

    /// Konteks project (dimuat sekali per folder; dimuat ulang saat folder berganti).
    fn welcome_ctx(&mut self, cx: &mut Context<Self>) -> Option<WelcomeCtx> {
        let (_, root) = self.welcome_root(cx)?;
        if let Some((r, c)) = &self.sugg_ctx {
            if *r == root {
                return Some(c.clone());
            }
        }
        if self.sugg_loading.as_deref() != Some(root.as_str()) {
            self.sugg_loading = Some(root.clone());
            let r2 = root.clone();
            let task = cx.background_spawn(async move { load_ctx(&r2) });
            cx.spawn(async move |this, cx| {
                let c = task.await;
                this.update(cx, |s, cx| {
                    s.sugg_ctx = Some((root, c));
                    s.sugg_loading = None;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        None
    }

    /// Pil lingkup sesi baru (port SessionScope).
    pub(super) fn session_scope(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let project = self.welcome_root(cx);
        let wctx = self.welcome_ctx(cx);
        let me = cx.entity().downgrade();
        let projects: Vec<(String, String)> = self.store.read(cx).projects.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
        let sel_key = self.store.read(cx).selection.key.clone().unwrap_or_else(|| CHAT_KEY.into());
        let pick = |key: String| {
            let store = self.store.clone();
            move |_: &mut gpui::Window, cx: &mut App| store.update(cx, |s, cx| s.select(Some(key.clone()), None, cx))
        };
        let mut folder_entries = vec![entry(i.t("scope.chat"), pick(CHAT_KEY.into())).checked(project.is_none())];
        for (id, name) in &projects {
            folder_entries.push(entry(name.clone(), pick(id.clone())).checked(*id == sel_key));
        }
        let pill_btn = |id: &'static str, which: u8| {
            let me = me.clone();
            div()
                .id(id)
                .relative()
                .flex()
                .items_center()
                .gap(t.space(1))
                .min_w_0()
                .h(t.rem(rem::CONTROL_SM))
                .px(t.space(2))
                .rounded_full()
                .cursor_pointer()
                .hover(|s| s.bg(t.colors.hover))
                .on_click(move |_, _, cx| {
                    me.update(cx, |s, cx| {
                        s.scope_menu = if s.scope_menu == Some(which) { None } else { Some(which) };
                        cx.notify();
                    })
                    .ok();
                })
        };
        let close = {
            let me = me.clone();
            move |_: &mut gpui::Window, cx: &mut App| {
                me.update(cx, |s, cx| {
                    s.scope_menu = None;
                    cx.notify();
                })
                .ok();
            }
        };
        let folder = pill_btn("scope-folder", 1)
            .tooltip(crate::ui::tooltip::text_tooltip(project.as_ref().map(|p| p.1.clone()).unwrap_or_else(|| i.t("scope.chatHint")).into()))
            .child(icon(if project.is_some() { "folder" } else { "chat" }).size(IconSize::Sm))
            .child(div().min_w_0().truncate().text_color(t.colors.fg).child(project.as_ref().map(|p| p.0.clone()).unwrap_or_else(|| i.t("scope.chat"))))
            .child(icon("chevron_down").size(IconSize::Sm))
            .when(self.scope_menu == Some(1), |d| d.child(menu_popup("m-scope-folder", t, folder_entries, Anchor::TopLeft, close.clone())));
        let meta = |_: String| div().flex().items_center().gap(t.space(1)).min_w_0().truncate().text_color(t.colors.fg_subtle).font_family(t.font_code.clone());
        let mut pill = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .max_w_full()
            .p(t.space(1))
            .border_1()
            .border_color(t.colors.border_soft)
            .rounded_full()
            .text_color(t.colors.fg_muted)
            .small(t)
            .child(folder);
        match (&project, &wctx) {
            (None, _) => pill = pill.child(meta(String::new()).pr(t.space(2)).child(i.t("scope.chatHint"))),
            (Some(_), None) => pill = pill.child(meta(String::new()).pr(t.space(2)).child(icon("git").size(IconSize::Sm)).child(i.t("scope.gitLoading"))),
            (Some((_, root)), Some(w)) => {
                let text = if !w.is_repo {
                    i.t("scope.noGit")
                } else if w.detached {
                    i.t("scope.detached")
                } else {
                    w.branch.clone().unwrap_or_else(|| i.t("scope.detached"))
                };
                if w.is_repo && !w.branches.is_empty() {
                    let entries = w
                        .branches
                        .iter()
                        .map(|(name, current)| {
                            let (root, name2, cur, me) = (root.clone(), name.clone(), *current, me.clone());
                            entry(name.clone(), move |_, cx| {
                                if cur {
                                    return;
                                }
                                if let Err(e) = crate::git::switch(std::path::Path::new(&root), &name2) {
                                    let msg = cx.global::<I18n>().tp("sb.error", &[("error", &e)]);
                                    crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(msg) }, cx);
                                }
                                me.update(cx, |s, cx| {
                                    s.sugg_ctx = None;
                                    cx.notify();
                                })
                                .ok();
                            })
                            .checked(*current)
                        })
                        .collect();
                    pill = pill.child(
                        pill_btn("scope-branch", 2)
                            .child(icon("git").size(IconSize::Sm))
                            .child(meta(String::new()).child(text))
                            .child(icon("chevron_down").size(IconSize::Sm))
                            .when(self.scope_menu == Some(2), |d| d.child(menu_popup("m-scope-branch", t, entries, Anchor::TopLeft, close.clone()))),
                    );
                } else {
                    pill = pill.child(meta(String::new()).pr(t.space(2)).child(icon("git").size(IconSize::Sm)).child(text));
                }
            }
        }
        let _ = SharedString::default();
        pill.into_any_element()
    }

    /// Chip saran di bawah composer (port `.suggest`): disembunyikan tanpa melepas ruang saat mengetik.
    pub(super) fn suggestion_chips(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ctx = match self.welcome_root(cx) {
            None => SuggestionContext::default(),
            Some(_) => self.welcome_ctx(cx)?.ctx,
        };
        let list = suggest_prompts(&ctx, 4);
        if list.is_empty() {
            return None;
        }
        let hidden = !self.input.read(cx).text().is_empty();
        let mut row = div().flex().flex_wrap().justify_center().gap(t.space(1)).when(hidden, |d| d.invisible());
        for s in list {
            let params: Vec<(&str, &str)> = s.params.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let label = i.tp(&format!("sugg.{}.label", s.id), &params);
            let prompt = i.tp(&format!("sugg.{}.prompt", s.id), &params).replace("\\n", "\n");
            let p2 = prompt.clone();
            row = row.child(
                div()
                    .id(SharedString::from(format!("sugg-{}", s.id)))
                    .flex()
                    .items_center()
                    .h(t.rem(rem::CONTROL_SM))
                    .px(t.space(3))
                    .border_1()
                    .border_color(t.colors.border)
                    .rounded_full()
                    .small(t)
                    .text_color(t.colors.fg)
                    .cursor_pointer()
                    .hover(|d| d.bg(t.colors.hover))
                    .tooltip(crate::ui::tooltip::text_tooltip(prompt.into()))
                    .child(label)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.input.update(cx, |inp, cx| inp.set_text(&p2, cx));
                        let fh = gpui::Focusable::focus_handle(s.input.read(cx), cx);
                        fh.focus(w, cx);
                        cx.notify();
                    })),
            );
        }
        Some(row.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bagian_hari() {
        assert_eq!(super::day_part(3), "night");
        assert_eq!(super::day_part(4), "morning");
        assert_eq!(super::day_part(11), "afternoon");
        assert_eq!(super::day_part(18), "evening");
        assert_eq!(super::day_part(19), "night");
    }
}

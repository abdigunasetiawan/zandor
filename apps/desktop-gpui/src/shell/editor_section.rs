//! Tab "Editor" di Tampilan — port EditorTab (AppearancePage.tsx): pilih tema editor (otomatis, bawaan Monaco, atau
//! terpasang), impor tema VS Code (.json/.vsix), daftar tema terpasang dengan tombol hapus.
//! Pencarian Open VSX belum diport (butuh unduhan jaringan); impor berkas lokal tersedia.
use crate::core::appearance::AppearanceStore;
use crate::core::editor_theme::{convert_vscode_theme, themes_from_vsix, BUILTIN_EDITOR_THEMES, MAX_THEME_BYTES};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, select, ButtonVariant, ControlSize, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, Context, Entity, IntoElement, SharedString, Window};

pub struct EditorSection {
    store: Entity<AppearanceStore>,
    errors: Vec<String>,
}

impl EditorSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = AppearanceStore::global(cx);
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        Self { store, errors: vec![] }
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(p) = paths.into_iter().next() else { return };
            this.update(cx, |s, cx| {
                let i = cx.global::<I18n>().clone();
                s.errors.clear();
                let vsix = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("vsix"));
                let (themes, errs) = if vsix {
                    match std::fs::read(&p) {
                        Ok(b) => themes_from_vsix(&b, "berkas"),
                        Err(e) => (vec![], vec![e.to_string()]),
                    }
                } else if std::fs::metadata(&p).map(|m| m.len() > MAX_THEME_BYTES).unwrap_or(false) {
                    (vec![], vec![i.t("ap.import.tooBig")])
                } else {
                    let text = std::fs::read_to_string(&p).unwrap_or_default();
                    let dir = p.parent().map(|d| d.to_path_buf());
                    let resolve = move |inc: &str| dir.as_ref().and_then(|d| std::fs::read_to_string(d.join(inc)).ok());
                    let (t, e) = convert_vscode_theme(&text, None, None, "berkas", &resolve);
                    (t.into_iter().collect(), e)
                };
                s.errors = errs;
                let mut last = None;
                for t in themes {
                    let (id, name) = (t.id.clone(), t.name.clone());
                    match s.store.update(cx, |st, cx| st.install_editor_theme(t, cx)) {
                        Ok(()) => {
                            Toasts::show(ToastInput { title: i.tp("ap.editor2.imported", &[("name", &name)]).into(), description: None, tone: ToastTone::Success, timeout_ms: None, action: None }, cx);
                            last = Some(id);
                        }
                        Err(e) => s.errors.push(e),
                    }
                }
                if let Some(id) = last {
                    s.store.update(cx, |st, cx| st.set_editor_theme(&id, cx));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for EditorSection {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let (cur, installed) = {
            let s = self.store.read(cx);
            (s.editor_theme.clone(), s.editor_themes.iter().map(|t| (t.id.clone(), t.name.clone(), t.source.clone())).collect::<Vec<_>>())
        };
        let mut items: Vec<(SharedString, SharedString)> = vec![("auto".into(), i.t("ap.editor2.auto").into())];
        items.extend(BUILTIN_EDITOR_THEMES.iter().map(|b| (SharedString::from(*b), SharedString::from(i.tp("ap.editor2.builtin", &[("name", b)])))));
        items.extend(installed.iter().map(|(id, n, _)| (SharedString::from(id.clone()), SharedString::from(n.clone()))));
        let store = self.store.clone();
        let mut list = div().flex().flex_col().gap(t.space(2));
        if installed.is_empty() {
            list = list.child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.editor2.none")));
        }
        for (id, name, src) in installed {
            let st = self.store.clone();
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .p(t.space(3))
                    .rounded(t.rem(rem::RADIUS_LG))
                    .border_1()
                    .border_color(if id == cur { t.colors.focus } else { t.colors.border_soft })
                    .bg(t.colors.raised)
                    .child(div().flex_1().min_w_0().child(div().truncate().child(name)).child(div().small(&t).text_color(t.colors.fg_subtle).child(src)))
                    .child(
                        button(SharedString::from(format!("ed-rm-{id}")))
                            .label(i.t("ap.editor2.remove"))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .on_click(move |_, _, cx| st.update(cx, |s, cx| s.remove_editor_theme(&id, cx))),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.editor2.intro")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().flex_1().child(i.t("ap.editor2.select")))
                    .child(select("ed-theme", items, cur).on_change(move |v, _, cx| store.update(cx, |s, cx| s.set_editor_theme(&v, cx)))),
            )
            .child(div().child(button("ed-import").label(i.t("ap.editor2.importFile")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| s.import(cx)))))
            .children(self.errors.iter().map(|e| div().small(&t).text_color(t.colors.danger).child(e.clone())))
            .child(div().typo(&t, crate::theme::H4).child(i.t("ap.editor2.installed")))
            .child(list)
            .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("ap.editor2.unavailable")))
    }
}

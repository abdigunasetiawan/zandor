//! Tab "Editor" di Tampilan — port EditorTab (AppearancePage.tsx): pilih tema editor (otomatis, bawaan Monaco, atau
//! terpasang), impor tema VS Code (.json/.vsix), daftar tema terpasang dengan tombol hapus.
//! Pencarian Open VSX: klien bersama `openvsx.rs` (hanya https://open-vsx.org, ukuran dibatasi), unduh .vsix lalu ekstrak tema.
use crate::core::appearance::AppearanceStore;
use crate::core::editor_theme::{convert_vscode_theme, themes_from_vsix, BUILTIN_EDITOR_THEMES, MAX_THEME_BYTES};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, select, ButtonVariant, ControlSize, ToastInput, ToastTone, Toasts, TypeExt};
use crate::openvsx::OvsxItem;
use crate::ui::{InputSubmit, TextInput};
use gpui::{div, prelude::*, Context, Entity, IntoElement, SharedString, Window};

pub struct EditorSection {
    store: Entity<AppearanceStore>,
    errors: Vec<String>,
    query: Entity<TextInput>,
    results: Option<Result<Vec<OvsxItem>, String>>,
    searching: bool,
    busy: Option<String>,
}

impl EditorSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = AppearanceStore::global(cx);
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        let ph = cx.global::<I18n>().t("ap.editor2.search");
        let query = cx.new(|cx| {
            let mut ti = TextInput::new(cx, ph);
            ti.size = ControlSize::Sm;
            ti
        });
        cx.subscribe(&query, |s, _, _: &InputSubmit, cx| s.search(cx)).detach();
        Self { store, errors: vec![], query, results: None, searching: false, busy: None }
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        let q = self.query.read(cx).text().to_string();
        self.searching = true;
        cx.notify();
        let task = cx.background_spawn(async move { crate::openvsx::search_themes(&q) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.searching = false;
                s.results = Some(r);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn install(&mut self, item: OvsxItem, cx: &mut Context<Self>) {
        self.busy = Some(item.download.clone());
        cx.notify();
        let url = item.download.clone();
        let task = cx.background_spawn(async move { crate::openvsx::download_vsix(&url) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.busy = None;
                match r {
                    Ok(bytes) => {
                        let src = format!("openvsx:{}.{}@{}", item.namespace, item.name, item.version);
                        let (themes, errs) = themes_from_vsix(&bytes, &src);
                        s.add(themes, errs, cx);
                    }
                    Err(e) => s.errors = vec![e],
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn add(&mut self, themes: Vec<crate::core::editor_theme::EditorTheme>, errs: Vec<String>, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        self.errors = errs;
        let mut last = None;
        for t in themes {
            let (id, name) = (t.id.clone(), t.name.clone());
            match self.store.update(cx, |st, cx| st.install_editor_theme(t, cx)) {
                Ok(()) => {
                    Toasts::show(ToastInput { title: i.tp("ap.editor2.imported", &[("name", &name)]).into(), description: None, tone: ToastTone::Success, timeout_ms: None, action: None }, cx);
                    last = Some(id);
                }
                Err(e) => self.errors.push(e),
            }
        }
        if let Some(id) = last {
            self.store.update(cx, |st, cx| st.set_editor_theme(&id, cx));
        }
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
                s.add(themes, errs, cx);
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
            .child(div().typo(&t, crate::theme::H4).child(i.t("ap.editor2.search")))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.editor2.license")))
            .child(
                div()
                    .flex()
                    .gap(t.space(2))
                    .child(div().flex_1().child(self.query.clone()))
                    .child(button("ovsx-search").label(i.t("ap.editor2.searchBtn")).size(ControlSize::Sm).disabled(self.searching).on_click(cx.listener(|s, _, _, cx| s.search(cx)))),
            )
            .children(self.render_results(&t, &i, cx))
    }
}

impl EditorSection {
    fn render_results(&self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let r = self.results.as_ref()?;
        let items = match r {
            Err(e) => return Some(div().small(t).text_color(t.colors.danger).child(e.clone()).into_any_element()),
            Ok(v) => v.clone(),
        };
        let mut col = div().flex().flex_col().gap(t.space(2));
        if items.is_empty() {
            col = col.child(div().small(t).text_color(t.colors.fg_muted).child(i.t("ap.editor2.noResults")));
        }
        for it in items {
            let busy = self.busy.as_deref() == Some(it.download.as_str());
            let it2 = it.clone();
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .p(t.space(3))
                    .rounded(t.rem(rem::RADIUS_LG))
                    .border_1()
                    .border_color(t.colors.border_soft)
                    .bg(t.colors.raised)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(it.display_name.clone()))
                            .child(div().small(t).text_color(t.colors.fg_muted).child(format!("{}.{}@{}", it.namespace, it.name, it.version))),
                    )
                    .child(div().small(t).text_color(t.colors.fg_muted).child(i.tp("ap.editor2.downloads", &[("n", &it.downloads.to_string())])))
                    .child(
                        button(SharedString::from(format!("ovsx-{}", it.download)))
                            .label(i.t("ap.editor2.install"))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Solid)
                            .disabled(busy || self.busy.is_some())
                            .on_click(cx.listener(move |s, _, _, cx| s.install(it2.clone(), cx))),
                    ),
            );
        }
        Some(col.into_any_element())
    }
}

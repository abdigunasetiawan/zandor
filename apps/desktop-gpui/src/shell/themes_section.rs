//! Tab "Tema" di Tampilan — port ThemesTab + ThemeEditor (AppearancePage.tsx): kartu preset/kustom dengan swatch,
//! pakai, ekspor, impor (JSON), buat/ubah tema (warna per token gelap/terang, kontras WCAG), hapus.
use crate::core::appearance::{contrast_ratio, is_color, parse_color, AppearanceStore, ThemeDef, COLOR_KEYS, SWATCH_KEYS};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{badge, button, select, BadgeTone, ButtonVariant, ControlSize, TextInput, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, px, Context, Entity, FontWeight, IntoElement, SharedString, Window};

struct Editor {
    existing: Option<String>,
    name: Entity<TextInput>,
    from: String,
    dark: Vec<Entity<TextInput>>,
    light: Vec<Entity<TextInput>>,
    errors: Vec<String>,
}

pub struct ThemesSection {
    store: Entity<AppearanceStore>,
    editor: Option<Editor>,
    import_errors: Vec<String>,
    confirm_reset: bool,
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for ch in s.to_lowercase().chars() {
        let c = match ch {
            'à'..='å' => 'a',
            'è'..='ë' => 'e',
            'ì'..='ï' => 'i',
            'ò'..='ö' => 'o',
            'ù'..='ü' => 'u',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(40).collect::<String>().trim_end_matches('-').to_string()
}

impl ThemesSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = AppearanceStore::global(cx);
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        Self { store, editor: None, import_errors: vec![], confirm_reset: false }
    }

    fn open_editor(&mut self, existing: Option<ThemeDef>, cx: &mut Context<Self>) {
        let ph = cx.global::<I18n>().t("ap.editor.default");
        let mk = |cx: &mut Context<Self>, v: String, ph: &str| {
            let e = cx.new(|cx| {
                let mut ti = TextInput::new(cx, ph.to_string());
                ti.size = ControlSize::Sm;
                ti.set_text(v, cx);
                ti
            });
            cx.observe(&e, |_, _, cx| cx.notify()).detach();
            let is_col = !ph.is_empty(); // kolom nama memakai placeholder kosong
            cx.subscribe(&e, move |_, ent, ev: &crate::ui::InputChanged, cx| {
                if !is_col {
                    return;
                }
                let v = ev.0.trim().to_string();
                ent.update(cx, |ti, cx| {
                    ti.invalid = !v.is_empty() && !is_color(&v);
                    cx.notify();
                });
            })
            .detach();
            e
        };
        let name = mk(cx, existing.as_ref().map(|t| t.name.clone()).unwrap_or_default(), "");
        let col = |cx: &mut Context<Self>, dark: bool| -> Vec<Entity<TextInput>> {
            COLOR_KEYS
                .iter()
                .map(|k| mk(cx, existing.as_ref().and_then(|t| t.get(dark, k)).unwrap_or("").to_string(), &ph))
                .collect()
        };
        let (dark, light) = (col(cx, true), col(cx, false));
        self.editor = Some(Editor { existing: existing.map(|t| t.id), name, from: String::new(), dark, light, errors: vec![] });
        cx.notify();
    }

    fn copy_from(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(src) = self.store.read(cx).find(id) else { return };
        let Some(ed) = self.editor.as_mut() else { return };
        ed.from = id.to_string();
        let (dark, light) = (ed.dark.clone(), ed.light.clone());
        for (n, k) in COLOR_KEYS.iter().enumerate() {
            let (d, l) = (src.get(true, k).unwrap_or("").to_string(), src.get(false, k).unwrap_or("").to_string());
            dark[n].update(cx, |ti, cx| ti.set_text(d, cx));
            light[n].update(cx, |ti, cx| ti.set_text(l, cx));
        }
        cx.notify();
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.as_ref() else { return };
        let name = ed.name.read(cx).text().to_string();
        let id = ed.existing.clone().unwrap_or_else(|| slug(&name));
        let read = |v: &Vec<Entity<TextInput>>| -> Vec<(String, String)> {
            COLOR_KEYS
                .iter()
                .zip(v)
                .filter_map(|(k, e)| {
                    let s = e.read(cx).text().trim().to_string();
                    (!s.is_empty()).then(|| (k.to_string(), s))
                })
                .collect()
        };
        let theme = ThemeDef { id: id.clone(), name, dark: Some(read(&ed.dark)), light: Some(read(&ed.light)), builtin: false };
        match self.store.update(cx, |s, cx| s.save_custom(theme, cx)) {
            Ok(()) => {
                self.store.update(cx, |s, cx| s.set_preset(&id, cx));
                self.editor = None;
            }
            Err(e) => {
                if let Some(ed) = self.editor.as_mut() {
                    ed.errors = e;
                }
            }
        }
        cx.notify();
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(p) = paths.into_iter().next() else { return };
            this.update(cx, |s, cx| {
                s.import_errors.clear();
                let i = cx.global::<I18n>().clone();
                if std::fs::metadata(&p).map(|m| m.len() > 100_000).unwrap_or(false) {
                    s.import_errors = vec![i.t("ap.import.tooBig")];
                } else {
                    let text = std::fs::read_to_string(&p).unwrap_or_default();
                    match s.store.update(cx, |st, cx| st.import_theme(&text, cx)) {
                        Ok(id) => {
                            Toasts::show(ToastInput { title: i.tp("ap.import.ok", &[("id", &id)]).into(), description: None, tone: ToastTone::Success, timeout_ms: None, action: None }, cx);
                        }
                        Err(e) => s.import_errors = e,
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn export(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(text) = self.store.read(cx).export_theme(&id) else { return };
        let dir = dirs_home();
        let rx = cx.prompt_for_new_path(&dir, Some(&format!("zandor-tema-{id}.json")));
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                if let Err(e) = std::fs::write(&path, text) {
                    let msg = e.to_string();
                    cx.update(|cx| Toasts::show(ToastInput { title: msg.into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None }, cx));
                }
            }
        })
        .detach();
    }
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(Into::into).unwrap_or_else(|| ".".into())
}

impl Render for ThemesSection {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let dark = t.dark;
        let store = self.store.read(cx);
        let (active, themes) = (store.preset.clone(), store.themes());

        let actions = div()
            .flex()
            .gap(t.space(2))
            .child(button("ap-import").label(i.t("ap.import")).on_click(cx.listener(|s, _, _, cx| s.import(cx))))
            .child(button("ap-new").label(i.t("ap.new")).on_click(cx.listener(|s, _, _, cx| s.open_editor(None, cx))))
            .child(div().flex_1())
            .child(button("ap-reset").label(i.t("ap.reset")).variant(ButtonVariant::Danger).on_click(cx.listener(|s, _, _, cx| {
                s.confirm_reset = true;
                cx.notify();
            })));
        // konfirmasi reset (useConfirm di TS) ditampilkan sebaris
        let confirm = self.confirm_reset.then(|| {
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .rounded(t.rem(rem::RADIUS_LG))
                .border_1()
                .border_color(t.colors.danger)
                .bg(t.colors.raised)
                .child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("ap.resetTitle")))
                .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.resetBody")))
                .child(
                    div()
                        .flex()
                        .gap(t.space(2))
                        .child(button("ap-reset-ok").label(i.t("ap.reset")).variant(ButtonVariant::Danger).on_click(cx.listener(|s, _, _, cx| {
                            s.confirm_reset = false;
                            s.store.update(cx, |st, cx| st.reset_all(cx));
                        })))
                        .child(button("ap-reset-no").label(i.t("ap.editor.cancel")).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| {
                            s.confirm_reset = false;
                            cx.notify();
                        }))),
                )
        });

        let mut grid = div().flex().flex_wrap().gap(t.space(3));
        for th in &themes {
            let on = th.id == active;
            let mut sw = div().flex().h(t.rem(1.5)).rounded(t.rem(rem::RADIUS_MD)).overflow_hidden().border_1().border_color(t.colors.border);
            for k in SWATCH_KEYS {
                let col = th.get(dark, k).and_then(parse_color).unwrap_or(match k {
                    "bg" => t.colors.bg,
                    "raised" => t.colors.raised,
                    "fg" => t.colors.fg,
                    "accent" => t.colors.accent,
                    _ => t.colors.border_strong,
                });
                sw = sw.child(div().flex_1().h_full().bg(col));
            }
            let (id1, id2, id3, id4) = (th.id.clone(), th.id.clone(), th.clone(), th.id.clone());
            let mut btns = div()
                .flex()
                .flex_wrap()
                .gap(t.space(1))
                .child(
                    button(SharedString::from(format!("ap-use-{}", th.id)))
                        .label(i.t("ap.use"))
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Solid)
                        .disabled(on)
                        .on_click(cx.listener(move |s, _, _, cx| s.store.update(cx, |st, cx| st.set_preset(&id1, cx)))),
                )
                .child(
                    button(SharedString::from(format!("ap-exp-{}", th.id)))
                        .label(i.t("ap.export"))
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(move |s, _, _, cx| s.export(id2.clone(), cx))),
                );
            if !th.builtin {
                btns = btns
                    .child(
                        button(SharedString::from(format!("ap-edit-{}", th.id)))
                            .label(i.t("ap.edit"))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |s, _, _, cx| s.open_editor(Some(id3.clone()), cx))),
                    )
                    .child(
                        button(SharedString::from(format!("ap-del-{}", th.id)))
                            .label(i.t("ap.delete"))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Danger)
                            .on_click(cx.listener(move |s, _, _, cx| s.store.update(cx, |st, cx| st.remove_custom(&id4, cx)))),
                    );
            }
            grid = grid.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(2))
                    .w(t.rem(15.))
                    .p(t.space(3))
                    .rounded(t.rem(rem::RADIUS_LG))
                    .border_1()
                    .border_color(if on { t.colors.focus } else { t.colors.border_soft })
                    .bg(t.colors.raised)
                    .child(sw)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(1))
                            .child(div().flex_1().truncate().font_weight(FontWeight::MEDIUM).child(th.name.clone()))
                            .when(on, |d| d.child(badge(i.t("ap.active")).tone(BadgeTone::Success)))
                            .when(!th.builtin, |d| d.child(badge(i.t("ap.custom")).tone(BadgeTone::Outline))),
                    )
                    .child(btns),
            );
        }

        let errors = |errs: &[String]| {
            div().flex().flex_col().small(&t).text_color(t.colors.danger).children(errs.iter().map(|e| div().child(format!("• {e}"))))
        };

        let editor = self.editor.as_ref().map(|ed| {
            let column = |title: String, inputs: &Vec<Entity<TextInput>>, cx: &Context<Self>| {
                let vals: Vec<String> = inputs.iter().map(|e| e.read(cx).text().trim().to_string()).collect();
                let mut col = div().flex().flex_col().gap(t.space(1)).flex_1().min_w(t.rem(16.)).child(div().font_weight(FontWeight::SEMIBOLD).child(title));
                for (n, k) in COLOR_KEYS.iter().enumerate() {
                    let v = &vals[n];
                    let swc = parse_color(v).unwrap_or(gpui::transparent_black());
                    col = col.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .child(div().w(t.rem(7.)).flex_none().small(&t).code(&t).child(k.to_string()))
                            .child(div().size(t.rem(1.)).flex_none().rounded(px(3.)).border_1().border_color(t.colors.border).bg(swc))
                            .child(div().flex_1().child(inputs[n].clone())),
                    );
                }
                let get = |k: &str| COLOR_KEYS.iter().position(|x| *x == k).map(|p| vals[p].clone()).unwrap_or_default();
                if let Some(r) = contrast_ratio(&get("fg"), &get("bg")) {
                    let low = r < 4.5;
                    col = col.child(
                        div()
                            .small(&t)
                            .text_color(if low { t.colors.warning } else { t.colors.fg_muted })
                            .child(format!("{} {}", i.tp("ap.editor.contrast", &[("ratio", &format!("{r:.2}"))]), if low { i.t("ap.editor.contrastLow") } else { String::new() })),
                    );
                }
                col
            };
            let from_sel = ed.existing.is_none().then(|| {
                let me = cx.entity().downgrade();
                let items = themes.iter().map(|th| (SharedString::from(th.id.clone()), SharedString::from(th.name.clone()))).collect();
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .child(div().font_weight(FontWeight::MEDIUM).child(i.t("ap.editor.from")))
                    .child(select("ap-from", items, ed.from.clone()).on_change(move |v, _, cx| {
                        me.update(cx, |s, cx| s.copy_from(&v, cx)).ok();
                    }))
            });
            div()
                .flex()
                .flex_col()
                .gap(t.space(3))
                .p(t.space(4))
                .rounded(t.rem(rem::RADIUS_LG))
                .border_1()
                .border_color(t.colors.border)
                .bg(t.colors.surface)
                .child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("ap.editor.title")))
                .child(div().flex().flex_col().gap(t.space(1)).child(div().font_weight(FontWeight::MEDIUM).child(i.t("ap.editor.name"))).child(ed.name.clone()))
                .children(from_sel)
                .child(div().flex().flex_wrap().gap(t.space(4)).child(column(i.t("ap.editor.dark"), &ed.dark, cx)).child(column(i.t("ap.editor.light"), &ed.light, cx)))
                .when(!ed.errors.is_empty(), |d| d.child(errors(&ed.errors)))
                .child(
                    div()
                        .flex()
                        .gap(t.space(2))
                        .child(button("ap-ed-save").label(i.t("ap.editor.save")).variant(ButtonVariant::Solid).on_click(cx.listener(|s, _, _, cx| s.save_editor(cx))))
                        .child(button("ap-ed-cancel").label(i.t("ap.editor.cancel")).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| {
                            s.editor = None;
                            cx.notify();
                        }))),
                )
        });

        div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.themes.intro")))
            .child(actions)
            .children(confirm)
            .when(!self.import_errors.is_empty(), |d| d.child(errors(&self.import_errors)))
            .child(grid)
            .children(editor)
    }
}


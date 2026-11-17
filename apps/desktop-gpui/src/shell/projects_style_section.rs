//! Tab "Section project" di Tampilan — port ProjectsTab (AppearancePage.tsx): per project, logo (teks/emoji), warna
//! border, warna latar, dan warna teks & ikon. Kosong = bawaan; diterapkan saat Enter.
//! Juga: gradien (CSS atau templat), gambar latar (posisi/zoom), overlay (jenis/warna/kepekatan), logo gambar.
use crate::core::appearance::{AppearanceStore, PROJECT_STYLE_KEYS};
use crate::core::projects::ProjectStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H4};
use crate::core::appearance::PROJECT_OVERLAYS;
use crate::core::gradient;
use crate::ui::{button, select, ButtonVariant, ControlSize, InputChanged, InputSubmit, TextInput, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::SharedString;
use gpui::{div, prelude::*, Context, Entity, IntoElement, Window};

struct Field {
    project: String,
    key: &'static str,
    input: Entity<TextInput>,
    error: Option<String>,
}

pub struct ProjectsStyleSection {
    store: Entity<AppearanceStore>,
    projects: Entity<ProjectStore>,
    fields: Vec<Field>,
}

impl ProjectsStyleSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = AppearanceStore::global(cx);
        let projects = ProjectStore::global(cx);
        cx.observe(&projects, |this, _, cx| {
            this.rebuild(cx);
            cx.notify();
        })
        .detach();
        let mut me = Self { store, projects, fields: vec![] };
        me.rebuild(cx);
        me
    }

    /// Buat ulang kolom bila daftar project berubah.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let list: Vec<(String, String)> = self.projects.read(cx).projects.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
        let have: Vec<String> = self.fields.iter().filter(|f| f.key == "logo").map(|f| f.project.clone()).collect();
        if have == list.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>() {
            return;
        }
        let i = cx.global::<I18n>().clone();
        self.fields.clear();
        for (pid, _) in list {
            for key in PROJECT_STYLE_KEYS {
                let cur = self.store.read(cx).project_style(&pid, key).unwrap_or_default();
                let ph = match key {
                    "logo" => "🚀".to_string(),
                    "gradient" => "linear-gradient(135deg, #0f766e 0%, #7c3aed 100%)".to_string(),
                    "overlayOpacity" | "imageY" => "50".to_string(),
                    "imageZoom" => "100".to_string(),
                    _ => i.t("ap.projects.colorHint"),
                };
                let input = cx.new(|cx| {
                    let mut ti = TextInput::new(cx, ph);
                    ti.size = ControlSize::Sm;
                    ti.set_text(cur, cx);
                    ti
                });
                let n = self.fields.len();
                cx.subscribe(&input, move |this, _, e: &InputSubmit, cx| this.commit(n, e.0.to_string(), cx)).detach();
                cx.subscribe(&input, move |this, _, e: &crate::ui::InputBlur, cx| this.commit(n, e.0.to_string(), cx)).detach();
                cx.subscribe(&input, move |this, _, _: &InputChanged, cx| {
                    if let Some(f) = this.fields.get_mut(n) {
                        if f.error.take().is_some() {
                            f.input.update(cx, |ti, _| ti.invalid = false);
                            cx.notify();
                        }
                    }
                })
                .detach();
                self.fields.push(Field { project: pid.clone(), key, input, error: None });
            }
        }
    }

    fn commit(&mut self, n: usize, raw: String, cx: &mut Context<Self>) {
        let Some(f) = self.fields.get(n) else { return };
        let (pid, key) = (f.project.clone(), f.key);
        let v = raw.trim().to_string();
        let err = self.store.update(cx, |s, cx| s.set_project_style(&pid, key, (!v.is_empty()).then_some(v.as_str()), cx));
        let bad = err.is_some();
        self.fields[n].error = err.map(String::from);
        self.fields[n].input.update(cx, |ti, _| ti.invalid = bad);
        cx.notify();
    }

    /// Set satu kunci langsung (select/templat/gambar) dan segarkan kolom teksnya.
    fn put(&mut self, pid: &str, key: &str, v: Option<String>, cx: &mut Context<Self>) {
        let err = self.store.update(cx, |s, cx| s.set_project_style(pid, key, v.as_deref(), cx));
        if let Some(e) = err {
            Toasts::show(ToastInput { title: e.into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None }, cx);
        }
        if let Some(f) = self.fields.iter().find(|f| f.project == pid && f.key == key) {
            let cur = v.unwrap_or_default();
            f.input.update(cx, |ti, cx| ti.set_text(cur, cx));
        }
        cx.notify();
    }

    fn pick_image(&mut self, pid: String, key: &'static str, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(p) = paths.into_iter().next() else { return };
            let r = if key == "logoImage" {
                gradient::image_to_data_url(&p, 64, gradient::OutFmt::Png, gradient::MAX_LOGO_IMAGE_CHARS)
            } else {
                gradient::image_to_data_url(&p, 1280, gradient::OutFmt::Jpeg(80), gradient::MAX_STYLE_IMAGE_CHARS)
            };
            this.update(cx, |s, cx| match r {
                Ok(url) => s.put(&pid, key, Some(url), cx),
                Err(e) => {
                    Toasts::show(ToastInput { title: e.into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None }, cx);
                }
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ProjectsStyleSection {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let names: Vec<(String, String)> = self.projects.read(cx).projects.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
        if names.is_empty() {
            return div().small(&t).text_color(t.colors.fg_muted).child(i.t("mem.settings.projectsEmpty"));
        }
        let label = |k: &str| match k {
            "logo" => i.t("ap.projects.logo"),
            "border" => i.t("ap.projects.border"),
            "background" => i.t("ap.projects.background"),
            "gradient" => i.t("pstyle.bg.gradient"),
            "overlayColor" => i.t("pstyle.overlay.color"),
            "overlayOpacity" => i.tp("pstyle.overlay.opacity", &[("pct", "0–100")]),
            "imageY" => i.tp("pstyle.bg.posY", &[("pct", "0–100")]),
            "imageZoom" => i.tp("pstyle.bg.zoom", &[("pct", "100–300")]),
            _ => i.t("pstyle.fg"),
        };
        let mut col = div().flex().flex_col().gap(t.space(3)).child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.projects.intro")));
        for (pid, name) in names {
            let mut row = div().flex().flex_wrap().gap(t.space(2));
            for f in self.fields.iter().filter(|f| f.project == pid) {
                row = row.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.space(1))
                        .flex_1()
                        .min_w(t.rem(10.))
                        .child(div().small(&t).child(format!("{name} · {}", label(f.key))))
                        .child(f.input.clone())
                        .children(f.error.clone().map(|e| div().small(&t).text_color(t.colors.danger).child(e))),
                );
            }
            let (overlay, has_img, has_logo) = {
                let s = self.store.read(cx);
                (s.project_style(&pid, "overlay").unwrap_or_else(|| "none".into()), s.project_style(&pid, "image").is_some(), s.project_style(&pid, "logoImage").is_some())
            };
            let mut ov_items: Vec<(SharedString, SharedString)> = vec![("none".into(), i.t("pstyle.overlay.none").into())];
            for (k, l) in PROJECT_OVERLAYS.iter().zip(["pstyle.overlay.full", "pstyle.overlay.gradient", "pstyle.overlay.gradientRtl", "pstyle.overlay.gradientTtb", "pstyle.overlay.gradientBtt"]) {
                ov_items.push(((*k).into(), i.t(l).into()));
            }
            let mut templates = div().flex().flex_wrap().gap(t.space(1));
            for (n, (tid, tname, ..)) in gradient::TEMPLATES.iter().enumerate() {
                let p2 = pid.clone();
                templates = templates.child(
                    button(SharedString::from(format!("tpl-{pid}-{tid}")))
                        .label(*tname)
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(move |s, _, _, cx| {
                            let css = gradient::template(n).map(|g| gradient::gradient_css(&g));
                            s.put(&p2, "gradient", css, cx);
                        })),
                );
            }
            let (p_ov, p_img, p_rimg, p_logo, p_rlogo) = (pid.clone(), pid.clone(), pid.clone(), pid.clone(), pid.clone());
            let extra = div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("pstyle.templates")))
                .child(templates)
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(t.space(2))
                        .child(div().small(&t).child(i.t("pstyle.overlay.type")))
                        .child(select(SharedString::from(format!("ov-{pid}")), ov_items, overlay).on_change({
                            let me = cx.entity();
                            move |v, _, cx| {
                                let v = (v.as_ref() != "none").then(|| v.to_string());
                                me.update(cx, |s, cx| s.put(&p_ov, "overlay", v, cx));
                            }
                        }))
                        .child(button(SharedString::from(format!("img-{pid}"))).label(i.t("pstyle.bg.upload")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.pick_image(p_img.clone(), "image", cx))))
                        .when(has_img, |d| d.child(button(SharedString::from(format!("rimg-{pid}"))).label(i.t("pstyle.bg.removeImage")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| s.put(&p_rimg, "image", None, cx)))))
                        .child(button(SharedString::from(format!("logo-{pid}"))).label(i.t("pstyle.logo.upload")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.pick_image(p_logo.clone(), "logoImage", cx))))
                        .when(has_logo, |d| d.child(button(SharedString::from(format!("rlogo-{pid}"))).label(i.t("pstyle.logo.remove")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| s.put(&p_rlogo, "logoImage", None, cx))))),
                )
                .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("pstyle.bg.imageHint")));
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(2))
                    .p(t.space(3))
                    .rounded(t.rem(rem::RADIUS_LG))
                    .border_1()
                    .border_color(t.colors.border_soft)
                    .bg(t.colors.raised)
                    .child(div().typo(&t, H4).child(name.clone()))
                    .child(row)
                    .child(extra),
            );
        }
        col
    }
}

//! Tab "Section project" di Tampilan — port ProjectsTab (AppearancePage.tsx): per project, logo (teks/emoji), warna
//! border, warna latar, dan warna teks & ikon. Kosong = bawaan; diterapkan saat Enter.
//! Belum diport: dialog gaya lengkap (gradien, gambar latar, overlay, logo gambar).
use crate::core::appearance::{AppearanceStore, PROJECT_STYLE_KEYS};
use crate::core::projects::ProjectStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H4};
use crate::ui::{ControlSize, InputChanged, InputSubmit, TextInput, TypeExt};
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
                let ph = if key == "logo" { "🚀".to_string() } else { i.t("ap.projects.colorHint") };
                let input = cx.new(|cx| {
                    let mut ti = TextInput::new(cx, ph);
                    ti.size = ControlSize::Sm;
                    ti.set_text(cur, cx);
                    ti
                });
                let n = self.fields.len();
                cx.subscribe(&input, move |this, _, e: &InputSubmit, cx| this.commit(n, e.0.to_string(), cx)).detach();
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
                    .child(row),
            );
        }
        col
    }
}

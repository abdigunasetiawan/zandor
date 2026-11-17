//! Kategori "Profil" di Pengaturan — port ProfileSettings.tsx: pratinjau, nama tampilan, avatar (inisial/gambar).
//! Tab "Ilustrasi": DiceBear asli dirender lokal (core/dicebear.rs): pilih gaya, acak 12 pilihan, atribusi.
use crate::core::profile::{os_name, profile_avatar, Profile, MAX_PROFILE_NAME};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, tabs, ControlSize, InputChanged, TabItem, TextInput, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, Context, Entity, FontWeight, IntoElement, SharedString, Window};

pub struct ProfileSection {
    profile: Entity<Profile>,
    name: Entity<TextInput>,
    tab: SharedString,
    pub heading: bool,
    style: String,
    seeds: Vec<String>,
}

const CHOICES: usize = 12;

fn fail(cx: &mut gpui::App, key: &str, err: String) {
    let msg = cx.global::<I18n>().tp(key, &[("error", &err)]);
    Toasts::show(ToastInput { title: msg.into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None }, cx);
}

impl ProfileSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let profile = Profile::global(cx);
        cx.observe(&profile, |_, _, cx| cx.notify()).detach();
        let ph = cx.global::<I18n>().t("profile.namePlaceholder");
        let cur = profile.read(cx).name.clone().unwrap_or_default();
        let name = cx.new(|cx| {
            let mut ti = TextInput::new(cx, ph);
            ti.set_text(cur, cx);
            ti
        });
        cx.subscribe(&name, |this, _, e: &InputChanged, cx| {
            let v: String = e.0.chars().take(MAX_PROFILE_NAME).collect();
            if let Err(err) = this.profile.update(cx, |p, cx| p.set_name(&v, cx)) {
                fail(cx, "profile.error", err);
            }
        })
        .detach();
        let (tab, style, first) = {
            let p = profile.read(cx);
            match (&p.avatar, &p.illustration) {
                (Some(_), _) => ("image", "lorelei".to_string(), None),
                (None, Some((s, d))) => ("illustration", s.clone(), Some(d.clone())),
                _ => ("illustration", "lorelei".to_string(), None),
            }
        };
        let mut seeds: Vec<String> = (0..CHOICES).map(|_| crate::core::dicebear::random_seed()).collect();
        if let Some(d) = first {
            seeds[0] = d;
        }
        Self { profile, name, tab: tab.into(), heading: true, style, seeds }
    }
}

impl Render for ProfileSection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let p = self.profile.read(cx);
        let (display, has_img) = (p.display_name(), p.avatar.is_some() || p.illustration.is_some());
        let cur_illu = p.illustration.clone();
        let me = cx.entity().downgrade();

        let card = div()
            .flex()
            .items_center()
            .gap(t.space(3))
            .p(t.space(3))
            .rounded(t.rem(rem::RADIUS_LG))
            .border_1()
            .border_color(t.colors.border_soft)
            .bg(t.colors.raised)
            .child(profile_avatar(cx).size(ControlSize::Lg))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(display))
                    .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("profile.previewHint"))),
            );

        let name_field = div()
            .flex()
            .flex_col()
            .gap(t.space(1))
            .child(div().font_weight(FontWeight::MEDIUM).child(i.t("profile.name")))
            .child(self.name.clone())
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.tp("profile.nameHint", &[("os", &os_name())])));

        let panel = if self.tab == "illustration" {
            use crate::core::dicebear::{credit, image, random_seed, AVATAR_STYLES};
            let items: Vec<(SharedString, SharedString)> = AVATAR_STYLES.iter().map(|s| (SharedString::from(*s), SharedString::from(credit(s).title))).collect();
            let me2 = cx.entity();
            let mut grid = div().flex().flex_wrap().gap(t.space(2));
            for (n, seed) in self.seeds.iter().enumerate() {
                let on = cur_illu.as_ref().is_some_and(|(s, d)| *s == self.style && d == seed);
                let (st, sd) = (self.style.clone(), seed.clone());
                grid = grid.child(
                    div()
                        .id(SharedString::from(format!("prof-ill-{n}")))
                        .size(t.rem(4.))
                        .rounded(t.rem(rem::RADIUS_LG))
                        .overflow_hidden()
                        .border_2()
                        .border_color(if on { t.colors.focus } else { t.colors.border_soft })
                        .bg(t.colors.surface)
                        .cursor_pointer()
                        .hover(|s| s.border_color(t.colors.border_strong))
                        .tooltip(crate::ui::tooltip::text_tooltip(i.tp("profile.pick", &[("n", &(n + 1).to_string())]).into()))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            if let Err(e) = s.profile.update(cx, |p, cx| p.set_illustration(&st, &sd, cx)) {
                                fail(cx, "profile.error", e);
                            }
                        }))
                        .children(image(&self.style, seed).map(|img| gpui::img(img).size_full())),
                );
            }
            let c = credit(&self.style);
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .child(crate::ui::select("prof-style", items, self.style.clone()).on_change(move |v, _, cx| {
                            me2.update(cx, |s, cx| {
                                s.style = v.to_string();
                                cx.notify();
                            })
                        }))
                        .child(button("prof-shuffle").child(crate::ui::icon("refresh").size(crate::ui::IconSize::Sm)).label(i.t("profile.shuffle")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| {
                            s.seeds = (0..CHOICES).map(|_| random_seed()).collect();
                            cx.notify();
                        }))),
                )
                .child(grid)
                .child(div().small(&t).text_color(t.colors.fg_muted).child(i.tp("profile.credit", &[("title", &c.title), ("creator", &c.creator), ("license", &c.license)])))
        } else if self.tab == "image" {
            div()
                .flex()
                .items_center()
                .gap(t.space(3))
                .child(
                    button("prof-upload").child(crate::ui::icon("upload").size(crate::ui::IconSize::Sm)).label(i.t("profile.upload")).size(ControlSize::Sm).on_click(cx.listener(|_, _, _, cx| {
                        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
                        cx.spawn(async move |this, cx| {
                            if let Ok(Ok(Some(paths))) = rx.await {
                                if let Some(path) = paths.into_iter().next() {
                                    this.update(cx, |s, cx| {
                                        if let Err(e) = s.profile.update(cx, |p, cx| p.set_image(&path, cx)) {
                                            fail(cx, "profile.uploadError", e);
                                        }
                                    })
                                    .ok();
                                }
                            }
                        })
                        .detach();
                    })),
                )
                .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("profile.imageHint")))
        } else {
            div()
                .flex()
                .items_center()
                .gap(t.space(3))
                .child(div().flex_1().small(&t).text_color(t.colors.fg_muted).child(i.t("profile.initialsHint")))
                .child(
                    button("prof-initials")
                        .label(i.t("profile.useInitials"))
                        .size(ControlSize::Sm)
                        .disabled(!has_img)
                        .on_click(cx.listener(|s, _, _, cx| s.profile.update(cx, |p, cx| p.use_initials(cx)))),
                )
        };
        let tab_items = vec![
            TabItem { value: "initials".into(), label: i.t("profile.avatar.initials").into(), icon: Some("user"), disabled: false },
            TabItem { value: "image".into(), label: i.t("profile.avatar.image").into(), icon: Some("image"), disabled: false },
            TabItem { value: "illustration".into(), label: i.t("profile.avatar.illustration").into(), icon: Some("sparkles"), disabled: false },
        ];
        let avatar_group = div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("profile.avatar")))
            .child(tabs("prof-tabs", tab_items, self.tab.clone()).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.tab = v.clone();
                    cx.notify();
                })
                .ok();
            }))
            .child(panel);

        div().flex().flex_col().gap(t.space(4)).child(card).child(name_field).child(avatar_group)
    }
}

//! Pengaturan → Notifikasi dan suara, bagian khusus — port NotificationSettings.tsx: izin notifikasi sistem, uji
//! notifikasi (toast + sistem + suara sesuai pengaturan), dan per peristiwa: berkas suara kustom + uji suara.
use crate::core::settings::SettingsStore;
use crate::i18n::I18n;
use crate::sound;
use crate::theme::Theme;
use crate::ui::{button, ButtonVariant, ControlSize, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, App, Context, FontWeight, IntoElement, SharedString, Window};

pub struct NotificationSection;

impl NotificationSection {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self
    }
}

fn toast(cx: &mut App, title: String, desc: Option<String>, tone: ToastTone, ms: Option<u64>) {
    Toasts::show(ToastInput { title: title.into(), description: desc.map(Into::into), tone, timeout_ms: ms, action: None }, cx);
}

fn choice_for(ev: &str, cx: &App) -> (String, f64) {
    let s = SettingsStore::global(cx).read(cx);
    let key = match ev {
        "done" => "soundDone",
        "failed" => "soundFailed",
        _ => "soundAction",
    };
    (s.str(key), s.num("notifyVolume"))
}

fn test_sound(ev: &str, cx: &mut App) {
    let (choice, vol) = choice_for(ev, cx);
    if !sound::play(&choice, vol, ev) && choice != "none" {
        let i = cx.global::<I18n>().clone();
        toast(cx, i.t(if choice == "custom" { "notif.custom.missing" } else { "notif.test.failed" }), None, ToastTone::Warning, None);
    }
}

impl Render for NotificationSection {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let row = |t: &Theme, title: String, body: String, ctl: gpui::AnyElement| {
            div()
                .flex()
                .items_center()
                .gap(t.space(4))
                .py(t.space(3))
                .child(div().flex().flex_col().flex_1().min_w_0().gap(t.space(1)).child(div().font_weight(FontWeight::MEDIUM).child(title)).child(div().small(t).text_color(t.colors.fg_muted).child(body)))
                .child(div().flex().flex_none().gap(t.space(2)).child(ctl))
        };
        // Windows: toast sistem selalu tersedia (izin diatur di Pengaturan Windows → Notifikasi).
        let mut col = div()
            .flex()
            .flex_col()
            .child(row(
                &t,
                i.t("notif.permission.title"),
                i.t("notif.permission.granted"),
                button("notif-perm")
                    .size(ControlSize::Sm)
                    .label(i.t("notif.permission.request"))
                    .on_click(|_, _, cx| cx.open_url("ms-settings:notifications"))
                    .into_any_element(),
            ))
            .child(row(
                &t,
                i.t("notif.testAll.title"),
                i.t("notif.testAll.help"),
                button("notif-test-all")
                    .size(ControlSize::Sm)
                    .label(i.t("notif.testAll.run"))
                    .on_click(|_, _, cx| {
                        let i = cx.global::<I18n>().clone();
                        let (title, body) = (i.t("notify.done"), i.t("notif.testAll.body"));
                        toast(cx, title.clone(), Some(body.clone()), ToastTone::Success, Some(6000));
                        let s = SettingsStore::global(cx).read(cx);
                        let (sys, snd) = (s.bool("notifySystem"), s.bool("notifySound"));
                        if sys {
                            if let Err(e) = sound::system_notify(&title, &body) {
                                toast(cx, i.tp("sb.error", &[("error", &e)]), None, ToastTone::Danger, None);
                            }
                        }
                        if snd {
                            test_sound("done", cx);
                        }
                    })
                    .into_any_element(),
            ))
            .child(div().mt(t.space(3)).font_weight(FontWeight::MEDIUM).child(i.t("notif.custom.title")))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("notif.custom.help")));
        for ev in sound::EVENTS {
            let custom = sound::custom_sound(ev);
            let ev_s = ev.to_string();
            let body = match &custom {
                Some((n, _)) => i.tp("notif.custom.current", &[("name", n)]),
                None => i.t("notif.custom.none"),
            };
            let (e1, e2, e3) = (ev_s.clone(), ev_s.clone(), ev_s.clone());
            let ctl = div()
                .flex()
                .gap(t.space(2))
                .child(
                    button(SharedString::from(format!("snd-pick-{ev}")))
                        .size(ControlSize::Sm)
                        .tooltip_text(i.tp("notif.custom.pick", &[("event", &i.t(&format!("notif.event.{ev}")))]))
                        .label(i.t("notif.custom.choose"))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
                            let ev = e1.clone();
                            cx.spawn(async move |this, cx| {
                                if let Ok(Ok(Some(paths))) = rx.await {
                                    if let Some(p) = paths.into_iter().next() {
                                        this.update(cx, |_, cx| {
                                            let i = cx.global::<I18n>().clone();
                                            match sound::save_custom_sound(&ev, &p) {
                                                Ok(name) => toast(cx, i.tp("notif.custom.saved", &[("name", &name)]), None, ToastTone::Success, Some(2000)),
                                                Err(e) => toast(cx, i.tp("notif.custom.error", &[("error", &e)]), None, ToastTone::Danger, None),
                                            }
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                }
                            })
                            .detach();
                        })),
                )
                .when(custom.is_some(), |d| {
                    d.child(button(SharedString::from(format!("snd-rm-{ev}"))).size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("notif.custom.remove")).on_click(cx.listener(move |_, _, _, cx| {
                        sound::clear_custom_sound(&e2);
                        cx.notify();
                    })))
                })
                .child(button(SharedString::from(format!("snd-test-{ev}"))).size(ControlSize::Sm).label(i.t("notif.test")).on_click(move |_, _, cx| test_sound(&e3, cx)));
            col = col.child(row(&t, i.t(&format!("notif.event.{ev}")), body, ctl.into_any_element()));
        }
        col
    }
}

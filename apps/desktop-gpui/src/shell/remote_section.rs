//! Pengaturan → Engine remote — port RemoteEngines.tsx: daftar engine opencode remote (URL + Basic Auth) dengan status
//! health nyata, tambah/ubah (form), cek, hapus (ditolak bila masih dipakai project), serta tambah project di engine itu
//! (path folder di mesin remote). Zandor tidak memasang/menjalankan engine di sana.
use crate::core::projects::{ProjectStore, RemoteStatus};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{badge, button, BadgeTone, ButtonVariant, ControlSize, TextInput, ToastInput, ToastTone, Toasts, TypeExt};
use gpui::{div, prelude::*, Context, Entity, IntoElement, SharedString, Window};

struct Form {
    id: Option<String>,
    name: Entity<TextInput>,
    url: Entity<TextInput>,
    user: Entity<TextInput>,
    pass: Entity<TextInput>,
    error: Option<String>,
}

pub struct RemoteSection {
    store: Entity<ProjectStore>,
    form: Option<Form>,
    confirm_remove: Option<String>,
    add_path: Option<(String, Entity<TextInput>)>,
}

fn input(cx: &mut Context<RemoteSection>, ph: String, val: &str, masked: bool) -> Entity<TextInput> {
    let v = val.to_string();
    cx.new(|cx| {
        let mut ti = TextInput::new(cx, ph);
        ti.size = ControlSize::Sm;
        ti.masked = masked;
        ti.set_text(v, cx);
        ti
    })
}

fn toast(msg: String, tone: ToastTone, cx: &mut gpui::App) {
    Toasts::show(ToastInput { title: msg.into(), description: None, tone, timeout_ms: None, action: None }, cx);
}

impl RemoteSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = ProjectStore::global(cx);
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        Self { store, form: None, confirm_remove: None, add_path: None }
    }

    fn open_form(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        let r = id.as_ref().and_then(|id| self.store.read(cx).remote_by_id(id).cloned());
        let (n, u, us, p) = r.map(|r| (r.name, r.url, r.username, r.password)).unwrap_or_else(|| (String::new(), String::new(), "opencode".into(), String::new()));
        self.form = Some(Form {
            id,
            name: input(cx, i.t("remote.namePlaceholder"), &n, false),
            url: input(cx, "https://engine.contoh.id".into(), &u, false),
            user: input(cx, "opencode".into(), &us, false),
            pass: input(cx, String::new(), &p, true),
            error: None,
        });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(f) = &self.form else { return };
        let (id, name, url, user, pass) = (f.id.clone(), f.name.read(cx).text().to_string(), f.url.read(cx).text().to_string(), f.user.read(cx).text().to_string(), f.pass.read(cx).text().to_string());
        let r = self.store.update(cx, |s, cx| s.save_remote(id, &name, &url, &user, &pass, cx));
        let i = cx.global::<I18n>().clone();
        match r {
            Ok(_) => self.form = None,
            Err(e) => {
                let msg = match e.as_str() {
                    "invalid" => i.t("remote.urlInvalid"),
                    "insecure" => i.t("remote.urlInsecure"),
                    _ => e,
                };
                if let Some(f) = &mut self.form {
                    f.error = Some(msg);
                }
            }
        }
        cx.notify();
    }

    fn render_form(&self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let f = self.form.as_ref()?;
        let field = |label: String, hint: Option<String>, el: Entity<TextInput>| {
            div().flex().flex_col().gap(t.space(1)).child(div().small(t).child(label)).child(el).children(hint.map(|h| div().small(t).text_color(t.colors.fg_subtle).child(h)))
        };
        Some(
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .rounded(t.rem(rem::RADIUS_LG))
                .border_1()
                .border_color(t.colors.focus)
                .bg(t.colors.raised)
                .child(div().typo(t, crate::theme::H4).child(i.t(if f.id.is_some() { "remote.editTitle" } else { "remote.addTitle" })))
                .child(div().small(t).text_color(t.colors.fg_muted).child(i.t("remote.formBody")))
                .child(field(i.t("remote.name"), None, f.name.clone()))
                .child(field(i.t("remote.url"), Some(i.t("remote.urlHint")), f.url.clone()))
                .child(field(i.t("remote.username"), None, f.user.clone()))
                .child(field(i.t("remote.password"), Some(i.t("remote.passwordHint")), f.pass.clone()))
                .children(f.error.clone().map(|e| div().small(t).text_color(t.colors.danger).child(e)))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(t.space(2))
                        .child(button("rf-cancel").label(i.t("ui.cancel")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| {
                            s.form = None;
                            cx.notify();
                        })))
                        .child(button("rf-save").label(i.t("remote.save")).size(ControlSize::Sm).variant(ButtonVariant::Solid).on_click(cx.listener(|s, _, _, cx| s.save(cx)))),
                )
                .into_any_element(),
        )
    }
}

impl Render for RemoteSection {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let (remotes, status, used): (Vec<_>, Vec<_>, Vec<_>) = {
            let s = self.store.read(cx);
            let r = s.remotes.clone();
            let st = r.iter().map(|x| s.remote_status.get(&x.id).cloned()).collect();
            let u = r.iter().map(|x| s.projects_using(&x.id)).collect();
            (r, st, u)
        };
        let mut list = div().flex().flex_col().gap(t.space(2));
        if remotes.is_empty() {
            list = list.child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("remote.empty")));
        }
        for ((r, st), n) in remotes.iter().zip(status).zip(used) {
            let st_el = match st {
                None | Some(RemoteStatus::Checking) => div().small(&t).text_color(t.colors.fg_muted).child(i.t("remote.checking")).into_any_element(),
                Some(RemoteStatus::Ready(v)) => badge(i.tp("remote.ready", &[("version", if v.is_empty() { "?" } else { &v })])).tone(BadgeTone::Success).into_any_element(),
                Some(RemoteStatus::Error(e)) => div()
                    .flex()
                    .flex_col()
                    .child(badge(i.t("remote.failed")).tone(BadgeTone::Danger))
                    .child(div().small(&t).text_color(t.colors.danger).child(e))
                    .into_any_element(),
            };
            let (id1, id2, id3, id4, id5) = (r.id.clone(), r.id.clone(), r.id.clone(), r.id.clone(), r.id.clone());
            let confirming = self.confirm_remove.as_deref() == Some(r.id.as_str());
            let adding = self.add_path.as_ref().filter(|(e, _)| *e == r.id).map(|(_, inp)| inp.clone());
            let mut card = div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .rounded(t.rem(rem::RADIUS_LG))
                .border_1()
                .border_color(t.colors.border_soft)
                .bg(t.colors.raised)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().truncate().child(r.name.clone()))
                                .child(div().small(&t).text_color(t.colors.fg_muted).truncate().child(format!("{} · {}", r.url, i.tp("remote.projects", &[("n", &n.to_string())])))),
                        )
                        .child(st_el)
                        .child(button(SharedString::from(format!("rc-{}", r.id))).label(i.t("remote.check")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| s.store.update(cx, |p, cx| p.check_remote(&id1, cx)))))
                        .child(button(SharedString::from(format!("re-{}", r.id))).label(i.t("remote.edit")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| s.open_form(Some(id2.clone()), cx))))
                        .child(button(SharedString::from(format!("rr-{}", r.id))).label(i.t("remote.remove")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| {
                            s.confirm_remove = Some(id3.clone());
                            cx.notify();
                        }))),
                );
            if confirming {
                card = card.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.space(2))
                        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(i.tp("remote.removeTitle", &[("name", &r.name)])))
                        .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("remote.removeBody")))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(t.space(2))
                                .child(button(SharedString::from(format!("rrc-{}", r.id))).label(i.t("ui.cancel")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| {
                                    s.confirm_remove = None;
                                    cx.notify();
                                })))
                                .child(button(SharedString::from(format!("rry-{}", r.id))).label(i.t("remote.remove")).size(ControlSize::Sm).variant(ButtonVariant::Danger).on_click(cx.listener(move |s, _, _, cx| {
                                    s.confirm_remove = None;
                                    if let Err(e) = s.store.update(cx, |p, cx| p.remove_remote(&id4, cx)) {
                                        toast(e, ToastTone::Danger, cx);
                                    }
                                    cx.notify();
                                }))),
                        ),
                );
            }
            card = card.child(match adding {
                Some(inp) => {
                    let eid = id5.clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.space(1))
                        .child(div().flex().gap(t.space(2)).child(div().flex_1().child(inp)).child(button(SharedString::from(format!("rap-{}", r.id))).label(i.t("remote.save")).size(ControlSize::Sm).variant(ButtonVariant::Solid).on_click(cx.listener(move |s, _, _, cx| {
                            let Some((_, inp)) = s.add_path.take() else { return };
                            let path = inp.read(cx).text().to_string();
                            if let Err(e) = s.store.update(cx, |p, cx| p.add_remote_project(&path, &eid, cx)) {
                                toast(e, ToastTone::Danger, cx);
                            }
                            cx.notify();
                        }))))
                        .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("remote.pathHint")))
                        .into_any_element()
                }
                None => div()
                    .child(button(SharedString::from(format!("rpa-{}", r.id))).label(i.t("sb.addByPathTitle")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| {
                        let inp = input(cx, "/home/saya/app".into(), "", false);
                        s.add_path = Some((id5.clone(), inp));
                        cx.notify();
                    })))
                    .into_any_element(),
            });
            list = list.child(card);
        }
        div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("remote.intro")))
            .child(list)
            .children(self.render_form(&t, &i, cx))
            .when(self.form.is_none(), |d| d.child(div().child(button("remote-add").label(i.t("remote.add")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, cx| s.open_form(None, cx))))))
    }
}

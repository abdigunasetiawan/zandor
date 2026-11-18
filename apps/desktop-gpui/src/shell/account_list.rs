//! Akun/kunci ganda pada satu provider (ADR-056) — port AccountList.tsx: daftar akun, pilih yang aktif, ganti nama,
//! hapus (dengan konfirmasi), dan tambah akun baru. Kunci hanya lewat keychain (providers.rs/secrets.rs bersama).
use crate::core::services::{app_data_dir, EngineStatus};
use crate::i18n::I18n;
use crate::providers::{AccountView, ProviderStore};
use crate::theme::Theme;
use crate::ui::{badge, button, icon, icon_button, BadgeTone, ButtonVariant, ControlSize, IconSize, TextInput, TypeExt};
use gpui::{div, prelude::*, Context, Entity, FontWeight, IntoElement, SharedString, Window};

fn store() -> ProviderStore {
    ProviderStore::new(&app_data_dir().join("providers.json"), Box::new(crate::secrets::Keychain::new(crate::secrets::SERVICE)))
}

/// Nama akun: label, "Akun utama" untuk akun lama, selain itu "Akun N".
pub fn account_name(i: &I18n, a: &AccountView, index: usize) -> String {
    if !a.label.is_empty() {
        return a.label.clone();
    }
    if a.id == crate::providers::LEGACY_ACCOUNT { i.t("acc.main") } else { i.tp("acc.unnamed", &[("n", &(index + 1).to_string())]) }
}

pub struct AccountList {
    provider_id: String,
    name: String,
    accounts: Vec<AccountView>,
    renaming: Option<String>,
    draft: Entity<TextInput>,
    adding: bool,
    label: Entity<TextInput>,
    key: Entity<TextInput>,
    confirm: Option<(String, String)>,
    /// false = tanpa tombol Tambah akun (OpenWork menambah akun lewat login).
    pub with_add: bool,
}

impl AccountList {
    pub fn new(provider_id: String, name: String, cx: &mut Context<Self>) -> Self {
        let i = cx.global::<I18n>().clone();
        let draft = cx.new(|cx| TextInput::new(cx, ""));
        let label = cx.new(|cx| TextInput::new(cx, i.t("acc.labelHint")));
        let key = cx.new(|cx| {
            let mut k = TextInput::new(cx, i.t("pv.apiKey"));
            k.masked = true;
            k
        });
        cx.subscribe(&key, |_, _, _: &crate::ui::InputChanged, cx| cx.notify()).detach();
        let mut me = Self { provider_id, name, accounts: vec![], renaming: None, draft, adding: false, label, key, confirm: None, with_add: true };
        me.reload();
        me
    }

    pub fn reload(&mut self) {
        self.accounts = store().views().ok().and_then(|v| v.into_iter().find(|p| p.id == self.provider_id)).map(|p| p.accounts).unwrap_or_default();
    }

    /// Jalankan aksi; galat → toast; berhasil → muat ulang + mulai ulang engine (kunci aktif dibaca saat engine mulai).
    fn run(&mut self, ok: Option<String>, f: impl FnOnce(&ProviderStore) -> Result<(), String>, cx: &mut Context<Self>) -> bool {
        let i = cx.global::<I18n>().clone();
        let r = f(&store());
        let good = r.is_ok();
        match r {
            Ok(()) => {
                if let Some(t) = ok {
                    toast(cx, t, crate::ui::ToastTone::Success);
                }
                EngineStatus::global(cx).read(cx).restart();
            }
            Err(e) => toast(cx, i.tp("sb.error", &[("error", &e)]), crate::ui::ToastTone::Danger),
        }
        self.reload();
        cx.notify();
        good
    }
}

fn toast(cx: &mut gpui::App, title: String, tone: crate::ui::ToastTone) {
    crate::ui::Toasts::show(crate::ui::ToastInput { title: title.into(), description: None, tone, timeout_ms: None, action: None }, cx);
}

impl Render for AccountList {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let mut col = div().flex().flex_col().gap(t.space(1)).child(
            div()
                .flex()
                .items_center()
                .gap(t.space(2))
                .child(div().font_weight(FontWeight::MEDIUM).child(i.t("acc.title")))
                .child(div().small(&t).text_color(t.colors.fg_muted).flex_1().child(i.tp("acc.count", &[("count", &self.accounts.len().to_string())])))
                .when(!self.adding && self.with_add, |d| {
                    d.child(button("acc-add").size(ControlSize::Sm).child(icon("plus").size(IconSize::Sm)).label(i.t("acc.add")).on_click(cx.listener(|s, _, _, cx| {
                        s.adding = true;
                        cx.notify();
                    })))
                }),
        );
        for (ix, a) in self.accounts.clone().into_iter().enumerate() {
            let display = account_name(&i, &a, ix);
            let aid = a.id.clone();
            let mut row = div().flex().items_center().gap(t.space(2)).py(t.space(1));
            if self.renaming.as_deref() == Some(a.id.as_str()) {
                let aid2 = aid.clone();
                row = row
                    .child(div().flex_1().child(self.draft.clone()))
                    .child(button(SharedString::from(format!("acc-rs-{aid}"))).size(ControlSize::Sm).variant(ButtonVariant::Solid).label(i.t("pv.save")).on_click(cx.listener(move |s, _, _, cx| {
                        let (pid, label) = (s.provider_id.clone(), s.draft.read(cx).text().to_string());
                        if s.run(None, |st| st.rename_account(&pid, &aid2, &label), cx) {
                            s.renaming = None;
                        }
                    })))
                    .child(button(SharedString::from(format!("acc-rc-{aid}"))).size(ControlSize::Sm).label(i.t("ow.cancel")).on_click(cx.listener(|s, _, _, cx| {
                        s.renaming = None;
                        cx.notify();
                    })));
            } else {
                let (a1, a2, a3, d1, d2) = (aid.clone(), aid.clone(), aid.clone(), display.clone(), display.clone());
                let lbl = a.label.clone();
                row = row
                    .child(div().flex_1().truncate().child(display.clone()))
                    .child(if a.active {
                        badge(i.t("acc.active")).tone(BadgeTone::Success).into_any_element()
                    } else {
                        button(SharedString::from(format!("acc-use-{aid}")))
                            .size(ControlSize::Sm)
                            .tooltip_text(i.tp("acc.useFor", &[("name", &display)]))
                            .label(i.t("acc.use"))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let pid = s.provider_id.clone();
                                let ok = cx.global::<I18n>().tp("acc.switched", &[("name", &d1)]);
                                s.run(Some(ok), |st| st.activate_account(&pid, &a1), cx);
                            }))
                            .into_any_element()
                    })
                    .child(icon_button(SharedString::from(format!("acc-ren-{aid}")), "edit", i.tp("acc.renameFor", &[("name", &display)])).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| {
                        s.draft.update(cx, |d, cx| d.set_text(lbl.clone(), cx));
                        s.renaming = Some(a2.clone());
                        cx.notify();
                    })))
                    .child(icon_button(SharedString::from(format!("acc-del-{aid}")), "trash", i.tp("acc.removeFor", &[("name", &display)])).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| {
                        s.confirm = Some((a3.clone(), d2.clone()));
                        cx.notify();
                    })));
            }
            col = col.child(row);
        }
        if self.adding {
            let empty = self.key.read(cx).text().trim().is_empty();
            col = col.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().flex_1().min_w(t.rem(8.)).child(self.label.clone()))
                    .child(div().flex_1().min_w(t.rem(8.)).child(self.key.clone()))
                    .child(button("acc-add-save").variant(ButtonVariant::Solid).disabled(empty).label(i.t("pv.save")).on_click(cx.listener(|s, _, _, cx| {
                        let (pid, label, key) = (s.provider_id.clone(), s.label.read(cx).text().trim().to_string(), s.key.read(cx).text().trim().to_string());
                        let ok = cx.global::<I18n>().t("pv.saved");
                        if s.run(Some(ok), |st| st.add_account(&pid, &label, &key, true).map(|_| ()), cx) {
                            s.adding = false;
                            s.label.update(cx, |l, cx| l.set_text("", cx));
                            s.key.update(cx, |k, cx| k.set_text("", cx));
                        }
                    })))
                    .child(button("acc-add-cancel").label(i.t("ow.cancel")).on_click(cx.listener(|s, _, _, cx| {
                        s.adding = false;
                        cx.notify();
                    }))),
            );
        }
        if let Some((aid, name)) = self.confirm.clone() {
            col = col.child(
                crate::ui::dialog(i.tp("acc.confirmTitle", &[("name", &name)]))
                    .description(i.t("acc.confirmBody"))
                    .on_dismiss(cx.listener(|s, _, _, cx| {
                        s.confirm = None;
                        cx.notify();
                    }))
                    .footer(button("acc-cf-no").label(i.t("ui.cancel")).on_click(cx.listener(|s, _, _, cx| {
                        s.confirm = None;
                        cx.notify();
                    })))
                    .footer(button("acc-cf-yes").variant(ButtonVariant::Danger).label(i.t("acc.remove")).on_click(cx.listener(move |s, _, _, cx| {
                        s.confirm = None;
                        let pid = s.provider_id.clone();
                        s.run(None, |st| st.remove_account(&pid, &aid).map(|_| ()), cx);
                    }))),
            );
        }
        let _ = &self.name;
        col
    }
}

//! Provider & kunci API — port bagian koneksi ModelSettings/ConnectionPanel: daftar koneksi (providers.json +
//! keychain lewat providers.rs/secrets.rs bersama), tambah/ubah (katalog engine, endpoint kustom, lokal), hapus.
//! Setiap perubahan memulai ulang engine agar konfigurasi terbaca (sama dengan perintah Tauri).
use crate::core::engine::CatalogProvider;
use crate::core::projects::ProjectStore;
use crate::core::services::{app_data_dir, EngineStatus};
use crate::i18n::I18n;
use crate::providers::{Kind, Provider, ProviderModel, ProviderStore, ProviderView};
use crate::theme::{rem, Theme};
use crate::ui::{badge, button, select, BadgeTone, ButtonVariant, ControlSize, TextInput, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};

fn store() -> ProviderStore {
    ProviderStore::new(&app_data_dir().join("providers.json"), Box::new(crate::secrets::Keychain::new(crate::secrets::SERVICE)))
}

struct Form {
    editing: bool,
    tab: SharedString,
    catalog_id: SharedString,
    protocol: SharedString,
    id: Entity<TextInput>,
    name: Entity<TextInput>,
    url: Entity<TextInput>,
    key: Entity<TextInput>,
    models: Entity<TextInput>,
    error: Option<String>,
    accounts: Option<Entity<super::account_list::AccountList>>,
}

pub struct ProvidersSection {
    views: Result<Vec<ProviderView>, String>,
    catalog: Vec<CatalogProvider>,
    form: Option<Form>,
    confirm_delete: Option<(String, String)>,
}

impl ProvidersSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&EngineStatus::global(cx), |s, _, cx| {
            s.load_catalog(cx);
            cx.notify();
        })
        .detach();
        let mut me = Self { views: store().views(), catalog: vec![], form: None, confirm_delete: None };
        me.load_catalog(cx);
        me
    }

    fn load_catalog(&mut self, cx: &mut Context<Self>) {
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        let task = cx.background_spawn(async move { api.list_catalog().unwrap_or_default() });
        cx.spawn(async move |this, cx| {
            let c = task.await;
            this.update(cx, |s, cx| {
                s.catalog = c;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn input(cx: &mut Context<Self>, ph: &str, v: &str) -> Entity<TextInput> {
        let (ph, v) = (ph.to_string(), v.to_string());
        cx.new(|cx| {
            let mut t = TextInput::new(cx, ph);
            t.set_text(v, cx);
            t
        })
    }

    fn open_form(&mut self, p: Option<ProviderView>, cx: &mut Context<Self>) {
        let pv = p.as_ref();
        let models = pv.map(|p| p.models.iter().map(|m| m.id.clone()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
        let tab = match pv.map(|p| p.kind) {
            Some(Kind::Builtin) => "catalog",
            Some(Kind::Local) => "local",
            _ => "custom",
        };
        self.form = Some(Form {
            editing: p.is_some(),
            tab: tab.into(),
            catalog_id: pv.filter(|p| p.kind == Kind::Builtin).map(|p| p.id.clone()).unwrap_or_default().into(),
            protocol: pv.and_then(|p| p.protocol.clone()).unwrap_or_else(|| "openai-compatible".into()).into(),
            id: Self::input(cx, "my-provider", &pv.map(|p| p.id.clone()).unwrap_or_default()),
            name: Self::input(cx, "", &pv.map(|p| p.name.clone()).unwrap_or_default()),
            url: Self::input(cx, "https://…/v1", &pv.and_then(|p| p.base_url.clone()).unwrap_or_default()),
            key: {
                let k = Self::input(cx, "", "");
                k.update(cx, |k, _| k.masked = true);
                k
            },
            models: Self::input(cx, "model-a, model-b", &models),
            error: None,
            accounts: pv.map(|p| (p.id.clone(), p.name.clone())).map(|(id, name)| cx.new(|cx| super::account_list::AccountList::new(id, name, cx))),
        });
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(f) = self.form.as_mut() else { return };
        let txt = |e: &Entity<TextInput>, cx: &Context<Self>| e.read(cx).text().trim().to_string();
        let key = txt(&f.key, cx);
        let models: Vec<ProviderModel> = txt(&f.models, cx)
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|id| ProviderModel { id: id.into(), ..Default::default() })
            .collect();
        let provider = if f.tab.as_ref() == "catalog" {
            let id = f.catalog_id.to_string();
            let name = self.catalog.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_else(|| id.clone());
            Provider { id, name, kind: Kind::Builtin, models, ..Default::default() }
        } else {
            Provider {
                id: txt(&f.id, cx),
                name: txt(&f.name, cx),
                kind: if f.tab.as_ref() == "local" { Kind::Local } else { Kind::Custom },
                base_url: Some(txt(&f.url, cx)).filter(|u| !u.is_empty()),
                protocol: Some(f.protocol.to_string()).filter(|p| p != "openai-compatible"),
                models,
                ..Default::default()
            }
        };
        if let Err(e) = provider.validate() {
            f.error = Some(e);
            cx.notify();
            return;
        }
        if !key.is_empty() {
            if let Err(e) = crate::providers::validate_api_key(&key) {
                f.error = Some(e);
                cx.notify();
                return;
            }
        }
        match store().save(provider, (!key.is_empty()).then_some(key)) {
            Ok(()) => {
                self.form = None;
                self.views = store().views();
                EngineStatus::global(cx).read(cx).restart();
            }
            Err(e) => {
                if let Some(f) = self.form.as_mut() {
                    f.error = Some(e);
                }
            }
        }
        cx.notify();
    }

    fn set_form(&mut self, f: impl FnOnce(&mut Form), cx: &mut Context<Self>) {
        if let Some(form) = self.form.as_mut() {
            f(form);
        }
        cx.notify();
    }
}

impl Render for ProvidersSection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let mut col = div().flex().flex_col().gap(t.space(2));
        col = col.child(
            div()
                .flex()
                .items_center()
                .child(div().flex_1().flex().flex_col().child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("ms.providers"))).child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ms.providersBody"))))
                .child(button("pv-add").label(i.t("pv.add")).size(ControlSize::Sm).variant(ButtonVariant::Solid).on_click(cx.listener(|s, _, _, cx| s.open_form(None, cx)))),
        );
        col = col.child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("pv.secure.yes")));

        // ---- formulir ----
        if let Some(f) = &self.form {
            let field = |label: String, el: AnyElement| div().flex().flex_col().gap(t.space(1)).child(div().small(&t).font_weight(FontWeight::MEDIUM).child(label)).child(el);
            let me = cx.entity().downgrade();
            let tabs_el = div().flex().gap(t.space(1)).children(["catalog", "custom", "local"].into_iter().map(|k| {
                let me = me.clone();
                button(SharedString::from(format!("pv-tab-{k}")))
                    .label(i.t(&format!("pv.tab.{k}")))
                    .size(ControlSize::Sm)
                    .variant(if f.tab.as_ref() == k { ButtonVariant::Solid } else { ButtonVariant::Outline })
                    .disabled(f.editing)
                    .on_click(move |_, _, cx| {
                        me.update(cx, |s, cx| s.set_form(|f| f.tab = k.into(), cx)).ok();
                    })
            }));
            let mut form = div().flex().flex_col().gap(t.space(3)).p(t.space(3)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border).bg(t.colors.bg).child(tabs_el);
            if f.tab.as_ref() == "catalog" {
                let items: Vec<(SharedString, SharedString)> = self.catalog.iter().map(|c| (c.id.clone().into(), format!("{} ({})", c.name, c.model_count).into())).collect();
                let me2 = me.clone();
                form = form.child(field(
                    i.t("pv.tab.catalog"),
                    if items.is_empty() {
                        div().text_color(t.colors.fg_muted).child(i.t("pv.status.engineDown")).into_any_element()
                    } else {
                        select("pv-cat", items, f.catalog_id.clone())
                            .on_change(move |v, _, cx| {
                                me2.update(cx, |s, cx| s.set_form(|f| f.catalog_id = v, cx)).ok();
                            })
                            .into_any_element()
                    },
                ));
            } else {
                let me3 = me.clone();
                form = form
                    .child(field(i.t("pv.id"), f.id.clone().into_any_element()))
                    .when(f.editing, |d| d.child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("pv.idLocked"))))
                    .child(field(i.t("pv.name"), f.name.clone().into_any_element()))
                    .child(field(i.t("pv.baseUrl"), f.url.clone().into_any_element()))
                    .child(field(
                        i.t("pv.protocol"),
                        select(
                            "pv-proto",
                            vec![
                                ("openai-compatible".into(), "OpenAI-compatible".into()),
                                ("openai".into(), i.t("pv.protocol.openai").into()),
                                ("anthropic".into(), i.t("pv.protocol.anthropic").into()),
                                ("google".into(), i.t("pv.protocol.google").into()),
                            ],
                            f.protocol.clone(),
                        )
                        .on_change(move |v, _, cx| {
                            me3.update(cx, |s, cx| s.set_form(|f| f.protocol = v, cx)).ok();
                        })
                        .into_any_element(),
                    ))
                    .child(field(format!("{} ({})", i.t("pv.models"), i.t("pv.manualModelId")), f.models.clone().into_any_element()));
            }
            form = form
                .child(field(i.t("pv.apiKey"), f.key.clone().into_any_element()))
                .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t(if f.editing { "pv.apiKeyKeep" } else if f.tab.as_ref() == "local" { "pv.apiKeyOptional" } else { "pv.secure.yes" })))
                .children(f.accounts.clone())
                .children(f.error.clone().map(|e| div().text_color(t.colors.danger).child(e)))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(t.space(2))
                        .child(button("pv-cancel").label(i.t("ui.cancel")).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| {
                            s.form = None;
                            cx.notify();
                        })))
                        .child(button("pv-save").label(i.t("sb.save")).variant(ButtonVariant::Solid).on_click(cx.listener(|s, _, _, cx| s.save(cx)))),
                );
            col = col.child(form);
        }

        // ---- daftar ----
        match &self.views {
            Err(e) => col = col.child(div().text_color(t.colors.danger).child(e.clone())),
            Ok(v) if v.is_empty() => col = col.child(div().text_color(t.colors.fg_muted).child(i.t("pv.empty"))),
            Ok(v) => {
                for p in v.clone() {
                    let connected = self.catalog.iter().any(|c| c.id == p.id && c.connected);
                    let (pe, id, name) = (p.clone(), p.id.clone(), p.name.clone());
                    let confirming = self.confirm_delete.as_ref().is_some_and(|(x, _)| *x == p.id);
                    col = col.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(t.space(1))
                            .py(t.space(2))
                            .border_t_1()
                            .border_color(t.colors.border_soft)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(t.space(2))
                                    .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(p.name.clone()))
                                    .child(badge(i.t(match p.kind {
                                        Kind::Builtin => "pv.kind.builtin",
                                        Kind::Local => "pv.kind.local",
                                        Kind::Custom => "pv.kind.custom",
                                    })).tone(BadgeTone::Outline))
                                    .child(badge(i.t(if p.has_key { "pv.key.saved" } else { "pv.key.none" })).tone(if p.has_key { BadgeTone::Success } else { BadgeTone::Neutral }))
                                    .child(button(SharedString::from(format!("pv-e-{}", p.id))).label(i.t("pv.edit")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| s.open_form(Some(pe.clone()), cx))))
                                    .child(button(SharedString::from(format!("pv-d-{}", p.id))).label(i.t("pv.delete")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| {
                                        s.confirm_delete = Some((id.clone(), name.clone()));
                                        cx.notify();
                                    }))),
                            )
                            .child(div().small(&t).text_color(t.colors.fg_muted).child(format!(
                                "{} · {} {}",
                                i.t(if connected { "pv.status.connected" } else { "pv.status.notConnected" }),
                                p.models.len(),
                                i.t("pv.models").to_lowercase()
                            )))
                            .when(confirming, |d| {
                                let pid = p.id.clone();
                                d.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(t.space(2))
                                        .p(t.space(2))
                                        .rounded(t.rem(rem::RADIUS_MD))
                                        .bg(t.colors.raised)
                                        .child(div().font_weight(FontWeight::MEDIUM).child(i.tp("pv.confirmDeleteTitle", &[("name", &p.name)])))
                                        .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("pv.confirmDeleteBody")))
                                        .child(
                                            div()
                                                .flex()
                                                .gap(t.space(2))
                                                .child(button(SharedString::from(format!("pv-dc-{}", p.id))).label(i.t("ui.cancel")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| {
                                                    s.confirm_delete = None;
                                                    cx.notify();
                                                })))
                                                .child(button(SharedString::from(format!("pv-dy-{}", p.id))).label(i.t("pv.delete")).size(ControlSize::Sm).variant(ButtonVariant::Danger).on_click(cx.listener(move |s, _, _, cx| {
                                                    if let Err(e) = store().delete(&pid) {
                                                        eprintln!("[zandor] {e}");
                                                    }
                                                    s.confirm_delete = None;
                                                    s.views = store().views();
                                                    EngineStatus::global(cx).read(cx).restart();
                                                    cx.notify();
                                                }))),
                                        ),
                                )
                            }),
                    );
                }
            }
        }
        col
    }
}

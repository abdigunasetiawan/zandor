//! Provider & kunci API — port bagian koneksi ModelSettings + ConnectionPanel.tsx: daftar koneksi (providers.json +
//! keychain lewat providers.rs/secrets.rs bersama) dan formulir koneksi inline (Kembali + judul) dengan tab Kustom
//! (EndpointForm: ambil daftar model dari endpoint, metadata, filter, pilih semua, model manual), Lokal (deteksi server
//! lokal, preset Ollama/LM Studio), Katalog (cari provider engine, kunci API sederhana, akun) dan Masuk (SignInTab).
//! Setiap perubahan memulai ulang engine agar konfigurasi terbaca (sama dengan perintah Tauri).
use crate::core::engine::CatalogProvider;
use crate::core::projects::ProjectStore;
use crate::core::services::{app_data_dir, EngineStatus};
use crate::i18n::I18n;
use crate::netprobe::{LocalFound, ModelInfo};
use crate::providers::{Kind, Provider, ProviderModel, ProviderStore, ProviderView};
use crate::theme::{rem, Theme, H4};
use crate::ui::{badge, button, checkbox, icon, select, tabs, BadgeTone, ButtonVariant, ControlSize, IconSize, InputChanged, InputSubmit, TabItem, TextInput, Toasts, ToastInput, ToastTone, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn store() -> ProviderStore {
    ProviderStore::new(&app_data_dir().join("providers.json"), Box::new(crate::secrets::Keychain::new(crate::secrets::SERVICE)))
}

/// Formulir endpoint (kustom/lokal) — port EndpointForm.
struct Endpoint {
    kind: Kind,
    protocol: SharedString,
    id: Entity<TextInput>,
    name: Entity<TextInput>,
    url: Entity<TextInput>,
    key: Entity<TextInput>,
    filter: Entity<TextInput>,
    manual: Entity<TextInput>,
    known: Vec<String>,
    selected: BTreeSet<String>,
    info: BTreeMap<String, ModelInfo>,
    existing: Vec<ProviderModel>,
    probing: bool,
    probe_msg: Option<String>,
    saving: bool,
}

struct Form {
    /// Koneksi yang diubah (None = tambah baru).
    editing: Option<ProviderView>,
    tab: SharedString,
    endpoint: Endpoint,
    /// Tab Lokal: hasil deteksi (None = belum), sedang mendeteksi, formulir preset manual.
    found: Option<Vec<LocalFound>>,
    detecting: bool,
    local_endpoint: Option<Endpoint>,
    cat_query: Entity<TextInput>,
    picked: Option<String>,
    cat_key: Entity<TextInput>,
    accounts: HashMap<String, Entity<super::account_list::AccountList>>,
    signin: Option<Entity<super::signin_tab::SignInTab>>,
}

pub struct ProvidersSection {
    views: Result<Vec<ProviderView>, String>,
    catalog: Vec<CatalogProvider>,
    /// Metode auth per provider (GET /provider/auth) untuk isSimpleKeyProvider.
    methods: Value,
    catalog_loading: bool,
    form: Option<Form>,
    confirm_delete: Option<(String, String)>,
    /// Onboarding: hanya formulir (tanpa judul dan daftar), seperti ConnectionPanel.
    pub form_only: bool,
}

/// Provider katalog yang cukup satu kunci API sederhana (satu env var, tanpa prompt tambahan pada metode API).
fn is_simple_key(p: &CatalogProvider, methods: &Value) -> bool {
    if p.env.len() != 1 {
        return false;
    }
    let api_prompts = methods.get(&p.id).and_then(Value::as_array).into_iter().flatten().filter(|m| m["type"] == "api").any(|m| m["prompts"].as_array().is_some_and(|a| !a.is_empty()));
    !api_prompts
}

/// Port slugifyProviderId: id dari nama (huruf kecil, non-alfanumerik → "-", maks 36), unik terhadap `taken`.
fn slugify_provider_id(name: &str, taken: &[String]) -> String {
    let mut base = String::new();
    for c in name.to_lowercase().chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '-' };
        if c == '-' && base.ends_with('-') {
            continue;
        }
        base.push(c);
    }
    let mut base: String = base.trim_matches('-').chars().take(36).collect();
    if base.is_empty() {
        base = "provider".into();
    }
    if !taken.contains(&base) {
        return base;
    }
    (2..1000).map(|i| format!("{base}-{i}")).find(|c| !taken.contains(c)).unwrap_or(base)
}

fn taken_ids() -> Vec<String> {
    store().views().map(|v| v.into_iter().map(|p| p.id).collect()).unwrap_or_default()
}

fn toast(cx: &mut gpui::App, title: String, tone: ToastTone) {
    Toasts::show(ToastInput { tone, ..ToastInput::new(title) }, cx);
}

impl ProvidersSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&EngineStatus::global(cx), |s, _, cx| {
            s.load_catalog(cx);
            cx.notify();
        })
        .detach();
        let mut me = Self { views: store().views(), catalog: vec![], methods: Value::Null, catalog_loading: false, form: None, confirm_delete: None, form_only: false };
        me.load_catalog(cx);
        me
    }

    fn load_catalog(&mut self, cx: &mut Context<Self>) {
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        self.catalog_loading = true;
        let task = cx.background_spawn(async move { (api.list_catalog().unwrap_or_default(), api.list_auth_methods().unwrap_or(Value::Null)) });
        cx.spawn(async move |this, cx| {
            let (c, m) = task.await;
            this.update(cx, |s, cx| {
                s.catalog = c;
                s.methods = m;
                s.catalog_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn input(cx: &mut Context<Self>, ph: &str, v: &str) -> Entity<TextInput> {
        let (ph, v) = (ph.to_string(), v.to_string());
        let e = cx.new(|cx| {
            let mut t = TextInput::new(cx, ph);
            t.set_text(v, cx);
            t
        });
        cx.subscribe(&e, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        e
    }

    fn endpoint(cx: &mut Context<Self>, editing: Option<&ProviderView>, kind: Kind, name: &str, url: &str, models: &[String]) -> Endpoint {
        let i = cx.global::<I18n>().clone();
        let mut known: Vec<String> = editing.map(|p| p.models.iter().map(|m| m.id.clone()).collect()).unwrap_or_default();
        for m in models {
            if !known.contains(m) {
                known.push(m.clone());
            }
        }
        let selected: BTreeSet<String> = known.iter().cloned().collect();
        let key = Self::input(cx, "", "");
        key.update(cx, |k, _| k.masked = true);
        let manual = Self::input(cx, &i.t("pv.manualModelId"), "");
        cx.subscribe(&manual, |s, _, _: &InputSubmit, cx| s.add_manual(cx)).detach();
        Endpoint {
            kind,
            protocol: editing.and_then(|p| p.protocol.clone()).unwrap_or_else(|| "openai-compatible".into()).into(),
            id: Self::input(cx, "my-provider", &editing.map(|p| p.id.clone()).unwrap_or_default()),
            name: Self::input(cx, "", editing.map(|p| p.name.as_str()).unwrap_or(name)),
            url: Self::input(cx, "http://localhost:8045/v1", editing.and_then(|p| p.base_url.as_deref()).unwrap_or(url)),
            key,
            filter: Self::input(cx, &i.t("pv.modelFilter"), ""),
            manual,
            known,
            selected,
            info: BTreeMap::new(),
            existing: editing.map(|p| p.models.clone()).unwrap_or_default(),
            probing: false,
            probe_msg: None,
            saving: false,
        }
    }

    pub fn form_open(&self) -> bool {
        self.form.is_some()
    }

    pub fn open_form(&mut self, p: Option<ProviderView>, cx: &mut Context<Self>) {
        let kind = match p.as_ref().map(|p| p.kind) {
            Some(Kind::Local) => Kind::Local,
            _ => Kind::Custom,
        };
        let tab = if p.as_ref().is_some_and(|p| p.kind == Kind::Builtin) { "catalog" } else { "custom" };
        let endpoint = Self::endpoint(cx, p.as_ref().filter(|p| p.kind != Kind::Builtin), kind, "", "", &[]);
        let i = cx.global::<I18n>().clone();
        let cat_query = Self::input(cx, &i.t("pv.catalog.search"), "");
        let cat_key = Self::input(cx, "", "");
        cat_key.update(cx, |k, _| k.masked = true);
        cx.subscribe(&cat_key, |s, _, _: &InputSubmit, cx| s.connect_catalog(cx)).detach();
        self.form = Some(Form {
            picked: p.as_ref().filter(|p| p.kind == Kind::Builtin).map(|p| p.id.clone()),
            editing: p,
            tab: tab.into(),
            endpoint,
            found: None,
            detecting: false,
            local_endpoint: None,
            cat_query,
            cat_key,
            accounts: HashMap::new(),
            signin: None,
        });
        cx.notify();
    }

    fn close_form(&mut self, cx: &mut Context<Self>) {
        self.form = None;
        cx.notify();
    }

    /// Setelah tersimpan: muat ulang daftar, mulai ulang engine, tutup formulir.
    fn saved(&mut self, cx: &mut Context<Self>) {
        let t = cx.global::<I18n>().t("pv.saved");
        toast(cx, t, ToastTone::Success);
        self.form = None;
        self.views = store().views();
        EngineStatus::global(cx).read(cx).restart();
        cx.notify();
    }

    fn fail(cx: &mut gpui::App, e: impl std::fmt::Display) {
        let t = cx.global::<I18n>().tp("sb.error", &[("error", &e.to_string())]);
        toast(cx, t, ToastTone::Danger);
    }

    /// Endpoint yang sedang tampil: formulir preset lokal bila ada, selain itu formulir utama.
    fn ep(&mut self) -> Option<&mut Endpoint> {
        let f = self.form.as_mut()?;
        if f.tab.as_ref() == "local" {
            return f.local_endpoint.as_mut();
        }
        Some(&mut f.endpoint)
    }

    fn fetch_models(&mut self, cx: &mut Context<Self>) {
        let editing_id = self.form.as_ref().and_then(|f| f.editing.as_ref().map(|p| p.id.clone()));
        let Some(ep) = self.ep() else { return };
        let base = ep.url.read(cx).text().trim().to_string();
        let typed = ep.key.read(cx).text().trim().to_string();
        ep.probing = true;
        ep.probe_msg = None;
        let task = cx.background_spawn(async move {
            let key = match (Some(typed).filter(|k| !k.is_empty()), editing_id) {
                (Some(k), _) => Some(k),
                (None, Some(id)) => store().api_key(&id).ok().flatten(),
                _ => None,
            };
            crate::netprobe::probe(&base, key.as_deref(), std::time::Duration::from_secs(8))
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                let i = cx.global::<I18n>().clone();
                if let Some(ep) = s.ep() {
                    ep.probing = false;
                    if !r.ok {
                        ep.probe_msg = Some(i.tp("pv.probeFailed", &[("error", r.error.as_deref().unwrap_or("?"))]));
                    } else {
                        for m in &r.models {
                            if !ep.known.contains(m) {
                                ep.known.push(m.clone());
                            }
                        }
                        ep.known.sort();
                        ep.info.extend(r.info.clone());
                        ep.probe_msg = Some(i.tp("pv.modelsFound", &[("count", &r.models.len().to_string())]));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn add_manual(&mut self, cx: &mut Context<Self>) {
        let Some(ep) = self.ep() else { return };
        let id = ep.manual.read(cx).text().trim().to_string();
        if id.is_empty() {
            return;
        }
        if !ep.known.contains(&id) {
            ep.known.push(id.clone());
            ep.known.sort();
        }
        ep.selected.insert(id);
        let m = ep.manual.clone();
        m.update(cx, |m, cx| m.set_text("", cx));
        cx.notify();
    }

    fn save_endpoint(&mut self, cx: &mut Context<Self>) {
        let editing_id = self.form.as_ref().and_then(|f| f.editing.as_ref().map(|p| p.id.clone()));
        let Some(ep) = self.ep() else { return };
        let txt = |e: &Entity<TextInput>| e.read(cx).text().trim().to_string();
        let key = txt(&ep.key);
        let custom_id = txt(&ep.id).to_lowercase();
        let models: Vec<ProviderModel> = ep
            .selected
            .iter()
            .map(|id| {
                ep.existing.iter().find(|m| &m.id == id).cloned().unwrap_or_else(|| {
                    let inf = ep.info.get(id);
                    ProviderModel {
                        id: id.clone(),
                        name: inf.and_then(|x| x.name.clone()),
                        context: inf.and_then(|x| x.context),
                        output: inf.and_then(|x| x.output),
                        reasoning: inf.and_then(|x| x.reasoning),
                        tool_call: inf.and_then(|x| x.tool_call),
                        attachment: inf.and_then(|x| x.attachment),
                    }
                })
            })
            .collect();
        let name = txt(&ep.name);
        if name.is_empty() {
            return Self::fail(cx, "nama koneksi wajib diisi");
        }
        if models.is_empty() {
            return Self::fail(cx, "pilih atau tambahkan minimal satu model");
        }
        let id = editing_id.unwrap_or_else(|| if custom_id.is_empty() { slugify_provider_id(&name, &taken_ids()) } else { custom_id });
        let provider = Provider {
            id,
            name,
            kind: ep.kind,
            base_url: Some(txt(&ep.url).trim_end_matches('/').to_string()).filter(|u| !u.is_empty()),
            protocol: Some(ep.protocol.to_string()).filter(|p| p != "openai-compatible"),
            models,
            ..Default::default()
        };
        let r = provider.validate().and_then(|_| if key.is_empty() { Ok(()) } else { crate::providers::validate_api_key(&key) }).and_then(|_| store().save(provider, (!key.is_empty()).then_some(key)));
        match r {
            Ok(()) => self.saved(cx),
            Err(e) => Self::fail(cx, e),
        }
    }

    fn detect(&mut self, cx: &mut Context<Self>) {
        if let Some(f) = self.form.as_mut() {
            f.detecting = true;
        }
        let task = cx.background_spawn(async move { crate::netprobe::detect_local() });
        cx.spawn(async move |this, cx| {
            let found = task.await;
            this.update(cx, |s, cx| {
                if let Some(f) = s.form.as_mut() {
                    f.detecting = false;
                    f.found = Some(found);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn add_local(&mut self, l: LocalFound, cx: &mut Context<Self>) {
        let p = Provider {
            id: slugify_provider_id(&l.name, &taken_ids()),
            name: l.name.clone(),
            kind: Kind::Local,
            base_url: Some(l.base_url.clone()),
            models: l.models.iter().map(|id| ProviderModel { id: id.clone(), ..Default::default() }).collect(),
            ..Default::default()
        };
        match p.validate().and_then(|_| store().save(p, None)) {
            Ok(()) => self.saved(cx),
            Err(e) => Self::fail(cx, e),
        }
    }

    fn connect_catalog(&mut self, cx: &mut Context<Self>) {
        let Some(f) = self.form.as_ref() else { return };
        let Some(id) = f.picked.clone() else { return };
        let key = f.cat_key.read(cx).text().trim().to_string();
        if key.is_empty() {
            return;
        }
        let name = self.catalog.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_else(|| id.clone());
        let p = Provider { id, name, kind: Kind::Builtin, ..Default::default() };
        match crate::providers::validate_api_key(&key).and_then(|_| store().save(p, Some(key))) {
            Ok(()) => self.saved(cx),
            Err(e) => Self::fail(cx, e),
        }
    }

    fn set_tab(&mut self, k: SharedString, cx: &mut Context<Self>) {
        if k.as_ref() == "signin" && self.form.as_ref().is_some_and(|f| f.signin.is_none()) {
            let e = cx.new(super::signin_tab::SignInTab::new);
            cx.subscribe(&e, |s, _, _: &super::signin_tab::SignedIn, cx| {
                s.views = store().views();
                s.load_catalog(cx);
                cx.notify();
            })
            .detach();
            if let Some(f) = self.form.as_mut() {
                f.signin = Some(e);
            }
        }
        if let Some(f) = self.form.as_mut() {
            f.tab = k;
        }
        cx.notify();
    }

    fn caps(t: &Theme, i: &I18n, m: &ModelInfo) -> AnyElement {
        let mut row = div().flex().flex_wrap().gap(t.space(1));
        if let Some(c) = m.context {
            row = row.child(badge(i.tp("ms.cap.context", &[("n", &super::models_section::compact_tokens(c))])).tone(BadgeTone::Outline));
        }
        if let Some(o) = m.output {
            row = row.child(badge(i.tp("ms.cap.output", &[("n", &super::models_section::compact_tokens(o))])).tone(BadgeTone::Outline));
        }
        if m.reasoning == Some(true) {
            row = row.child(badge(i.t("ms.cap.reasoning")).tone(BadgeTone::Success));
        }
        if m.tool_call == Some(true) {
            row = row.child(badge(i.t("ms.cap.tools")));
        }
        if m.attachment == Some(true) {
            row = row.child(badge(i.t("ms.cap.attachment")));
        }
        row.into_any_element()
    }

    fn endpoint_form(&self, t: &Theme, i: &I18n, ep: &Endpoint, editing: Option<&ProviderView>, cx: &mut Context<Self>) -> AnyElement {
        let field = |label: String, desc: Option<String>, el: AnyElement| {
            div()
                .flex()
                .flex_col()
                .gap(t.space(1))
                .child(div().small(t).font_weight(FontWeight::MEDIUM).child(label))
                .child(el)
                .children(desc.map(|d| div().small(t).text_color(t.colors.fg_subtle).child(d)))
        };
        let me = cx.entity().downgrade();
        let protocols: Vec<(SharedString, SharedString)> = crate::providers::PROTOCOLS.iter().map(|(p, _)| ((*p).into(), i.t(&format!("pv.protocol.{p}")).into())).collect();
        let q = ep.filter.read(cx).text().trim().to_lowercase();
        let shown: Vec<String> = ep.known.iter().filter(|m| m.to_lowercase().contains(&q)).cloned().collect();
        let mut form = div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(field(i.t("pv.name"), None, ep.name.clone().into_any_element()))
            .child(field(i.t("pv.id"), Some(i.t(if editing.is_some() { "pv.idLocked" } else { "pv.idHelp" })), div().when(editing.is_some(), |d| d.opacity(0.6)).child(ep.id.clone()).into_any_element()))
            .child(field(
                i.t("pv.protocol"),
                Some(i.t("pv.protocolHelp")),
                select("pv-proto", protocols, ep.protocol.clone())
                    .on_change(move |v, _, cx| {
                        me.update(cx, |s, cx| {
                            if let Some(ep) = s.ep() {
                                ep.protocol = v;
                            }
                            cx.notify();
                        })
                        .ok();
                    })
                    .into_any_element(),
            ))
            .child(field(i.t("pv.baseUrl"), None, ep.url.clone().into_any_element()))
            .child(field(
                i.t("pv.apiKey"),
                if editing.is_some_and(|p| p.has_key) {
                    Some(i.t("pv.apiKeyKeep"))
                } else if ep.kind == Kind::Local {
                    Some(i.t("pv.apiKeyOptional"))
                } else {
                    None
                },
                ep.key.clone().into_any_element(),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(button("pv-fetch").label(i.t(if ep.probing { "pv.fetching" } else { "pv.fetchModels" })).disabled(ep.probing || ep.url.read(cx).text().trim().is_empty()).on_click(cx.listener(|s, _, _, cx| s.fetch_models(cx))))
                    .children(ep.probe_msg.clone().map(|m| div().small(t).child(m))),
            );
        if ep.known.len() > 8 {
            form = form.child(ep.filter.clone());
        }
        if !ep.known.is_empty() {
            let (s1, s2) = (shown.clone(), shown.clone());
            form = form.child(
                div()
                    .flex()
                    .gap(t.space(2))
                    .child(button("pv-all").size(ControlSize::Sm).label(i.t("ms.selectAll")).disabled(shown.is_empty()).on_click(cx.listener(move |s, _, _, cx| {
                        if let Some(ep) = s.ep() {
                            ep.selected.extend(s1.iter().cloned());
                        }
                        cx.notify();
                    })))
                    .child(button("pv-none").size(ControlSize::Sm).label(i.t("ms.deselectAll")).disabled(shown.is_empty()).on_click(cx.listener(move |s, _, _, cx| {
                        if let Some(ep) = s.ep() {
                            for m in &s2 {
                                ep.selected.remove(m);
                            }
                        }
                        cx.notify();
                    }))),
            );
            let mut list = div().id("pv-models").flex().flex_col().gap(t.space(1)).max_h(t.rem(18.)).overflow_y_scroll();
            for m in shown {
                let inf = ep.info.get(&m).cloned().or_else(|| {
                    ep.existing.iter().find(|x| x.id == m).map(|x| ModelInfo { name: x.name.clone(), context: x.context, output: x.output, reasoning: x.reasoning, tool_call: x.tool_call, attachment: x.attachment })
                });
                let display = ep.info.get(&m).and_then(|x| x.name.clone());
                let mm = m.clone();
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .child(checkbox(SharedString::from(format!("pvm-{m}")), "", ep.selected.contains(&m)).on_change({
                            let me = cx.entity().downgrade();
                            move |on, _, cx| {
                                let mm = mm.clone();
                                me.update(cx, |s, cx| {
                                    if let Some(ep) = s.ep() {
                                        if on {
                                            ep.selected.insert(mm);
                                        } else {
                                            ep.selected.remove(&mm);
                                        }
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .child(div().truncate().child(display.clone().unwrap_or_else(|| m.clone())))
                                .when(display.is_some(), |d| d.child(div().small(t).text_color(t.colors.fg_subtle).truncate().child(m.clone()))),
                        )
                        .child(div().flex_1())
                        .children(inf.map(|x| Self::caps(t, i, &x))),
                );
            }
            form = form.child(list);
        }
        form.child(
            div()
                .flex()
                .gap(t.space(2))
                .child(div().flex_1().child(ep.manual.clone()))
                .child(button("pv-addm").label(i.t("pv.addModel")).disabled(ep.manual.read(cx).text().trim().is_empty()).on_click(cx.listener(|s, _, _, cx| s.add_manual(cx)))),
        )
        .child(div().small(t).child(i.tp("pv.selected", &[("count", &ep.selected.len().to_string())])))
        .child(div().flex().justify_end().child(button("pv-save").variant(ButtonVariant::Solid).label(i.t("pv.save")).disabled(ep.saving).on_click(cx.listener(|s, _, _, cx| s.save_endpoint(cx)))))
        .into_any_element()
    }

    fn local_tab(&self, t: &Theme, i: &I18n, f: &Form, cx: &mut Context<Self>) -> AnyElement {
        if let Some(ep) = &f.local_endpoint {
            return self.endpoint_form(t, i, ep, None, cx);
        }
        let mut col = div().flex().flex_col().gap(t.space(3)).child(div().child(button("pv-detect").variant(ButtonVariant::Solid).label(i.t(if f.detecting { "pv.local.detecting" } else { "pv.local.detect" })).disabled(f.detecting).on_click(cx.listener(|s, _, _, cx| s.detect(cx)))));
        if f.found.as_ref().is_some_and(|v| v.is_empty()) {
            col = col.child(div().small(t).child(i.t("pv.local.none")));
        }
        for l in f.found.clone().unwrap_or_default() {
            let l2 = l.clone();
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().flex_1().child(i.tp("pv.local.found", &[("name", &l.name), ("count", &l.models.len().to_string())])))
                    .child(button(SharedString::from(format!("pv-ladd-{}", l.id))).size(ControlSize::Sm).variant(ButtonVariant::Solid).label(i.tp("pv.local.add", &[("name", &l.name)])).on_click(cx.listener(move |s, _, _, cx| s.add_local(l2.clone(), cx)))),
            );
        }
        let mut presets = div().flex().gap(t.space(2));
        for (id, name, url) in [("ollama", "Ollama", "http://localhost:11434/v1"), ("lmstudio", "LM Studio", "http://localhost:1234/v1")] {
            presets = presets.child(button(SharedString::from(format!("pv-pre-{id}"))).size(ControlSize::Sm).label(name).on_click(cx.listener(move |s, _, _, cx| {
                let ep = Self::endpoint(cx, None, Kind::Local, name, url, &[]);
                if let Some(f) = s.form.as_mut() {
                    f.local_endpoint = Some(ep);
                }
                cx.notify();
            })));
        }
        col.child(presets).into_any_element()
    }

    fn catalog_tab(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let engine_ready = ProjectStore::global(cx).read(cx).engine_ready;
        if !engine_ready {
            return div().small(t).child(i.t("pv.catalog.engineDown")).into_any_element();
        }
        if self.catalog_loading && self.catalog.is_empty() {
            return div().small(t).child(i.t("pv.catalog.loading")).into_any_element();
        }
        let Some(f) = self.form.as_ref() else { return div().into_any_element() };
        let editing = f.editing.clone();
        let q = f.cat_query.read(cx).text().trim().to_lowercase();
        let list: Vec<CatalogProvider> = self
            .catalog
            .iter()
            .filter(|p| editing.as_ref().is_none_or(|e| e.id == p.id))
            .filter(|p| q.is_empty() || p.name.to_lowercase().contains(&q) || p.id.contains(&q))
            .take(80)
            .cloned()
            .collect();
        let views = self.views.clone().unwrap_or_default();
        let picked = f.picked.clone();
        let mut col = div().flex().flex_col().gap(t.space(2));
        if editing.is_none() {
            col = col.child(f.cat_query.clone());
        }
        if list.is_empty() {
            col = col.child(div().small(t).child(i.t("pv.catalog.none")));
        }
        let cat_key = f.cat_key.clone();
        let mut rows = div().id("pv-cat-list").flex().flex_col().max_h(t.rem(26.)).overflow_y_scroll();
        for p in list {
            let simple = is_simple_key(&p, &self.methods);
            let open = picked.as_deref() == Some(p.id.as_str());
            let saved = views.iter().find(|x| x.id == p.id && x.kind == Kind::Builtin).cloned();
            let has_accounts = saved.as_ref().is_some_and(|s| !s.accounts.is_empty());
            let pid = p.id.clone();
            let mut line = div()
                .flex()
                .items_center()
                .gap(t.space(2))
                .child(div().font_weight(FontWeight::MEDIUM).child(p.name.clone()))
                .child(div().small(t).text_color(t.colors.fg_muted).child(i.tp("pv.catalog.models", &[("count", &p.model_count.to_string())])))
                .when(p.connected, |d| d.child(badge(i.t("pv.status.connected")).tone(BadgeTone::Success)))
                .child(div().flex_1());
            if simple && !has_accounts {
                line = line.child(button(SharedString::from(format!("pv-con-{}", p.id))).size(ControlSize::Sm).label(format!("{} {}", i.t("pv.catalog.connect"), p.name)).pressed(open).on_click(cx.listener(move |s, _, _, cx| {
                    if let Some(f) = s.form.as_mut() {
                        f.picked = if f.picked.as_deref() == Some(pid.as_str()) { None } else { Some(pid.clone()) };
                    }
                    cx.notify();
                })));
            } else if !simple {
                line = line.child(div().small(t).text_color(t.colors.fg_muted).child(i.t("pv.catalog.needsMore")));
            }
            let mut row = div().flex().flex_col().gap(t.space(2)).py(t.space(2)).border_b_1().border_color(t.colors.border_soft).child(line);
            if let (Some(sv), true) = (&saved, has_accounts) {
                let acc = if let Some(a) = self.form.as_ref().and_then(|f| f.accounts.get(&sv.id)).cloned() {
                    a
                } else {
                    let (id, name) = (sv.id.clone(), p.name.clone());
                    let a = cx.new(|cx| super::account_list::AccountList::new(id.clone(), name, cx));
                    if let Some(f) = self.form.as_mut() {
                        f.accounts.insert(id, a.clone());
                    }
                    a
                };
                row = row.child(acc);
            }
            if open && simple && !has_accounts {
                let env = p.env.first().cloned().unwrap_or_default();
                cat_key.update(cx, |k, _| k.set_placeholder(env));
                row = row.child(
                    div()
                        .flex()
                        .gap(t.space(2))
                        .child(div().flex_1().child(cat_key.clone()))
                        .child(button("pv-cat-save").variant(ButtonVariant::Solid).label(i.t("pv.save")).disabled(cat_key.read(cx).text().trim().is_empty()).on_click(cx.listener(|s, _, _, cx| s.connect_catalog(cx)))),
                );
            }
            rows = rows.child(row);
        }
        col.child(rows).into_any_element()
    }
}

impl Render for ProvidersSection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let mut col = div().flex().flex_col().gap(t.space(2));

        // ---- formulir koneksi inline (menggantikan daftar sampai selesai/kembali) ----
        if self.form.is_some() {
            let (editing, tab) = {
                let f = self.form.as_ref().unwrap();
                (f.editing.clone(), f.tab.clone())
            };
            let title = i.t(if editing.is_some() { "pv.dialog.editTitle" } else { "pv.dialog.title" });
            let head = div()
                .flex()
                .items_center()
                .gap(t.space(3))
                .child(button("pv-back").child(icon("arrow_left").size(IconSize::Sm)).label(i.t("ms.back")).on_click(cx.listener(|s, _, _, cx| s.close_form(cx))))
                .child(div().typo(&t, H4).child(title));
            let body: AnyElement = match editing.as_ref().filter(|p| p.kind != Kind::Builtin) {
                Some(p) => {
                    let me: &Self = self;
                    me.endpoint_form(&t, &i, &me.form.as_ref().unwrap().endpoint, Some(p), cx)
                }
                None => {
                    let content: AnyElement = match tab.as_ref() {
                        "local" => {
                            let me: &Self = self;
                            me.local_tab(&t, &i, me.form.as_ref().unwrap(), cx)
                        }
                        "catalog" => self.catalog_tab(&t, &i, cx),
                        "signin" => div().children(self.form.as_ref().and_then(|f| f.signin.clone())).into_any_element(),
                        _ => {
                            let me: &Self = self;
                            me.endpoint_form(&t, &i, &me.form.as_ref().unwrap().endpoint, None, cx)
                        }
                    };
                    let me = cx.entity().downgrade();
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.space(3))
                        .child(
                            tabs("pv-tabs", ["custom", "local", "catalog", "signin"].iter().map(|k| TabItem { value: (*k).into(), label: i.t(&format!("pv.tab.{k}")).into(), icon: None, disabled: false }).collect(), tab.clone()).on_change(move |v, _, cx| {
                                me.update(cx, |s, cx| s.set_tab(v, cx)).ok();
                            }),
                        )
                        .child(content)
                        .into_any_element()
                }
            };
            col = col.child(div().flex().flex_col().gap(t.space(3)).child(head).child(body));
            return col;
        }

        if !self.form_only {
            col = col.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("ms.providers"))).child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ms.providersBody"))))
                    .child(button("pv-add").label(i.t("pv.add")).size(ControlSize::Sm).variant(ButtonVariant::Solid).on_click(cx.listener(|s, _, _, cx| s.open_form(None, cx)))),
            );
            col = col.child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t("pv.secure.yes")));
        }

        // ---- daftar ----
        if self.form_only {
            return col;
        }
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

#[cfg(test)]
mod tests {
    #[test]
    fn slug() {
        assert_eq!(super::slugify_provider_id("LM Studio", &[]), "lm-studio");
        assert_eq!(super::slugify_provider_id("  !! ", &[]), "provider");
        assert_eq!(super::slugify_provider_id("Ollama", &["ollama".into()]), "ollama-2");
    }
}

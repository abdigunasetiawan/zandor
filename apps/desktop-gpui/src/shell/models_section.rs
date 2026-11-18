//! Pengaturan → Provider & model — port ModelSettings.tsx (DefaultsSection, SmallModelRow, daftar provider dengan
//! jumlah model tampil, ModelManager, ModelEditDialog) dan LogoDialog.tsx. Tambah/ubah/hapus koneksi tetap di
//! `providers_section` (ConnectionPanel).
use crate::core::engine::{CatalogProvider, ModelOption};
use crate::core::model_logo::detect_logo_key;
use crate::core::model_prefs::{is_valid_logo, ModelPrefsStore, Ref, MAX_LOGO_CHARS};
use crate::core::pings::{ping_label, PingState, PingStore};
use crate::core::projects::ProjectStore;
use crate::core::services::{app_data_dir, EngineStatus};
use crate::i18n::I18n;
use crate::providers::{Kind, ProviderModel, ProviderStore, ProviderView};
use crate::shell::widgets::model_logo;
use crate::theme::{rem, Theme};
use crate::ui::{badge, button, icon, icon_button, select, switch, BadgeTone, ButtonVariant, ControlSize, IconSize, InputChanged, TextInput, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, Context, Entity, FontWeight, IntoElement, SharedString, Window};

const AUTO: &str = "";

fn store() -> ProviderStore {
    ProviderStore::new(&app_data_dir().join("providers.json"), Box::new(crate::secrets::Keychain::new(crate::secrets::SERVICE)))
}

/// Port compactTokens: 128000 → "128K", 1_500_000 → "1.5M".
pub fn compact_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        let v = (n as f64 / 1_000_000.0 * 10.0).round() / 10.0;
        return if v.fract() == 0.0 { format!("{}M", v as u64) } else { format!("{v}M") };
    }
    if n >= 1000 {
        return format!("{}K", (n as f64 / 1000.0).round() as u64);
    }
    n.to_string()
}

pub fn capability_group(m: &ModelOption) -> &'static str {
    if m.reasoning == Some(true) {
        "reasoning"
    } else if m.attachment == Some(true) {
        "vision"
    } else if m.tool_call == Some(true) {
        "tools"
    } else {
        "other"
    }
}

fn key_of(m: &ModelOption) -> String {
    format!("{}/{}", m.provider_id, m.model_id)
}
fn ref_of(m: &ModelOption) -> Ref {
    Ref { provider_id: m.provider_id.clone(), model_id: m.model_id.clone() }
}

#[derive(Clone)]
struct ProviderRow {
    id: String,
    name: String,
    spec: Option<ProviderView>,
    models: Vec<ModelOption>,
}

/// Port useProviderRows: provider tersimpan + katalog yang tersambung + provider dari daftar model engine.
fn provider_rows(views: &[ProviderView], catalog: &[CatalogProvider], models: &[ModelOption]) -> Vec<ProviderRow> {
    let mut rows: Vec<ProviderRow> = views.iter().map(|p| ProviderRow { id: p.id.clone(), name: p.name.clone(), spec: Some(p.clone()), models: vec![] }).collect();
    for c in catalog.iter().filter(|c| c.connected) {
        if !rows.iter().any(|r| r.id == c.id) {
            rows.push(ProviderRow { id: c.id.clone(), name: c.name.clone(), spec: None, models: vec![] });
        }
    }
    for m in models {
        match rows.iter_mut().find(|r| r.id == m.provider_id) {
            Some(r) => r.models.push(m.clone()),
            None => rows.push(ProviderRow { id: m.provider_id.clone(), name: m.provider_name.clone(), spec: None, models: vec![m.clone()] }),
        }
    }
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    rows
}

/// Model engine atau, bila engine belum memuatnya, daftar tersimpan.
fn models_of(row: &ProviderRow) -> Vec<ModelOption> {
    if !row.models.is_empty() {
        return row.models.clone();
    }
    row.spec
        .iter()
        .flat_map(|s| s.models.iter())
        .map(|m| ModelOption {
            provider_id: row.id.clone(),
            provider_name: row.name.clone(),
            model_id: m.id.clone(),
            name: m.name.clone().unwrap_or_else(|| m.id.clone()),
            variants: vec![],
            context_limit: m.context,
            output_limit: m.output,
            reasoning: m.reasoning,
            tool_call: m.tool_call,
            attachment: m.attachment,
            inputs: vec![],
        })
        .collect()
}

struct EditForm {
    model_id: String,
    name: Entity<TextInput>,
    context: Entity<TextInput>,
    output: Entity<TextInput>,
    reasoning: bool,
    tool_call: bool,
    attachment: bool,
}

struct LogoEdit {
    model_id: Option<String>,
    draft: Option<String>,
    custom: Option<String>,
    error: Option<String>,
}

pub struct ModelsSection {
    models: Vec<ModelOption>,
    catalog: Vec<CatalogProvider>,
    views: Vec<ProviderView>,
    small: Option<Option<String>>,
    small_pending: bool,
    small_error: Option<String>,
    managing: Option<String>,
    search: Entity<TextInput>,
    group_by: String,
    fetching: bool,
    note: Option<String>,
    editing: Option<EditForm>,
    logo: Option<LogoEdit>,
}

impl ModelsSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.observe(&EngineStatus::global(cx), |s, _, cx| {
            s.reload(cx);
        })
        .detach();
        if let Some(p) = ModelPrefsStore::global(cx) {
            cx.observe(&p, |_, _, cx| cx.notify()).detach();
        }
        cx.observe(&PingStore::global(cx), |_, _, cx| cx.notify()).detach();
        let ph = cx.global::<I18n>().t("mp.search");
        let search = cx.new(|cx| TextInput::new(cx, ph));
        cx.subscribe(&search, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        let small = crate::engine::read_small_model(&app_data_dir().join("model-roles.json"));
        let mut me = Self {
            models: vec![],
            catalog: vec![],
            views: store().views().unwrap_or_default(),
            small: Some(small),
            small_pending: false,
            small_error: None,
            managing: None,
            search,
            group_by: "none".into(),
            fetching: false,
            note: None,
            editing: None,
            logo: None,
        };
        me.reload(cx);
        me
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.views = store().views().unwrap_or_default();
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        let task = cx.background_spawn(async move { (api.list_models().unwrap_or_default(), api.list_catalog().unwrap_or_default()) });
        cx.spawn(async move |this, cx| {
            let (m, c) = task.await;
            this.update(cx, |s, cx| {
                s.models = m;
                s.catalog = c;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_small(&mut self, v: Option<String>, cx: &mut Context<Self>) {
        self.small_error = None;
        if let Some(s) = v.as_deref() {
            if !crate::engine::valid_model_ref(s) {
                self.small_error = Some(format!("rujukan model tidak valid: {s}"));
                cx.notify();
                return;
            }
        }
        let dir = app_data_dir();
        let r = std::fs::create_dir_all(&dir).and_then(|_| {
            let tmp = dir.join("model-roles.json.tmp");
            std::fs::write(&tmp, serde_json::json!({ "small": v }).to_string())?;
            std::fs::rename(&tmp, dir.join("model-roles.json"))
        });
        match r {
            Ok(()) => {
                self.small = Some(v);
                self.small_pending = true;
            }
            Err(e) => self.small_error = Some(e.to_string()),
        }
        cx.notify();
    }

    fn save_models(&mut self, provider: &str, models: Vec<ProviderModel>, cx: &mut Context<Self>) -> Result<(), String> {
        let st = store();
        let (all, _) = st.load();
        let mut p = all.into_iter().find(|p| p.id == provider).ok_or_else(|| "provider tidak ditemukan".to_string())?;
        p.models = models;
        st.save(p, None)?;
        self.views = st.views().unwrap_or_default();
        EngineStatus::global(cx).read(cx).restart();
        Ok(())
    }

    fn fetch_models(&mut self, spec: ProviderView, cx: &mut Context<Self>) {
        let Some(base) = spec.base_url.clone() else { return };
        self.fetching = true;
        self.note = None;
        cx.notify();
        let id = spec.id.clone();
        let task = cx.background_spawn(async move {
            let key = store().api_key(&id).ok().flatten();
            crate::netprobe::probe(&base, key.as_deref(), std::time::Duration::from_secs(20))
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.fetching = false;
                let i = cx.global::<I18n>().clone();
                if !r.ok {
                    s.note = Some(i.tp("pv.probeFailed", &[("error", r.error.as_deref().unwrap_or("?"))]));
                } else {
                    let added: Vec<String> = r.models.iter().filter(|m| !spec.models.iter().any(|x| x.id == **m)).cloned().collect();
                    let mut note = Some(i.tp("ms.fetched", &[("found", &r.models.len().to_string()), ("added", &added.len().to_string())]));
                    if !added.is_empty() {
                        let mut list = spec.models.clone();
                        list.extend(added.into_iter().map(|id| ProviderModel { id, ..Default::default() }));
                        if let Err(e) = s.save_models(&spec.id, list, cx) {
                            note = Some(e);
                        }
                    }
                    s.note = note;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_edit(&mut self, m: &ProviderModel, cx: &mut Context<Self>) {
        let mk = |cx: &mut Context<Self>, ph: &str, v: String| {
            cx.new(|cx| {
                let mut ti = TextInput::new(cx, ph.to_string());
                ti.set_text(v, cx);
                ti
            })
        };
        self.editing = Some(EditForm {
            model_id: m.id.clone(),
            name: mk(cx, &m.id, m.name.clone().unwrap_or_default()),
            context: mk(cx, "128000", m.context.map(|c| c.to_string()).unwrap_or_default()),
            output: mk(cx, "16384", m.output.map(|c| c.to_string()).unwrap_or_default()),
            reasoning: m.reasoning.unwrap_or(false),
            tool_call: m.tool_call.unwrap_or(true),
            attachment: m.attachment.unwrap_or(false),
        });
        cx.notify();
    }

    fn save_edit(&mut self, spec: &ProviderView, cx: &mut Context<Self>) {
        let Some(f) = self.editing.as_ref() else { return };
        let num = |e: &Entity<TextInput>| e.read(cx).text().trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0).map(|n| n.round() as u64);
        let name = f.name.read(cx).text().trim().to_string();
        let next = ProviderModel {
            id: f.model_id.clone(),
            name: (!name.is_empty()).then_some(name),
            context: num(&f.context),
            output: num(&f.output),
            reasoning: Some(f.reasoning),
            tool_call: Some(f.tool_call),
            attachment: Some(f.attachment),
        };
        let list = spec.models.iter().map(|m| if m.id == next.id { next.clone() } else { m.clone() }).collect();
        let i = cx.global::<I18n>().clone();
        match self.save_models(&spec.id, list, cx) {
            Ok(()) => {
                self.editing = None;
                toast(cx, i.t("pv.saved"), crate::ui::ToastTone::Success, Some(2000));
            }
            Err(e) => toast(cx, i.tp("sb.error", &[("error", &e)]), crate::ui::ToastTone::Danger, None),
        }
        cx.notify();
    }

    fn pick_logo(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(path) = paths.into_iter().next() {
                    let r = file_to_logo(&path);
                    this.update(cx, |s, cx| {
                        let i = cx.global::<I18n>().clone();
                        if let Some(l) = s.logo.as_mut() {
                            match r {
                                Ok(u) => {
                                    l.draft = Some(u);
                                    l.error = None;
                                }
                                Err(k) => l.error = Some(i.t(&format!("logo.err.{k}"))),
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

fn toast(cx: &mut App, title: String, tone: crate::ui::ToastTone, ms: Option<u64>) {
    crate::ui::Toasts::show(crate::ui::ToastInput { title: title.into(), description: None, tone, timeout_ms: ms, action: None }, cx);
}

/// Port fileToLogo: SVG apa adanya; raster dikecilkan maks 128 px ke PNG. Galat: "type" | "size" | "decode".
pub fn file_to_logo(path: &std::path::Path) -> Result<String, &'static str> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "svg" {
        let bytes = std::fs::read(path).map_err(|_| "decode")?;
        let url = format!("data:image/svg+xml;base64,{}", crate::core::gradient::b64_encode(&bytes));
        return if is_valid_logo(&url) { Ok(url) } else if url.len() > MAX_LOGO_CHARS { Err("size") } else { Err("decode") };
    }
    if !["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
        return Err("type");
    }
    let url = crate::core::gradient::image_to_data_url(path, 128, crate::core::gradient::OutFmt::Png, usize::MAX).map_err(|_| "decode")?;
    if is_valid_logo(&url) { Ok(url) } else { Err("size") }
}

impl ModelsSection {
    fn defaults(&self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let prefs = ModelPrefsStore::global(cx).map(|p| p.read(cx).prefs.clone()).unwrap_or_default();
        let mut items: Vec<(SharedString, SharedString)> = vec![(AUTO.into(), i.t("ms.auto").into())];
        items.extend(self.models.iter().map(|m| (key_of(m).into(), format!("{} · {}", m.name, m.provider_name).into())));
        let known = |r: &Option<Ref>| r.as_ref().map(Ref::key).filter(|k| self.models.iter().any(|m| key_of(m) == *k)).unwrap_or_default();
        let models = self.models.clone();
        let models2 = self.models.clone();
        let ref_by = move |list: &[ModelOption], v: &str| list.iter().find(|m| key_of(m) == v).map(ref_of);
        let mut small_items: Vec<(SharedString, SharedString)> = vec![(AUTO.into(), i.t("ms.smallAuto").into())];
        small_items.extend(self.models.iter().map(|m| (key_of(m).into(), format!("{} · {}", m.name, m.provider_name).into())));
        let small_val = self.small.clone().flatten();
        if let Some(v) = &small_val {
            if !small_items.iter().any(|(k, _)| k.as_ref() == v) {
                small_items.push((v.clone().into(), i.tp("ms.smallMissing", &[("id", v)]).into()));
            }
        }
        let me = cx.entity().downgrade();
        let row = |title: String, body: String, ctl: AnyElement| {
            div()
                .flex()
                .items_center()
                .gap(t.space(4))
                .py(t.space(2))
                .child(div().flex().flex_col().flex_1().min_w_0().child(div().font_weight(FontWeight::MEDIUM).child(title)).child(div().small(t).text_color(t.colors.fg_muted).child(body)))
                .child(div().flex_none().max_w(t.rem(rem::SPACE[7] * 6.)).child(ctl))
        };
        div()
            .flex()
            .flex_col()
            .child(div().typo(t, crate::theme::H4).mb(t.space(2)).child(i.t("ms.defaults")))
            .child(row(
                i.t("ms.defaultChat"),
                i.t("ms.defaultChatBody"),
                select("ms-def-chat", items.clone(), known(&prefs.default_chat))
                    .on_change(move |v, _, cx| {
                        let r = ref_by(&models, v.as_ref());
                        if let Some(p) = ModelPrefsStore::global(cx) {
                            p.update(cx, |p, cx| p.set_default_chat(r, cx));
                        }
                    })
                    .into_any_element(),
            ))
            .child(
                row(
                    i.t("ms.small"),
                    i.t("ms.smallBody"),
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(t.space(1))
                        .child(select("ms-small", small_items, small_val.unwrap_or_default()).disabled(self.small.is_none()).on_change(move |v, _, cx| {
                            me.update(cx, |s, cx| s.set_small((!v.is_empty()).then(|| v.to_string()), cx)).ok();
                        }))
                        .when(self.small_pending, |d| {
                            d.child(button("ms-small-restart").size(ControlSize::Sm).label(i.t("ms.smallRestart")).on_click(cx.listener(|s, _, _, cx| {
                                s.small_pending = false;
                                EngineStatus::global(cx).read(cx).restart();
                                cx.notify();
                            })))
                        })
                        .children(self.small_error.clone().map(|e| div().small(t).text_color(t.colors.danger).child(e)))
                        .into_any_element(),
                ),
            )
            .child(row(
                i.t("ms.defaultImage"),
                i.t("ms.defaultImageBody"),
                select("ms-def-image", items, known(&prefs.default_image))
                    .on_change(move |v, _, cx| {
                        let r = ref_by(&models2, v.as_ref());
                        if let Some(p) = ModelPrefsStore::global(cx) {
                            p.update(cx, |p, cx| p.set_default_image(r, cx));
                        }
                    })
                    .into_any_element(),
            ))
            .into_any_element()
    }

    fn provider_list(&self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let rows = provider_rows(&self.views, &self.catalog, &self.models);
        let hidden = ModelPrefsStore::global(cx).map(|p| p.read(cx).prefs.hidden.clone()).unwrap_or_default();
        let ready = EngineStatus::global(cx).read(cx).manager.snapshot().status == "ready";
        let mut list = div().flex().flex_col();
        for r in rows {
            let keys: Vec<String> = if r.models.is_empty() { r.spec.iter().flat_map(|s| s.models.iter()).map(|m| format!("{}/{}", r.id, m.id)).collect() } else { r.models.iter().map(key_of).collect() };
            let nh = keys.iter().filter(|k| hidden.contains(k)).count();
            let connected = self.catalog.iter().any(|c| c.id == r.id && c.connected) || !r.models.is_empty();
            let sub = [r.spec.as_ref().and_then(|s| s.base_url.clone()), Some(i.tp("ms.modelCount", &[("shown", &(keys.len() - nh).to_string()), ("total", &keys.len().to_string())]))]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ");
            let kind = match r.spec.as_ref().map(|s| &s.kind) {
                Some(Kind::Custom) => "pv.kind.custom",
                Some(Kind::Local) => "pv.kind.local",
                _ => "pv.kind.builtin",
            };
            let (st_key, st_tone) = if !ready {
                ("pv.status.engineDown", BadgeTone::Warning)
            } else if connected {
                ("pv.status.connected", BadgeTone::Success)
            } else {
                ("pv.status.notConnected", BadgeTone::Warning)
            };
            let id = r.id.clone();
            let can = !r.models.is_empty() || r.spec.as_ref().is_some_and(|s| !s.models.is_empty());
            list = list.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(t.space(3))
                    .py(t.space(2))
                    .border_b_1()
                    .border_color(t.colors.border)
                    .child(model_logo(t, cx, Some(&r.id), None, &r.name, false, &format!("msp-{}", r.id)))
                    .child(div().flex().flex_col().flex_1().min_w_0().child(div().font_weight(FontWeight::MEDIUM).child(r.name.clone())).child(div().small(t).text_color(t.colors.fg_muted).truncate().child(sub)))
                    .child(
                        div()
                            .flex()
                            .gap(t.space(1))
                            .child(badge(i.t(kind)).tone(BadgeTone::Outline))
                            .children(r.spec.as_ref().map(|s| badge(i.t(if s.has_key { "pv.key.saved" } else { "pv.key.none" })).tone(if s.has_key { BadgeTone::Success } else { BadgeTone::Neutral })))
                            .child(badge(i.t(st_key)).tone(st_tone)),
                    )
                    .child(button(SharedString::from(format!("msp-manage-{}", r.id))).size(ControlSize::Sm).disabled(!can).label(i.t("ms.manageModels")).on_click(cx.listener(move |s, _, _, cx| {
                        s.managing = Some(id.clone());
                        s.note = None;
                        cx.notify();
                    }))),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(t.space(1))
            .child(div().typo(t, crate::theme::H4).child(i.t("ms.manageModels")))
            .when(self.views.is_empty() && self.models.is_empty(), |d| d.child(div().text_color(t.colors.fg_muted).child(i.t("ms.noProviders"))))
            .child(list)
            .into_any_element()
    }

    fn manager(&self, t: &Theme, i: &I18n, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let back = button("ms-back").child(icon("arrow_left").size(IconSize::Sm)).label(i.t("ms.back")).on_click(cx.listener(|s, _, _, cx| {
            s.managing = None;
            s.editing = None;
            s.logo = None;
            cx.notify();
        }));
        let rows = provider_rows(&self.views, &self.catalog, &self.models);
        let Some(row) = rows.into_iter().find(|r| r.id == id) else {
            return div().flex().flex_col().gap(t.space(2)).child(back).child(div().child(i.t("ms.providerGone"))).into_any_element();
        };
        let spec = row.spec.clone();
        let editable = spec.as_ref().is_some_and(|s| !matches!(s.kind, Kind::Builtin));
        let models = models_of(&row);
        let prefs = ModelPrefsStore::global(cx).map(|p| p.read(cx).prefs.clone()).unwrap_or_default();
        let q = self.search.read(cx).text().trim().to_lowercase();
        let shown: Vec<ModelOption> = models.iter().filter(|m| q.is_empty() || format!("{} {}", m.name, m.model_id).to_lowercase().contains(&q)).cloned().collect();
        let refs: Vec<Ref> = shown.iter().map(ref_of).collect();
        let hidden_n = models.iter().filter(|m| prefs.hidden.contains(&key_of(m))).count();
        let mut groups: Vec<(String, Vec<ModelOption>)> = vec![];
        for m in &shown {
            let k = match self.group_by.as_str() {
                "family" => detect_logo_key(Some(&m.model_id)).or_else(|| detect_logo_key(Some(&m.name))).unwrap_or_else(|| "other".into()),
                "capability" => capability_group(m).into(),
                _ => String::new(),
            };
            match groups.iter_mut().find(|g| g.0 == k) {
                Some(g) => g.1.push(m.clone()),
                None => groups.push((k, vec![m.clone()])),
            }
        }
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        let pings = PingStore::global(cx).read(cx).pings.clone();

        let mut head = div().flex().flex_wrap().items_center().justify_between().gap(t.space(2)).child(back);
        let mut acts = div().flex().gap(t.space(2));
        if editable {
            let sp = spec.clone().unwrap();
            acts = acts.child(button("ms-fetch").loading(self.fetching).label(i.t("pv.fetchModels")).on_click(cx.listener(move |s, _, _, cx| s.fetch_models(sp.clone(), cx))));
        }
        let rid = row.id.clone();
        acts = acts.child(button("ms-plogo").child(icon("image").size(IconSize::Sm)).label(i.t("ms.providerLogo")).on_click(cx.listener(move |s, _, _, cx| {
            let custom = ModelPrefsStore::global(cx).and_then(|p| p.read(cx).prefs.provider_logos.get(&rid).cloned());
            s.logo = Some(LogoEdit { model_id: None, draft: custom.clone(), custom, error: None });
            cx.notify();
        })));
        head = head.child(acts);

        let me = cx.entity().downgrade();
        let gsel = ["none", "family", "capability"].iter().map(|g| (SharedString::from(*g), SharedString::from(i.t(&format!("ms.groupBy.{g}"))))).collect();
        let (r1, r2) = (refs.clone(), refs.clone());
        let toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(t.space(2))
            .child(div().flex_1().min_w(t.rem(rem::SPACE[7] * 4.)).child(self.search.clone()))
            .child(select("ms-groupby", gsel, self.group_by.clone()).on_change(move |v, _, cx| {
                me.update(cx, |s, cx| {
                    s.group_by = v.to_string();
                    cx.notify();
                })
                .ok();
            }))
            .child(button("ms-all").disabled(refs.is_empty()).child(icon("eye").size(IconSize::Sm)).label(i.t("ms.selectAll")).on_click(move |_, _, cx| {
                if let Some(p) = ModelPrefsStore::global(cx) {
                    p.update(cx, |p, cx| p.set_visible(&r1, true, cx));
                }
            }))
            .child(button("ms-none").disabled(refs.is_empty()).child(icon("eye_off").size(IconSize::Sm)).label(i.t("ms.deselectAll")).on_click(move |_, _, cx| {
                if let Some(p) = ModelPrefsStore::global(cx) {
                    p.update(cx, |p, cx| p.set_visible(&r2, false, cx));
                }
            }));

        let mut body = div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(head)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(model_logo(t, cx, Some(&row.id), None, &row.name, false, "msm-logo"))
                    .child(div().typo(t, crate::theme::H4).child(row.name.clone()))
                    .child(div().small(t).text_color(t.colors.fg_muted).child(i.tp("ms.visibleOf", &[("shown", &(models.len() - hidden_n).to_string()), ("total", &models.len().to_string())]))),
            )
            .children(self.note.clone().map(|n| div().small(t).child(n)))
            .child(toolbar);
        if shown.is_empty() {
            body = body.child(div().text_color(t.colors.fg_muted).child(i.t("mp.none")));
        }
        for (k, list) in groups {
            let mut g = div().flex().flex_col();
            if self.group_by != "none" {
                let label = if self.group_by == "capability" { i.t(&format!("ms.group.{k}")) } else if k == "other" { i.t("ms.group.other") } else { k.clone() };
                g = g.child(div().flex().gap(t.space(1)).font_weight(FontWeight::MEDIUM).child(label).child(div().small(t).text_color(t.colors.fg_subtle).child(format!("({})", list.len()))));
            }
            for m in list {
                let visible = !prefs.hidden.contains(&key_of(&m));
                let r = ref_of(&m);
                let mk = key_of(&m);
                let ping = pings.get(&mk).cloned();
                let mid = m.model_id.clone();
                let mut caps = div().flex().flex_wrap().gap(t.space(1));
                if let Some(c) = m.context_limit {
                    caps = caps.child(badge(i.tp("ms.cap.context", &[("n", &compact_tokens(c))])).tone(BadgeTone::Outline));
                }
                if let Some(c) = m.output_limit {
                    caps = caps.child(badge(i.tp("ms.cap.output", &[("n", &compact_tokens(c))])).tone(BadgeTone::Outline));
                }
                if m.reasoning == Some(true) {
                    caps = caps.child(badge(i.t("ms.cap.reasoning")).tone(BadgeTone::Success));
                }
                if m.tool_call == Some(true) {
                    caps = caps.child(badge(i.t("ms.cap.tools")).tone(BadgeTone::Neutral));
                }
                if m.attachment == Some(true) {
                    caps = caps.child(badge(i.t("ms.cap.attachment")).tone(BadgeTone::Neutral));
                }
                if !m.variants.is_empty() {
                    caps = caps.child(badge(i.tp("ms.cap.variants", &[("n", &m.variants.len().to_string())])).tone(BadgeTone::Neutral));
                }
                let mut line = div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(t.space(2))
                    .py(t.space(2))
                    .border_b_1()
                    .border_color(t.colors.border)
                    .when(!visible, |d| d.opacity(0.6))
                    .child(switch(SharedString::from(format!("msv-{mk}")), "", visible).on_change(move |v, _, cx| {
                        if let Some(p) = ModelPrefsStore::global(cx) {
                            p.update(cx, |p, cx| p.set_visible(std::slice::from_ref(&r), v, cx));
                        }
                    }))
                    .child(
                        div()
                            .id(SharedString::from(format!("msl-{mk}")))
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip::text_tooltip(i.t("ms.modelLogo").into()))
                            .on_click(cx.listener({
                                let (pid, mid) = (m.provider_id.clone(), mid.clone());
                                move |s, _, _, cx| {
                                    let custom = ModelPrefsStore::global(cx).and_then(|p| p.read(cx).prefs.model_logos.get(&format!("{pid}/{mid}")).cloned());
                                    s.logo = Some(LogoEdit { model_id: Some(mid.clone()), draft: custom.clone(), custom, error: None });
                                    cx.notify();
                                }
                            }))
                            .child(model_logo(t, cx, Some(&m.provider_id), Some(&m.model_id), &m.name, false, &format!("msl-i-{mk}"))),
                    )
                    .child(div().flex().flex_col().flex_1().min_w(t.rem(rem::SPACE[7] * 3.)).child(div().font_weight(FontWeight::MEDIUM).truncate().child(m.name.clone())).child(div().small(t).text_color(t.colors.fg_muted).truncate().child(m.model_id.clone())))
                    .child(caps);
                if editable {
                    let (pp, pm) = (m.provider_id.clone(), m.model_id.clone());
                    line = line
                        .child(match &ping {
                            Some(PingState::Pending) => crate::ui::spinner(IconSize::Sm).into_any_element(),
                            Some(PingState::Done(r)) => div().small(t).text_color(if r.ok { t.colors.success } else { t.colors.danger }).child(ping_label(i, r)).into_any_element(),
                            None => div().into_any_element(),
                        })
                        .child(
                            icon_button(SharedString::from(format!("msping-{mk}")), "ping", i.tp("pv.ping", &[("name", &m.name)]))
                                .size(ControlSize::Sm)
                                .disabled(ping == Some(PingState::Pending))
                                .on_click(move |_, _, cx| PingStore::global(cx).update(cx, |p, cx| p.ping(&pp, &pm, cx))),
                        );
                    let spec_m = spec.as_ref().and_then(|s| s.models.iter().find(|x| x.id == m.model_id).cloned());
                    line = line.child(icon_button(SharedString::from(format!("msedit-{mk}")), "edit", i.tp("ms.editModel", &[("id", &m.name)])).size(ControlSize::Sm).disabled(spec_m.is_none()).on_click(cx.listener(move |s, _, _, cx| {
                        if let Some(pm) = &spec_m {
                            s.open_edit(pm, cx);
                        }
                    })));
                }
                g = g.child(line);
            }
            body = body.child(g);
        }
        // dialog ubah model
        if let (Some(f), Some(sp)) = (self.editing.as_ref(), spec.clone()) {
            let field = |label: String, el: Entity<TextInput>| div().flex().flex_col().gap(t.space(1)).flex_1().child(div().small(t).font_weight(FontWeight::MEDIUM).child(label)).child(el);
            let tog = |id: &'static str, label: String, v: bool, which: u8, cx: &mut Context<Self>| {
                let me = cx.entity().downgrade();
                switch(id, label, v).on_change(move |nv, _, cx| {
                    me.update(cx, |s, cx| {
                        if let Some(f) = s.editing.as_mut() {
                            match which {
                                0 => f.reasoning = nv,
                                1 => f.tool_call = nv,
                                _ => f.attachment = nv,
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                })
            };
            let d = crate::ui::dialog(i.tp("ms.editModel", &[("id", &f.model_id)]))
                .description(i.t("ms.editModelBody"))
                .on_dismiss(cx.listener(|s, _, _, cx| {
                    s.editing = None;
                    cx.notify();
                }))
                .child(field(i.t("pv.name"), f.name.clone()))
                .child(div().flex().gap(t.space(3)).child(field(i.t("ms.field.context"), f.context.clone())).child(field(i.t("ms.field.output"), f.output.clone())))
                .child(tog("mse-r", i.t("ms.cap.reasoning"), f.reasoning, 0, cx))
                .child(tog("mse-t", i.t("ms.cap.tools"), f.tool_call, 1, cx))
                .child(tog("mse-a", i.t("ms.cap.attachment"), f.attachment, 2, cx))
                .footer(button("mse-cancel").label(i.t("ui.cancel")).on_click(cx.listener(|s, _, _, cx| {
                    s.editing = None;
                    cx.notify();
                })))
                .footer(button("mse-save").variant(ButtonVariant::Solid).label(i.t("pv.save")).on_click(cx.listener(move |s, _, _, cx| s.save_edit(&sp, cx))));
            body = body.child(d);
        }
        // LogoDialog
        if let Some(l) = self.logo.as_ref() {
            let lm = l.model_id.as_ref().and_then(|mid| models.iter().find(|m| m.model_id == *mid));
            let name = lm.map(|m| m.name.clone()).unwrap_or_else(|| row.name.clone());
            let title = match &l.model_id {
                Some(mid) => i.tp("ms.modelLogoFor", &[("name", lm.map(|m| m.name.as_str()).unwrap_or(mid))]),
                None => i.t("ms.providerLogo"),
            };
            let tile = |el: AnyElement, cap: String| {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(t.space(1))
                    .child(div().flex().items_center().justify_center().size(t.rem(rem::SPACE[7] * 2.)).border_1().border_color(t.colors.border).rounded(t.rem(rem::RADIUS_LG)).child(el))
                    .child(div().small(t).text_color(t.colors.fg_muted).child(cap))
            };
            let auto_logo = crate::core::model_logo::model_logo_key(Some(&row.id), l.model_id.as_deref(), Some(&name))
                .and_then(|k| crate::core::model_logo::logo_image(&k, t.colors.fg_subtle))
                .map(|img| gpui::img(img).size(t.rem(rem::SPACE[6])).into_any_element())
                .unwrap_or_else(|| div().into_any_element());
            let draft_el = l.draft.as_deref().and_then(crate::core::gradient::image_of).map(|img| gpui::img(img).size(t.rem(rem::SPACE[6])).into_any_element()).unwrap_or_else(|| div().into_any_element());
            let changed = l.draft != l.custom;
            let (rid, mid) = (row.id.clone(), l.model_id.clone());
            let d = crate::ui::dialog(title)
                .on_dismiss(cx.listener(|s, _, _, cx| {
                    s.logo = None;
                    cx.notify();
                }))
                .child(div().flex().gap(t.space(5)).justify_center().child(tile(auto_logo, i.t("logo.auto"))).child(tile(draft_el, i.t(if l.draft.is_some() { "logo.custom" } else { "logo.noCustom" }))))
                .child(div().small(t).text_color(t.colors.fg_muted).child(i.t("logo.help")))
                .child(
                    div()
                        .flex()
                        .gap(t.space(2))
                        .child(button("logo-up").child(icon("upload").size(IconSize::Sm)).label(i.t("logo.upload")).on_click(cx.listener(|s, _, _, cx| s.pick_logo(cx))))
                        .child(button("logo-auto").variant(ButtonVariant::Ghost).disabled(l.draft.is_none()).child(icon("trash").size(IconSize::Sm)).label(i.t("logo.useAuto")).on_click(cx.listener(|s, _, _, cx| {
                            if let Some(l) = s.logo.as_mut() {
                                l.draft = None;
                            }
                            cx.notify();
                        }))),
                )
                .child(div().children(l.error.clone().map(|e| div().small(t).text_color(t.colors.danger).child(e))))
                .footer(button("logo-cancel").label(i.t("ui.cancel")).on_click(cx.listener(|s, _, _, cx| {
                    s.logo = None;
                    cx.notify();
                })))
                .footer(button("logo-apply").variant(ButtonVariant::Solid).disabled(!changed).label(i.t("logo.apply")).on_click(cx.listener(move |s, _, _, cx| {
                    let Some(draft) = s.logo.as_ref().map(|l| l.draft.clone()) else { return };
                    let ok = ModelPrefsStore::global(cx)
                        .map(|p| {
                            p.update(cx, |p, cx| match &mid {
                                Some(m) => p.set_model_logo(&Ref { provider_id: rid.clone(), model_id: m.clone() }, draft.clone(), cx),
                                None => p.set_provider_logo(&rid, draft.clone(), cx),
                            })
                        })
                        .unwrap_or(false);
                    let i = cx.global::<I18n>().clone();
                    if ok {
                        s.logo = None;
                        toast(cx, i.t("logo.saved"), crate::ui::ToastTone::Success, Some(2000));
                    } else if let Some(l) = s.logo.as_mut() {
                        l.error = Some(i.t("logo.err.decode"));
                    }
                    cx.notify();
                })));
            body = body.child(d);
        }
        body.into_any_element()
    }
}

impl Render for ModelsSection {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        if let Some(id) = self.managing.clone() {
            return self.manager(&t, &i, &id, cx);
        }
        div().flex().flex_col().gap(t.space(4)).child(self.defaults(&t, &i, cx)).child(self.provider_list(&t, &i, cx)).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_dan_grup() {
        assert_eq!(compact_tokens(128000), "128K");
        assert_eq!(compact_tokens(1_000_000), "1M");
        assert_eq!(compact_tokens(1_500_000), "1.5M");
        assert_eq!(compact_tokens(512), "512");
        let mut m = ModelOption { provider_id: "p".into(), provider_name: "P".into(), model_id: "m".into(), name: "M".into(), variants: vec![], context_limit: None, output_limit: None, reasoning: None, tool_call: Some(true), attachment: None, inputs: vec![] };
        assert_eq!(capability_group(&m), "tools");
        m.reasoning = Some(true);
        assert_eq!(capability_group(&m), "reasoning");
        assert_eq!(file_to_logo(std::path::Path::new("x.txt")), Err("type"));
    }
}

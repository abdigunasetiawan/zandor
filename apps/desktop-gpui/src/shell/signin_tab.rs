//! Tab Sign-in panel koneksi provider — port ConnectionPanel.tsx (SignInTab + SignInMethod), OpenWorkSignIn.tsx, dan
//! bagian OAuth/OpenWork di ProviderStore (packages/core/src/providers/store.ts) + perintah Rust `openwork_*`.
//! OAuth akun lewat engine (URL dibuka di browser sistem; "code" meminta kode dari pengguna). OpenWork memakai device
//! flow: token dan kunci gateway hanya ada di Rust (keychain), UI hanya melihat kode dan status.
use crate::core::engine::CatalogProvider;
use crate::core::projects::ProjectStore;
use crate::core::services::{app_data_dir, EngineStatus};
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H2};
use crate::ui::{badge, button, select, BadgeTone, ButtonVariant, ControlSize, IconSize, InputChanged, TextInput, Toasts, ToastInput, ToastTone, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, EventEmitter, FontWeight, IntoElement, SharedString, Window};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const OAUTH_WAIT: Duration = Duration::from_secs(5 * 60);
const OPENWORK_ID: &str = "openwork";

/// Dipancarkan setelah login berhasil (katalog/provider perlu dimuat ulang).
pub struct SignedIn;
impl EventEmitter<SignedIn> for SignInTab {}

#[derive(Clone)]
enum Ow {
    Idle,
    Starting,
    Waiting { code: String, uri: String, expires: Instant },
    Connected { account: String, models: usize },
    Error(String),
}

enum PollOut {
    Pending,
    Slow(u64),
    Connected(String, usize),
    Failed(String),
}

#[derive(Clone)]
struct Flow {
    provider: String,
    name: String,
    method: u32,
    code_stage: bool,
    instructions: String,
}

struct Method {
    index: u32,
    label: String,
    prompts: Vec<Value>,
}

pub struct SignInTab {
    methods: Option<Value>,
    catalog: Vec<CatalogProvider>,
    error: Option<String>,
    flow: Option<Flow>,
    flow_run: u64,
    code: Entity<TextInput>,
    query: Entity<TextInput>,
    /// Isian prompt per metode: kunci "provider|index|key" → teks (select) atau input.
    texts: HashMap<String, Entity<TextInput>>,
    selects: HashMap<String, String>,
    ow: Ow,
    ow_run: u64,
    ow_interval: u64,
    ow_flow: Option<crate::openwork::Flow>,
    accounts: Option<Entity<super::account_list::AccountList>>,
}

fn store() -> crate::providers::ProviderStore {
    crate::providers::ProviderStore::new(&app_data_dir().join("providers.json"), Box::new(crate::secrets::Keychain::new(crate::secrets::SERVICE)))
}

/// Port connect_openwork (lib.rs): kunci gateway → provider "openwork" + satu akun baru.
fn connect_openwork(ep: &crate::openwork::Endpoints, token: &str) -> Result<(String, usize), String> {
    let store = store();
    let gw = crate::openwork::resolve_gateway(ep, token)?;
    let Some(key) = gw.api_key else {
        return Err("Akun OpenWork tersambung, tetapi organisasi ini belum punya kredensial AI Gateway (org_credential_missing). Minta admin mengaktifkan inferensi atau coba akun lain.".into());
    };
    let mut ids = gw.models;
    if ids.is_empty() {
        ids = crate::netprobe::probe(&ep.inference, Some(&key), Duration::from_secs(10)).models;
    }
    if ids.is_empty() {
        return Err("Tidak ada model yang tersedia untuk akun ini.".into());
    }
    let account = match (gw.email.is_empty(), gw.org_name) {
        (false, Some(org)) => format!("{} · {org}", gw.email),
        (false, None) => gw.email,
        (true, Some(org)) => org,
        (true, None) => "Akun OpenWork".into(),
    };
    let existing = store.load().0.into_iter().find(|p| p.id == OPENWORK_ID);
    let models = ids
        .iter()
        .map(|id| existing.as_ref().and_then(|e| e.models.iter().find(|m| &m.id == id).cloned()).unwrap_or_else(|| crate::providers::ProviderModel { id: id.clone(), ..Default::default() }))
        .collect();
    store.save(
        crate::providers::Provider { id: OPENWORK_ID.into(), name: "OpenWork Cloud".into(), kind: crate::providers::Kind::Custom, base_url: Some(ep.inference.clone()), models, ..Default::default() },
        None,
    )?;
    store.add_account(OPENWORK_ID, &account, &key, true)?;
    Ok((account, ids.len()))
}

impl SignInTab {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let code = cx.new(|cx| TextInput::new(cx, cx.global::<I18n>().t("pv.signin.code")));
        let query = cx.new(|cx| TextInput::new(cx, cx.global::<I18n>().t("pv.catalog.search")));
        cx.subscribe(&code, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        cx.subscribe(&query, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        cx.observe(&EngineStatus::global(cx), |s, _, cx| s.load(cx)).detach();
        let mut me = Self {
            methods: None,
            catalog: vec![],
            error: None,
            flow: None,
            flow_run: 0,
            code,
            query,
            texts: HashMap::new(),
            selects: HashMap::new(),
            ow: Ow::Idle,
            ow_run: 0,
            ow_interval: 5,
            ow_flow: None,
            accounts: None,
        };
        me.load(cx);
        me.sync_accounts(cx);
        me
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        let task = cx.background_spawn(async move { (api.list_catalog().unwrap_or_default(), api.list_auth_methods().unwrap_or(Value::Null)) });
        cx.spawn(async move |this, cx| {
            let (catalog, methods) = task.await;
            this.update(cx, |s, cx| {
                s.catalog = catalog;
                s.methods = Some(methods);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn sync_accounts(&mut self, cx: &mut Context<Self>) {
        let has = store().load().0.iter().any(|p| p.id == OPENWORK_ID && !p.accounts.is_empty());
        match (&self.accounts, has) {
            (None, true) => {
                self.accounts = Some(cx.new(|cx| {
                    let mut a = super::account_list::AccountList::new(OPENWORK_ID.into(), "OpenWork Cloud".into(), cx);
                    a.with_add = false;
                    a
                }))
            }
            (Some(a), true) => a.update(cx, |a, _| a.reload()),
            (Some(_), false) => self.accounts = None,
            _ => {}
        }
    }

    fn entries(&self) -> Vec<(String, String, Vec<Method>)> {
        let Some(obj) = self.methods.as_ref().and_then(Value::as_object) else { return vec![] };
        obj.iter()
            .filter_map(|(id, list)| {
                let methods: Vec<Method> = list
                    .as_array()?
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m["type"] == "oauth")
                    .map(|(n, m)| Method { index: n as u32, label: m["label"].as_str().unwrap_or("").to_string(), prompts: m["prompts"].as_array().cloned().unwrap_or_default() })
                    .collect();
                let name = self.catalog.iter().find(|c| c.id == *id).map(|c| c.name.clone()).unwrap_or_else(|| id.clone());
                (!methods.is_empty()).then(|| (id.clone(), name, methods))
            })
            .collect()
    }

    fn fail(&mut self, e: String, cx: &mut Context<Self>) {
        self.error = Some(cx.global::<I18n>().tp("pv.signin.failed", &[("error", &e)]));
        self.flow = None;
        cx.notify();
    }

    fn start(&mut self, provider: String, name: String, m: u32, inputs: HashMap<String, String>, cx: &mut Context<Self>) {
        self.error = None;
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return self.fail("engine belum siap".into(), cx) };
        let body = (!inputs.is_empty()).then(|| json!(inputs));
        let (p2, task) = (provider.clone(), cx.background_spawn(async move { api.oauth_authorize(&provider, m, body) }));
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| match r {
                Err(e) => s.fail(e.to_string(), cx),
                Ok((url, _, _)) if !url.starts_with("https://") => s.fail("URL sign-in dari engine bukan https; dibatalkan".into(), cx),
                Ok((url, method, instructions)) => {
                    cx.open_url(&url);
                    let code_stage = method == "code";
                    s.flow = Some(Flow { provider: p2.clone(), name: name.clone(), method: m, code_stage, instructions });
                    if !code_stage {
                        s.finish(None, cx);
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Port finish: tunggu callback (maks 5 menit), lalu segarkan katalog.
    fn finish(&mut self, code: Option<String>, cx: &mut Context<Self>) {
        let Some(f) = self.flow.clone() else { return };
        let Some(api) = ProjectStore::global(cx).read(cx).api() else { return };
        self.flow_run += 1;
        let run = self.flow_run;
        let (prov, m) = (f.provider.clone(), f.method);
        let task = cx.background_spawn(async move { api.oauth_callback(&prov, m, code.as_deref()) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                if s.flow_run != run {
                    return;
                }
                match r {
                    Ok(()) => {
                        let title = cx.global::<I18n>().tp("pv.signin.done", &[("name", &f.name)]);
                        Toasts::show(ToastInput { tone: ToastTone::Success, ..ToastInput::new(title) }, cx);
                        s.flow = None;
                        s.load(cx);
                        cx.emit(SignedIn);
                    }
                    Err(e) => s.fail(e.to_string(), cx),
                }
            })
            .ok();
        })
        .detach();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(OAUTH_WAIT).await;
            this.update(cx, |s, cx| {
                if s.flow_run == run && s.flow.is_some() {
                    s.flow_run += 1;
                    s.fail("waktu habis menunggu login".into(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    // ---------- OpenWork (port ProviderStore.startOpenWork/pollOpenWork/cancelOpenWork) ----------

    fn ow_start(&mut self, cx: &mut Context<Self>) {
        self.ow_run += 1;
        let run = self.ow_run;
        self.ow = Ow::Starting;
        cx.notify();
        let task = cx.background_spawn(async { crate::openwork::start(&crate::openwork::Endpoints::default()) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                if s.ow_run != run {
                    return;
                }
                match r {
                    Err(e) => s.ow = Ow::Error(e),
                    Ok((info, flow)) => {
                        s.ow = Ow::Waiting { code: info.user_code, uri: info.verification_uri, expires: Instant::now() + Duration::from_secs(info.expires_in) };
                        s.ow_interval = info.interval.max(3);
                        s.ow_flow = Some(flow);
                        s.ow_schedule(run, cx);
                        s.ow_tick(run, cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Hitung mundur per detik selama menunggu.
    fn ow_tick(&self, run: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let alive = this
                .update(cx, |s, cx| {
                    cx.notify();
                    s.ow_run == run && matches!(s.ow, Ow::Waiting { .. })
                })
                .unwrap_or(false);
            if !alive {
                break;
            }
        })
        .detach();
    }

    fn ow_schedule(&self, run: u64, cx: &mut Context<Self>) {
        let wait = Duration::from_secs(self.ow_interval);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |s, cx| s.ow_poll(run, cx)).ok();
        })
        .detach();
    }

    fn ow_poll(&mut self, run: u64, cx: &mut Context<Self>) {
        if self.ow_run != run {
            return;
        }
        let Some(flow) = self.ow_flow.clone() else { return };
        let task = cx.background_spawn(async move {
            use crate::openwork::Poll;
            let ep = crate::openwork::Endpoints::default();
            match crate::openwork::poll(&ep, &flow) {
                Poll::Token(token) => match connect_openwork(&ep, &token) {
                    Ok((a, n)) => PollOut::Connected(a, n),
                    Err(m) => PollOut::Failed(m),
                },
                Poll::Pending => PollOut::Pending,
                Poll::SlowDown(i) => PollOut::Slow(i),
                Poll::Denied(m) | Poll::Error(m) => PollOut::Failed(m),
                Poll::Expired => PollOut::Failed("Kode kedaluwarsa. Mulai lagi untuk mendapat kode baru.".into()),
            }
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                if s.ow_run != run {
                    return;
                }
                match r {
                    PollOut::Pending => s.ow_schedule(run, cx),
                    PollOut::Slow(i) => {
                        s.ow_interval = i.max(3);
                        s.ow_schedule(run, cx);
                    }
                    PollOut::Connected(account, models) => {
                        s.ow = Ow::Connected { account, models };
                        s.ow_flow = None;
                        s.sync_accounts(cx);
                        EngineStatus::global(cx).read(cx).restart();
                        cx.emit(SignedIn);
                    }
                    PollOut::Failed(m) => {
                        s.ow = Ow::Error(m);
                        s.ow_flow = None;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn ow_cancel(&mut self, cx: &mut Context<Self>) {
        self.ow_run += 1;
        self.ow_flow = None;
        self.ow = Ow::Idle;
        cx.notify();
    }

    fn openwork_card(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let has_accounts = self.accounts.is_some();
        let running = matches!(self.ow, Ow::Starting | Ow::Waiting { .. });
        let mut c = div()
            .flex()
            .flex_col()
            .gap(t.space(2))
            .p(t.space(3))
            .rounded(t.rem(rem::RADIUS_MD))
            .border_1()
            .border_color(t.colors.border_soft)
            .child(div().flex().items_center().gap(t.space(2)).child(super::widgets::model_logo(t, cx, Some(OPENWORK_ID), None, "OpenWork", false, "ow-logo")).child(div().font_weight(FontWeight::SEMIBOLD).child("OpenWork Cloud")))
            .child(div().small(t).text_color(t.colors.fg_muted).child(i.t("ow.intro")));
        match self.ow.clone() {
            Ow::Starting => c = c.child(div().flex().items_center().gap(t.space(1)).small(t).child(crate::ui::spinner(IconSize::Sm)).child(i.t("ow.starting"))),
            Ow::Waiting { code, uri, expires } => {
                let left = expires.saturating_duration_since(Instant::now()).as_secs();
                c = c
                    .child(div().small(t).child(i.t("ow.step1")))
                    .child(div().child(button("ow-open").variant(ButtonVariant::Solid).label(i.t("ow.openPage")).on_click(move |_, _, cx| cx.open_url(&uri))))
                    .child(div().small(t).child(i.t("ow.step2")))
                    .child(div().typo(t, H2).font_family(t.font_code.clone()).child(code))
                    .child(div().flex().items_center().gap(t.space(1)).small(t).text_color(t.colors.fg_muted).child(crate::ui::spinner(IconSize::Sm)).child(i.tp("ow.waiting", &[("s", &left.to_string())])));
            }
            Ow::Connected { account, models } => c = c.child(div().small(t).text_color(t.colors.success).child(i.tp("ow.connected", &[("account", &account), ("n", &models.to_string())]))),
            Ow::Error(m) => c = c.child(div().small(t).text_color(t.colors.danger).child(m)),
            Ow::Idle => {}
        }
        let action = if running {
            button("ow-cancel").label(i.t("ow.cancel")).on_click(cx.listener(|s, _, _, cx| s.ow_cancel(cx)))
        } else {
            let label = if matches!(self.ow, Ow::Error(_)) { i.t("ow.retry") } else if has_accounts { i.t("acc.add") } else { i.t("ow.signIn") };
            button("ow-start").variant(ButtonVariant::Solid).label(label).on_click(cx.listener(|s, _, _, cx| s.ow_start(cx)))
        };
        c.child(div().child(action)).children(self.accounts.clone()).child(div().small(t).text_color(t.colors.fg_muted).child(i.t("ow.privacy"))).into_any_element()
    }
}

impl Render for SignInTab {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let ow = self.openwork_card(&t, &i, cx);
        let ready = EngineStatus::global(cx).read(cx).state.status == "ready";
        if !ready {
            return div().flex().flex_col().gap(t.space(2)).child(ow).child(div().small(&t).child(i.t("pv.catalog.engineDown"))).into_any_element();
        }
        let entries = self.entries();
        let q = self.query.read(cx).text().trim().to_lowercase();
        let mut col = div().flex().flex_col().gap(t.space(2)).child(div().small(&t).child(i.t("pv.signin.note"))).child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("acc.oauthNote")));
        if let Some(e) = &self.error {
            col = col.child(div().small(&t).text_color(t.colors.danger).child(e.clone()));
        }
        if let Some(f) = self.flow.clone() {
            if f.code_stage {
                let empty = self.code.read(cx).text().trim().is_empty();
                col = col.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.space(1))
                        .child(div().small(&t).child(f.instructions.clone()))
                        .child(div().flex().items_center().gap(t.space(2)).child(div().flex_1().child(self.code.clone())).child(button("si-code").variant(ButtonVariant::Solid).disabled(empty).label(i.t("pv.signin.submit")).on_click(cx.listener(|s, _, _, cx| {
                            let c = s.code.read(cx).text().trim().to_string();
                            s.finish(Some(c), cx);
                        })))),
                );
            } else {
                col = col.child(div().flex().items_center().gap(t.space(1)).small(&t).child(crate::ui::spinner(IconSize::Sm)).child(format!("{} {}", i.t("pv.signin.waiting"), f.instructions)));
            }
        }
        if entries.is_empty() && self.methods.is_some() {
            col = col.child(div().small(&t).child(i.t("pv.signin.none")));
        }
        if entries.len() > 6 {
            col = col.child(self.query.clone());
        }
        col = col.child(ow);
        let busy = self.flow.is_some();
        for (id, name, methods) in entries.into_iter().filter(|(id, name, _)| q.is_empty() || format!("{name} {id}").to_lowercase().contains(&q)) {
            let connected = self.catalog.iter().any(|c| c.id == id && c.connected);
            let mut card = div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .rounded(t.rem(rem::RADIUS_MD))
                .border_1()
                .border_color(t.colors.border_soft)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .child(super::widgets::model_logo(&t, cx, Some(&id), None, &name, false, &format!("si-logo-{id}")))
                        .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(name.clone()))
                        .when(connected, |d| d.child(badge(i.t("pv.status.connected")).tone(BadgeTone::Success))),
                );
            for m in methods {
                // nilai awal select = opsi pertama
                let mut values: HashMap<String, String> = HashMap::new();
                for p in &m.prompts {
                    let key = p["key"].as_str().unwrap_or("").to_string();
                    let k = format!("{id}|{}|{key}", m.index);
                    if p["type"] == "select" {
                        let first = p["options"][0]["value"].as_str().unwrap_or("").to_string();
                        values.insert(key, self.selects.get(&k).cloned().unwrap_or(first));
                    } else {
                        let ph = p["placeholder"].as_str().or(p["message"].as_str()).unwrap_or("").to_string();
                        let e = self.texts.entry(k).or_insert_with(|| cx.new(|cx| TextInput::new(cx, ph))).clone();
                        values.insert(key, e.read(cx).text().to_string());
                    }
                }
                let visible: Vec<Value> = m
                    .prompts
                    .iter()
                    .filter(|p| {
                        let w = &p["when"];
                        if !w.is_object() {
                            return true;
                        }
                        let cur = values.get(w["key"].as_str().unwrap_or("")).cloned().unwrap_or_default();
                        let want = w["value"].as_str().unwrap_or("");
                        if w["op"] == "eq" {
                            cur == want
                        } else {
                            cur != want
                        }
                    })
                    .cloned()
                    .collect();
                let mut fields = div().flex().flex_col().gap(t.space(1));
                for p in &visible {
                    let key = p["key"].as_str().unwrap_or("").to_string();
                    let k = format!("{id}|{}|{key}", m.index);
                    let msg = p["message"].as_str().unwrap_or("").to_string();
                    let el: AnyElement = if p["type"] == "select" {
                        let items: Vec<(SharedString, SharedString)> = p["options"].as_array().into_iter().flatten().map(|o| (o["value"].as_str().unwrap_or("").to_string().into(), o["label"].as_str().unwrap_or("").to_string().into())).collect();
                        let me = cx.entity().downgrade();
                        select(SharedString::from(format!("si-sel-{k}")), items, values.get(&key).cloned().unwrap_or_default())
                            .on_change(move |v, _, cx| {
                                let (k, v) = (k.clone(), v.to_string());
                                me.update(cx, |s, cx| {
                                    s.selects.insert(k, v);
                                    cx.notify();
                                })
                                .ok();
                            })
                            .into_any_element()
                    } else {
                        self.texts[&k].clone().into_any_element()
                    };
                    fields = fields.child(div().flex().flex_col().gap(t.space(1)).child(div().small(&t).text_color(t.colors.fg_muted).child(msg)).child(el));
                }
                let visible_keys: Vec<String> = visible.iter().map(|p| p["key"].as_str().unwrap_or("").to_string()).collect();
                let inputs: HashMap<String, String> = values.into_iter().filter(|(k, _)| visible_keys.contains(k)).collect();
                let (pid, pname, idx) = (id.clone(), name.clone(), m.index);
                card = card.child(
                    div().flex().flex_col().gap(t.space(2)).when(!visible.is_empty(), |d| d.child(fields)).child(
                        div().child(
                            button(SharedString::from(format!("si-start-{id}-{}", m.index)))
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Solid)
                                .disabled(busy)
                                .label(format!("{} · {}", i.t("pv.signin.start"), m.label))
                                .on_click(cx.listener(move |s, _, _, cx| s.start(pid.clone(), pname.clone(), idx, inputs.clone(), cx))),
                        ),
                    ),
                );
            }
            col = col.child(card);
        }
        col.into_any_element()
    }
}


//! Ping model — port bagian `ping`/`pings` di packages/core/src/providers/store.ts dan perintah Tauri `provider_ping`:
//! koneksi milik pengguna memakai base URL + kunci dari registry/keychain; provider katalog memakai endpoint dari engine.
use crate::netprobe::{self, ProbeResult};
use gpui::{App, AppContext, Context, Entity, Global};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub enum PingState {
    Pending,
    Done(ProbeResult),
}

pub fn ping_key(provider: &str, model: &str) -> String {
    format!("{provider}/{model}")
}

pub struct PingStore {
    pub pings: HashMap<String, PingState>,
}

struct PingHandle(Entity<PingStore>);
impl Global for PingHandle {}

fn provider_store() -> crate::providers::ProviderStore {
    crate::providers::ProviderStore::new(
        &super::services::app_data_dir().join("providers.json"),
        Box::new(crate::secrets::Keychain::new(crate::secrets::SERVICE)),
    )
}

/// Teks keterangan hasil ping (port pingLabel di ConnectionPanel).
pub fn ping_label(i: &crate::i18n::I18n, r: &ProbeResult) -> String {
    if r.ok {
        return i.t("pv.ping.ok").replace("{ms}", &r.latency_ms.to_string());
    }
    let head = i.t(&format!("pv.ping.{}", r.kind)).to_string();
    match &r.error {
        Some(e) if !e.is_empty() => format!("{head} — {e}"),
        _ => head,
    }
}

/// Provider milik pengguna (kustom/lokal), bukan bawaan katalog.
pub fn owned_provider(id: &str) -> bool {
    provider_store().load().0.iter().any(|p| p.id == id && p.base_url.is_some())
}

impl PingStore {
    pub fn init(cx: &mut App) {
        let e = cx.new(|_| PingStore { pings: HashMap::new() });
        cx.set_global(PingHandle(e));
    }
    pub fn global(cx: &App) -> Entity<PingStore> {
        cx.global::<PingHandle>().0.clone()
    }
    pub fn get(&self, provider: &str, model: &str) -> Option<&PingState> {
        self.pings.get(&ping_key(provider, model))
    }

    pub fn ping(&mut self, provider: &str, model: &str, cx: &mut Context<Self>) {
        let key = ping_key(provider, model);
        if self.pings.get(&key) == Some(&PingState::Pending) {
            return;
        }
        self.pings.insert(key.clone(), PingState::Pending);
        cx.notify();
        let snap = super::services::EngineStatus::global(cx).read(cx).manager.snapshot();
        let (provider, model) = (provider.to_string(), model.to_string());
        let task = cx.background_spawn(async move { run_ping(&provider, &model, snap) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.pings.insert(key, PingState::Done(r));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn run_ping(provider: &str, model: &str, snap: crate::engine::EngineSnapshot) -> ProbeResult {
    let store = provider_store();
    let stored_key = store.api_key(provider).ok().flatten();
    let timeout = Duration::from_secs(90);
    if let Some(base) = store.load().0.into_iter().find(|p| p.id == provider).and_then(|p| p.base_url) {
        return netprobe::ping_model(&base, stored_key.as_deref(), model, timeout);
    }
    if snap.status != "ready" {
        return netprobe::failed("network", "engine belum siap".into());
    }
    match netprobe::engine_target(&snap.url, snap.username, &snap.password, provider, model, Duration::from_secs(10)) {
        Ok(mut target) => {
            if target.key.is_none() {
                target.key = stored_key;
            }
            netprobe::ping_target(&target, model, timeout)
        }
        Err(f) => f,
    }
}

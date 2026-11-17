//! Layanan aplikasi: lokasi data, start engine (supervisor), sambungan ke ProjectStore — pengganti setup Tauri
//! (lib.rs `run()`) dan services.tsx.
use super::engine::{ConnectionInfo, EngineApi};
use super::projects::{ProjectHost, ProjectStore};
use crate::engine::{EngineManager, EnginePaths, EngineSnapshot};
use futures::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global};
use std::path::PathBuf;
use std::sync::Arc;

/// Status koneksi engine untuk UI (port engine/connection.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct EngineState {
    pub status: String,
    pub version: Option<String>,
    pub error: Option<String>,
    pub restarts: u32,
    pub info: Option<ConnectionInfo>,
}

pub struct EngineStatus {
    pub state: EngineState,
    pub manager: Arc<EngineManager>,
}
impl EngineStatus {
    pub fn restart(&self) {
        self.manager.restart();
    }
}

struct EngineHandle(Entity<EngineStatus>);
impl Global for EngineHandle {}

impl EngineStatus {
    pub fn global(cx: &App) -> Entity<EngineStatus> {
        cx.global::<EngineHandle>().0.clone()
    }
}

/// Direktori data aplikasi. Build debug memakai data dev (sama dengan Zandor Dev), rilis memakai data produksi.
pub fn app_data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join(if cfg!(debug_assertions) { "id.web.zandor.dev" } else { "id.web.zandor.app" })
}

fn repo_tauri_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../desktop/src-tauri")
}

/// Binary engine: di samping exe (rilis) atau binaries/ milik Tauri (dev).
fn sidecar_path() -> PathBuf {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)).unwrap_or_default();
    let beside = exe_dir.join(if cfg!(windows) { "opencode.exe" } else { "opencode" });
    if beside.exists() {
        return beside;
    }
    repo_tauri_dir().join("binaries").join(if cfg!(windows) { "opencode-x86_64-pc-windows-msvc.exe" } else { "opencode" })
}

fn plugin_path() -> PathBuf {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)).unwrap_or_default();
    let beside = exe_dir.join("resources").join("zandor-plugin.js");
    if beside.exists() { beside } else { repo_tauri_dir().join("resources").join("zandor-plugin.js") }
}

fn to_state(s: &EngineSnapshot) -> EngineState {
    let ready = s.status == "ready" && !s.url.is_empty();
    EngineState {
        status: s.status.to_string(),
        version: (!s.version.is_empty()).then(|| s.version.clone()),
        error: s.error.clone(),
        restarts: s.restarts,
        info: ready.then(|| ConnectionInfo { url: s.url.clone(), username: s.username.to_string(), password: s.password.clone() }),
    }
}

/// Mulai semua layanan: ProjectStore (registry) + engine. Dipanggil sekali saat aplikasi mulai.
pub fn init(cx: &mut App) {
    #[cfg(windows)]
    unsafe {
        // ADR-024: Ctrl+C di terminal/engine anak harus berfungsi.
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0);
    }
    let data = app_data_dir();
    let _ = std::fs::create_dir_all(&data);
    let projects = ProjectStore::init(
        ProjectHost { registry: crate::registry::registry_path(&data), chats: data.join("chats") },
        cx,
    );

    super::settings::SettingsStore::init(data.join("settings.json"), cx);
    super::automation::AutomationStore::init(data.join("automation.json"), cx);
    super::automation::start_scheduler(cx);
    let chat = super::chat_store::ChatStore::init(cx);
    let manager = Arc::new(EngineManager::default());
    let initial = to_state(&manager.snapshot());
    let status = cx.new(|_| EngineStatus { state: initial, manager: manager.clone() });
    cx.set_global(EngineHandle(status.clone()));

    let (tx, mut rx) = futures::channel::mpsc::unbounded::<EngineSnapshot>();
    manager.on_change(move |s| {
        let _ = tx.unbounded_send(s);
    });
    cx.spawn(async move |cx| {
        while let Some(snap) = rx.next().await {
            let st = to_state(&snap);
            eprintln!("[zandor] engine: {} {}", st.status, st.error.clone().unwrap_or_default());
            let api = st.info.clone().map(EngineApi::new);
            let changed_api = cx
                .update(|cx| {
                    let prev = status.read(cx).state.info.clone();
                    status.update(cx, |s, cx| {
                        s.state = st.clone();
                        cx.notify();
                    });
                    prev != st.info
                });
            if changed_api {
                let api2 = api.clone();
                cx.update(|cx| projects.update(cx, |p, cx| p.set_api(api2, cx)));
                let api3 = api.clone();
                cx.update(|cx| chat.update(cx, |c, cx| c.set_api(api3, cx)));
            }
        }
    })
    .detach();

    manager.start(EnginePaths::new(&data, sidecar_path(), plugin_path()));
}

/// Hentikan engine saat aplikasi keluar (tanpa proses yatim).
pub fn shutdown(cx: &mut App) {
    if let Some(h) = cx.try_global::<EngineHandle>() {
        h.0.read(cx).manager.shutdown();
    }
}

#[allow(dead_code)]
pub fn engine_ready<T>(cx: &Context<T>) -> bool {
    EngineStatus::global(cx).read(cx).state.info.is_some()
}

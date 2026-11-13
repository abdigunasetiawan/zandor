//! Spike S1 — app Tauri yang membundel `opencode serve` sebagai sidecar.
//!
//! - Binary sidecar: `bundle.externalBin = binaries/opencode` (Tauri menyalinnya ke
//!   sebelah executable sebagai `opencode[.exe]`).
//! - Supervisor: crate `zandor-engine-supervisor` (start, health, monitor, stop).
//! - Restart otomatis maksimal 3x kalau engine mati tak terduga.
//! - Self-test: env `ZANDOR_SELFTEST=<file.json>` → UI menjalankan skenario lalu app
//!   menulis laporan dan keluar.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, RunEvent};
use zandor_engine_supervisor::{Engine, EngineConfig};

const PINNED_VERSION: &str = "1.19.5";
const MAX_RESTARTS: u32 = 3;

#[derive(Default)]
struct EngineState {
    engine: Mutex<Option<Engine>>,
    /// Semua start/restart lewat satu thread supervisor yang hidup sepanjang app.
    /// Penting di Linux: PR_SET_PDEATHSIG terikat ke thread pembuat proses.
    start_tx: Mutex<Option<std::sync::mpsc::Sender<()>>>,
    restarts: Mutex<u32>,
    last_error: Mutex<Option<String>>,
}

#[derive(Serialize, Clone)]
struct EngineView {
    running: bool,
    url: String,
    username: &'static str,
    password: String,
    pid: u32,
    version: String,
    startup_ms: u128,
    restarts: u32,
    last_error: Option<String>,
}

fn sidecar_path() -> PathBuf {
    let dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    dir.join(if cfg!(windows) { "opencode.exe" } else { "opencode" })
}

fn start_engine(app: &AppHandle) {
    let mut cfg = EngineConfig::new(sidecar_path());
    cfg.expected_version = Some(PINNED_VERSION.into());
    let handle = app.clone();
    let result = Engine::start(cfg, move |exit| {
        // Dipanggil dari thread monitor: engine mati tanpa diminta.
        let _ = handle.emit("engine-exit", exit.code);
        let h2 = handle.clone();
        std::thread::spawn(move || {
            let st = h2.state::<EngineState>();
            let n = {
                let mut r = st.restarts.lock().unwrap();
                *r += 1;
                *r
            };
            st.engine.lock().unwrap().take(); // drop → stop (idempoten)
            if n <= MAX_RESTARTS {
                std::thread::sleep(Duration::from_millis(200 * n as u64));
                if let Some(tx) = st.start_tx.lock().unwrap().as_ref() {
                    let _ = tx.send(());
                }
            } else {
                *st.last_error.lock().unwrap() =
                    Some(format!("engine mati {n}x, restart otomatis dihentikan"));
                let _ = h2.emit("engine-state", ());
            }
        });
    });
    let st = app.state::<EngineState>();
    match result {
        Ok(e) => {
            *st.engine.lock().unwrap() = Some(e);
            *st.last_error.lock().unwrap() = None;
        }
        Err(e) => *st.last_error.lock().unwrap() = Some(e.to_string()),
    }
    let _ = app.emit("engine-state", ());
}

#[tauri::command]
fn engine_info(state: tauri::State<EngineState>) -> EngineView {
    let g = state.engine.lock().unwrap();
    let restarts = *state.restarts.lock().unwrap();
    let last_error = state.last_error.lock().unwrap().clone();
    match g.as_ref() {
        Some(e) => {
            let i = e.info();
            EngineView {
                running: e.is_running(),
                url: i.url.clone(),
                username: zandor_engine_supervisor::BASIC_USER,
                password: i.password.clone(),
                pid: i.pid,
                version: i.version.clone(),
                startup_ms: i.startup.as_millis(),
                restarts,
                last_error,
            }
        }
        None => EngineView {
            running: false,
            url: String::new(),
            username: zandor_engine_supervisor::BASIC_USER,
            password: String::new(),
            pid: 0,
            version: String::new(),
            startup_ms: 0,
            restarts,
            last_error,
        },
    }
}

#[tauri::command]
fn engine_crash_for_test(state: tauri::State<EngineState>) {
    if let Some(e) = state.engine.lock().unwrap().as_ref() {
        e.kill_main_process_for_test();
    }
}

#[tauri::command]
fn selftest_enabled() -> bool {
    std::env::var_os("ZANDOR_SELFTEST").is_some()
}

#[tauri::command]
fn selftest_done(app: AppHandle, report: serde_json::Value) {
    if let Some(path) = std::env::var_os("ZANDOR_SELFTEST") {
        let _ = std::fs::write(path, serde_json::to_string_pretty(&report).unwrap());
    }
    app.exit(0);
}

fn main() {
    let app = tauri::Builder::default()
        .manage(EngineState::default())
        .setup(|app| {
            let h = app.handle().clone();
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            *app.state::<EngineState>().start_tx.lock().unwrap() = Some(tx.clone());
            // Start di thread supervisor: jendela tampil tanpa menunggu engine.
            std::thread::Builder::new()
                .name("engine-supervisor".into())
                .spawn(move || {
                    for () in rx {
                        start_engine(&h);
                    }
                })?;
            tx.send(()).ok();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_info,
            engine_crash_for_test,
            selftest_enabled,
            selftest_done
        ])
        .build(tauri::generate_context!())
        .expect("gagal membangun app");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Jalur keluar normal. Jalur crash ditangani Job Object / PDEATHSIG.
            if let Some(mut e) = handle.state::<EngineState>().engine.lock().unwrap().take() {
                e.stop();
            }
        }
    });
}

//! Zandor desktop — titik masuk Tauri: engine opencode (sidecar), registry project, pemilih folder.

mod engine;
mod netprobe;
mod providers;
mod registry;
mod secrets;

use engine::{EngineManager, EnginePaths, EngineSnapshot};
use std::path::PathBuf;
use tauri::{Manager, RunEvent, State};

/// Windows: status "abaikan Ctrl+C" diwariskan dari proses induk ke semua turunan. Pulihkan sebelum
/// membuat engine/PTY agar Ctrl+C di terminal berfungsi (ADR-024).
fn reset_ctrl_c_inheritance() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0);
    }
}

fn app_data(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

#[tauri::command]
fn engine_state(manager: State<EngineManager>) -> EngineSnapshot {
    manager.snapshot()
}

#[tauri::command]
fn engine_restart(manager: State<EngineManager>) {
    manager.restart();
}

/// Direktori kerja untuk sesi tanpa project ("Chat").
#[tauri::command]
fn chats_dir(app: tauri::AppHandle) -> Result<String, String> {
    let d = app_data(&app)?.join("chats");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d.to_string_lossy().into_owned())
}

#[tauri::command]
fn registry_load(app: tauri::AppHandle) -> Result<Option<String>, String> {
    registry::load(&registry::registry_path(&app_data(&app)?))
}

#[tauri::command]
fn registry_save(app: tauri::AppHandle, text: String) -> Result<(), String> {
    registry::save(&registry::registry_path(&app_data(&app)?), &text)
}

// ---------- Provider (Fase 6) ----------

fn provider_store(app: &tauri::AppHandle) -> Result<providers::ProviderStore, String> {
    Ok(providers::ProviderStore::new(
        &app_data(app)?.join("providers.json"),
        Box::new(secrets::Keychain::new(secrets::SERVICE)),
    ))
}

#[tauri::command]
fn providers_list(app: tauri::AppHandle) -> Result<Vec<providers::ProviderView>, String> {
    provider_store(&app)?.views()
}

/// Simpan provider. `api_key` kosong/None mempertahankan kunci lama. Engine dimulai ulang agar konfigurasi terbaca.
#[tauri::command]
fn provider_save(
    app: tauri::AppHandle,
    manager: State<EngineManager>,
    provider: providers::Provider,
    api_key: Option<String>,
) -> Result<(), String> {
    let key = api_key.filter(|k| !k.is_empty());
    provider_store(&app)?.save(provider, key)?;
    manager.restart();
    Ok(())
}

#[tauri::command]
fn provider_delete(
    app: tauri::AppHandle,
    manager: State<EngineManager>,
    id: String,
) -> Result<(), String> {
    provider_store(&app)?.delete(&id)?;
    manager.restart();
    Ok(())
}

/// Ambil daftar model dari endpoint. Kunci: yang diketik pengguna, atau yang tersimpan untuk `provider_id`.
#[tauri::command]
async fn provider_probe(
    app: tauri::AppHandle,
    base_url: String,
    api_key: Option<String>,
    provider_id: Option<String>,
) -> Result<netprobe::ProbeResult, String> {
    let store = provider_store(&app)?;
    let key = match (api_key.filter(|k| !k.is_empty()), provider_id) {
        (Some(k), _) => Some(k),
        (None, Some(id)) => store.api_key(&id)?,
        _ => None,
    };
    tauri::async_runtime::spawn_blocking(move || {
        netprobe::probe(&base_url, key.as_deref(), std::time::Duration::from_secs(8))
    })
    .await
    .map_err(|e| e.to_string())
}

/// Ping satu model provider kustom/lokal (satu permintaan chat 1 token).
#[tauri::command]
async fn provider_ping(
    app: tauri::AppHandle,
    provider_id: String,
    model: String,
) -> Result<netprobe::ProbeResult, String> {
    let store = provider_store(&app)?;
    let (all, _) = store.load();
    let p = all
        .into_iter()
        .find(|p| p.id == provider_id)
        .ok_or("provider tidak ditemukan")?;
    let base = p
        .base_url
        .ok_or("provider ini tidak punya base URL (ping hanya untuk endpoint kustom/lokal)")?;
    let key = store.api_key(&provider_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        netprobe::ping_model(
            &base,
            key.as_deref(),
            &model,
            std::time::Duration::from_secs(90),
        )
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn providers_detect_local() -> Result<Vec<netprobe::LocalFound>, String> {
    tauri::async_runtime::spawn_blocking(netprobe::detect_local)
        .await
        .map_err(|e| e.to_string())
}

/// Buka URL (hanya https) di browser sistem — untuk sign-in provider.
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("hanya URL https yang boleh dibuka".into());
    }
    #[cfg(windows)]
    {
        // `cmd /c start` menafsirkan karakter khusus; PowerShell Start-Process dengan argumen terkutip aman.
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &url])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("belum didukung di platform ini (ADR-027)".into())
    }
}

fn sidecar_path() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_default();
    dir.join(if cfg!(windows) {
        "opencode.exe"
    } else {
        "opencode"
    })
}

fn plugin_path(app: &tauri::AppHandle) -> PathBuf {
    let from_resources = app
        .path()
        .resource_dir()
        .map(|d| d.join("resources").join("zandor-plugin.js"));
    match from_resources {
        Ok(p) if p.exists() => p,
        // Mode dev: berkas ada di src-tauri/resources (dibuat scripts/prepare-engine.ts).
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("zandor-plugin.js"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    reset_ctrl_c_inheritance();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(EngineManager::default())
        .setup(|app| {
            let handle = app.handle().clone();
            let data = app_data(&handle)?;
            std::fs::create_dir_all(&data)?;
            EngineManager::start(
                &handle,
                EnginePaths::new(&data, sidecar_path(), plugin_path(&handle)),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_state,
            engine_restart,
            chats_dir,
            registry_load,
            registry_save,
            providers_list,
            provider_save,
            provider_delete,
            provider_probe,
            provider_ping,
            providers_detect_local,
            open_external
        ])
        .build(tauri::generate_context!())
        .expect("gagal membangun Zandor");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Jalur keluar normal. Saat crash, Job Object menutup engine (ADR-019).
            handle.state::<EngineManager>().shutdown();
        }
    });
}

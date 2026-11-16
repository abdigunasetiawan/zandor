//! Zandor desktop — titik masuk Tauri: engine opencode (sidecar), registry project, pemilih folder.

mod browser;
mod bundled_skills;
mod engine;
mod files;
mod fonts;
mod git;
mod liveserver;
mod memory;
mod netprobe;
mod openvsx;
mod openwith;
mod openwork;
mod providers;
mod registry;
mod secrets;
mod shells;

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

/// Buka DevTools (Inspect Element) untuk jendela utama. Tersedia juga di build rilis lewat fitur `devtools` (ADR-062);
/// F12 dan klik kanan → Inspect memakai mekanisme WebView2 yang sama. Hanya webview UI lokal yang bisa memanggilnya.
#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
    window.open_devtools();
}

/// Direktori kerja untuk sesi tanpa project ("Chat").
#[tauri::command]
fn chats_dir(app: tauri::AppHandle) -> Result<String, String> {
    let d = app_data(&app)?.join("chats");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d.to_string_lossy().into_owned())
}

/// Buat folder kerja khusus satu sesi Chat di dalam folder Chat; mengembalikan path lengkapnya.
#[tauri::command]
fn chat_folder_new(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let root = app_data(&app)?.join("chats");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(files::chat_folder_create(&root, &name)?
        .to_string_lossy()
        .into_owned())
}

/// Path lengkap semua folder sesi Chat.
#[tauri::command]
fn chat_folders(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let root = app_data(&app)?.join("chats");
    Ok(files::chat_folder_list(&root)
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// Hapus folder sesi Chat hanya bila kosong.
#[tauri::command]
fn chat_folder_remove_if_empty(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let root = app_data(&app)?.join("chats");
    files::chat_folder_remove_if_empty(&root, std::path::Path::new(&path))
}

/// Nama pengguna OS (untuk sapaan dan info pengguna di sidebar). Kosong bila tidak diketahui.
#[tauri::command]
fn os_user() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default()
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

// ---------- Akun ganda per provider (ADR-056) ----------

#[tauri::command]
fn provider_account_add(
    app: tauri::AppHandle,
    manager: State<EngineManager>,
    provider_id: String,
    label: String,
    api_key: String,
    activate: Option<bool>,
) -> Result<String, String> {
    let id = provider_store(&app)?.add_account(
        &provider_id,
        &label,
        &api_key,
        activate.unwrap_or(true),
    )?;
    manager.restart();
    Ok(id)
}

#[tauri::command]
fn provider_account_activate(
    app: tauri::AppHandle,
    manager: State<EngineManager>,
    provider_id: String,
    account_id: String,
) -> Result<(), String> {
    provider_store(&app)?.activate_account(&provider_id, &account_id)?;
    manager.restart();
    Ok(())
}

#[tauri::command]
fn provider_account_rename(
    app: tauri::AppHandle,
    provider_id: String,
    account_id: String,
    label: String,
) -> Result<(), String> {
    provider_store(&app)?.rename_account(&provider_id, &account_id, &label)
}

#[tauri::command]
fn provider_account_remove(
    app: tauri::AppHandle,
    manager: State<EngineManager>,
    provider_id: String,
    account_id: String,
) -> Result<usize, String> {
    let left = provider_store(&app)?.remove_account(&provider_id, &account_id)?;
    manager.restart();
    Ok(left)
}

// ---------- OpenWork (device-code → kunci gateway → akun pada provider "openwork"; ADR-040, ADR-056) ----------

/// Alur device-code yang sedang berjalan (token/kode perangkat tidak pernah keluar dari Rust).
#[derive(Default)]
struct OpenWorkFlow(std::sync::Mutex<Option<openwork::Flow>>);

#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum OpenWorkPoll {
    Pending,
    SlowDown { interval: u64 },
    Denied { message: String },
    Expired,
    Error { message: String },
    Connected { account: String, models: usize },
}

#[tauri::command]
async fn openwork_start(flow: State<'_, OpenWorkFlow>) -> Result<openwork::DeviceInfo, String> {
    let (info, f) =
        tauri::async_runtime::spawn_blocking(|| openwork::start(&openwork::Endpoints::default()))
            .await
            .map_err(|e| e.to_string())??;
    *flow.0.lock().map_err(|_| "state rusak")? = Some(f);
    Ok(info)
}

#[tauri::command]
fn openwork_cancel(flow: State<'_, OpenWorkFlow>) {
    if let Ok(mut g) = flow.0.lock() {
        *g = None;
    }
}

/// Satu kali tanya. Saat disetujui: tukar token → kunci gateway → tambahkan sebagai akun pada provider "openwork"
/// (kunci di keychain) → mulai ulang engine. UI hanya menerima ringkasan akun (tanpa token/kunci).
#[tauri::command]
async fn openwork_poll(
    app: tauri::AppHandle,
    manager: State<'_, EngineManager>,
    flow: State<'_, OpenWorkFlow>,
) -> Result<OpenWorkPoll, String> {
    let current = flow.0.lock().map_err(|_| "state rusak")?.clone();
    let Some(current) = current else {
        return Ok(OpenWorkPoll::Error {
            message: "tidak ada proses masuk yang berjalan".into(),
        });
    };
    let store = provider_store(&app)?;
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        use openwork::Poll;
        let ep = openwork::Endpoints::default();
        match openwork::poll(&ep, &current) {
            Poll::Pending => (OpenWorkPoll::Pending, false, None),
            Poll::SlowDown(i) => (OpenWorkPoll::SlowDown { interval: i }, false, Some(i)),
            Poll::Denied(m) => (OpenWorkPoll::Denied { message: m }, true, None),
            Poll::Expired => (OpenWorkPoll::Expired, true, None),
            Poll::Error(m) => (OpenWorkPoll::Error { message: m }, true, None),
            Poll::Token(token) => match connect_openwork(&ep, &store, &token) {
                Ok(done) => (done, true, None),
                Err(m) => (OpenWorkPoll::Error { message: m }, true, None),
            },
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    let (out, finished, new_interval) = outcome;
    if let Ok(mut g) = flow.0.lock() {
        if finished {
            *g = None;
        } else if let (Some(f), Some(i)) = (g.as_mut(), new_interval) {
            f.interval = i;
        }
    }
    if matches!(out, OpenWorkPoll::Connected { .. }) {
        manager.restart();
    }
    Ok(out)
}

const OPENWORK_ID: &str = "openwork";

fn connect_openwork(
    ep: &openwork::Endpoints,
    store: &providers::ProviderStore,
    token: &str,
) -> Result<OpenWorkPoll, String> {
    let gw = openwork::resolve_gateway(ep, token)?;
    let Some(key) = gw.api_key else {
        return Err("Akun OpenWork tersambung, tetapi organisasi ini belum punya kredensial AI Gateway (org_credential_missing). Minta admin mengaktifkan inferensi atau coba akun lain.".into());
    };
    // daftar model: dari gateway, atau dari endpoint /models bila gateway tidak menyebutkannya
    let mut ids = gw.models;
    if ids.is_empty() {
        let probed = netprobe::probe(
            &ep.inference,
            Some(&key),
            std::time::Duration::from_secs(10),
        );
        ids = probed.models;
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
    // Provider dibuat sekali; model diperbarui, akun yang sudah ada dipertahankan (save menjaga `accounts`).
    let existing = store.load().0.into_iter().find(|p| p.id == OPENWORK_ID);
    let models = ids
        .iter()
        .map(|id| {
            existing
                .as_ref()
                .and_then(|e| e.models.iter().find(|m| &m.id == id).cloned())
                .unwrap_or_else(|| providers::ProviderModel {
                    id: id.clone(),
                    ..Default::default()
                })
        })
        .collect();
    store.save(
        providers::Provider {
            id: OPENWORK_ID.into(),
            name: "OpenWork Cloud".into(),
            kind: providers::Kind::Custom,
            base_url: Some(ep.inference.clone()),
            models,
            ..Default::default()
        },
        None,
    )?;
    store.add_account(OPENWORK_ID, &account, &key, true)?;
    Ok(OpenWorkPoll::Connected {
        account,
        models: ids.len(),
    })
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
    manager: State<'_, EngineManager>,
    provider_id: String,
    model: String,
) -> Result<netprobe::ProbeResult, String> {
    let store = provider_store(&app)?;
    let (all, _) = store.load();
    let stored_key = store.api_key(&provider_id)?;
    let base = all
        .into_iter()
        .find(|p| p.id == provider_id)
        .and_then(|p| p.base_url);
    let timeout = std::time::Duration::from_secs(90);
    if let Some(base) = base {
        // koneksi kustom/lokal milik pengguna: base URL dan kunci dari registry/keychain
        return tauri::async_runtime::spawn_blocking(move || {
            netprobe::ping_model(&base, stored_key.as_deref(), &model, timeout)
        })
        .await
        .map_err(|e| e.to_string());
    }
    // provider katalog: endpoint, adapter, dan kunci diambil Rust dari engine (tidak lewat JavaScript)
    let snap = manager.snapshot();
    if snap.status != "ready" {
        return Ok(netprobe::failed("network", "engine belum siap".into()));
    }
    tauri::async_runtime::spawn_blocking(move || {
        match netprobe::engine_target(
            &snap.url,
            snap.username,
            &snap.password,
            &provider_id,
            &model,
            std::time::Duration::from_secs(10),
        ) {
            Ok(mut target) => {
                if target.key.is_none() {
                    target.key = stored_key;
                }
                netprobe::ping_target(&target, &model, timeout)
            }
            Err(failed) => failed,
        }
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

// ---------- Tema editor dari Open VSX (Fase 10) ----------

#[tauri::command]
async fn openvsx_search(query: String) -> Result<Vec<openvsx::OvsxItem>, String> {
    tauri::async_runtime::spawn_blocking(move || openvsx::search_themes(&query))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn openvsx_download(url: String) -> Result<tauri::ipc::Response, String> {
    let bytes = tauri::async_runtime::spawn_blocking(move || openvsx::download_vsix(&url))
        .await
        .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(bytes))
}

// ---------- Memori dan skill (Fase 9) ----------

fn memory_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data(app)?.join("memory"))
}

fn skills_dir(
    app: &tauri::AppHandle,
    scope: &str,
    project: Option<&str>,
) -> Result<PathBuf, String> {
    let data = app_data(app)?;
    let project_dir = match (scope, project) {
        ("project", Some(p)) => Some(files::resolve_dir(&files::allowed_roots(&data), p)?),
        _ => None,
    };
    memory::skills_root(scope, project_dir.as_deref(), &data.join("engine"))
}

#[tauri::command]
fn memory_list(app: tauri::AppHandle) -> Result<Vec<memory::MemoryFile>, String> {
    Ok(memory::list_memory(&memory_dir(&app)?))
}

#[tauri::command]
fn memory_read(app: tauri::AppHandle, rel: String) -> Result<String, String> {
    memory::read_memory(&memory_dir(&app)?, &rel)
}

#[tauri::command]
fn memory_write(app: tauri::AppHandle, rel: String, text: String) -> Result<(), String> {
    memory::write_memory(&memory_dir(&app)?, &rel, &text)
}

#[tauri::command]
fn memory_delete(app: tauri::AppHandle, rel: String) -> Result<(), String> {
    memory::delete_memory(&memory_dir(&app)?, &rel)
}

#[tauri::command]
fn skill_list(
    app: tauri::AppHandle,
    scope: String,
    project: Option<String>,
) -> Result<Vec<memory::SkillInfo>, String> {
    let root = skills_dir(&app, &scope, project.as_deref())?;
    Ok(memory::list_skills(&root, &scope))
}

#[tauri::command]
fn skill_read(
    app: tauri::AppHandle,
    scope: String,
    project: Option<String>,
    name: String,
) -> Result<String, String> {
    memory::read_skill(&skills_dir(&app, &scope, project.as_deref())?, &name)
}

#[tauri::command]
fn skill_write(
    app: tauri::AppHandle,
    scope: String,
    project: Option<String>,
    name: String,
    markdown: String,
    overwrite: bool,
) -> Result<String, String> {
    memory::write_skill(
        &skills_dir(&app, &scope, project.as_deref())?,
        &name,
        &markdown,
        overwrite,
    )
}

#[tauri::command]
fn skill_delete(
    app: tauri::AppHandle,
    scope: String,
    project: Option<String>,
    name: String,
) -> Result<(), String> {
    memory::delete_skill(&skills_dir(&app, &scope, project.as_deref())?, &name)
}

// ---------- Berkas untuk Panel 3 (Fase 7) ----------

fn resolve_file(app: &tauri::AppHandle, root: &str, path: &str) -> Result<PathBuf, String> {
    let roots = files::allowed_roots(&app_data(app)?);
    files::resolve_in_root(&roots, root, path)
}

/// Baca byte persis sebuah berkas di dalam project terdaftar (maks 100 MB).
#[tauri::command]
fn fs_read(
    app: tauri::AppHandle,
    root: String,
    path: String,
) -> Result<tauri::ipc::Response, String> {
    let file = resolve_file(&app, &root, &path)?;
    Ok(tauri::ipc::Response::new(files::read_limited(
        &file,
        files::MAX_READ_BYTES,
    )?))
}

/// Ukuran dan waktu modifikasi berkas (untuk mendeteksi perubahan di luar Zandor sebelum menyimpan).
#[tauri::command]
fn fs_stat(app: tauri::AppHandle, root: String, path: String) -> Result<files::FileStat, String> {
    files::stat(&resolve_file(&app, &root, &path)?)
}

/// Simpan teks ke berkas project yang sudah ada (atomik). `expected_modified` mendeteksi perubahan di disk.
#[tauri::command]
fn fs_write(
    app: tauri::AppHandle,
    root: String,
    path: String,
    text: String,
    expected_modified: Option<u64>,
) -> Result<u64, String> {
    files::write_text(&resolve_file(&app, &root, &path)?, &text, expected_modified)
}

/// Buat berkas kosong atau folder baru di dalam folder `parent` milik project terdaftar (tidak pernah menimpa).
#[tauri::command]
fn fs_create(
    app: tauri::AppHandle,
    root: String,
    parent: String,
    name: String,
    dir: bool,
) -> Result<String, String> {
    let parent_dir = resolve_file(&app, &root, &parent)?;
    let created = files::create_entry(&parent_dir, &name, dir)?;
    Ok(created.to_string_lossy().into_owned())
}

/// Salin berkas project ke lokasi yang dipilih pengguna lewat dialog "Simpan sebagai".
#[tauri::command]
fn fs_copy_to(
    app: tauri::AppHandle,
    root: String,
    path: String,
    dest: String,
) -> Result<(), String> {
    let file = resolve_file(&app, &root, &path)?;
    files::read_limited(&file, files::MAX_READ_BYTES)?; // validasi jenis dan ukuran
    std::fs::copy(&file, &dest).map_err(|e| format!("gagal menyalin: {e}"))?;
    Ok(())
}

/// Simpan byte hasil ekspor respons (teks/PNG/SVG) ke lokasi yang dipilih pengguna lewat dialog "Simpan sebagai".
#[tauri::command]
fn fs_save_as(dest: String, data: Vec<u8>) -> Result<(), String> {
    files::save_bytes(std::path::Path::new(&dest), &data)
}

/// Buka berkas dengan aplikasi bawaan sistem. Berkas yang dapat dieksekusi ditolak.
#[tauri::command]
fn fs_open_external(app: tauri::AppHandle, root: String, path: String) -> Result<(), String> {
    let file = resolve_file(&app, &root, &path)?;
    if files::is_executable_ext(&file) {
        return Err("berkas yang dapat dieksekusi tidak dibuka dari Zandor".into());
    }
    #[cfg(windows)]
    {
        // rundll32 dengan argumen terpisah (bukan lewat shell) → tidak ada penafsiran karakter khusus.
        std::process::Command::new("rundll32")
            .arg("url.dll,FileProtocolHandler")
            .arg(&file)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Err("belum didukung di platform ini (ADR-027)".into())
    }
}

/// Shell yang terpasang di mesin ini (beserta ikon PNG base64) untuk pemilih shell panel Terminal.
#[tauri::command]
async fn terminal_shells(refresh: bool) -> Result<Vec<shells::ShellInfo>, String> {
    git_blocking(move || shells::detect(refresh)).await
}

/// Keluarga font yang terinstal di sistem (pemilih jenis huruf kustomisasi Panel 2).
#[tauri::command]
async fn system_fonts(refresh: bool) -> Result<Vec<String>, String> {
    git_blocking(move || fonts::list(refresh)).await
}

/// Daftar aplikasi terdaftar untuk ekstensi berkas (beserta ikon PNG base64) — untuk menu "Buka dengan".
#[tauri::command]
async fn fs_open_with_list(
    app: tauri::AppHandle,
    root: String,
    path: String,
    text_like: bool,
    refresh: bool,
) -> Result<Vec<openwith::OpenApp>, String> {
    let file = resolve_file(&app, &root, &path)?;
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("berkas tanpa ekstensi")?
        .to_string();
    git_blocking(move || openwith::apps_for(&ext, text_like, refresh)).await
}

/// Buka berkas dengan aplikasi pilihan; hanya aplikasi yang ada di daftar ekstensi itu yang diterima.
#[tauri::command]
async fn fs_open_with(
    app: tauri::AppHandle,
    root: String,
    path: String,
    app_path: String,
    text_like: bool,
) -> Result<(), String> {
    let file = resolve_file(&app, &root, &path)?;
    if files::is_executable_ext(&file) {
        return Err("berkas yang dapat dieksekusi tidak dibuka dari Zandor".into());
    }
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("berkas tanpa ekstensi")?
        .to_string();
    git_blocking(move || {
        let apps = openwith::apps_for(&ext, text_like, false)?;
        if !openwith::app_allowed(&apps, &app_path) {
            return Err("aplikasi ini tidak terdaftar untuk jenis berkas tersebut".to_string());
        }
        openwith::launch(&app_path, &file)
    })
    .await
}

/// Tampilkan berkas di File Explorer.
#[tauri::command]
fn fs_reveal(app: tauri::AppHandle, root: String, path: String) -> Result<(), String> {
    openwith::reveal(&resolve_file(&app, &root, &path)?)
}

/// Buka live server untuk folder project dan kembalikan URL berkas HTML `path` (Browser Panel 4, auto-reload).
#[tauri::command]
fn doc_live_server(app: tauri::AppHandle, root: String, path: String) -> Result<String, String> {
    let file = resolve_file(&app, &root, &path)?;
    let dir = files::resolve_dir(&files::allowed_roots(&app_data(&app)?), &root)?;
    let rel = file
        .strip_prefix(&dir)
        .map_err(|_| "berkas berada di luar project".to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    liveserver::start(&dir, &rel)
}

// ---------- Git (Fase 8) ----------

fn git_dir(app: &tauri::AppHandle, root: &str) -> Result<PathBuf, String> {
    let roots = files::allowed_roots(&app_data(app)?);
    // root project terdaftar, atau anak foldernya (repo git di dalam monorepo)
    files::resolve_dir_within(&roots, root)
}

/// Repo git di anak folder project (untuk monorepo yang akarnya bukan repo).
#[tauri::command]
async fn git_repos(app: tauri::AppHandle, root: String) -> Result<Vec<String>, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || Ok(git::find_repos(&dir))).await
}

async fn git_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn git_push(app: tauri::AppHandle, root: String) -> Result<String, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::push(&dir)).await
}

#[tauri::command]
async fn git_pull(app: tauri::AppHandle, root: String) -> Result<String, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::pull(&dir)).await
}

#[tauri::command]
async fn git_stash_push(
    app: tauri::AppHandle,
    root: String,
    message: String,
) -> Result<String, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::stash_push(&dir, &message)).await
}

#[tauri::command]
async fn git_stash_pop(app: tauri::AppHandle, root: String) -> Result<String, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::stash_pop(&dir)).await
}

#[tauri::command]
async fn git_stash_count(app: tauri::AppHandle, root: String) -> Result<usize, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::stash_count(&dir)).await
}

/// Destruktif: UI wajib sudah meminta konfirmasi.
#[tauri::command]
async fn git_discard(
    app: tauri::AppHandle,
    root: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::discard(&dir, &paths)).await
}

#[tauri::command]
async fn git_status(app: tauri::AppHandle, root: String) -> Result<git::GitStatus, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::status(&dir)).await
}

#[tauri::command]
async fn git_diff(
    app: tauri::AppHandle,
    root: String,
    path: String,
    staged: bool,
    untracked: bool,
) -> Result<String, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::diff(&dir, &path, staged, untracked)).await
}

#[tauri::command]
async fn git_stage(app: tauri::AppHandle, root: String, paths: Vec<String>) -> Result<(), String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::stage(&dir, &paths)).await
}

#[tauri::command]
async fn git_unstage(
    app: tauri::AppHandle,
    root: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::unstage(&dir, &paths)).await
}

#[tauri::command]
async fn git_commit(
    app: tauri::AppHandle,
    root: String,
    message: String,
) -> Result<String, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::commit(&dir, &message)).await
}

#[tauri::command]
async fn git_branches(app: tauri::AppHandle, root: String) -> Result<Vec<git::Branch>, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::branches(&dir)).await
}

#[tauri::command]
async fn git_switch(app: tauri::AppHandle, root: String, name: String) -> Result<(), String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::switch(&dir, &name)).await
}

#[tauri::command]
async fn git_create_branch(
    app: tauri::AppHandle,
    root: String,
    name: String,
) -> Result<(), String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::create_branch(&dir, &name)).await
}

#[tauri::command]
async fn git_log(
    app: tauri::AppHandle,
    root: String,
    count: usize,
) -> Result<Vec<git::CommitInfo>, String> {
    let dir = git_dir(&app, &root)?;
    git_blocking(move || git::log(&dir, count)).await
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
        // Harus plugin pertama: instans kedua langsung keluar (DB engine terkunci bila dua Zandor jalan) dan jendela
        // instans pertama dimunculkan.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(EngineManager::default())
        .manage(OpenWorkFlow::default())
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
            open_devtools,
            os_user,
            chat_folder_new,
            chat_folders,
            chat_folder_remove_if_empty,
            registry_load,
            registry_save,
            providers_list,
            provider_save,
            provider_delete,
            provider_account_add,
            provider_account_activate,
            provider_account_rename,
            provider_account_remove,
            openwork_start,
            openwork_poll,
            openwork_cancel,
            provider_probe,
            provider_ping,
            providers_detect_local,
            open_external,
            openvsx_search,
            openvsx_download,
            memory_list,
            memory_read,
            memory_write,
            memory_delete,
            skill_list,
            skill_read,
            skill_write,
            skill_delete,
            fs_read,
            fs_stat,
            fs_write,
            fs_create,
            fs_copy_to,
            fs_save_as,
            fs_open_external,
            fs_open_with_list,
            terminal_shells,
            system_fonts,
            fs_open_with,
            fs_reveal,
            doc_live_server,
            git_status,
            git_push,
            git_pull,
            git_stash_push,
            git_stash_pop,
            git_stash_count,
            git_discard,
            git_repos,
            git_diff,
            git_stage,
            git_unstage,
            git_commit,
            git_branches,
            git_switch,
            git_create_branch,
            git_log,
            browser::browser_mode,
            browser::browser_home,
            browser::browser_open,
            browser::browser_set_bounds,
            browser::browser_visible,
            browser::browser_navigate,
            browser::browser_history,
            browser::browser_close
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

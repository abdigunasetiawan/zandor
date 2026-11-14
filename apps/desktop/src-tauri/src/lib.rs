//! Zandor desktop — titik masuk Tauri: engine opencode (sidecar), registry project, pemilih folder.

mod browser;
mod engine;
mod files;
mod git;
mod memory;
mod netprobe;
mod openvsx;
mod openwith;
mod openwork;
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

// ---------- Masuk dengan OpenWork (Fase 12) ----------

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

/// Satu kali tanya. Saat disetujui: tukar token → kunci gateway → simpan provider "openwork" (kunci di keychain) →
/// mulai ulang engine. UI hanya menerima ringkasan akun (tanpa token/kunci).
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
    let provider = providers::Provider {
        id: "openwork".into(),
        name: "OpenWork Cloud".into(),
        kind: providers::Kind::Custom,
        base_url: Some(ep.inference.clone()),
        models: ids
            .iter()
            .map(|id| providers::ProviderModel {
                id: id.clone(),
                name: None,
            })
            .collect(),
        api_key: None,
    };
    store.save(provider, Some(key))?;
    let account = match (gw.email.is_empty(), gw.org_name) {
        (false, Some(org)) => format!("{} · {org}", gw.email),
        (false, None) => gw.email,
        (true, Some(org)) => org,
        (true, None) => "Akun OpenWork".into(),
    };
    Ok(OpenWorkPoll::Connected {
        account,
        models: ids.len(),
    })
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

// ---------- Pratinjau HTML dengan sumber luar (opt-in) ----------

type Previews = std::sync::Mutex<std::collections::VecDeque<(String, Vec<u8>)>>;
fn previews() -> &'static Previews {
    static P: std::sync::OnceLock<Previews> = std::sync::OnceLock::new();
    P.get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()))
}

/// Simpan HTML di memori dan kembalikan token untuk skema `zpreview://`. Hanya 8 pratinjau terakhir disimpan.
#[tauri::command]
fn doc_preview_register(html: String) -> Result<String, String> {
    if html.len() > files::MAX_WRITE_BYTES {
        return Err("HTML terlalu besar untuk pratinjau".into());
    }
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let token = format!(
        "p{:x}{:x}",
        nanos,
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let mut q = previews().lock().map_err(|e| e.to_string())?;
    q.push_back((token.clone(), html.into_bytes()));
    while q.len() > 8 {
        q.pop_front();
    }
    Ok(token)
}

/// CSP dokumen pratinjau: sumber https dan skrip diizinkan (pengguna memilihnya), tetapi tanpa form, tanpa base, dan
/// iframe-nya tetap ber-sandbox tanpa `allow-same-origin` (origin buram, tidak bisa menjangkau aplikasi).
const PREVIEW_CSP: &str = "default-src 'none'; img-src https: data: blob:; media-src https: data: blob:; font-src https: data:; style-src https: 'unsafe-inline'; script-src https: 'unsafe-inline' 'unsafe-eval'; connect-src https:; frame-src https:; form-action 'none'; base-uri 'none'";

fn preview_response(request: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let token = request.uri().path().trim_matches('/').to_string();
    let body = previews()
        .lock()
        .ok()
        .and_then(|q| q.iter().find(|(t, _)| *t == token).map(|(_, b)| b.clone()));
    match body {
        Some(b) => tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Content-Security-Policy", PREVIEW_CSP)
            .header("X-Content-Type-Options", "nosniff")
            .header("Cache-Control", "no-store")
            .body(b)
            .unwrap_or_default(),
        None => tauri::http::Response::builder()
            .status(404)
            .body(Vec::new())
            .unwrap_or_default(),
    }
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .register_uri_scheme_protocol("zpreview", |_ctx, request| preview_response(&request))
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
            os_user,
            chat_folder_new,
            chat_folders,
            chat_folder_remove_if_empty,
            registry_load,
            registry_save,
            openwork_start,
            openwork_poll,
            openwork_cancel,
            providers_list,
            provider_save,
            provider_delete,
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
            fs_open_with,
            fs_reveal,
            doc_preview_register,
            git_status,
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

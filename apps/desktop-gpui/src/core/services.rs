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

    /// Beri tahu pengamat engine bahwa daftar model berubah tanpa memulai ulang engine (mis. agent CLI
    /// diaktifkan atau modelnya diambil ulang); pemilih model dan Pengaturan memuat ulang daftarnya.
    pub fn models_changed(cx: &mut App) {
        if let Some(h) = cx.try_global::<EngineHandle>().map(|h| h.0.clone()) {
            h.update(cx, |_, cx| cx.notify());
        }
    }
}

/// Nama folder data: `%APPDATA%\Zandor` (rilis) dan `%APPDATA%\ZandorDev` (build debug, agar data
/// pengembangan tidak mencampuri data harian). Nama lama (identifier Tauri) dimigrasikan sekali (ADR-084).
const DATA_NAME: &str = if cfg!(debug_assertions) { "ZandorDev" } else { "Zandor" };
const LEGACY_NAME: &str = if cfg!(debug_assertions) { "id.web.zandor.dev" } else { "id.web.zandor.app" };

struct DataDirs {
    current: PathBuf,
    /// Jalur lama yang masih menunjuk ke data yang sama (junction setelah migrasi). Sesi engine menyimpan
    /// direktori kerja sebagai jalur absolut, jadi sesi Chat lama tercatat di bawah jalur ini.
    legacy: Option<PathBuf>,
}

static DATA_DIRS: std::sync::OnceLock<DataDirs> = std::sync::OnceLock::new();

fn data_dirs() -> &'static DataDirs {
    DATA_DIRS.get_or_init(|| {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        resolve_data_dirs(&base.join(DATA_NAME), &base.join(LEGACY_NAME), make_junction)
    })
}

/// Tentukan folder data dan migrasikan folder lama bila perlu: pindahkan (rename, atomik di volume yang
/// sama) lalu pasang junction di jalur lama supaya jalur absolut yang tersimpan tetap berlaku. Bila
/// pemindahan gagal (mis. Zandor versi lama masih berjalan), sesi ini tetap memakai folder lama dan
/// migrasi dicoba lagi pada start berikutnya.
fn resolve_data_dirs(
    new: &std::path::Path,
    old: &std::path::Path,
    link: impl Fn(&std::path::Path, &std::path::Path) -> bool,
) -> DataDirs {
    let legacy = |p: &std::path::Path| p.exists().then(|| p.to_path_buf());
    if new.exists() {
        return DataDirs { current: new.to_path_buf(), legacy: legacy(old) };
    }
    if !old.exists() {
        return DataDirs { current: new.to_path_buf(), legacy: None };
    }
    match std::fs::rename(old, new) {
        Ok(()) => {
            if !link(old, new) {
                eprintln!("[zandor] data: junction {} gagal; sesi Chat lama mungkin tidak terbuka", old.display());
            }
            eprintln!("[zandor] data: dipindah {} -> {}", old.display(), new.display());
            DataDirs { current: new.to_path_buf(), legacy: legacy(old) }
        }
        Err(e) => {
            eprintln!("[zandor] data: gagal memindah {} ({e}); memakai folder lama", old.display());
            DataDirs { current: old.to_path_buf(), legacy: None }
        }
    }
}

#[cfg(windows)]
fn make_junction(link: &std::path::Path, target: &std::path::Path) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(not(windows))]
fn make_junction(link: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

/// Direktori data aplikasi (lihat `DATA_NAME`). Panggilan pertama menjalankan migrasi folder lama.
pub fn app_data_dir() -> PathBuf {
    data_dirs().current.clone()
}

/// Jalur lama folder data yang masih menunjuk ke data yang sama (atau `None`).
pub fn legacy_data_dir() -> Option<PathBuf> {
    data_dirs().legacy.clone()
}

fn repo_tauri_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../desktop/src-tauri")
}

/// Binary engine: di samping exe (rilis) atau binaries/ milik Tauri (dev).
pub(crate) fn sidecar_path() -> PathBuf {
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
    super::activity::Activity::init(cx);
    let data = app_data_dir();
    let _ = std::fs::create_dir_all(&data);
    let projects = ProjectStore::init(
        ProjectHost {
            registry: crate::registry::registry_path(&data),
            chats: data.join("chats"),
            legacy_chats: legacy_data_dir().map(|d| d.join("chats")),
        },
        cx,
    );

    super::profile::Profile::init(cx);
    super::model_prefs::ModelPrefsStore::init(cx);
    super::pings::PingStore::init(cx);
    super::appearance::AppearanceStore::init(data.join("appearance.json"), cx);
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
                    if status.read(cx).state.status != st.status {
                        let lvl = if st.error.is_some() { "error" } else { "info" };
                        let msg = format!("engine: {}{}", st.status, st.error.as_ref().map(|e| format!(" — {e}")).unwrap_or_default());
                        super::activity::log(cx, "engine", lvl, msg);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zandor-data-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn folder_lama_dipindah_ke_nama_baru() {
        let base = tmp("pindah");
        let (new, old) = (base.join("Zandor"), base.join("id.web.zandor.app"));
        std::fs::create_dir_all(old.join("chats")).unwrap();
        std::fs::write(old.join("prefs.json"), "{}").unwrap();
        let d = resolve_data_dirs(&new, &old, |l, t| {
            assert_eq!(t, new.as_path());
            std::fs::create_dir_all(l).is_ok()
        });
        assert_eq!(d.current, new);
        assert!(new.join("prefs.json").exists() && new.join("chats").is_dir());
        assert_eq!(d.legacy.as_deref(), Some(old.as_path()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn folder_baru_sudah_ada_tidak_dimigrasi() {
        let base = tmp("ada");
        let (new, old) = (base.join("Zandor"), base.join("lama"));
        std::fs::create_dir_all(&new).unwrap();
        let d = resolve_data_dirs(&new, &old, |_, _| panic!("tidak boleh membuat junction"));
        assert_eq!(d.current, new);
        assert!(d.legacy.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn instalasi_baru_memakai_nama_baru() {
        let base = tmp("baru");
        let d = resolve_data_dirs(&base.join("Zandor"), &base.join("lama"), |_, _| panic!("tidak ada yang dipindah"));
        assert_eq!(d.current, base.join("Zandor"));
        assert!(d.legacy.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }
}

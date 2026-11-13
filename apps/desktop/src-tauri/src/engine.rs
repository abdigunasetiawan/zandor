//! Manajer engine: menjalankan `opencode serve` (sidecar) lewat `zandor-engine-supervisor`, memuat plugin guard
//! (ADR-022) dan memverifikasinya, memantau, me-restart bila mati, dan mengabarkan status ke UI (event
//! `engine-state`). Semua start/restart lewat SATU thread supervisor yang hidup sepanjang aplikasi.

use crate::providers::{self, Provider};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use zandor_engine_supervisor::{http, Engine, EngineConfig, ExitInfo, BASIC_USER};

/// Versi engine yang di-pin. Harus sama dengan `engine.json` (dijaga test).
pub const PINNED_ENGINE_VERSION: &str = "1.19.5";
const MAX_RESTARTS_IN_WINDOW: usize = 3;
const RESTART_WINDOW: Duration = Duration::from_secs(60);
const GUARD_WAIT: Duration = Duration::from_secs(8);

#[derive(Clone, Serialize)]
pub struct EngineSnapshot {
    /// stopped | starting | ready | restarting | error
    pub status: &'static str,
    pub url: String,
    pub username: &'static str,
    pub password: String,
    pub version: String,
    pub pid: u32,
    pub restarts: u32,
    pub error: Option<String>,
    pub startup_ms: u64,
}

impl EngineSnapshot {
    fn new(status: &'static str) -> Self {
        Self {
            status,
            url: String::new(),
            username: BASIC_USER,
            password: String::new(),
            version: String::new(),
            pid: 0,
            restarts: 0,
            error: None,
            startup_ms: 0,
        }
    }
}

/// Lokasi berkas yang dipakai engine. Semua data engine milik Zandor (terisolasi dari konfigurasi opencode user).
#[derive(Clone, Debug)]
pub struct EnginePaths {
    pub sidecar: PathBuf,
    pub plugin: PathBuf,
    /// Akar XDG_* (data/config/cache/state) untuk engine.
    pub engine_root: PathBuf,
    /// Direktori kerja sesi tanpa project ("Chat").
    pub chats: PathBuf,
    pub marker: PathBuf,
    /// Daftar provider model kustom (berisi kunci API; di luar repo).
    pub providers: PathBuf,
}

impl EnginePaths {
    pub fn new(app_data: &Path, sidecar: PathBuf, plugin: PathBuf) -> Self {
        let engine_root = app_data.join("engine");
        Self {
            sidecar,
            plugin,
            marker: engine_root.join("guard-loaded"),
            engine_root,
            chats: app_data.join("chats"),
            providers: app_data.join("providers.json"),
        }
    }
}

/// `file:///C:/a%20b/x.js` dari path Windows/Unix.
pub fn file_url(path: &Path) -> String {
    // Tauri `resource_dir()` di Windows memakai awalan verbatim `\\?\`; bila tidak dibuang, URL tidak valid dan
    // opencode melewati plugin secara diam-diam (guard tidak termuat).
    let raw = path.to_string_lossy();
    let raw = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let s = raw.replace('\\', "/");
    let encoded: String = s
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '?' => "%3F".to_string(),
            c => c.to_string(),
        })
        .collect();
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

/// Percent-encode untuk query string (semua selain unreserved RFC 3986).
pub fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b => format!("%{b:02X}"),
        })
        .collect()
}

/// Isi `OPENCODE_CONFIG_CONTENT`: plugin guard + provider kustom. Tidak pernah memuat nilai secret: kunci API
/// hanya berupa rujukan `{env:...}` yang nilainya dikirim lewat environment proses engine.
pub fn config_content(plugin: &Path, marker: &Path, providers: &[Provider]) -> String {
    serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "autoupdate": false,
        "plugin": [[file_url(plugin), { "markerFile": marker.to_string_lossy() }]],
        // Bawaan aman: opencode mengizinkan edit/shell tanpa bertanya. Zandor meminta persetujuan (kartu izin di chat).
        "permission": { "edit": "ask", "bash": "ask", "webfetch": "ask", "external_directory": "ask" },
        "provider": providers::config_section(providers),
    })
    .to_string()
}

fn engine_config(paths: &EnginePaths) -> EngineConfig {
    let mut cfg = EngineConfig::new(&paths.sidecar);
    cfg.cwd = Some(paths.engine_root.clone());
    cfg.expected_version = Some(PINNED_ENGINE_VERSION.to_string());
    cfg.ready_timeout = Duration::from_secs(20);
    for (k, dir) in [
        ("DATA", "data"),
        ("CONFIG", "config"),
        ("CACHE", "cache"),
        ("STATE", "state"),
    ] {
        cfg.env.push((
            format!("XDG_{k}_HOME"),
            paths.engine_root.join(dir).to_string_lossy().into_owned(),
        ));
    }
    let (providers, warnings) = providers::load(&paths.providers);
    for w in warnings {
        eprintln!("[zandor] {w}");
    }
    for p in &providers {
        cfg.env.push((p.env_name(), p.api_key.clone()));
    }
    cfg.env.push((
        "OPENCODE_CONFIG_CONTENT".into(),
        config_content(&paths.plugin, &paths.marker, &providers),
    ));
    cfg
}

enum Msg {
    Start,
    Restart,
    Crashed(ExitInfo),
    Shutdown,
}

pub struct EngineManager {
    snapshot: Mutex<EngineSnapshot>,
    tx: Mutex<Option<Sender<Msg>>>,
}

impl Default for EngineManager {
    fn default() -> Self {
        Self {
            snapshot: Mutex::new(EngineSnapshot::new("stopped")),
            tx: Mutex::new(None),
        }
    }
}

impl EngineManager {
    pub fn snapshot(&self) -> EngineSnapshot {
        self.snapshot.lock().unwrap().clone()
    }

    fn set(&self, app: &AppHandle, snap: EngineSnapshot) {
        *self.snapshot.lock().unwrap() = snap.clone();
        let _ = app.emit("engine-state", snap);
    }

    fn send(&self, msg: Msg) {
        if let Some(tx) = self.tx.lock().unwrap().as_ref() {
            let _ = tx.send(msg);
        }
    }

    pub fn restart(&self) {
        self.send(Msg::Restart);
    }

    pub fn shutdown(&self) {
        self.send(Msg::Shutdown);
    }

    /// Mulai thread supervisor dan jalankan engine.
    pub fn start(app: &AppHandle, paths: EnginePaths) {
        let manager = app.state::<EngineManager>();
        let (tx, rx) = mpsc::channel::<Msg>();
        *manager.tx.lock().unwrap() = Some(tx.clone());
        let app_thread = app.clone();
        // cwd engine dan data XDG harus ada sebelum spawn (os error 267 bila tidak).
        for dir in [&paths.chats, &paths.engine_root] {
            let _ = std::fs::create_dir_all(dir);
        }
        let spawned = std::thread::Builder::new().name("engine-supervisor".into()).spawn(move || {
            let app = app_thread;
            let manager = app.state::<EngineManager>();
            let mut engine: Option<Engine> = None;
            let mut restarts: Vec<Instant> = Vec::new();
            let mut total_restarts = 0u32;
            for msg in rx {
                match msg {
                    Msg::Shutdown => {
                        engine.take();
                        manager.set(&app, EngineSnapshot::new("stopped"));
                        break;
                    }
                    Msg::Crashed(exit) => {
                        engine.take();
                        let now = Instant::now();
                        restarts.retain(|t| now.duration_since(*t) < RESTART_WINDOW);
                        let tail = exit.last_logs.iter().rev().take(5).rev().cloned().collect::<Vec<_>>().join("\n");
                        if restarts.len() >= MAX_RESTARTS_IN_WINDOW {
                            let mut s = EngineSnapshot::new("error");
                            s.restarts = total_restarts;
                            s.error = Some(format!(
                                "engine berhenti {} kali dalam {} detik; restart otomatis dihentikan.\n{tail}",
                                restarts.len() + 1,
                                RESTART_WINDOW.as_secs()
                            ));
                            manager.set(&app, s);
                            continue;
                        }
                        restarts.push(now);
                        total_restarts += 1;
                        let mut s = EngineSnapshot::new("restarting");
                        s.restarts = total_restarts;
                        manager.set(&app, s);
                        std::thread::sleep(Duration::from_millis(300 * restarts.len() as u64));
                        let _ = tx.send(Msg::Start);
                    }
                    Msg::Start | Msg::Restart => {
                        let user_retry = matches!(msg, Msg::Restart);
                        engine.take();
                        if user_retry {
                            restarts.clear();
                        }
                        let mut starting = EngineSnapshot::new("starting");
                        starting.restarts = total_restarts;
                        manager.set(&app, starting);
                        match launch(&paths, tx.clone()) {
                            Ok(e) => {
                                let i = e.info().clone();
                                manager.set(
                                    &app,
                                    EngineSnapshot {
                                        status: "ready",
                                        url: i.url.clone(),
                                        username: BASIC_USER,
                                        password: i.password.clone(),
                                        version: i.version.clone(),
                                        pid: i.pid,
                                        restarts: total_restarts,
                                        error: None,
                                        startup_ms: i.startup.as_millis() as u64,
                                    },
                                );
                                engine = Some(e);
                            }
                            Err(err) => {
                                let mut s = EngineSnapshot::new("error");
                                s.restarts = total_restarts;
                                s.error = Some(err);
                                manager.set(&app, s);
                            }
                        }
                    }
                }
            }
        });
        if let Err(e) = spawned {
            let mut s = EngineSnapshot::new("error");
            s.error = Some(format!("gagal membuat thread supervisor: {e}"));
            manager.set(app, s);
        } else {
            manager.send(Msg::Start);
        }
    }
}

fn launch(paths: &EnginePaths, tx: Sender<Msg>) -> Result<Engine, String> {
    if !paths.sidecar.exists() {
        return Err(format!(
            "binary engine tidak ditemukan: {:?} (jalankan `bun run engine:fetch`)",
            paths.sidecar
        ));
    }
    if !paths.plugin.exists() {
        return Err(format!(
            "plugin Zandor tidak ditemukan: {:?} (jalankan `bun run prepare:engine`)",
            paths.plugin
        ));
    }
    let _ = std::fs::remove_file(&paths.marker);
    let mut engine = Engine::start(engine_config(paths), move |exit| {
        let _ = tx.send(Msg::Crashed(exit));
    })
    .map_err(|e| e.to_string())?;

    // opencode memuat plugin per-instance (saat request pertama berdirektori), bukan saat server start. Pancing
    // instance direktori chat sekarang supaya plugin guard termuat dan penandanya bisa diverifikasi.
    let warmup = {
        let info = engine.info();
        let path = format!(
            "/session?directory={}",
            percent_encode(&paths.chats.to_string_lossy())
        );
        match http(
            &info.host_port,
            "GET",
            &path,
            Some(&info.basic_auth_header()),
            Duration::from_secs(10),
        ) {
            Ok((code, _)) => format!("HTTP {code}"),
            Err(e) => format!("galat: {e}"),
        }
    };

    // Plugin guard wajib termuat: tanpa itu proses anak engine mewarisi secret & token server (ADR-022).
    let deadline = Instant::now() + GUARD_WAIT;
    while !paths.marker.exists() {
        if Instant::now() > deadline {
            let tail = engine
                .logs()
                .iter()
                .rev()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(" | ");
            engine.stop();
            return Err(format!(
                "plugin guard env tidak termuat; engine dihentikan demi keamanan (ADR-022). Penanda: {:?}; pancingan: {warmup}; log: {tail}",
                paths.marker
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(engine)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_drops_windows_verbatim_prefix() {
        assert_eq!(
            file_url(Path::new(r"\\?\C:\Program Files\Zandor\p.js")),
            "file:///C:/Program%20Files/Zandor/p.js"
        );
    }

    #[test]
    fn percent_encode_handles_windows_paths_and_spaces() {
        assert_eq!(percent_encode(r"C:\a b\ü"), "C%3A%5Ca%20b%5C%C3%BC");
        assert_eq!(percent_encode("abc-_.~09"), "abc-_.~09");
    }

    #[test]
    fn pinned_version_matches_engine_json() {
        let json = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../engine.json"),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["version"], PINNED_ENGINE_VERSION);
    }

    #[test]
    fn file_url_windows_and_unix() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\a b\p#1\x.js")),
            "file:///C:/Users/a%20b/p%231/x.js"
        );
        assert_eq!(
            file_url(Path::new("/opt/x y/z.js")),
            "file:///opt/x%20y/z.js"
        );
    }

    #[test]
    fn config_has_plugin_and_marker_but_no_secret() {
        let c = config_content(
            Path::new(r"C:\app\zandor-plugin.js"),
            Path::new(r"C:\data\guard-loaded"),
            &[],
        );
        let v: serde_json::Value = serde_json::from_str(&c).unwrap();
        assert_eq!(v["plugin"][0][0], "file:///C:/app/zandor-plugin.js");
        assert_eq!(v["plugin"][0][1]["markerFile"], r"C:\data\guard-loaded");
        assert_eq!(v["autoupdate"], false);
        for k in ["edit", "bash", "webfetch", "external_directory"] {
            assert_eq!(v["permission"][k], "ask", "{k} harus meminta izin");
        }
        let lower = c.to_lowercase();
        assert!(
            !lower.contains("password") && !lower.contains("token") && !lower.contains("apikey")
        );
    }

    #[test]
    fn engine_config_isolates_xdg_and_pins_version() {
        let p = EnginePaths::new(
            Path::new(r"C:\data"),
            PathBuf::from("opencode.exe"),
            PathBuf::from("p.js"),
        );
        let cfg = engine_config(&p);
        for k in [
            "XDG_DATA_HOME",
            "XDG_CONFIG_HOME",
            "XDG_CACHE_HOME",
            "XDG_STATE_HOME",
        ] {
            let v = &cfg.env.iter().find(|(n, _)| n == k).unwrap().1;
            assert!(
                v.starts_with(r"C:\data\engine")
                    || v.starts_with("C:\\data/engine")
                    || v.contains("engine"),
                "{k}={v}"
            );
        }
        assert_eq!(cfg.expected_version.as_deref(), Some(PINNED_ENGINE_VERSION));
        assert!(cfg.env.iter().any(|(n, _)| n == "OPENCODE_CONFIG_CONTENT"));
    }

    #[test]
    fn paths_layout() {
        let p = EnginePaths::new(Path::new("/d"), PathBuf::from("s"), PathBuf::from("p"));
        assert!(p.marker.ends_with("guard-loaded"));
        assert!(p.chats.ends_with("chats"));
    }
}

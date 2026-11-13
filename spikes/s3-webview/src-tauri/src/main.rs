//! Spike S3 — browser bawaan sebagai child webview Tauri (feature `unstable`).
//!
//! Satu jendela, dua webview:
//! - `ui`: UI lokal (auto-resize memenuhi jendela), punya IPC.
//! - `browser`: konten remote (default google.com), diposisikan di atas elemen
//!   panel milik UI. UI mengirim bounds lewat `browser_set_bounds` setiap kali
//!   panel berubah ukuran (ResizeObserver).
//!
//! Fallback: Linux Wayland (tanpa `GDK_BACKEND=x11`) → buka di browser sistem.
//! Self-test: env `ZANDOR_SELFTEST=<file.json>`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use tauri::webview::{PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

const BROWSER: &str = "browser";
const UI: &str = "ui";
const HOME: &str = "https://www.google.com/";
/// Host khusus untuk "probe": skrip di halaman browser menavigasi ke sini, kita
/// cegat di `on_navigation` (dibatalkan) dan teruskan isinya ke UI. Dipakai hanya
/// untuk self-test, karena halaman remote sengaja tidak punya IPC.
const PROBE_HOST: &str = "zandor-probe.invalid";

#[derive(Serialize, Clone)]
struct LoadPayload {
    url: String,
    finished: bool,
}

#[derive(Serialize)]
struct Bounds {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    scale: f64,
}

fn browser_mode() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let forced_x11 = std::env::var("GDK_BACKEND").map(|v| v == "x11").unwrap_or(false);
        if wayland && !forced_x11 {
            return "system-browser";
        }
    }
    "child-webview"
}

fn browser(app: &AppHandle) -> Result<tauri::Webview, String> {
    app.get_webview(BROWSER).ok_or_else(|| "browser webview tidak ada".into())
}

#[tauri::command]
fn get_browser_mode() -> &'static str {
    browser_mode()
}

#[tauri::command]
fn browser_set_bounds(app: AppHandle, x: f64, y: f64, w: f64, h: f64) -> Result<(), String> {
    let b = browser(&app)?;
    b.set_position(LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    b.set_size(LogicalSize::new(w.max(0.0), h.max(0.0))).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn browser_get_bounds(app: AppHandle) -> Result<Bounds, String> {
    let b = browser(&app)?;
    let scale = b.window().scale_factor().map_err(|e| e.to_string())?;
    let p = b.position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let s = b.size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    Ok(Bounds { x: p.x, y: p.y, w: s.width, h: s.height, scale })
}

#[tauri::command]
fn browser_navigate(app: AppHandle, url: String) -> Result<(), String> {
    let url = if url.contains("://") { url } else { format!("https://{url}") };
    let parsed = url.parse::<Url>().map_err(|e| e.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("hanya http/https".into());
    }
    if browser_mode() == "system-browser" {
        return open_system_browser(parsed.as_str());
    }
    browser(&app)?.navigate(parsed).map_err(|e| e.to_string())
}

#[tauri::command]
fn browser_history(app: AppHandle, action: String) -> Result<(), String> {
    let js = match action.as_str() {
        "back" => "history.back()",
        "forward" => "history.forward()",
        "reload" => "location.reload()",
        _ => return Err("aksi tidak dikenal".into()),
    };
    browser(&app)?.eval(js).map_err(|e| e.to_string())
}

#[tauri::command]
fn browser_set_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    let b = browser(&app)?;
    if visible { b.show() } else { b.hide() }.map_err(|e| e.to_string())
}

/// Self-test: jalankan ekspresi JS di halaman browser dan kirim hasilnya (string)
/// kembali lewat navigasi ke PROBE_HOST yang dicegat.
#[tauri::command]
fn browser_probe(app: AppHandle, expr: String) -> Result<(), String> {
    let js = format!(
        "(function(){{var go=function(v){{v=String(v);location.href='https://{PROBE_HOST}/?v='+encodeURIComponent(v.slice(0,2000));}};\
         try{{Promise.resolve({expr}).then(go,function(e){{go('REJECT:'+(e&&e.message||e))}})}}catch(e){{go('ERR:'+e)}}}})()"
    );
    browser(&app)?.eval(&js).map_err(|e| e.to_string())
}

#[tauri::command]
fn window_set_size(app: AppHandle, w: f64, h: f64) -> Result<(), String> {
    let win = app.get_window("main").ok_or("no window")?;
    win.set_size(LogicalSize::new(w, h)).map_err(|e| e.to_string())
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

fn open_system_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let window = WindowBuilder::new(app, "main")
                .title("Zandor Spike S3 — child webview browser")
                .inner_size(1200.0, 800.0)
                .min_inner_size(700.0, 450.0)
                .build()?;
            let size = window.inner_size()?.to_logical::<f64>(window.scale_factor()?);

            window.add_child(
                WebviewBuilder::new(UI, WebviewUrl::App("index.html".into())).auto_resize(),
                LogicalPosition::new(0.0, 0.0),
                size,
            )?;

            if browser_mode() == "child-webview" {
                let h_load = app.handle().clone();
                let h_nav = app.handle().clone();
                // Posisi awal sementara; UI langsung mengirim bounds sebenarnya.
                window.add_child(
                    WebviewBuilder::new(BROWSER, WebviewUrl::External(HOME.parse().unwrap()))
                        .on_page_load(move |_wv, p| {
                            let _ = h_load.emit_to(
                                UI,
                                "browser-load",
                                LoadPayload {
                                    url: p.url().to_string(),
                                    finished: matches!(p.event(), PageLoadEvent::Finished),
                                },
                            );
                        })
                        .on_navigation(move |url| {
                            if url.host_str() == Some(PROBE_HOST) {
                                let v = url
                                    .query_pairs()
                                    .find(|(k, _)| k == "v")
                                    .map(|(_, v)| v.into_owned())
                                    .unwrap_or_default();
                                let _ = h_nav.emit_to(UI, "browser-probe", v);
                                return false;
                            }
                            true
                        }),
                    LogicalPosition::new(size.width / 2.0, 48.0),
                    LogicalSize::new(size.width / 2.0, size.height - 48.0),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_browser_mode,
            browser_set_bounds,
            browser_get_bounds,
            browser_navigate,
            browser_history,
            browser_set_visible,
            browser_probe,
            window_set_size,
            selftest_enabled,
            selftest_done
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan app");
}

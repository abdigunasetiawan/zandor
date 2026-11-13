//! Spike S8 — layout 5 panel (react-resizable-panels) + child webview browser + zoom `--ui-scale`.
//! Turunan S3: jendela berisi webview `ui` (auto-resize) dan `browser` (child, di atas panel 4).
//! Self-test: env `ZANDOR_SELFTEST=<file.json>` (+ `<file>.log`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::webview::{PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl};

const BROWSER: &str = "browser";
const UI: &str = "ui";
const PROBE_HOST: &str = "zandor-probe.invalid";
static BROWSER_VISIBLE: AtomicBool = AtomicBool::new(true);

#[derive(Serialize)]
struct Bounds {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    scale: f64,
    visible: bool,
}

fn browser(app: &AppHandle) -> Result<tauri::Webview, String> {
    app.get_webview(BROWSER).ok_or_else(|| "browser webview tidak ada".into())
}

#[tauri::command]
fn browser_set_bounds(app: AppHandle, x: f64, y: f64, w: f64, h: f64) -> Result<(), String> {
    let b = browser(&app)?;
    b.set_position(LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    b.set_size(LogicalSize::new(w.max(0.0), h.max(0.0))).map_err(|e| e.to_string())
}

#[tauri::command]
fn browser_get_bounds(app: AppHandle) -> Result<Bounds, String> {
    let b = browser(&app)?;
    let scale = b.window().scale_factor().map_err(|e| e.to_string())?;
    let p = b.position().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let s = b.size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    Ok(Bounds { x: p.x, y: p.y, w: s.width, h: s.height, scale, visible: BROWSER_VISIBLE.load(Ordering::SeqCst) })
}

#[tauri::command]
fn browser_set_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    let b = browser(&app)?;
    if visible { b.show() } else { b.hide() }.map_err(|e| e.to_string())?;
    BROWSER_VISIBLE.store(visible, Ordering::SeqCst);
    Ok(())
}

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
    app.get_window("main").ok_or("no window")?.set_size(LogicalSize::new(w, h)).map_err(|e| e.to_string())
}

#[tauri::command]
fn ui_reload(app: AppHandle) -> Result<(), String> {
    app.get_webview(UI).ok_or("no ui")?.eval("location.reload()").map_err(|e| e.to_string())
}

#[tauri::command]
fn selftest_path() -> Option<String> {
    std::env::var("ZANDOR_SELFTEST").ok()
}

#[tauri::command]
fn selftest_log(msg: String) {
    if let Ok(p) = std::env::var("ZANDOR_SELFTEST") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(format!("{p}.log")) {
            let _ = writeln!(f, "{msg}");
        }
    }
}

#[tauri::command]
fn selftest_done(app: AppHandle, report: serde_json::Value) {
    if let Ok(p) = std::env::var("ZANDOR_SELFTEST") {
        let _ = std::fs::write(p, serde_json::to_string_pretty(&report).unwrap());
    }
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let window = WindowBuilder::new(app, "main")
                .title("Zandor Spike S8 — layout 5 panel")
                .inner_size(1366.0, 768.0)
                .min_inner_size(800.0, 500.0)
                .build()?;
            let size = window.inner_size()?.to_logical::<f64>(window.scale_factor()?);
            window.add_child(
                WebviewBuilder::new(UI, WebviewUrl::App("index.html".into())).auto_resize(),
                LogicalPosition::new(0.0, 0.0),
                size,
            )?;
            let h_load = app.handle().clone();
            let h_nav = app.handle().clone();
            window.add_child(
                WebviewBuilder::new(BROWSER, WebviewUrl::External("https://www.google.com/".parse().unwrap()))
                    .on_page_load(move |_wv, p| {
                        let _ = h_load.emit_to(UI, "browser-load", (p.url().to_string(), matches!(p.event(), PageLoadEvent::Finished)));
                    })
                    .on_navigation(move |url| {
                        if url.host_str() == Some(PROBE_HOST) {
                            let v = url.query_pairs().find(|(k, _)| k == "v").map(|(_, v)| v.into_owned()).unwrap_or_default();
                            let _ = h_nav.emit_to(UI, "browser-probe", v);
                            return false;
                        }
                        true
                    }),
                LogicalPosition::new(size.width - 400.0, 40.0),
                LogicalSize::new(360.0, size.height - 40.0),
            )?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            browser_set_bounds,
            browser_get_bounds,
            browser_set_visible,
            browser_probe,
            window_set_size,
            ui_reload,
            selftest_path,
            selftest_log,
            selftest_done
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan app");
}

// PENTING: perintah yang membuat/mengubah webview WAJIB `async` — perintah sinkron berjalan di thread utama dan
// membuat child webview WebView2 macet di about:blank (Tauri: pembuatan window/webview di perintah sinkron = deadlock).
// Browser bawaan Panel 4 sebagai child webview Tauri (ADR-020, fitur `unstable`). Webview anak berisi konten remote
// TANPA capability apa pun (tidak ada IPC yang diizinkan); UI memantau posisi panel dan mengirim bounds. Hanya
// http/https yang boleh dimuat. Di Linux Wayland (tanpa GDK_BACKEND=x11) child webview tidak dipakai (fallback sistem).
use serde::Serialize;
use tauri::webview::{PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

const LABEL: &str = "browser";
const HOME: &str = "https://www.google.com/";

#[derive(Serialize, Clone)]
struct LoadPayload {
    url: String,
    finished: bool,
}

pub fn mode() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let forced_x11 = std::env::var("GDK_BACKEND")
            .map(|v| v == "x11")
            .unwrap_or(false);
        if wayland && !forced_x11 {
            return "system-browser";
        }
    }
    "child-webview"
}

/// Normalisasi masukan bilah alamat: tanpa skema → https; hanya http/https; panjang dibatasi.
pub fn parse_url(input: &str) -> Result<Url, String> {
    let t = input.trim();
    if t.is_empty() || t.len() > 4096 {
        return Err("URL kosong atau terlalu panjang".into());
    }
    let with_scheme = if t.contains("://") {
        t.to_string()
    } else {
        format!("https://{t}")
    };
    let url = with_scheme
        .parse::<Url>()
        .map_err(|e| format!("URL tidak valid: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("hanya URL http/https yang dibuka di browser".into());
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err("URL tanpa host".into());
    }
    Ok(url)
}

fn existing(app: &AppHandle) -> Option<tauri::Webview> {
    app.get_webview(LABEL)
}

fn ensure(
    app: &AppHandle,
    url: &Url,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<tauri::Webview, String> {
    if let Some(b) = existing(app) {
        return Ok(b);
    }
    let window = app
        .get_window("main")
        .ok_or("jendela utama tidak ditemukan")?;
    let handle = app.clone();
    let builder = WebviewBuilder::new(LABEL, WebviewUrl::External(url.clone()))
        .on_page_load(move |_wv, p| {
            let _ = handle.emit_to(
                "main",
                "browser-load",
                LoadPayload {
                    url: p.url().to_string(),
                    finished: matches!(p.event(), PageLoadEvent::Finished),
                },
            );
        })
        // Navigasi (termasuk redirect dan tautan) hanya ke http/https/about:blank.
        .on_navigation(|u| matches!(u.scheme(), "http" | "https" | "about"));
    window
        .add_child(
            builder,
            LogicalPosition::new(x, y),
            LogicalSize::new(w.max(1.0), h.max(1.0)),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn browser_mode() -> &'static str {
    mode()
}

#[tauri::command]
pub fn browser_home() -> &'static str {
    HOME
}

/// Tampilkan browser pada bounds (piksel logis CSS) dan, bila `url` diberikan dan belum dimuat, buka.
#[tauri::command]
pub async fn browser_open(
    app: AppHandle,
    url: Option<String>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<(), String> {
    if mode() != "child-webview" {
        return Err("browser tertanam tidak tersedia di platform ini".into());
    }
    let first = existing(&app).is_none();
    let target = parse_url(url.as_deref().unwrap_or(HOME))?;
    let b = ensure(&app, &target, x, y, w, h)?;
    b.set_position(LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    b.set_size(LogicalSize::new(w.max(1.0), h.max(1.0)))
        .map_err(|e| e.to_string())?;
    b.show().map_err(|e| e.to_string())?;
    // Webview baru: navigasi eksplisit juga (WebView2 anak kadang tetap di about:blank bila hanya mengandalkan URL awal).
    if first || url.is_some() {
        b.navigate(target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_set_bounds(
    app: AppHandle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<(), String> {
    let Some(b) = existing(&app) else {
        return Ok(());
    };
    b.set_position(LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    b.set_size(LogicalSize::new(w.max(1.0), h.max(1.0)))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    let Some(b) = existing(&app) else {
        return Ok(());
    };
    if visible { b.show() } else { b.hide() }.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn browser_navigate(app: AppHandle, url: String) -> Result<String, String> {
    let target = parse_url(&url)?;
    let b = existing(&app).ok_or("browser belum dibuka")?;
    b.navigate(target.clone()).map_err(|e| e.to_string())?;
    Ok(target.to_string())
}

#[tauri::command]
pub async fn browser_history(app: AppHandle, action: String) -> Result<(), String> {
    let js = match action.as_str() {
        "back" => "history.back()",
        "forward" => "history.forward()",
        "reload" => "location.reload()",
        _ => return Err("aksi tidak dikenal".into()),
    };
    existing(&app)
        .ok_or("browser belum dibuka")?
        .eval(js)
        .map_err(|e| e.to_string())
}

/// Tutup webview browser (membebaskan sumber daya).
#[tauri::command]
pub async fn browser_close(app: AppHandle) -> Result<(), String> {
    if let Some(b) = existing(&app) {
        b.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_normalizes_and_restricts_schemes() {
        assert_eq!(
            parse_url("example.com").unwrap().as_str(),
            "https://example.com/"
        );
        assert_eq!(
            parse_url("  http://localhost:3000/x ").unwrap().scheme(),
            "http"
        );
        assert_eq!(parse_url("https://a.b/c?d=1#e").unwrap().path(), "/c");
        for bad in [
            "",
            "   ",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "data:text/html,x",
            "ftp://x.y",
            "tauri://localhost",
            "https://",
            "http://:80/",
        ] {
            assert!(parse_url(bad).is_err(), "{bad:?}");
        }
        assert!(parse_url(&"a".repeat(5000)).is_err());
    }
}

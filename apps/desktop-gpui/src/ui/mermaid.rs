//! Diagram Mermaid — port rich/Mermaid.tsx. Mermaid butuh DOM, jadi dijalankan di WebView2 tersembunyi (wry, jendela
//! anak tak terlihat) yang memuat mermaid.min.js tertanam (12.3.0, sama dengan versi React) lewat protokol lokal — tanpa
//! jaringan. Aman: securityLevel "strict", label teks SVG (htmlLabels mati) agar bisa digambar ke kanvas. Hasil: SVG +
//! PNG 2× (dirasterisasi di webview) → ditampilkan GPUI sebagai gambar, di-cache per (kode, tema).
//! Diagram dirender setelah kode tidak berubah ±600 ms (pengganti status "streaming" versi React).
use crate::theme::Theme;
use gpui::{div, prelude::*, px, AnyElement, IntoElement, Window};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MERMAID_JS: &[u8] = include_bytes!("../../assets/mermaid.min.js");
const DEBOUNCE: Duration = Duration::from_millis(600);

const PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8"></head><body style="margin:0;background:transparent">
<div id="host" style="position:absolute;left:0;top:0;width:1600px"></div>
<script src="mermaid.min.js"></script>
<script>
let seq = 0;
async function zrender(id, code, dark) {
  const post = (m) => window.ipc.postMessage(JSON.stringify(m));
  try {
    mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: dark ? "dark" : "default", htmlLabels: false, flowchart: { htmlLabels: false }, layout: "dagre" });
    const { svg } = await mermaid.render("zm" + (seq++), code);
    const holder = document.getElementById("host");
    holder.innerHTML = svg;
    const el = holder.querySelector("svg");
    const box = el.getBoundingClientRect();
    let w = Math.ceil(box.width) || 400, h = Math.ceil(box.height) || 300;
    const vb = el.viewBox && el.viewBox.baseVal;
    if (vb && vb.width && vb.height) { h = Math.ceil(w * vb.height / vb.width); }
    el.setAttribute("width", w); el.setAttribute("height", h);
    const xml = new XMLSerializer().serializeToString(el);
    holder.innerHTML = "";
    const img = new Image();
    img.onload = () => {
      try {
        const c = document.createElement("canvas");
        c.width = w * 2; c.height = h * 2;
        const g = c.getContext("2d");
        g.scale(2, 2); g.drawImage(img, 0, 0, w, h);
        post({ t: "ok", id, svg: xml, png: c.toDataURL("image/png").split(",")[1], w, h });
      } catch (e) { post({ t: "err", id, error: String(e && e.message || e) }); }
    };
    img.onerror = () => post({ t: "err", id, error: "gagal merasterisasi SVG" });
    img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(xml);
  } catch (e) {
    post({ t: "err", id, error: String(e && e.message || e) });
  }
}
window.ipc.postMessage(JSON.stringify({ t: "ready" }));
</script></body></html>"#;

#[derive(Clone)]
pub enum State {
    Pending,
    Done { png: Arc<gpui::Image>, svg: String, w: f32, h: f32 },
    Error(String),
}

struct Job {
    code: String,
    dark: bool,
    first: Instant,
    sent: bool,
}

#[derive(Default)]
struct Svc {
    cache: HashMap<u64, State>,
    jobs: HashMap<u64, Job>,
    ready: bool,
    dirty: bool,
    view: Option<wry::WebView>,
    failed: Option<String>,
}

thread_local! {
    static SVC: RefCell<Svc> = RefCell::new(Svc::default());
}

fn key(code: &str, dark: bool) -> u64 {
    super::rich::block_key(&["mermaid", code, if dark { "d" } else { "l" }])
}

/// Status diagram (mengantrekan render bila belum ada).
pub fn get(code: &str, dark: bool) -> State {
    let k = key(code, dark);
    SVC.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(e) = &s.failed {
            return State::Error(e.clone());
        }
        if let Some(st) = s.cache.get(&k) {
            return st.clone();
        }
        s.jobs.entry(k).or_insert_with(|| Job { code: code.to_string(), dark, first: Instant::now(), sent: false });
        State::Pending
    })
}

fn on_ipc(body: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else { return };
    SVC.with(|s| {
        let mut s = s.borrow_mut();
        match v["t"].as_str() {
            Some("ready") => s.ready = true,
            Some("ok") => {
                let Some(id) = v["id"].as_str().and_then(|x| u64::from_str_radix(x, 16).ok()) else { return };
                let png = v["png"].as_str().and_then(crate::images::decode_b64);
                let st = match png {
                    Some(bytes) => State::Done {
                        png: Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes)),
                        svg: v["svg"].as_str().unwrap_or("").to_string(),
                        w: v["w"].as_f64().unwrap_or(400.) as f32,
                        h: v["h"].as_f64().unwrap_or(300.) as f32,
                    },
                    None => State::Error("PNG tidak valid".into()),
                };
                if let State::Done { w, h, png, .. } = &st {
                    eprintln!("[zandor] mermaid ok {w}x{h} png={}B", png.bytes().len());
                }
                s.jobs.remove(&id);
                s.cache.insert(id, st);
                s.dirty = true;
            }
            Some("err") => {
                let Some(id) = v["id"].as_str().and_then(|x| u64::from_str_radix(x, 16).ok()) else { return };
                s.jobs.remove(&id);
                eprintln!("[zandor] mermaid galat: {}", v["error"].as_str().unwrap_or("?"));
                s.cache.insert(id, State::Error(v["error"].as_str().unwrap_or("?").to_string()));
                s.dirty = true;
            }
            _ => {}
        }
    });
}

fn build(window: &Window) -> Result<wry::WebView, String> {
    use wry::http::{header::CONTENT_TYPE, Response};
    let builder = wry::WebViewBuilder::new()
        .with_custom_protocol("zmermaid".into(), |_id, req| {
            let path = req.uri().path().trim_start_matches('/');
            let (body, mime): (std::borrow::Cow<'static, [u8]>, &str) = match path {
                "mermaid.min.js" => (MERMAID_JS.into(), "text/javascript"),
                _ => (PAGE.as_bytes().into(), "text/html; charset=utf-8"),
            };
            Response::builder().header(CONTENT_TYPE, mime).body(body).unwrap_or_else(|_| Response::new(std::borrow::Cow::Borrowed(&[][..])))
        })
        .with_ipc_handler(|req| on_ipc(req.body()))
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_visible(false)
        .with_bounds(wry::Rect { position: wry::dpi::LogicalPosition::new(0., 0.).into(), size: wry::dpi::LogicalSize::new(1., 1.).into() })
        .with_url(if cfg!(windows) { "http://zmermaid.localhost/index.html" } else { "zmermaid://localhost/index.html" });
    builder.build_as_child(window).map_err(|e| e.to_string())
}

/// Dipanggil Root berkala: bangun webview bila perlu, kirim pekerjaan yang sudah stabil, segarkan bila ada hasil.
pub fn pump(window: &mut Window, cx: &mut gpui::App) {
    let mut refresh = false;
    SVC.with(|s| {
        let mut s = s.borrow_mut();
        if s.jobs.is_empty() && !s.dirty {
            return;
        }
        if s.view.is_none() && s.failed.is_none() && !s.jobs.is_empty() {
            match build(window) {
                Ok(v) => s.view = Some(v),
                Err(e) => {
                    s.failed = Some(format!("WebView2: {e}"));
                    s.dirty = true;
                }
            }
        }
        if s.ready {
            let now = Instant::now();
            let due: Vec<(u64, String, bool)> = s.jobs.iter().filter(|(_, j)| !j.sent && now.duration_since(j.first) >= DEBOUNCE).map(|(k, j)| (*k, j.code.clone(), j.dark)).collect();
            for (k, code, dark) in due {
                let js = format!("zrender({}, {}, {})", serde_json::json!(format!("{k:x}")), serde_json::json!(code), dark);
                if let Some(v) = &s.view {
                    let _ = v.evaluate_script(&js);
                }
                if let Some(j) = s.jobs.get_mut(&k) {
                    j.sent = true;
                }
            }
        }
        if s.dirty {
            s.dirty = false;
            refresh = true;
        }
        // pekerjaan yang belum jatuh tempo: periksa lagi pada detak berikutnya
    });
    if refresh {
        cx.refresh_windows();
    }
}

/// Swa-uji (ZANDOR_MERMAID_SELFTEST=1): antrekan satu diagram saat mulai; hasil tercatat di log.
pub fn selftest() {
    if std::env::var_os("ZANDOR_MERMAID_SELFTEST").is_some() {
        let _ = get("graph TD
  A[Mulai] --> B{Cek}
  B -->|ya| C[Selesai]", false);
    }
}

/// Ada pekerjaan tertunda? (Root mempercepat detak bila ya.)
pub fn busy() -> bool {
    SVC.with(|s| {
        let s = s.borrow();
        !s.jobs.is_empty() || s.dirty
    })
}

/// Blok diagram (port komponen Mermaid): gambar + bilah aksi, atau status memuat/galat dengan kode sumber.
pub fn block(t: &Theme, i: &crate::i18n::I18n, code: &str) -> AnyElement {
    let note = |s: String| div().small(t).text_color(t.colors.fg_muted).child(s);
    let src = || div().px(t.space(3)).py(t.space(2)).rounded(t.rem(crate::theme::rem::RADIUS_MD)).bg(t.colors.raised).font_family(t.font_code.clone()).code(t).child(code.to_string());
    match get(code, t.dark) {
        State::Pending => div().flex().flex_col().gap(t.space(1)).child(note(i.t("doc.mermaidLoading").to_string())).into_any_element(),
        State::Error(e) => div()
            .flex()
            .flex_col()
            .gap(t.space(1))
            .child(div().small(t).text_color(t.colors.danger).child(i.tp("doc.mermaidError", &[("error", e.lines().next().unwrap_or(""))])))
            .child(src())
            .into_any_element(),
        State::Done { png, svg, w, h } => {
            let scale = t.scale;
            div()
                .flex()
                .flex_col()
                .gap(t.space(1))
                .w_full()
                .child(super::rich::mermaid_bar(t, i, code, &svg, png.clone()))
                .child(
                    div()
                        .id(gpui::ElementId::Name(format!("mmd-{:x}", key(code, t.dark)).into()))
                        .w_full()
                        .overflow_x_scroll()
                        .flex()
                        .justify_center()
                        .child(gpui::img(png).w(px(w * scale)).h(px(h * scale)).flex_none()),
                )
                .into_any_element()
        }
    }
}

use super::TypeExt;

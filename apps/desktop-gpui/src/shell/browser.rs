//! Panel alat Browser — port BrowserPanel.tsx + browser.ts: webview native (WebView2 lewat wry) sebagai jendela anak
//! di atas UI GPUI, satu webview per tab (maks 8). Hanya tab aktif yang tampil, diposisikan mengikuti area panel tiap
//! frame; disembunyikan bila panel lain dipilih atau area terlalu kecil. Konten remote tidak punya IPC ke aplikasi.
//! Webview anak digambar di atas UI ("airspace"), jadi disembunyikan selama ada menu/dialog/palet (penanda overlay_deferred).
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{icon, icon_button, ButtonVariant, ControlSize, IconSize, InputSubmit, TextInput, TypeExt};
use gpui::{canvas, div, prelude::*, Bounds, Context, Entity, IntoElement, Pixels, SharedString, Window};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

pub const MAX_BROWSER_TABS: usize = 8;
/// Di bawah ukuran ini (px) webview disembunyikan (mis. panel dilipat).
const MIN_SIZE: f32 = 64.;

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub id: String,
    pub url: String,
    /// Tab milik agent (profil terpisah, navigasi dibatasi izin domain; lihat browser_agent.rs).
    pub agent: bool,
}

/// Penunjuk global ke panel Browser (diisi saat panel dibuat) untuk kendali agent.
pub struct BrowserHandle(pub gpui::WeakEntity<BrowserPanel>);
impl gpui::Global for BrowserHandle {}

/// Port normalizeBrowserUrl: tambah https:// bila tanpa skema; hanya http/https dengan host.
pub fn normalize_browser_url(input: &str) -> Result<String, &'static str> {
    let t = input.trim();
    if t.is_empty() || t.len() > 4096 {
        return Err("URL kosong atau terlalu panjang");
    }
    let with = if t.contains("://") { t.to_string() } else { format!("https://{t}") };
    let (scheme, rest) = with.split_once("://").ok_or("URL tidak valid")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("hanya URL http/https yang dibuka");
    }
    if with.chars().any(char::is_whitespace) {
        return Err("URL tidak valid");
    }
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let hostport = auth.rsplit('@').next().unwrap_or("");
    let (host, port) = hostport.split_once(':').unwrap_or((hostport, ""));
    if host.is_empty() {
        return Err("URL tanpa host");
    }
    if !port.is_empty() && !(port.len() <= 5 && port.chars().all(|c| c.is_ascii_digit())) {
        return Err("URL tidak valid");
    }
    let path = &rest[auth.len()..];
    Ok(format!("{scheme}://{}{}", auth.to_ascii_lowercase(), if path.is_empty() { "/" } else { path }))
}

/// Port browserTabLabel: hostname tanpa "www.".
pub fn tab_label(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return String::new() };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("");
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

/// Lama (ms) webview tetap disembunyikan setelah overlay terakhir terdeteksi.
const OVERLAY_HOLD_MS: u64 = 250;
static LAST_OVERLAY_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
fn big_ok_and(settled: bool, big: bool) -> bool {
    settled && big
}

fn decode_snapshot(bytes: &[u8]) -> Option<gpui::RenderImage> {
    let mut rgba = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).ok()?.to_rgba8();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2); // GPUI memakai BGRA
    }
    Some(gpui::RenderImage::new(vec![image::Frame::new(rgba)]))
}

/// Halaman awal tab baru.
const HOME_URL: &str = "https://www.google.com/";

fn next_id(tabs: &[Tab]) -> String {
    (1..).map(|n| format!("b{n}")).find(|id| !tabs.iter().any(|t| &t.id == id)).unwrap_or_default()
}

type Views = Rc<RefCell<HashMap<String, wry::WebView>>>;
/// Id tab yang webview-nya sedang di level memori Low (hanya panggil WebView2 saat status berubah).
type LowSet = Rc<RefCell<std::collections::HashSet<String>>>;

/// Turunkan (`low`) / pulihkan level memori webview. WebView2 boleh membuang cache / men-swap memori webview yang
/// tidak aktif; skrip dan koneksi tetap jalan, jadi tidak ada state yang hilang. Hanya Windows; tempat lain no-op.
fn set_mem_level(low_set: &LowSet, id: &str, v: &wry::WebView, low: bool) {
    let changed = if low { low_set.borrow_mut().insert(id.to_string()) } else { low_set.borrow_mut().remove(id) };
    if !changed {
        return;
    }
    #[cfg(windows)]
    {
        use wry::WebViewExtWindows;
        let lvl = if low { wry::MemoryUsageLevel::Low } else { wry::MemoryUsageLevel::Normal };
        let _ = v.set_memory_usage_level(lvl);
    }
    #[cfg(not(windows))]
    let _ = v;
}

pub struct BrowserPanel {
    pub tabs: Vec<Tab>,
    pub active: String,
    address: Entity<TextInput>,
    views: Views,
    low: LowSet,
    loading: HashMap<String, bool>,
    tx: futures::channel::mpsc::UnboundedSender<(String, String, bool)>,
    error: Option<String>,
    /// Webview tertanam gagal dibuat: alamat dibuka di browser sistem (br.systemFallback).
    system_fallback: bool,
    /// true saat panel Browser sedang ditampilkan (diatur ToolsHost).
    pub shown: bool,
    /// Tangkapan terakhir halaman aktif (JPEG -> gambar GPUI). Digambar DI BAWAH webview: tertutup webview saat tampil,
    /// terlihat saat webview disembunyikan oleh menu/dialog (airspace), jadi panel tidak tampak kosong.
    snap: Rc<RefCell<Option<(String, Arc<gpui::RenderImage>)>>>,
    /// Gambar lama yang harus dilepas dari atlas GPU (dilakukan di canvas yang punya Window).
    trash: Rc<RefCell<Vec<Arc<gpui::RenderImage>>>>,
    /// Hasil mentah (id tab, JPEG) dari callback CDP, didekode di tick berikutnya.
    snap_raw: Rc<RefCell<Option<(String, Vec<u8>)>>>,
    snap_busy: Rc<std::cell::Cell<Option<std::time::Instant>>>,
    /// true bila webview tab aktif sedang benar-benar tampil (hanya saat itu CDP screenshot aman).
    visible_now: Rc<std::cell::Cell<bool>>,
    /// Sejak kapan panel disembunyikan (None saat tampil). Lewat `IDLE_DROP` webview dibuang (lihat `drop_idle_views`).
    hidden_at: Option<std::time::Instant>,
}

/// Lama panel Browser disembunyikan sebelum webview-nya (dan proses WebView2 yang menyertainya) dibuang.
const IDLE_DROP: std::time::Duration = std::time::Duration::from_secs(180);

impl BrowserPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.set_global(BrowserHandle(cx.weak_entity()));
        {
            let st = crate::core::settings::SettingsStore::global(cx);
            super::browser_agent::set_enabled(st.read(cx).bool("browserAgent"));
            cx.observe(&st, |_, st, cx| super::browser_agent::set_enabled(st.read(cx).bool("browserAgent"))).detach();
        }
        cx.observe_global::<super::layout::PanelShown>(|s, cx| {
            if !cx.global::<super::layout::PanelShown>().tools {
                s.hide_all();
            }
        })
        .detach();
        START.get_or_init(std::time::Instant::now);
        let ph = cx.global::<I18n>().t("br.address");
        let address = cx.new(|cx| {
            let mut ti = TextInput::new(cx, ph);
            ti.size = ControlSize::Sm;
            ti.set_text(HOME_URL.to_string(), cx);
            ti
        });
        cx.subscribe(&address, |s, _, e: &InputSubmit, cx| s.go(e.0.to_string(), cx)).detach();
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<(String, String, bool)>();
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while let Some((id, url, done)) = rx.next().await {
                if this
                    .update(cx, |s, cx| {
                        s.loading.insert(id.clone(), !done);
                        if !done {
                            s.clear_snap();
                        }
                        if let Some(t) = s.tabs.iter_mut().find(|t| t.id == id) {
                            t.url = url.clone();
                        }
                        if s.active == id {
                            s.address.update(cx, |a, cx| a.set_text(url, cx));
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        // Pemeriksa idle: tiap 30 dtk, buang webview bila panel lama disembunyikan.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_secs(30)).await;
            if this.update(cx, |s, cx| s.drop_idle_views(cx)).is_err() {
                break;
            }
        })
        .detach();
        Self { tabs: vec![Tab { id: "b1".into(), url: HOME_URL.into(), agent: false }], active: "b1".into(), address, views: Default::default(), low: Default::default(), loading: HashMap::new(), tx, error: None, system_fallback: false, shown: false, snap: Default::default(), trash: Default::default(), snap_raw: Default::default(), snap_busy: Default::default(), visible_now: Default::default(), hidden_at: None }
    }

    /// Buang semua webview bila panel sudah lama disembunyikan. Tab, URL, dan tangkapan terakhir tetap ada; saat panel
    /// dibuka lagi, canvas membuat ulang webview tab aktif dari URL tersimpan (yang hilang hanya state halaman).
    fn drop_idle_views(&mut self, cx: &mut Context<Self>) {
        if self.shown || !self.hidden_at.is_some_and(|t| t.elapsed() >= IDLE_DROP) || self.views.borrow().is_empty() {
            return;
        }
        self.views.borrow_mut().clear();
        self.low.borrow_mut().clear();
        self.loading.clear();
        self.visible_now.set(false);
        cx.notify();
    }

    /// Sembunyikan semua webview (panel lain dipilih / panel ditutup).
    pub fn hide_all(&mut self) {
        self.shown = false;
        self.hidden_at.get_or_insert_with(std::time::Instant::now);
        for (k, v) in self.views.borrow().iter() {
            let _ = v.set_visible(false);
            set_mem_level(&self.low, k, v, true);
        }
    }

    fn clear_snap(&mut self) {
        if let Some((_, old)) = self.snap.borrow_mut().take() {
            self.trash.borrow_mut().push(old);
        }
    }

    /// Dipanggil berkala oleh Root (~2 dtk): terapkan hasil capture sebelumnya, lalu minta yang baru bila webview tampil.
    pub fn capture_snapshot(&mut self, cx: &mut Context<Self>) {
        if let Some((id, bytes)) = self.snap_raw.borrow_mut().take() {
            if let Some(img) = decode_snapshot(&bytes) {
                if let Some((_, old)) = self.snap.borrow_mut().replace((id, Arc::new(img))) {
                    self.trash.borrow_mut().push(old);
                }
                cx.notify();
            }
            self.snap_busy.set(None);
        }
        if !self.shown || !self.visible_now.get() || self.loading.get(&self.active).copied().unwrap_or(false) {
            return;
        }
        if self.snap_busy.get().is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(5)) {
            return;
        }
        let (id, raw) = (self.active.clone(), self.snap_raw.clone());
        let id2 = id.clone();
        self.snap_busy.set(Some(std::time::Instant::now()));
        let r = self.cdp_call(&id, "Page.captureScreenshot", r#"{"format":"jpeg","quality":55}"#, move |s| {
            use base64::Engine;
            if s.starts_with("ERR") {
                return;
            }
            let b64 = serde_json::from_str::<serde_json::Value>(&s).ok().and_then(|v| v["data"].as_str().map(str::to_string));
            if let Some(bytes) = b64.and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok()) {
                *raw.borrow_mut() = Some((id2, bytes));
            }
        });
        if r.is_err() {
            self.snap_busy.set(None);
        }
    }

    pub fn go(&mut self, raw: String, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        match normalize_browser_url(&raw) {
            Ok(url) => {
                self.error = None;
                self.clear_snap();
                let id = self.active.clone();
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                    t.url = url.clone();
                }
                self.address.update(cx, |a, cx| a.set_text(url.clone(), cx));
                if self.system_fallback {
                    cx.open_url(&url);
                } else if let Some(v) = self.views.borrow().get(&id) {
                    let _ = v.load_url(&url);
                }
            }
            Err(e) => self.error = Some(format!("{} ({e})", i.t("br.badUrl"))),
        }
        cx.notify();
    }

    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.active = id.clone();
        let url = self.tabs.iter().find(|t| t.id == id).map(|t| t.url.clone()).unwrap_or_default();
        self.address.update(cx, |a, cx| a.set_text(url, cx));
        for (k, v) in self.views.borrow().iter() {
            if *k != id {
                let _ = v.set_visible(false);
                set_mem_level(&self.low, k, v, true);
            }
        }
        cx.notify();
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() >= MAX_BROWSER_TABS {
            return;
        }
        let id = next_id(&self.tabs);
        self.tabs.push(Tab { id: id.clone(), url: HOME_URL.into(), agent: false });
        self.select(id, cx);
    }

    fn close(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(i) = self.tabs.iter().position(|t| t.id == id) else { return };
        self.tabs.remove(i);
        self.views.borrow_mut().remove(&id);
        self.low.borrow_mut().remove(&id);
        if self.tabs.is_empty() {
            let fresh = if id == "b1" { "b2" } else { "b1" };
            self.tabs.push(Tab { id: fresh.into(), url: HOME_URL.into(), agent: false });
        }
        if self.active == id {
            let n = self.tabs[i.min(self.tabs.len() - 1)].id.clone();
            self.select(n, cx);
        }
        cx.notify();
    }

    // ---- kendali agent (browser_agent.rs) ----

    pub fn agent_tab_id(&self) -> Option<String> {
        self.tabs.iter().find(|t| t.agent).map(|t| t.id.clone())
    }

    /// (url, sedang memuat, webview sudah dibuat) untuk satu tab.
    pub fn tab_state(&self, id: &str) -> (String, bool, bool) {
        let url = self.tabs.iter().find(|t| t.id == id).map(|t| t.url.clone()).unwrap_or_default();
        (url, self.loading.get(id).copied().unwrap_or(false), self.views.borrow().contains_key(id))
    }

    /// (id, url, milik agent, aktif)
    pub fn tab_list(&self) -> Vec<(String, String, bool, bool)> {
        self.tabs.iter().map(|t| (t.id.clone(), t.url.clone(), t.agent, t.id == self.active)).collect()
    }

    /// Buka `url` di tab Agent (dibuat bila belum ada) dan jadikan aktif. URL harus sudah lolos izin domain.
    pub fn agent_navigate(&mut self, url: String, cx: &mut Context<Self>) -> Result<String, String> {
        let id = match self.agent_tab_id() {
            Some(id) => id,
            None => {
                if self.tabs.len() >= MAX_BROWSER_TABS {
                    return Err("Tab browser sudah penuh; tutup satu tab lalu ulangi.".into());
                }
                let id = next_id(&self.tabs);
                self.tabs.push(Tab { id: id.clone(), url: String::new(), agent: true });
                id
            }
        };
        self.select(id.clone(), cx);
        self.error = None;
        self.clear_snap();
        self.loading.insert(id.clone(), true);
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            t.url = url.clone();
        }
        self.address.update(cx, |a, cx| a.set_text(url.clone(), cx));
        if let Some(v) = self.views.borrow().get(&id) {
            let _ = v.load_url(&url);
        }
        cx.notify();
        Ok(id)
    }

    /// Jalankan JS di tab dan kirim hasilnya (JSON) ke `cb` (dipanggil di thread UI).
    pub fn eval_cb(&self, id: &str, js: &str, cb: impl Fn(String) + Send + 'static) -> Result<(), String> {
        let views = self.views.borrow();
        let v = views.get(id).ok_or("tab belum punya webview (panel Browser harus tampil)")?;
        v.evaluate_script_with_callback(js, cb).map_err(|e| e.to_string())
    }

    /// Panggil satu metode Chrome DevTools Protocol lewat WebView2; hasil (JSON) atau "ERR ..." dikirim ke `done`.
    #[cfg(windows)]
    pub fn cdp_call(&self, id: &str, method: &str, params: &str, done: impl FnOnce(String) + 'static) -> Result<(), String> {
        use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
        use windows62::core::PCWSTR;
        use wry::WebViewExtWindows;
        let views = self.views.borrow();
        let v = views.get(id).ok_or("tab belum punya webview (panel Browser harus tampil)")?;
        let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
        let (m, p) = (wide(method), wide(params));
        unsafe {
            let core = v.controller().CoreWebView2().map_err(|e| format!("CoreWebView2: {e}"))?;
            let h = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |e, r| {
                done(match e {
                    Ok(()) => r,
                    Err(e) => format!("ERR {e}"),
                });
                Ok(())
            }));
            core.CallDevToolsProtocolMethod(PCWSTR(m.as_ptr()), PCWSTR(p.as_ptr()), &h).map_err(|e| format!("CDP: {e}"))
        }
    }
    #[cfg(not(windows))]
    pub fn cdp_call(&self, _id: &str, _method: &str, _params: &str, _done: impl FnOnce(String) + 'static) -> Result<(), String> {
        Err("CDP hanya tersedia di Windows (WebView2)".into())
    }

    fn script(&self, js: &str) {
        if let Some(v) = self.views.borrow().get(&self.active) {
            let _ = v.evaluate_script(js);
        }
    }
}

/// Permintaan mengembalikan fokus keyboard Win32 dari webview ke jendela GPUI (dipakai sekali oleh canvas berikutnya).
static RELEASE_FOCUS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Panggil saat input GPUI (mis. composer) difokuskan: webview anak memegang fokus Win32 sehingga ketikan masuk ke
/// halaman (mis. kotak cari Google), bukan ke GPUI.
pub fn release_webview_focus() {
    RELEASE_FOCUS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Buat webview tab sebagai anak jendela GPUI.
fn create_view(window: &Window, id: &str, url: &str, agent: bool, tx: futures::channel::mpsc::UnboundedSender<(String, String, bool)>) -> Result<wry::WebView, String> {
    let (id1, tx1) = (id.to_string(), tx.clone());
    // Profil agen terpisah dari tab pengguna (tanpa login pengguna). WebContext dibuat SEKALI per proses (sebelumnya
    // di-leak tiap view agen dibuat) dan hanya dipinjam selama build di thread UI.
    thread_local! {
        static AGENT_CTX: RefCell<Option<wry::WebContext>> = const { RefCell::new(None) };
    }
    fn finish<'a>(b: wry::WebViewBuilder<'a>, url: &str, id1: String, tx1: futures::channel::mpsc::UnboundedSender<(String, String, bool)>) -> wry::WebViewBuilder<'a> {
        b.with_url(url)
            .with_focused(false)
            .with_on_page_load_handler(move |ev, u| {
                let done = matches!(ev, wry::PageLoadEvent::Finished);
                let _ = tx1.unbounded_send((id1.clone(), u, done));
            })
            .with_new_window_req_handler(|_u, _features| wry::NewWindowResponse::Deny)
    }
    if agent {
        AGENT_CTX.with(|c| {
            let mut c = c.borrow_mut();
            let ctx = c.get_or_insert_with(|| {
                let dir = crate::core::services::app_data_dir().join("browser-agent").join("profile");
                wry::WebContext::new(Some(dir))
            });
            let b = wry::WebViewBuilder::new_with_web_context(ctx)
                .with_initialization_script(super::browser_agent::SNAPSHOT_JS)
                .with_navigation_handler(|u| super::browser_agent::nav_ok(&u))
                .with_download_started_handler(|_, _| false);
            finish(b, url, id1, tx1).build_as_child(window).map_err(|e| e.to_string())
        })
    } else {
        finish(wry::WebViewBuilder::new(), url, id1, tx1).build_as_child(window).map_err(|e| e.to_string())
    }
}

impl Render for BrowserPanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        self.shown = true;
        self.hidden_at = None;
        let mut tabs = div().flex().items_center().gap(t.space(1)).px(t.space(2)).py(t.space(1)).overflow_hidden().border_b_1().border_color(t.colors.border_soft);
        for tab in &self.tabs {
            let title = Some(tab_label(&tab.url)).filter(|s| !s.is_empty()).unwrap_or_else(|| i.t("br.newTab"));
            let title = if tab.agent { format!("{} · {title}", i.t("br.agentTab")) } else { title };
            let active = tab.id == self.active;
            let (id1, id2) = (tab.id.clone(), tab.id.clone());
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("brt-{}", tab.id)))
                    .flex()
                    .items_center()
                    .gap(t.space(1))
                    .max_w(t.rem(10.))
                    .pl(t.space(2))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .cursor_pointer()
                    .small(&t)
                    .when(active, |d| d.bg(t.colors.active).text_color(t.colors.fg))
                    .when(!active, |d| d.text_color(t.colors.fg_muted).hover(|s| s.bg(t.colors.hover)))
                    .on_click(cx.listener(move |s, _, _, cx| s.select(id1.clone(), cx)))
                    .child(div().min_w_0().truncate().child(title.clone()))
                    .child(icon_button(SharedString::from(format!("brx-{}", tab.id)), "close", i.tp("br.closeTab", &[("title", &title)])).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| {
                        cx.stop_propagation();
                        s.close(id2.clone(), cx);
                    }))),
            );
        }
        tabs = tabs.child(
            icon_button("br-new", "plus", i.t("br.newTabAction"))
                .size(ControlSize::Sm)
                .disabled(self.tabs.len() >= MAX_BROWSER_TABS)
                .on_click(cx.listener(|s, _, _, cx| s.add(cx))),
        );
        let cur_url = self.tabs.iter().find(|x| x.id == self.active).map(|x| x.url.clone()).unwrap_or_default();
        let ext = cur_url.clone();
        let bar = div()
            .flex()
            .items_center()
            .gap(t.space(1))
            .px(t.space(2))
            .py(t.space(1))
            .border_b_1()
            .border_color(t.colors.border_soft)
            .child(icon_button("br-back", "arrow_left", i.t("br.back")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, _| s.script("history.back()"))))
            .child(icon_button("br-fwd", "arrow_right", i.t("br.forward")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, _| s.script("history.forward()"))))
            .child(icon_button("br-reload", "refresh", i.t("br.reload")).size(ControlSize::Sm).on_click(cx.listener(|s, _, _, _| s.script("location.reload()"))))
            .child(div().flex_1().child(self.address.clone()))
            .child(
                icon_button("br-ext", "external_link", i.t("br.external"))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .disabled(cur_url.is_empty())
                    .on_click(move |_, _, cx| cx.open_url(&ext)),
            );
        let agent_row = (super::browser_agent::session_active() || super::browser_agent::halted()).then(|| {
            let halted = super::browser_agent::halted();
            div()
                .flex()
                .items_center()
                .gap(t.space(2))
                .px(t.space(2))
                .py(t.space(1))
                .border_b_1()
                .border_color(t.colors.border_soft)
                .small(&t)
                .text_color(if halted { t.colors.danger } else { t.colors.accent })
                .child(div().flex_1().min_w_0().truncate().child(if halted { i.t("br.agentStopped") } else { i.t("br.agentActive") }))
                .child(
                    crate::ui::button("br-agent-stop")
                        .label(if halted { i.t("br.agentResume") } else { i.t("br.agentStop") })
                        .variant(if halted { ButtonVariant::Outline } else { ButtonVariant::Danger })
                        .on_click(cx.listener(|_, _, _, cx| {
                            super::browser_agent::toggle_halt();
                            cx.notify();
                        })),
                )
        });
        let snap_el = self
            .snap
            .borrow()
            .as_ref()
            .filter(|(id, _)| *id == self.active && !cur_url.is_empty())
            .map(|(_, img)| div().absolute().inset_0().child(gpui::img(img.clone()).size_full()));
        let loading = self.loading.get(&self.active).copied().unwrap_or(false);
        let progress = div().h(gpui::px(2.)).w_full().when(loading, |d| d.bg(t.colors.accent));

        // Host webview: canvas mengukur area tiap frame lalu menempatkan/membuat webview tab aktif.
        let (views, active, url, tx) = (self.views.clone(), self.active.clone(), cur_url.clone(), self.tx.clone());
    let active_agent = self.tabs.iter().find(|t| t.id == self.active).is_some_and(|t| t.agent);
        let (trash2, vis2, low2) = (self.trash.clone(), self.visible_now.clone(), self.low.clone());
        let err_slot: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let err2 = err_slot.clone();
        let host = canvas(
            move |bounds: Bounds<Pixels>, window, _cx| {
                // Penanda overlay hanya menyala di frame saat menu/dialog ikut digambar. Saat hover item menu, sebagian frame
                // tidak menggambarnya → webview sempat tampil menutupi menu (kedip). Jadi overlay dianggap tetap aktif
                // OVERLAY_HOLD_MS sejak terakhir terdeteksi.
                let now_ms = START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64;
                if crate::ui::take_overlay() {
                    LAST_OVERLAY_MS.store(now_ms + 1, std::sync::atomic::Ordering::Relaxed);
                }
                let last = LAST_OVERLAY_MS.load(std::sync::atomic::Ordering::Relaxed);
                let overlay = last != 0 && now_ms < last - 1 + OVERLAY_HOLD_MS;
                if overlay {
                    // periksa lagi tiap frame sampai overlay tertutup (view bisa di-cache)
                    window.request_animation_frame();
                }
                // Membuat WebView2 memompa pesan; saat startup tugas async lain (engine, dll.) bisa berjalan ulang di tengahnya
                // dan panik ("already borrowed"). Tunda pembuatan webview pertama sampai startup tenang.
                let settled = START.get_or_init(std::time::Instant::now).elapsed() >= std::time::Duration::from_secs(4);
                if !settled {
                    window.request_animation_frame();
                }
                let big = big_ok_and(settled, !overlay && f32::from(bounds.size.width) >= MIN_SIZE && f32::from(bounds.size.height) >= MIN_SIZE);
                for old in trash2.borrow_mut().drain(..) {
                    let _ = window.drop_image(old);
                }
                let mut map = views.borrow_mut();
                if big && !url.is_empty() && !map.contains_key(&active) {
                    match create_view(window, &active, &url, active_agent, tx.clone()) {
                        Ok(v) => {
                            map.insert(active.clone(), v);
                        }
                        Err(e) => *err2.borrow_mut() = Some(e),
                    }
                }
                for (k, v) in map.iter() {
                    let show = big && *k == active;
                    if *k == active && big {
                        set_mem_level(&low2, k, v, false);
                    }
                    if show {
                        let rect = wry::Rect {
                            position: wry::dpi::LogicalPosition::new(f32::from(bounds.origin.x) as f64, f32::from(bounds.origin.y) as f64).into(),
                            size: wry::dpi::LogicalSize::new(f32::from(bounds.size.width) as f64, f32::from(bounds.size.height) as f64).into(),
                        };
                        let _ = v.set_bounds(rect);
                    }
                    let _ = v.set_visible(show);
                }
                if RELEASE_FOCUS.swap(false, std::sync::atomic::Ordering::Relaxed) {
                    if let Some(v) = map.get(&active) {
                        let _ = v.focus_parent();
                    }
                }
                vis2.set(big && map.contains_key(&active));
            },
            |_, _, _, _| {},
        )
        .size_full();
        if let Some(e) = err_slot.borrow_mut().take() {
            eprintln!("webview gagal: {e}");
            if !self.system_fallback {
                self.system_fallback = true;
                if !cur_url.is_empty() {
                    cx.open_url(&cur_url);
                }
            }
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(tabs)
            .child(bar)
            .children(agent_row)
            .child(progress)
            .children(self.error.clone().map(|e| div().px(t.space(2)).small(&t).text_color(t.colors.danger).child(e)))
            .when(self.system_fallback, |d| d.child(div().px(t.space(2)).small(&t).text_color(t.colors.fg_muted).child(i.t("br.systemFallback"))))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .when(cur_url.is_empty(), |d| {
                        d.child(div().absolute().inset_0().flex().items_center().justify_center().gap(t.space(2)).text_color(t.colors.fg_subtle).child(icon("globe").size(IconSize::Md)).child(i.t("br.address")))
                    })
                    .children(snap_el)
                    .child(host),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(normalize_browser_url("contoh.id").unwrap(), "https://contoh.id/");
        assert_eq!(normalize_browser_url("http://Localhost:3000/a?b").unwrap(), "http://localhost:3000/a?b");
        assert!(normalize_browser_url("file:///c:/x").is_err());
        assert!(normalize_browser_url("javascript:alert(1)").is_err());
        assert!(normalize_browser_url("").is_err());
        assert_eq!(tab_label("https://www.contoh.id/x"), "contoh.id");
        assert_eq!(tab_label(""), "");
    }
}

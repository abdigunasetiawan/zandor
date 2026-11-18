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

pub const MAX_BROWSER_TABS: usize = 8;
/// Di bawah ukuran ini (px) webview disembunyikan (mis. panel dilipat).
const MIN_SIZE: f32 = 64.;

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub id: String,
    pub url: String,
}

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

fn next_id(tabs: &[Tab]) -> String {
    (1..).map(|n| format!("b{n}")).find(|id| !tabs.iter().any(|t| &t.id == id)).unwrap_or_default()
}

type Views = Rc<RefCell<HashMap<String, wry::WebView>>>;

pub struct BrowserPanel {
    pub tabs: Vec<Tab>,
    pub active: String,
    address: Entity<TextInput>,
    views: Views,
    loading: HashMap<String, bool>,
    tx: futures::channel::mpsc::UnboundedSender<(String, String, bool)>,
    error: Option<String>,
    /// Webview tertanam gagal dibuat: alamat dibuka di browser sistem (br.systemFallback).
    system_fallback: bool,
    /// true saat panel Browser sedang ditampilkan (diatur ToolsHost).
    pub shown: bool,
}

impl BrowserPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ph = cx.global::<I18n>().t("br.address");
        let address = cx.new(|cx| {
            let mut ti = TextInput::new(cx, ph);
            ti.size = ControlSize::Sm;
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
        Self { tabs: vec![Tab { id: "b1".into(), url: String::new() }], active: "b1".into(), address, views: Default::default(), loading: HashMap::new(), tx, error: None, system_fallback: false, shown: false }
    }

    /// Sembunyikan semua webview (panel lain dipilih / panel ditutup).
    pub fn hide_all(&mut self) {
        self.shown = false;
        for v in self.views.borrow().values() {
            let _ = v.set_visible(false);
        }
    }

    pub fn go(&mut self, raw: String, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        match normalize_browser_url(&raw) {
            Ok(url) => {
                self.error = None;
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
            }
        }
        cx.notify();
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() >= MAX_BROWSER_TABS {
            return;
        }
        let id = next_id(&self.tabs);
        self.tabs.push(Tab { id: id.clone(), url: String::new() });
        self.select(id, cx);
    }

    fn close(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(i) = self.tabs.iter().position(|t| t.id == id) else { return };
        self.tabs.remove(i);
        self.views.borrow_mut().remove(&id);
        if self.tabs.is_empty() {
            let fresh = if id == "b1" { "b2" } else { "b1" };
            self.tabs.push(Tab { id: fresh.into(), url: String::new() });
        }
        if self.active == id {
            let n = self.tabs[i.min(self.tabs.len() - 1)].id.clone();
            self.select(n, cx);
        }
        cx.notify();
    }

    fn script(&self, js: &str) {
        if let Some(v) = self.views.borrow().get(&self.active) {
            let _ = v.evaluate_script(js);
        }
    }
}

/// Buat webview tab sebagai anak jendela GPUI.
fn create_view(window: &Window, id: &str, url: &str, tx: futures::channel::mpsc::UnboundedSender<(String, String, bool)>) -> Result<wry::WebView, String> {
    let (id1, tx1) = (id.to_string(), tx.clone());
    let b = wry::WebViewBuilder::new()
        .with_url(url)
        .with_on_page_load_handler(move |ev, u| {
            let done = matches!(ev, wry::PageLoadEvent::Finished);
            let _ = tx1.unbounded_send((id1.clone(), u, done));
        })
        .with_new_window_req_handler(|_u, _features| wry::NewWindowResponse::Deny);
    b.build_as_child(window).map_err(|e| e.to_string())
}

impl Render for BrowserPanel {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        self.shown = true;
        let mut tabs = div().flex().items_center().gap(t.space(1)).px(t.space(2)).pt(t.space(2)).overflow_hidden();
        for tab in &self.tabs {
            let title = Some(tab_label(&tab.url)).filter(|s| !s.is_empty()).unwrap_or_else(|| i.t("br.newTab"));
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
        let loading = self.loading.get(&self.active).copied().unwrap_or(false);
        let progress = div().h(gpui::px(2.)).w_full().when(loading, |d| d.bg(t.colors.accent));

        // Host webview: canvas mengukur area tiap frame lalu menempatkan/membuat webview tab aktif.
        let (views, active, url, tx) = (self.views.clone(), self.active.clone(), cur_url.clone(), self.tx.clone());
        let err_slot: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let err2 = err_slot.clone();
        let host = canvas(
            move |bounds: Bounds<Pixels>, window, _cx| {
                let overlay = crate::ui::take_overlay();
                if overlay {
                    // periksa lagi tiap frame sampai overlay tertutup (view bisa di-cache)
                    window.request_animation_frame();
                }
                let big = !overlay && f32::from(bounds.size.width) >= MIN_SIZE && f32::from(bounds.size.height) >= MIN_SIZE;
                let mut map = views.borrow_mut();
                if big && !url.is_empty() && !map.contains_key(&active) {
                    match create_view(window, &active, &url, tx.clone()) {
                        Ok(v) => {
                            map.insert(active.clone(), v);
                        }
                        Err(e) => *err2.borrow_mut() = Some(e),
                    }
                }
                for (k, v) in map.iter() {
                    let show = big && *k == active;
                    if show {
                        let rect = wry::Rect {
                            position: wry::dpi::LogicalPosition::new(f32::from(bounds.origin.x) as f64, f32::from(bounds.origin.y) as f64).into(),
                            size: wry::dpi::LogicalSize::new(f32::from(bounds.size.width) as f64, f32::from(bounds.size.height) as f64).into(),
                        };
                        let _ = v.set_bounds(rect);
                    }
                    let _ = v.set_visible(show);
                }
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
            .gap(t.space(1))
            .child(tabs)
            .child(bar)
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

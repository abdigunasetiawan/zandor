//! Penampil audio/video (port MediaViewer di simple-viewers.tsx): pemutar bawaan WebView2 (lewat wry) yang membuka
//! berkas media langsung, sebagai jendela anak di atas area penampil. Seperti Browser, webview disembunyikan selama ada
//! menu/dialog (airspace) dan dihancurkan saat tab lain aktif (entity dibuang oleh DocsPanel).
use crate::i18n::I18n;
use crate::theme::Theme;
use crate::ui::TypeExt;
use gpui::{canvas, div, prelude::*, Bounds, Context, IntoElement, Pixels, Window};
use std::cell::RefCell;
use std::rc::Rc;

pub struct MediaView {
    url: String,
    view: Rc<RefCell<Option<wry::WebView>>>,
    error: Rc<RefCell<Option<String>>>,
}

/// file:///C:/a%20b/c.mp4
pub fn file_url(path: &str) -> String {
    let p = path.replace('\\', "/");
    let mut out = String::from("file:///");
    for c in p.trim_start_matches('/').chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '/' | ':' | '-' | '_' | '.' | '~' => out.push(c),
            _ => {
                let mut b = [0u8; 4];
                for x in c.encode_utf8(&mut b).bytes() {
                    out.push_str(&format!("%{x:02X}"));
                }
            }
        }
    }
    out
}

impl MediaView {
    pub fn new(path: &str, cx: &mut Context<Self>) -> Self {
        let me = Self { url: file_url(path), view: Rc::new(RefCell::new(None)), error: Rc::new(RefCell::new(None)) };
        let view = me.view.clone();
        cx.observe_global::<super::layout::PanelShown>(move |_, cx| {
            if !cx.global::<super::layout::PanelShown>().docs {
                if let Some(w) = view.borrow().as_ref() {
                    let _ = w.set_visible(false);
                }
            }
        })
        .detach();
        me
    }
}

impl Render for MediaView {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        if let Some(e) = self.error.borrow().clone() {
            return div().p(t.space(3)).text_color(t.colors.danger).child(i.tp("doc.mediaError", &[("error", &e)])).into_any_element();
        }
        let (view, err, url) = (self.view.clone(), self.error.clone(), self.url.clone());
        let me = cx.entity().downgrade();
        let host = canvas(
            move |bounds: Bounds<Pixels>, window, cx| {
                let overlay = crate::ui::take_overlay();
                if overlay {
                    window.request_animation_frame();
                }
                let big = !overlay && f32::from(bounds.size.width) >= 32. && f32::from(bounds.size.height) >= 32.;
                let mut v = view.borrow_mut();
                if big && v.is_none() && err.borrow().is_none() {
                    match wry::WebViewBuilder::new().with_url(&url).with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny).build_as_child(window) {
                        Ok(w) => *v = Some(w),
                        Err(e) => {
                            *err.borrow_mut() = Some(e.to_string());
                            me.update(cx, |_, cx| cx.notify()).ok();
                        }
                    }
                }
                if let Some(w) = v.as_ref() {
                    if big {
                        let _ = w.set_bounds(wry::Rect {
                            position: wry::dpi::LogicalPosition::new(f32::from(bounds.origin.x) as f64, f32::from(bounds.origin.y) as f64).into(),
                            size: wry::dpi::LogicalSize::new(f32::from(bounds.size.width) as f64, f32::from(bounds.size.height) as f64).into(),
                        });
                    }
                    let _ = w.set_visible(big);
                }
            },
            |_, _, _, _| {},
        )
        .size_full();
        div().flex().flex_col().flex_1().min_h(t.rem(16.)).size_full().child(host).child(div().px(t.space(2)).py(t.space(1)).small(&t).text_color(t.colors.fg_subtle).child(self.url.clone())).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn url_berkas_di_encode() {
        assert_eq!(super::file_url(r"C:\a b\vid.mp4"), "file:///C:/a%20b/vid.mp4");
    }
}

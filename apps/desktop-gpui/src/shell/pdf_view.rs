//! Penampil PDF (port PdfViewer.tsx) dengan perender bawaan Windows (core/pdf.rs): bilah halaman "n dari total",
//! zoom −/+/100% (tahap sama dengan React), halaman digambar di thread latar sesuai lebar × zoom dan di-cache.
//! Cari teks: Windows.Data.Pdf tidak memberi lapisan teks, jadi teks tiap halaman dibaca dengan OCR bawaan Windows
//! (core::pdf::page_text) di thread latar; temuan disorot di atas gambar halaman. Pilih/salin teks belum tersedia.
use crate::i18n::I18n;
use crate::theme::Theme;
use crate::ui::{button, icon_button, ControlSize, TypeExt};
use gpui::{div, prelude::*, px, Context, IntoElement, ScrollHandle, SharedString, Window};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const ZOOMS: [f32; 7] = [0.5, 0.75, 1., 1.25, 1.5, 2., 3.];
/// Lebar dasar halaman pada zoom 100% (px logis).
const BASE_W: f32 = 720.;
const MAX_PAGES: usize = 500;

pub struct PdfView {
    path: String,
    info: Option<Result<crate::core::pdf::PdfInfo, String>>,
    scale: f32,
    /// (halaman, lebar px) → gambar.
    pages: HashMap<(usize, u32), Arc<gpui::Image>>,
    pending: HashSet<(usize, u32)>,
    scroll: ScrollHandle,
    find: Option<gpui::Entity<crate::ui::TextInput>>,
    /// Teks OCR per halaman (None = belum selesai dibaca).
    texts: Option<Vec<Vec<crate::core::pdf::TextLine>>>,
    texts_started: bool,
    hit: usize,
}

impl PdfView {
    pub fn new(path: String, cx: &mut Context<Self>) -> Self {
        let p = path.clone();
        let task = cx.background_spawn(async move { crate::core::pdf::info(&p) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.info = Some(r);
                cx.notify();
            })
            .ok();
        })
        .detach();
        Self { path, info: None, scale: 1., pages: HashMap::new(), pending: HashSet::new(), scroll: ScrollHandle::new(), find: None, texts: None, texts_started: false, hit: 0 }
    }

    fn request(&mut self, page: usize, w: u32, cx: &mut Context<Self>) {
        if self.pages.contains_key(&(page, w)) || !self.pending.insert((page, w)) {
            return;
        }
        let p = self.path.clone();
        let task = cx.background_spawn(async move { crate::core::pdf::render_page(&p, page as u32, w) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.pending.remove(&(page, w));
                if let Ok(png) = r {
                    s.pages.insert((page, w), Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png)));
                    // simpan hanya resolusi terbaru per halaman
                    s.pages.retain(|(pg, ww), _| *pg != page || *ww == w);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Baca teks semua halaman sekali (OCR), dipicu saat pertama kali mencari.
    fn load_texts(&mut self, n: usize, cx: &mut Context<Self>) {
        if self.texts_started {
            return;
        }
        self.texts_started = true;
        let p = self.path.clone();
        let task = cx.background_spawn(async move { (0..n.min(MAX_PAGES)).map(|k| crate::core::pdf::page_text(&p, k as u32).unwrap_or_default()).collect::<Vec<_>>() });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.texts = Some(r);
                s.scroll_to_hit(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Daftar temuan (halaman, kotak-kotak) untuk kueri sekarang.
    fn hits(&self, cx: &gpui::App) -> Vec<(usize, Vec<[f32; 4]>)> {
        let q = self.find.as_ref().map(|f| f.read(cx).text().to_string()).unwrap_or_default();
        let Some(texts) = &self.texts else { return vec![] };
        texts.iter().enumerate().flat_map(|(n, l)| crate::core::pdf::find_in_lines(l, &q).into_iter().map(move |b| (n, b))).collect()
    }

    fn scroll_to_hit(&mut self, cx: &mut Context<Self>) {
        let hits = self.hits(cx);
        if hits.is_empty() {
            return;
        }
        let k = self.hit % hits.len();
        self.scroll.scroll_to_item(hits[k].0);
    }

    fn step(&mut self, d: isize, cx: &mut Context<Self>) {
        let n = self.hits(cx).len();
        if n == 0 {
            return;
        }
        self.hit = ((self.hit as isize + d).rem_euclid(n as isize)) as usize;
        self.scroll_to_hit(cx);
        cx.notify();
    }

    fn set_scale(&mut self, v: f32, cx: &mut Context<Self>) {
        self.scale = v;
        cx.notify();
    }

    /// Halaman yang sedang di sepertiga atas viewport (port onScroll).
    fn current(&self, heights: &[f32], gap: f32) -> usize {
        let top = -f32::from(self.scroll.offset().y);
        let view = f32::from(self.scroll.bounds().size.height);
        if f32::from(self.scroll.max_offset().y) - top < 2. {
            return heights.len().saturating_sub(1);
        }
        let mid = top + view / 3.;
        let mut y = 0.;
        for (n, h) in heights.iter().enumerate() {
            y += h + gap;
            if y > mid {
                return n;
            }
        }
        heights.len().saturating_sub(1)
    }
}

impl Render for PdfView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let info = match &self.info {
            None => return div().p(t.space(3)).small(&t).child(i.t("doc.loading")).into_any_element(),
            Some(Err(e)) => return div().p(t.space(3)).text_color(t.colors.danger).child(i.tp("doc.pdfError", &[("error", e)])).into_any_element(),
            Some(Ok(x)) => x.clone(),
        };
        let total = info.pages.len();
        let zi = ZOOMS.iter().position(|z| (*z - self.scale).abs() < 1e-3);
        let page_w = BASE_W * self.scale * t.scale;
        let dpi = window.scale_factor();
        let px_w = (page_w * dpi).round() as u32;
        let gap = f32::from(t.space(3));
        let heights: Vec<f32> = info.pages.iter().map(|(w, h)| page_w * h / w.max(1.)).collect();
        let cur = self.current(&heights, gap) + 1;

        // muat halaman di sekitar viewport saja
        let top = -f32::from(self.scroll.offset().y);
        let view = f32::from(self.scroll.bounds().size.height).max(800.);
        let mut y = 0.;
        let mut want = vec![];
        for (n, h) in heights.iter().enumerate().take(MAX_PAGES) {
            if y + h >= top - view && y <= top + 2. * view {
                want.push(n);
            }
            y += h + gap;
        }
        for n in want {
            self.request(n, px_w, cx);
        }

        let find = match &self.find {
            Some(f) => f.clone(),
            None => {
                let f = cx.new(|cx| crate::ui::TextInput::new(cx, i.t("doc.find")));
                cx.observe(&f, move |s, _, cx| {
                    s.hit = 0;
                    s.load_texts(total, cx);
                    s.scroll_to_hit(cx);
                    cx.notify();
                })
                .detach();
                self.find = Some(f.clone());
                f
            }
        };
        let q = find.read(cx).text().trim().to_string();
        let hits = self.hits(cx);
        let cur_hit = if hits.is_empty() { usize::MAX } else { self.hit % hits.len() };
        let find_status = if q.is_empty() {
            String::new()
        } else if self.texts.is_none() {
            i.t("doc.pdfFindSearching").to_string()
        } else if hits.is_empty() {
            i.t("ui.noResults").to_string()
        } else {
            i.tp("doc.pdfFindCount", &[("i", &(cur_hit + 1).to_string()), ("n", &hits.len().to_string())]).to_string()
        };
        let bar = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(t.space(1))
            .px(t.space(2))
            .py(t.space(1))
            .border_b_1()
            .border_color(t.colors.border_soft)
            .child(div().small(&t).child(i.tp("doc.pdfPage", &[("n", &cur.to_string()), ("total", &total.to_string())])))
            .child(div().flex_1())
            .child(div().w(t.rem(11.)).child(find.clone()))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(find_status))
            .child(icon_button("pdf-fprev", "chevron_up", i.t("diff.prev")).size(ControlSize::Sm).disabled(hits.is_empty()).on_click(cx.listener(|s, _, _, cx| s.step(-1, cx))))
            .child(icon_button("pdf-fnext", "chevron_down", i.t("diff.next")).size(ControlSize::Sm).disabled(hits.is_empty()).on_click(cx.listener(|s, _, _, cx| s.step(1, cx))))
            .child(icon_button("pdf-zout", "chevron_down", i.t("doc.zoomOut")).size(ControlSize::Sm).disabled(zi == Some(0)).on_click(cx.listener(move |s, _, _, cx| {
                let k = ZOOMS.iter().position(|z| (*z - s.scale).abs() < 1e-3).unwrap_or(2);
                s.set_scale(ZOOMS[k.saturating_sub(1)], cx);
            })))
            .child(div().small(&t).child(format!("{}%", (self.scale * 100.).round())))
            .child(icon_button("pdf-zin", "chevron_up", i.t("doc.zoomIn")).size(ControlSize::Sm).disabled(zi == Some(ZOOMS.len() - 1)).on_click(cx.listener(move |s, _, _, cx| {
                let k = ZOOMS.iter().position(|z| (*z - s.scale).abs() < 1e-3).unwrap_or(2);
                s.set_scale(ZOOMS[(k + 1).min(ZOOMS.len() - 1)], cx);
            })))
            .child(button("pdf-100").size(ControlSize::Sm).label("100%").on_click(cx.listener(|s, _, _, cx| s.set_scale(1., cx))));

        let mut list = div().flex().flex_col().items_center().gap(px(gap)).p(t.space(3));
        for (n, h) in heights.iter().enumerate().take(MAX_PAGES) {
            let img = self.pages.iter().find(|((pg, _), _)| *pg == n).map(|(_, v)| v.clone());
            list = list.child(
                div()
                    .id(SharedString::from(format!("pdfp-{n}")))
                    .w(px(page_w))
                    .h(px(*h))
                    .flex_none()
                    .bg(gpui::white())
                    .shadow(t.shadow_1())
                    .relative()
                    .children(img.map(|im| gpui::img(im).size_full()))
                    .children(hits.iter().enumerate().filter(|(_, x)| x.0 == n).flat_map(|(k, x)| {
                        let c = if k == cur_hit { t.colors.warning.opacity(0.55) } else { t.colors.warning.opacity(0.25) };
                        let (pw, ph) = (page_w, *h);
                        x.1.iter().map(move |b| div().absolute().left(px(b[0] * pw)).top(px(b[1] * ph)).w(px(b[2] * pw)).h(px(b[3] * ph)).bg(c)).collect::<Vec<_>>()
                    })),
            );
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(bar)
            .child(div().id("pdf-pages").flex_1().min_h_0().overflow_scroll().track_scroll(&self.scroll).on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify())).child(list))
            .into_any_element()
    }
}

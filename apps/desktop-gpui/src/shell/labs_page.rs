//! Halaman Labs (`/labs`) — tempat bereksperimen dengan desain sebelum dipasang ke aplikasi (menu Dev Tools > Labs).
//! Tiap halaman lab berdiri sendiri; saat ini: Inline code (10 alternatif gaya). Tidak ada data palsu: contoh teks
//! bisa diubah sendiri (apit dengan backtick untuk menandai kode).
use super::state::AppState;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, FONT_CODE, H2, H3, LABEL};
use crate::ui::{button, ButtonVariant, ControlSize, TextInput, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontStyle, FontWeight, Hsla, IntoElement, SharedString, StyledText, UnderlineStyle, Window};

/// Halaman lab yang tersedia: (id, kunci i18n judul).
const PAGES: [(&str, &str); 2] = [("inline-code", "labs.page.inlineCode"), ("notification", "labs.page.notification")];

const DEFAULT_SAMPLE: &str = "Proyek aktif: `fpk-be`: Backend, `fpk-public`: Publik, `fpk-admin`: Admin. Atribut: `alt=\"Abdi\"` dan `loading=\"lazy\"`.";

/// Gaya satu varian inline code. Semua nilai warna dibaca dari tema saat dibangun (`variants`).
#[derive(Clone, Default)]
struct Spec {
    mono: bool,
    weight: Option<FontWeight>,
    italic: bool,
    color: Option<Hsla>,
    bg: Option<Hsla>,
    underline: Option<Hsla>,
    /// Spasi tak-pecah di kedua sisi (pengganti padding, agar latar terlihat seperti chip).
    pad: bool,
    /// Pembungkus yang ikut tampil (mis. backtick).
    wrap: Option<(&'static str, &'static str)>,
    /// Warna/latar pembungkus bila berbeda dari isi (marker berwarna, chip dua nada).
    wrap_color: Option<Hsla>,
    wrap_bg: Option<Hsla>,
    /// Huruf kapital dengan jarak renggang (gaya label).
    upper_spaced: bool,
    /// Keluarga font selain mono bawaan (mis. serif editorial).
    family: Option<&'static str>,
    wavy: bool,
    /// Tebal garis bawah dalam px (0 = 1px).
    ul_px: f32,
}

fn variants(t: &Theme) -> [(&'static str, Spec); 30] {
    let c = &t.colors;
    [
        ("labs.inlineCode.v1", Spec { mono: true, bg: Some(c.active), color: Some(c.fg), pad: true, ..Default::default() }),
        ("labs.inlineCode.v2", Spec { mono: true, bg: Some(c.hover), color: Some(c.fg), pad: true, ..Default::default() }),
        ("labs.inlineCode.v3", Spec { weight: Some(FontWeight::MEDIUM), color: Some(c.fg), ..Default::default() }),
        ("labs.inlineCode.v4", Spec { mono: true, color: Some(c.fg_muted), ..Default::default() }),
        ("labs.inlineCode.v5", Spec { mono: true, color: Some(c.fg), underline: Some(c.border_strong), ..Default::default() }),
        ("labs.inlineCode.v6", Spec { mono: true, color: Some(c.accent), ..Default::default() }),
        ("labs.inlineCode.v7", Spec { mono: true, color: Some(c.fg_muted), wrap: Some(("`", "`")), ..Default::default() }),
        ("labs.inlineCode.v8", Spec { italic: true, color: Some(c.fg), ..Default::default() }),
        ("labs.inlineCode.v9", Spec { mono: true, bg: Some(c.hover), color: Some(c.fg), underline: Some(c.accent), pad: true, ..Default::default() }),
        ("labs.inlineCode.v10", Spec { weight: Some(FontWeight::MEDIUM), bg: Some(c.warning.opacity(0.28)), color: Some(c.fg), pad: true, ..Default::default() }),
        ("labs.inlineCode.v11", Spec { mono: true, weight: Some(FontWeight::SEMIBOLD), color: Some(c.fg), ..Default::default() }),
        ("labs.inlineCode.v12", Spec { mono: true, italic: true, color: Some(c.fg), ..Default::default() }),
        ("labs.inlineCode.v13", Spec { mono: true, color: Some(c.fg_muted), wrap: Some(("‹", "›")), ..Default::default() }),
        ("labs.inlineCode.v14", Spec { mono: true, bg: Some(c.accent.opacity(0.18)), color: Some(c.accent), pad: true, ..Default::default() }),
        ("labs.inlineCode.v15", Spec { weight: Some(FontWeight::MEDIUM), color: Some(c.fg), underline: Some(c.accent), ..Default::default() }),
        ("labs.inlineCode.v16", Spec { mono: true, bg: Some(c.fg), color: Some(c.bg), pad: true, ..Default::default() }),
        ("labs.inlineCode.v17", Spec { mono: true, bg: Some(c.raised), color: Some(c.fg), pad: true, ..Default::default() }),
        ("labs.inlineCode.v18", Spec { mono: true, bg: Some(c.border_soft), color: Some(c.fg_muted), pad: true, ..Default::default() }),
        ("labs.inlineCode.v19", Spec { weight: Some(FontWeight::MEDIUM), color: Some(c.fg), wrap: Some(("▸ ", "")), ..Default::default() }),
        ("labs.inlineCode.v20", Spec { mono: true, bg: Some(c.warning.opacity(0.28)), color: Some(c.fg), pad: true, ..Default::default() }),
        ("labs.inlineCode.v21", Spec { mono: true, bg: Some(c.hover), color: Some(c.fg), pad: true, wrap: Some(("▎", "")), wrap_color: Some(c.accent), ..Default::default() }),
        ("labs.inlineCode.v22", Spec { mono: true, bg: Some(c.hover), color: Some(c.fg), pad: true, wrap: Some(("$", "")), wrap_color: Some(c.fg_subtle), ..Default::default() }),
        ("labs.inlineCode.v23", Spec { weight: Some(FontWeight::MEDIUM), color: Some(c.fg), underline: Some(c.accent), wavy: true, ..Default::default() }),
        ("labs.inlineCode.v24", Spec { mono: true, weight: Some(FontWeight::MEDIUM), color: Some(c.fg_muted), upper_spaced: true, ..Default::default() }),
        ("labs.inlineCode.v25", Spec { mono: true, color: Some(c.fg), wrap: Some(("#", "")), wrap_color: Some(c.accent), ..Default::default() }),
        ("labs.inlineCode.v26", Spec { family: Some("Georgia"), italic: true, color: Some(c.fg), ..Default::default() }),
        ("labs.inlineCode.v27", Spec { weight: Some(FontWeight::MEDIUM), color: Some(c.fg), underline: Some(c.warning.opacity(0.7)), ul_px: 4., ..Default::default() }),
        ("labs.inlineCode.v28", Spec { mono: true, color: Some(c.fg), wrap: Some(("「", "」")), wrap_color: Some(c.border_strong), ..Default::default() }),
        ("labs.inlineCode.v29", Spec { mono: true, color: Some(c.fg), wrap: Some(("/", "")), wrap_color: Some(c.fg_subtle), ..Default::default() }),
        ("labs.inlineCode.v30", Spec { mono: true, bg: Some(c.hover), color: Some(c.fg), pad: true, wrap: Some(("‹", "›")), wrap_color: Some(c.accent), ..Default::default() }),
    ]
}

/// Pecah teks di backtick: bagian ganjil = kode. Backtick tak berpasangan diperlakukan sebagai teks biasa.
fn split_code(text: &str) -> Vec<(String, bool)> {
    let parts: Vec<&str> = text.split('`').collect();
    let paired = parts.len() % 2 == 1;
    parts
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.is_empty())
        .map(|(n, p)| (p.to_string(), n % 2 == 1 && (paired || n + 1 < parts.len())))
        .collect()
}

/// Satu kalimat contoh dengan gaya kode `spec`. `HighlightStyle` tidak punya `font_family`, jadi run dibangun sendiri.
#[derive(IntoElement)]
struct Sample {
    segs: Vec<(String, bool)>,
    spec: Spec,
}

impl RenderOnce for Sample {
    fn render(self, window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let base = window.text_style();
        let mut text = String::new();
        let mut runs = Vec::with_capacity(self.segs.len());
        for (seg, code) in &self.segs {
            if !*code {
                text.push_str(seg);
                runs.push(base.to_run(seg.len()));
                continue;
            }
            let s = &self.spec;
            let style = |color: Option<Hsla>, bg: Option<Hsla>| {
                let mut st = base.clone();
                if s.mono {
                    st.font_family = FONT_CODE.into();
                }
                if let Some(f) = s.family {
                    st.font_family = f.into();
                }
                if let Some(w) = s.weight {
                    st.font_weight = w;
                }
                if s.italic {
                    st.font_style = FontStyle::Italic;
                }
                if let Some(c) = color {
                    st.color = c;
                }
                st.background_color = bg;
                st.underline = s.underline.map(|c| UnderlineStyle { thickness: gpui::px(s.ul_px.max(1.)), color: Some(c), wavy: s.wavy });
                st
            };
            let inner = if s.upper_spaced { seg.to_uppercase().chars().map(String::from).collect::<Vec<_>>().join("\u{200a}") } else { seg.clone() };
            let pad = if s.pad { "\u{a0}" } else { "" };
            let (pre, post) = s.wrap.unwrap_or(("", ""));
            let wrap_style = style(s.wrap_color.or(s.color), s.wrap_bg.or(s.bg));
            let mut push = |piece: String, st: gpui::TextStyle| {
                if !piece.is_empty() {
                    runs.push(st.to_run(piece.len()));
                    text.push_str(&piece);
                }
            };
            push(pre.to_string(), wrap_style.clone());
            push(format!("{pad}{inner}{pad}"), style(s.color, s.bg));
            push(post.to_string(), wrap_style);
        }
        StyledText::new(text).with_runs(runs)
    }
}

pub struct LabsPage {
    app: Entity<AppState>,
    page: &'static str,
    input: Entity<TextInput>,
}

impl LabsPage {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(cx, "Teks contoh"));
        input.update(cx, |i, cx| i.set_text(DEFAULT_SAMPLE, cx));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        Self { app, page: PAGES[0].0, input }
    }

    fn inline_code(&self, t: &Theme, i: &I18n, cx: &Context<Self>) -> AnyElement {
        let segs = split_code(&self.input.read(cx).text());
        let cards = variants(t).into_iter().enumerate().map(|(n, (key, spec))| {
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .rounded(t.rem(rem::RADIUS_LG))
                .border_1()
                .border_color(t.colors.border_soft)
                .child(div().typo(t, LABEL).child(format!("{}. {}", n + 1, i.t(key))))
                .child(div().child(Sample { segs: segs.clone(), spec }))
        });
        div()
            .flex()
            .flex_col()
            .gap(t.space(4))
            .child(div().typo(t, H2).child(i.t("labs.page.inlineCode")))
            .child(div().small(t).text_color(t.colors.fg_muted).child(i.t("labs.inlineCode.desc")))
            .child(div().flex().flex_col().gap(t.space(1)).child(div().typo(t, LABEL).child(i.t("labs.inlineCode.sample"))).child(self.input.clone()))
            .child(div().typo(t, H3).child(i.t("labs.inlineCode.variants")))
            .child(div().flex().flex_col().gap(t.space(3)).children(cards))
            .into_any_element()
    }
}

impl Render for LabsPage {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let app = self.app.clone();
        let header = div()
            .flex()
            .items_center()
            .gap(t.space(3))
            .px(t.space(5))
            .py(t.space(3))
            .border_b_1()
            .border_color(t.colors.border)
            .child(div().typo(&t, H2).flex_1().child(i.t("labs.title")))
            .child(button("labs-back").size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("labs.back")).on_click(move |_, _, cx| app.update(cx, |a, cx| a.push("/", cx))));
        let nav = div().flex_none().w(t.rem(11.)).p(t.space(3)).flex().flex_col().gap(t.space(1)).border_r_1().border_color(t.colors.border_soft).children(PAGES.iter().map(|(id, key)| {
            let on = *id == self.page;
            div()
                .id(SharedString::from(format!("labs-nav-{id}")))
                .px(t.space(2))
                .py(t.space(1))
                .rounded(t.rem(rem::RADIUS_SM))
                .cursor_pointer()
                .when(on, |d| d.bg(t.colors.active))
                .hover(|s| s.bg(t.colors.hover))
                .child(i.t(key))
                .on_click(cx.listener({
                    let id = *id;
                    move |s, _, _, cx| {
                        s.page = id;
                        cx.notify();
                    }
                }))
        }));
        let body = match self.page {
            "notification" => super::labs_notif::view(&t, &i),
            _ => self.inline_code(&t, &i, cx),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .child(div().flex().flex_1().min_h_0().child(nav).child(div().id("labs-content").flex_1().min_w_0().overflow_y_scroll().p(t.space(5)).child(body)))
    }
}

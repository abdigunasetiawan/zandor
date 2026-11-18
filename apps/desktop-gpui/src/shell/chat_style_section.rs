//! Tab "Panel chat" di Tampilan — port ChatTab + ChatStyleControls.tsx: kerapatan, jenis huruf, ketebalan, slider
//! lebar/rasio composer/ukuran/transparansi/tinggi baris/jarak huruf/jarak pesan/ukuran kode, dan kembalikan bawaan.
use crate::core::appearance::AppearanceStore;
use crate::core::settings::SettingsStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, select, ButtonVariant, ControlSize, Slider, SliderChanged, TypeExt};
use gpui::{div, prelude::*, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use serde_json::{json, Value};

/// (kunci, label, min, max, step, default, satuan)
const SPECS: [(&str, &str, f32, f32, f32, f32, &str); 8] = [
    ("width", "chatStyle.width", 28., 120., 1., 46., "rem"),
    ("composerRatio", "chatStyle.composerRatio", 1., 1.6, 0.05, 1.2, ""),
    ("size", "chatStyle.size", 0.6, 1.6, 0.025, 0.875, "rem"),
    ("opacity", "chatStyle.opacity", 0.3, 1., 0.05, 1., ""),
    ("lineHeight", "chatStyle.lineHeight", 1., 2.4, 0.05, 1.55, ""),
    ("tracking", "chatStyle.tracking", -0.05, 0.2, 0.005, 0., "em"),
    ("gap", "chatStyle.gap", 0., 3., 0.125, 0.75, "rem"),
    ("codeSize", "chatStyle.codeSize", 0.6, 1.6, 0.025, 0.8125, "rem"),
];
pub const FONTS: [(&str, &str); 4] = [
    ("", "chatStyle.font.default"),
    ("system-ui, 'Segoe UI', sans-serif", "chatStyle.font.system"),
    ("Georgia, 'Times New Roman', serif", "chatStyle.font.serif"),
    ("'JetBrains Mono Variable', Consolas, monospace", "chatStyle.font.mono"),
];
const WEIGHTS: [u32; 5] = [300, 400, 500, 600, 700];
/// (nama, gap, lineHeight)
const DENSITY: [(&str, Option<&str>, Option<f64>); 3] = [("compact", Some("0.375rem"), Some(1.35)), ("cozy", None, None), ("comfy", Some("1.5rem"), Some(1.8))];

fn round(n: f32) -> f32 {
    (n * 10000.).round() / 10000.
}

pub struct ChatStyleSection {
    store: Entity<AppearanceStore>,
    sliders: Vec<Entity<Slider>>,
    font: Entity<super::font_picker::FontPicker>,
}

fn width_def(cx: &gpui::App) -> f32 {
    match SettingsStore::global(cx).read(cx).str("chatWidth").as_str() {
        "narrow" => 38.,
        "wide" => 56.,
        _ => 46.,
    }
}

fn current(chat: &serde_json::Map<String, Value>, key: &str, def: f32) -> f32 {
    match chat.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(def as f64) as f32,
        Some(Value::String(s)) => s.trim_end_matches(|c: char| c.is_alphabetic()).parse().unwrap_or(def),
        _ => def,
    }
}

impl ChatStyleSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = AppearanceStore::global(cx);
        let i = cx.global::<I18n>().clone();
        let wdef = width_def(cx);
        let chat = store.read(cx).chat.clone();
        let mut sliders = vec![];
        for (n, (key, label, min, max, step, def, _)) in SPECS.iter().enumerate() {
            let def = if *key == "width" { wdef } else { *def };
            let v = current(&chat, key, def);
            let s = cx.new(|_| Slider::new(v, *min, *max, *step, i.t(label)));
            cx.subscribe(&s, move |this, _, e: &SliderChanged, cx| this.set(n, e.0, cx)).detach();
            sliders.push(s);
        }
        cx.observe(&store, |this, _, cx| {
            this.sync(cx);
            cx.notify();
        })
        .detach();
        let cur_font = chat.get("font").and_then(Value::as_str).map(str::to_string);
        let label = i.t("chatStyle.font");
        let font = cx.new(|cx| super::font_picker::FontPicker::new("cs-font", cur_font, FONTS.to_vec(), label, cx));
        cx.subscribe(&font, |this, _, e: &super::font_picker::FontPicked, cx| {
            let v = e.0.clone();
            this.store.update(cx, |s, cx| s.set_chat("font", v.map(|v| json!(v)), cx));
        })
        .detach();
        Self { store, sliders, font }
    }

    fn def_of(&self, n: usize, cx: &gpui::App) -> f32 {
        if SPECS[n].0 == "width" { width_def(cx) } else { SPECS[n].5 }
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let chat = self.store.read(cx).chat.clone();
        let f = chat.get("font").and_then(Value::as_str).map(str::to_string);
        self.font.update(cx, |p, cx| p.set_value(f, cx));
        for n in 0..SPECS.len() {
            let v = current(&chat, SPECS[n].0, self.def_of(n, cx));
            self.sliders[n].update(cx, |s, cx| {
                if (s.value - v).abs() > 1e-6 {
                    s.value = v;
                    cx.notify();
                }
            });
        }
    }

    fn set(&mut self, n: usize, v: f32, cx: &mut Context<Self>) {
        let (key, .., unit) = SPECS[n];
        let r = round(v);
        let val = if (r - self.def_of(n, cx)).abs() < 1e-6 {
            None
        } else if unit.is_empty() {
            Some(json!(r))
        } else {
            Some(json!(format!("{r}{unit}")))
        };
        self.store.update(cx, |s, cx| s.set_chat(key, val, cx));
    }
}

impl Render for ChatStyleSection {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let chat = self.store.read(cx).chat.clone();
        let row = |label: String, control: gpui::AnyElement| {
            div().flex().items_center().gap(t.space(3)).child(div().flex_1().font_weight(FontWeight::MEDIUM).child(label)).child(control)
        };

        let mut dens = div().flex().gap(t.space(1));
        for (name, gap, lh) in DENSITY {
            let on = chat.get("gap").and_then(Value::as_str) == gap && chat.get("lineHeight").and_then(Value::as_f64) == lh;
            dens = dens.child(
                button(SharedString::from(format!("dens-{name}")))
                    .label(i.t(&format!("chatStyle.density.{name}")))
                    .size(ControlSize::Sm)
                    .variant(if on { ButtonVariant::Solid } else { ButtonVariant::Ghost })
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.store.update(cx, |st, cx| {
                            st.set_chat("gap", gap.map(|g| json!(g)), cx);
                            st.set_chat("lineHeight", lh.map(|l| json!(l)), cx);
                        });
                    })),
            );
        }

        let font_sel = self.font.clone();
        let store = self.store.clone();
        let weights = WEIGHTS.iter().map(|w| (SharedString::from(w.to_string()), SharedString::from(i.t(&format!("chatStyle.weight.{w}"))))).collect();
        let cur_w = chat.get("weight").and_then(Value::as_u64).unwrap_or(400).to_string();
        let weight_sel = select("cs-weight", weights, cur_w).on_change(move |v, _, cx| {
            let n: u32 = v.parse().unwrap_or(400);
            store.update(cx, |s, cx| s.set_chat("weight", (n != 400).then(|| json!(n)), cx));
        });

        let mut col = div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.chat.intro")))
            .child(row(i.t("chatStyle.density"), dens.into_any_element()))
            .child(row(i.t("chatStyle.font"), font_sel.into_any_element()))
            .child(row(i.t("chatStyle.weight"), weight_sel.into_any_element()));
        for (n, (_, label, .., unit)) in SPECS.iter().enumerate() {
            let v = round(self.sliders[n].read(cx).value);
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .child(
                        div()
                            .flex()
                            .child(div().flex_1().font_weight(FontWeight::MEDIUM).child(i.t(label)))
                            .child(div().small(&t).code(&t).child(format!("{v}{unit}"))),
                    )
                    .child(self.sliders[n].clone()),
            );
        }
        // pratinjau memakai gaya chat aktif
        let cv = t.chat.clone();
        let preview = div()
            .p(t.space(3))
            .rounded(t.rem(rem::RADIUS_LG))
            .border_1()
            .border_color(t.colors.border_soft)
            .bg(t.colors.surface)
            .text_size(t.rem(cv.size.unwrap_or(t.ty(crate::theme::P).size)))
            .line_height(gpui::relative(cv.line.unwrap_or(t.ty(crate::theme::P).lh)))
            .font_weight(gpui::FontWeight(cv.weight.unwrap_or(t.ty(crate::theme::P).weight)))
            .opacity(cv.opacity.unwrap_or(1.))
            .when_some(cv.font.clone(), |d, f| d.font_family(f))
            .child(i.t("ap.chat.preview"));
        col.child(preview).child(
            div().child(
                button("cs-reset")
                    .label(i.t("chatStyle.reset"))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .disabled(chat.is_empty())
                    .on_click(cx.listener(|s, _, _, cx| {
                        let keys: Vec<String> = s.store.read(cx).chat.keys().cloned().collect();
                        s.store.update(cx, |st, cx| {
                            for k in keys {
                                st.set_chat(&k, None, cx);
                            }
                        });
                    })),
            ),
        )
    }
}

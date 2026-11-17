//! Tab "Tipografi" di Tampilan — port TypeTab (AppearancePage.tsx): font antarmuka/kode/terminal dan nilai per
//! variabel tipografi (ukuran/ketebalan/tinggi baris/jarak huruf). Kosong = bawaan; diterapkan saat Enter.
//! Beda dari TS: font diisi sebagai teks (FontPicker dengan daftar preset belum diport); nilai juga belum
//! diterapkan saat kolom ditinggalkan (blur) karena TextInput belum memancarkan event blur.
use crate::core::appearance::{AppearanceStore, FONT_SLOTS, TYPE_PROPS};
use crate::i18n::I18n;
use crate::theme::{Theme, H4, TYPE_VARS};
use crate::ui::{ControlSize, InputChanged, InputSubmit, TextInput, TypeExt};
use gpui::{div, prelude::*, Context, Entity, FontWeight, IntoElement, Window};

struct Field {
    /// ("font", slot, "") atau ("type", var, prop)
    kind: &'static str,
    a: &'static str,
    b: &'static str,
    input: Entity<TextInput>,
    error: Option<String>,
}

pub struct TypeSection {
    store: Entity<AppearanceStore>,
    fields: Vec<Field>,
}

impl TypeSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let store = AppearanceStore::global(cx);
        let ph = cx.global::<I18n>().t("ap.editor.default");
        let mut specs: Vec<(&'static str, &'static str, &'static str, String)> = vec![];
        {
            let st = store.read(cx);
            for slot in FONT_SLOTS {
                specs.push(("font", slot, "", st.fonts.get(slot).cloned().unwrap_or_default()));
            }
            for v in TYPE_VARS {
                for p in TYPE_PROPS {
                    let cur = st.typography.get(v).and_then(|m| m.get(p)).cloned().unwrap_or_default();
                    specs.push(("type", v, p, cur));
                }
            }
        }
        let mut fields = vec![];
        for (n, (kind, a, b, cur)) in specs.into_iter().enumerate() {
            let input = cx.new(|cx| {
                let mut ti = TextInput::new(cx, ph.clone());
                ti.size = ControlSize::Sm;
                ti.set_text(cur, cx);
                ti
            });
            cx.subscribe(&input, move |this, _, e: &InputSubmit, cx| this.commit(n, e.0.to_string(), cx)).detach();
            cx.subscribe(&input, move |this, _, e: &crate::ui::InputBlur, cx| this.commit(n, e.0.to_string(), cx)).detach();
            cx.subscribe(&input, move |this, _, _: &InputChanged, cx| {
                if this.fields[n].error.take().is_some() {
                    this.fields[n].input.update(cx, |ti, _| ti.invalid = false);
                    cx.notify();
                }
            })
            .detach();
            fields.push(Field { kind, a, b, input, error: None });
        }
        Self { store, fields }
    }

    fn commit(&mut self, n: usize, raw: String, cx: &mut Context<Self>) {
        let v = raw.trim().to_string();
        let val = (!v.is_empty()).then_some(v.as_str());
        let (kind, a, b) = (self.fields[n].kind, self.fields[n].a, self.fields[n].b);
        let err = self.store.update(cx, |s, cx| if kind == "font" { s.set_font(a, val, cx) } else { s.set_typography(a, b, val, cx) });
        self.fields[n].error = err.map(String::from);
        let bad = err.is_some();
        self.fields[n].input.update(cx, |ti, _| ti.invalid = bad);
        cx.notify();
    }
}

impl Render for TypeSection {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let field = |f: &Field, label: String| {
            div()
                .flex()
                .flex_col()
                .gap(t.space(1))
                .flex_1()
                .min_w(t.rem(9.))
                .child(div().small(&t).child(label))
                .child(f.input.clone())
                .children(f.error.clone().map(|e| div().small(&t).text_color(t.colors.danger).child(e)))
        };
        let mut col = div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.type.intro")))
            .child(div().typo(&t, H4).child(i.t("ap.type.fonts")))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.type.fontHint")));
        for f in self.fields.iter().filter(|f| f.kind == "font") {
            col = col.child(field(f, i.t(&format!("ap.font.{}", f.a))));
        }
        col = col
            .child(div().typo(&t, H4).child(i.t("ap.type.vars")))
            .child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("ap.type.varsHint")));
        for v in TYPE_VARS {
            let mut row = div().flex().flex_wrap().gap(t.space(2));
            for f in self.fields.iter().filter(|f| f.kind == "type" && f.a == v) {
                row = row.child(field(f, format!("{v} · {}", i.t(&format!("ap.prop.{}", f.b)))));
            }
            col = col.child(div().flex().flex_col().gap(t.space(1)).child(div().font_weight(FontWeight::MEDIUM).child(v)).child(row));
        }
        col
    }
}

//! Tab "Tipografi" di Tampilan — port TypeTab (AppearancePage.tsx): font antarmuka/kode/terminal dan nilai per
//! variabel tipografi (ukuran/ketebalan/tinggi baris/jarak huruf). Kosong = bawaan; diterapkan saat Enter.
//! Font dipilih lewat pemilih (port FontPicker: bawaan Zandor + font terinstal dari sistem, disaring teks kolom) atau
//! diketik langsung. Nilai disimpan saat Enter atau saat kolom ditinggalkan (blur).
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
    /// Indeks kolom font yang pemilihnya terbuka.
    picker: Option<usize>,
    system_fonts: Option<Vec<String>>,
}

/// Font yang dibundel GPUI (assets/fonts, OFL).
const BUNDLED_FONTS: [&str; 2] = ["Plus Jakarta Sans", "JetBrains Mono"];

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
        Self { store, fields, picker: None, system_fonts: None }
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
        let font_idx: Vec<usize> = self.fields.iter().enumerate().filter(|(_, f)| f.kind == "font").map(|(n, _)| n).collect();
        for n in font_idx {
            let f = &self.fields[n];
            let open = self.picker == Some(n);
            let trigger = crate::ui::icon_button(gpui::SharedString::from(format!("fp-{}", f.a)), "chevron_down", i.t("fontPicker.search"))
                .size(ControlSize::Sm)
                .on_click(cx.listener(move |s, _, _, cx| {
                    s.picker = if s.picker == Some(n) { None } else { Some(n) };
                    if s.system_fonts.is_none() {
                        let mut v: Vec<String> = cx.text_system().all_font_names();
                        v.sort_by_key(|a| a.to_lowercase());
                        v.dedup();
                        s.system_fonts = Some(v);
                    }
                    cx.notify();
                }));
            let mut wrap = div().flex().items_end().gap(t.space(1)).child(field(f, i.t(&format!("ap.font.{}", f.a)))).child(trigger);
            if open {
                let q = f.input.read(cx).text().trim().to_lowercase();
                let hit = |name: &str| q.is_empty() || name.to_lowercase().contains(&q);
                let item = |name: String, cx: &mut Context<Self>| {
                    crate::ui::menu_item(gpui::SharedString::from(format!("fpi-{n}-{name}")), name.clone()).on_click(cx.listener(move |s, _, _, cx| {
                        s.fields[n].input.update(cx, |ti, cx| ti.set_text(name.clone(), cx));
                        s.picker = None;
                        s.commit(n, name.clone(), cx);
                    }))
                };
                let mut list = div().id(gpui::SharedString::from(format!("fpl-{n}"))).flex().flex_col().max_h(t.rem(18.)).overflow_y_scroll();
                list = list.child(div().px(t.space(2)).pt(t.space(1)).small(&t).text_color(t.colors.fg_muted).child(i.t("fontPicker.bundled")));
                for b in BUNDLED_FONTS.iter().filter(|b| hit(b)) {
                    list = list.child(item((*b).to_string(), cx));
                }
                list = list.child(div().px(t.space(2)).pt(t.space(2)).small(&t).text_color(t.colors.fg_muted).child(i.t("fontPicker.installed")));
                let sys: Vec<String> = self.system_fonts.clone().unwrap_or_default().into_iter().filter(|x| hit(x)).take(300).collect();
                if sys.is_empty() {
                    list = list.child(div().px(t.space(2)).small(&t).text_color(t.colors.fg_subtle).child(i.t("ui.noResults")));
                }
                for name in sys {
                    list = list.child(item(name, cx));
                }
                wrap = wrap.child(crate::ui::overlay_deferred(
                    gpui::anchored().snap_to_window().child(
                        crate::ui::popup(&t)
                            .mt(t.space(1))
                            .w(t.rem(18.))
                            .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                                s.picker = None;
                                cx.notify();
                            }))
                            .child(list),
                    ),
                ));
            }
            col = col.child(div().relative().child(wrap));
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

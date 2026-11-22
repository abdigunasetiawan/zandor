//! Port DocStyleControls.tsx: kontrol gaya Panel 3 per kelompok (kode, dokumen, tabel, gambar). Dipakai di popover
//! Kustomisasi panel Dokumen (kelompok sesuai penampil aktif) dan di Pengaturan → Dokumen (semua kelompok).
use crate::core::appearance::{AppearanceStore, DOC_CODE_SIZE, DOC_IMAGE_BGS, DOC_THEMES};
use crate::core::editor_theme::BUILTIN_EDITOR_THEMES;
use crate::i18n::I18n;
use crate::theme::Theme;
use crate::ui::{button, select, switch, ButtonVariant, ControlSize, Slider, SliderChanged, TypeExt};
use gpui::{div, prelude::*, AnyElement, Context, Entity, FontWeight, IntoElement, SharedString, Window};
use serde_json::{json, Map, Value};

pub const GROUPS: [&str; 4] = ["code", "prose", "table", "image"];

/// Kelompok kontrol untuk satu jenis penampil (port groupsForViewer).
pub fn groups_for_viewer(viewer: &str) -> Vec<&'static str> {
    match viewer {
        "code" => vec!["code"],
        "markdown" | "docx" | "pptx" => vec!["prose"],
        "notebook" => vec!["prose", "code"],
        "table" | "xlsx" | "json" => vec!["table"],
        "image" => vec!["image"],
        _ => vec![],
    }
}

fn group_keys(g: &str) -> &'static [&'static str] {
    match g {
        "code" => &["codeSize", "codeLineHeight", "codeFont", "minimap", "lineNumbers", "ligatures", "whitespace"],
        "prose" => &["theme", "proseFont", "proseSize", "proseLine", "proseWidth"],
        "table" => &["tableSize"],
        _ => &["imageBg"],
    }
}

const CODE_FONTS: [(&str, &str); 4] = [
    ("", "docStyle.font.codeDefault"),
    ("'JetBrains Mono Variable', Consolas, monospace", "docStyle.font.jetbrains"),
    ("Consolas, 'Courier New', monospace", "docStyle.font.consolas"),
    ("'Cascadia Code', Consolas, monospace", "docStyle.font.cascadia"),
];
const PROSE_FONTS: [(&str, &str); 4] = [
    ("", "chatStyle.font.default"),
    ("system-ui, 'Segoe UI', sans-serif", "chatStyle.font.system"),
    ("Georgia, 'Times New Roman', serif", "chatStyle.font.serif"),
    ("'JetBrains Mono Variable', Consolas, monospace", "chatStyle.font.mono"),
];
const BOOLS: [(&str, &str, bool); 4] = [("lineNumbers", "docStyle.lineNumbers", true), ("minimap", "docStyle.minimap", false), ("ligatures", "docStyle.ligatures", false), ("whitespace", "docStyle.whitespace", false)];

/// (kelompok, kunci, label, min, max, step, bawaan, satuan)
const NUMS: [(&str, &str, &str, f32, f32, f32, f32, &str); 6] = [
    ("code", "codeSize", "docStyle.codeSize", DOC_CODE_SIZE.0 as f32, DOC_CODE_SIZE.1 as f32, 1., DOC_CODE_SIZE.2 as f32, ""),
    ("code", "codeLineHeight", "docStyle.codeLineHeight", 1., 2.4, 0.05, 1.5, ""),
    ("prose", "proseSize", "docStyle.proseSize", 0.6, 2., 0.025, 0.875, "rem"),
    ("prose", "proseLine", "docStyle.proseLine", 1., 2.4, 0.05, 1.55, ""),
    ("prose", "proseWidth", "docStyle.proseWidth", 28., 120., 1., 48., "rem"),
    ("table", "tableSize", "docStyle.tableSize", 0.6, 1.6, 0.025, 0.8125, "rem"),
];

fn round(n: f32) -> f32 {
    (n * 10000.).round() / 10000.
}

/// Nilai angka gaya dokumen (angka atau "<n>rem"), atau bawaan.
pub fn doc_num(d: &Map<String, Value>, key: &str, def: f32) -> f32 {
    match d.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(def as f64) as f32,
        Some(Value::String(s)) => s.trim_end_matches(|c: char| c.is_alphabetic()).parse().unwrap_or(def),
        _ => def,
    }
}

pub struct DocStyleControls {
    store: Entity<AppearanceStore>,
    pub groups: Vec<&'static str>,
    sliders: Vec<Entity<Slider>>,
}

impl DocStyleControls {
    pub fn new(groups: Vec<&'static str>, cx: &mut Context<Self>) -> Self {
        let store = AppearanceStore::global(cx);
        let i = cx.global::<I18n>().clone();
        let d = store.read(cx).docs.clone();
        let mut sliders = vec![];
        for (n, (_, key, label, min, max, step, def, _)) in NUMS.iter().enumerate() {
            let s = cx.new(|_| Slider::new(doc_num(&d, key, *def), *min, *max, *step, i.t(label)));
            cx.subscribe(&s, move |this, _, e: &SliderChanged, cx| this.set_num(n, e.0, cx)).detach();
            sliders.push(s);
        }
        cx.observe(&store, |this, _, cx| {
            this.sync(cx);
            cx.notify();
        })
        .detach();
        Self { store, groups, sliders }
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let d = self.store.read(cx).docs.clone();
        for (n, (_, key, .., def, _)) in NUMS.iter().enumerate() {
            let v = doc_num(&d, key, *def);
            self.sliders[n].update(cx, |s, cx| {
                if (s.value - v).abs() > 1e-6 {
                    s.value = v;
                    cx.notify();
                }
            });
        }
    }

    fn set_num(&mut self, n: usize, v: f32, cx: &mut Context<Self>) {
        let (_, key, .., def, unit) = NUMS[n];
        let r = round(v);
        let val = if (r - def).abs() < 1e-6 {
            None
        } else if unit.is_empty() {
            Some(if key == "codeSize" { json!(r.round() as i64) } else { json!(r) })
        } else {
            Some(json!(format!("{r}{unit}")))
        };
        self.store.update(cx, |s, cx| s.set_doc(key, val, cx));
    }

    fn font_select(&self, id: &'static str, key: &'static str, list: &[(&str, &str)], label: &str, t: &Theme, i: &I18n, cx: &Context<Self>) -> AnyElement {
        let cur = self.store.read(cx).docs.get(key).and_then(Value::as_str).unwrap_or("").to_string();
        let mut items: Vec<(SharedString, SharedString)> = list.iter().map(|(v, l)| (SharedString::from(if v.is_empty() { "default".to_string() } else { v.to_string() }), SharedString::from(i.t(l)))).collect();
        if !cur.is_empty() && !list.iter().any(|(v, _)| *v == cur) {
            items.push((cur.clone().into(), cur.clone().into()));
        }
        let store = self.store.clone();
        row(t, i.t(label), select(id, items, if cur.is_empty() { "default".to_string() } else { cur }).on_change(move |v, _, cx| {
            store.update(cx, |s, cx| s.set_doc(key, (v.as_ref() != "default").then(|| json!(v.to_string())), cx));
        }).into_any_element())
    }
}

fn row(t: &Theme, label: String, control: AnyElement) -> AnyElement {
    div().flex().items_center().gap(t.space(3)).child(div().flex_1().font_weight(FontWeight::MEDIUM).child(label)).child(control).into_any_element()
}

impl Render for DocStyleControls {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        if self.groups.is_empty() {
            return div().small(&t).text_color(t.colors.fg_muted).child(i.t("docStyle.none")).into_any_element();
        }
        let (d, editor_theme, editor_themes) = {
            let s = self.store.read(cx);
            (s.docs.clone(), s.editor_theme.clone(), s.editor_themes.iter().map(|x| (x.id.clone(), x.name.clone())).collect::<Vec<_>>())
        };
        let mut col = div().flex().flex_col().gap(t.space(4));
        for g in self.groups.clone() {
            let keys = group_keys(g);
            let has = keys.iter().any(|k| d.contains_key(*k));
            let store = self.store.clone();
            let mut sec = div().flex().flex_col().gap(t.space(2)).child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().small(&t).font_weight(FontWeight::SEMIBOLD).text_color(t.colors.fg_muted).child(i.t(&format!("docStyle.group.{g}")).to_uppercase()))
                    .child(button(SharedString::from(format!("ds-reset-{g}"))).size(ControlSize::Sm).variant(ButtonVariant::Ghost).disabled(!has).label(i.t("chatStyle.reset")).on_click(move |_, _, cx| store.update(cx, |s, cx| s.reset_docs(keys, cx)))),
            );
            let sliders = |sec: gpui::Div| {
                let mut sec = sec;
                for (n, (grp, key, label, .., unit)) in NUMS.iter().enumerate() {
                    if *grp != g {
                        continue;
                    }
                    let v = round(self.sliders[n].read(cx).value);
                    let unit = if unit.is_empty() && *key == "codeSize" { "px" } else { unit };
                    sec = sec.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(t.space(1))
                            .child(div().flex().child(div().flex_1().font_weight(FontWeight::MEDIUM).child(i.t(label))).child(div().small(&t).code(&t).child(format!("{v}{unit}"))))
                            .child(self.sliders[n].clone()),
                    );
                }
                sec
            };
            match g {
                "code" => {
                    let mut items: Vec<(SharedString, SharedString)> = vec![("auto".into(), i.t("ap.editor2.auto").into())];
                    if !editor_themes.iter().any(|(id, _)| id == crate::core::editor_theme::DEFAULT_EDITOR_THEME) {
                        items.push((crate::core::editor_theme::DEFAULT_EDITOR_THEME.into(), i.tp("ap.editor2.bundled", &[("name", &crate::core::editor_theme::bundled_theme().name)]).into()));
                    }
                    items.extend(BUILTIN_EDITOR_THEMES.iter().map(|b| (SharedString::from(*b), SharedString::from(i.tp("ap.editor2.builtin", &[("name", b)])))));
                    items.extend(editor_themes.iter().map(|(id, name)| (SharedString::from(id.clone()), SharedString::from(name.clone()))));
                    let store = self.store.clone();
                    sec = sec.child(row(&t, i.t("docStyle.editorTheme"), select("ds-etheme", items, editor_theme.clone()).on_change(move |v, _, cx| store.update(cx, |s, cx| s.set_editor_theme(&v, cx))).into_any_element()));
                    sec = sec.child(self.font_select("ds-cfont", "codeFont", &CODE_FONTS, "docStyle.codeFont", &t, &i, cx));
                    sec = sliders(sec);
                    sec = sec.child(div().small(&t).text_color(t.colors.fg_muted).child(i.t("docStyle.zoomHint")));
                    for (key, label, def) in BOOLS {
                        let on = d.get(key).and_then(Value::as_bool).unwrap_or(def);
                        let store = self.store.clone();
                        sec = sec.child(row(&t, i.t(label), switch(SharedString::from(format!("ds-{key}")), "", on).on_change(move |c, _, cx| { store.update(cx, |s, cx| s.set_doc(key, (c != def).then(|| json!(c)), cx)); }).into_any_element()));
                    }
                }
                "prose" => {
                    let store = self.store.clone();
                    let cur = d.get("theme").and_then(Value::as_str).unwrap_or("auto").to_string();
                    sec = sec.child(row(&t, i.t("docStyle.theme"), select("ds-theme", DOC_THEMES.iter().map(|x| (SharedString::from(*x), SharedString::from(i.t(&format!("docStyle.theme.{x}"))))).collect(), cur).on_change(move |v, _, cx| { store.update(cx, |s, cx| s.set_doc("theme", (v.as_ref() != "auto").then(|| json!(v.to_string())), cx)); }).into_any_element()));
                    sec = sec.child(self.font_select("ds-pfont", "proseFont", &PROSE_FONTS, "docStyle.proseFont", &t, &i, cx));
                    sec = sliders(sec);
                }
                "table" => sec = sliders(sec),
                _ => {
                    let store = self.store.clone();
                    let cur = d.get("imageBg").and_then(Value::as_str).unwrap_or("auto").to_string();
                    sec = sec.child(row(&t, i.t("docStyle.imageBg"), select("ds-imgbg", DOC_IMAGE_BGS.iter().map(|x| (SharedString::from(*x), SharedString::from(i.t(&format!("docStyle.imageBg.{x}"))))).collect(), cur).on_change(move |v, _, cx| { store.update(cx, |s, cx| s.set_doc("imageBg", (v.as_ref() != "auto").then(|| json!(v.to_string())), cx)); }).into_any_element()));
                }
            }
            col = col.child(sec);
        }
        col.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn kelompok_per_penampil() {
        assert_eq!(super::groups_for_viewer("notebook"), vec!["prose", "code"]);
        assert_eq!(super::groups_for_viewer("json"), vec!["table"]);
        assert!(super::groups_for_viewer("pdf").is_empty());
    }
}

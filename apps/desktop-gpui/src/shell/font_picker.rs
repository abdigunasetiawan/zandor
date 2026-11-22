//! Pemilih jenis huruf — port FontPicker.tsx: pemicu bergaya Select → popover dengan kolom cari (langsung terfokus)
//! dan daftar font: pilihan bawaan (preset), font yang dibundel aplikasi, dan font terinstal di komputer (dari sistem
//! teks GPUI; dimuat sekali lalu di-cache). Tiap nama ditampilkan dengan font-nya sendiri sebagai pratinjau.
//! Catatan jujur: GPUI membundel Inter, Fira Code, Plus Jakarta Sans, dan JetBrains Mono (versi web membundel 9 font).
use crate::core::appearance::{first_family, validate_font_family};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{icon, IconSize, InputChanged, TextInput, TypeExt};
use gpui::{anchored, div, prelude::*, px, Context, Entity, EventEmitter, Focusable, IntoElement, SharedString, Window};
use std::sync::OnceLock;

/// Font yang dibundel aplikasi GPUI (assets/fonts, OFL).
pub const BUNDLED_FONTS: [(&str, &str); 4] = [("Inter", "'Inter', sans-serif"), ("Fira Code", "'Fira Code', monospace"), ("Plus Jakarta Sans", "'Plus Jakarta Sans', sans-serif"), ("JetBrains Mono", "'JetBrains Mono', monospace")];

static SYSTEM_FONTS: OnceLock<Vec<String>> = OnceLock::new();

/// Nilai CSS untuk satu keluarga font terinstal (dikutip, dengan cadangan).
pub fn family_value(name: &str) -> String {
    format!("\"{name}\", sans-serif")
}

/// Pilihan baru (None = bawaan).
pub struct FontPicked(pub Option<String>);

pub struct FontPicker {
    id: SharedString,
    pub value: Option<String>,
    /// (nilai CSS; "" = bawaan, kunci i18n label)
    presets: Vec<(&'static str, &'static str)>,
    label: String,
    open: bool,
    query: Entity<TextInput>,
}

impl EventEmitter<FontPicked> for FontPicker {}

impl FontPicker {
    pub fn new(id: impl Into<SharedString>, value: Option<String>, presets: Vec<(&'static str, &'static str)>, label: String, cx: &mut Context<Self>) -> Self {
        let ph = cx.global::<I18n>().t("fontPicker.search");
        let query = cx.new(|cx| TextInput::new(cx, ph));
        cx.subscribe(&query, |_, _, _: &InputChanged, cx| cx.notify()).detach();
        Self { id: id.into(), value, presets, label, open: false, query }
    }

    pub fn set_value(&mut self, v: Option<String>, cx: &mut Context<Self>) {
        if self.value != v {
            self.value = v;
            cx.notify();
        }
    }

    fn pick(&mut self, v: Option<String>, cx: &mut Context<Self>) {
        self.value = v.clone();
        self.open = false;
        self.query.update(cx, |q, cx| q.set_text("", cx));
        cx.emit(FontPicked(v));
        cx.notify();
    }

    fn current_label(&self, i: &I18n) -> String {
        let v = self.value.clone().unwrap_or_default();
        if let Some((_, l)) = self.presets.iter().find(|(pv, _)| *pv == v) {
            return i.t(l);
        }
        if let Some((n, _)) = BUNDLED_FONTS.iter().find(|(_, bv)| *bv == v) {
            return n.to_string();
        }
        first_family(&v)
    }
}

impl Render for FontPicker {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let cur = self.value.clone();
        let fam = |v: &str| -> Option<SharedString> { (!v.is_empty()).then(|| first_family(v).into()) };
        let trigger = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap(t.space(2))
            .h(t.rem(crate::ui::ControlSize::Sm.height_rem()))
            .min_w(t.rem(rem::SPACE[7] * 2.5))
            .max_w(t.rem(16.))
            .px(t.space(3))
            .border_1()
            .border_color(if self.open { t.colors.focus } else { t.colors.border })
            .rounded(t.rem(rem::RADIUS_MD))
            .bg(t.colors.surface)
            .text_size(t.rem(t.ty(crate::theme::BUTTON).size))
            .cursor_pointer()
            .hover(|s| s.bg(t.colors.hover))
            .tooltip(crate::ui::tooltip::text_tooltip(self.label.clone().into()))
            .on_click(cx.listener(|s, _, w, cx| {
                s.open = !s.open;
                if s.open {
                    let fh = s.query.read(cx).focus_handle(cx);
                    w.focus(&fh, cx);
                    if SYSTEM_FONTS.get().is_none() {
                        let mut v: Vec<String> = cx.text_system().all_font_names();
                        v.sort_by_key(|a| a.to_lowercase());
                        v.dedup();
                        let _ = SYSTEM_FONTS.set(v);
                    }
                } else {
                    s.query.update(cx, |q, cx| q.set_text("", cx));
                }
                cx.notify();
            }))
            .child(div().flex_1().min_w_0().truncate().when_some(cur.as_deref().and_then(fam), |d, f| d.font_family(f)).child(self.current_label(&i)))
            .child(icon("chevron_down").size(IconSize::Sm).color(t.colors.fg_muted));

        let mut el = div().relative().child(trigger);
        if self.open {
            let q = self.query.read(cx).text().trim().to_lowercase();
            let hit = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
            let id = self.id.clone();
            let row = |value: Option<String>, label: String, cx: &mut Context<Self>| {
                let on = value == cur;
                let v2 = value.clone();
                div()
                    .id(SharedString::from(format!("{id}-{}", value.clone().unwrap_or_default())))
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .h(t.rem(rem::CONTROL_SM))
                    .px(t.space(2))
                    .rounded(t.rem(rem::RADIUS_SM))
                    .cursor_pointer()
                    .hover(|s| s.bg(t.colors.hover))
                    .when(on, |d| d.bg(t.colors.active))
                    .tooltip(crate::ui::tooltip::text_tooltip(label.clone().into()))
                    .on_click(cx.listener(move |s, _, _, cx| s.pick(v2.clone(), cx)))
                    .child(div().flex_1().min_w_0().truncate().when_some(value.as_deref().and_then(fam), |d, f| d.font_family(f)).child(label))
                    .when(on, |d| d.child(icon("check").size(IconSize::Sm)))
            };
            let heading = |s: String| div().px(t.space(2)).pt(t.space(2)).pb(t.space(1)).small(&t).font_weight(gpui::FontWeight::MEDIUM).text_color(t.colors.fg_muted).child(s);
            let note = |s: String| div().px(t.space(2)).py(t.space(1)).small(&t).text_color(t.colors.fg_subtle).child(s);

            let mut list = div().id(SharedString::from(format!("{}-list", self.id))).flex().flex_col().max_h(t.rem(20.)).overflow_y_scroll();
            for (v, l) in self.presets.clone() {
                let label = i.t(l);
                if hit(&label) {
                    list = list.child(row((!v.is_empty()).then(|| v.to_string()), label, cx));
                }
            }
            let bundled: Vec<_> = BUNDLED_FONTS.iter().filter(|(n, _)| hit(n)).collect();
            if !bundled.is_empty() {
                list = list.child(heading(i.t("fontPicker.bundled")));
                for (n, v) in bundled {
                    list = list.child(row(Some(v.to_string()), n.to_string(), cx));
                }
            }
            list = list.child(heading(i.t("fontPicker.installed")));
            match SYSTEM_FONTS.get() {
                None => list = list.child(note(i.t("fontPicker.loading"))),
                Some(all) => {
                    let sys: Vec<&String> = all.iter().filter(|f| validate_font_family(&family_value(f)).is_none() && hit(f)).collect();
                    if sys.is_empty() {
                        list = list.child(note(i.t("ui.noResults")));
                    }
                    for f in sys {
                        list = list.child(row(Some(family_value(f)), f.clone(), cx));
                    }
                }
            }
            el = el.child(crate::ui::overlay_deferred(
                anchored().anchor(gpui::Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(
                    crate::ui::popup(&t)
                        .occlude()
                        .mt(t.space(1))
                        .w(t.rem(18.))
                        .flex()
                        .flex_col()
                        .gap(t.space(1))
                        .on_mouse_down_out(cx.listener(|s, _, _, cx| {
                            s.open = false;
                            s.query.update(cx, |q, cx| q.set_text("", cx));
                            cx.notify();
                        }))
                        .child(div().p(t.space(1)).child(self.query.clone()))
                        .child(list),
                ),
            ));
        }
        el
    }
}

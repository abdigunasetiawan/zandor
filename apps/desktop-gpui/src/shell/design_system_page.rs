//! Halaman Design System (`/design-system`) — port design-system/DesignSystemPage.tsx + TokensSection.tsx + demos.tsx.
//! Semua nilai dibaca dari tema aktif (bukan nilai sendiri): warna, tipografi per variabel, ruang, radius, bayangan,
//! font; contoh atom/molekul memakai komponen asli; daftar shortcut dari registry perintah.
use super::state::AppState;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, BUTTON, CODE, H1, H2, H3, H4, LABEL, LINK, P, SMALL, UI_SCALES};
use crate::ui::{
    avatar, badge, button, checkbox, entry, icon_button, kbd, menu_popup, select, skeleton, spinner, switch, tabs, working, BadgeTone, ButtonVariant, ControlSize, IconSize, TabItem, TextInput, ToastInput, ToastTone, Toasts, TypeExt, WorkingVariant,
};
use gpui::{div, prelude::*, px, AnyElement, Context, Entity, IntoElement, SharedString, Window};

const SECTIONS: [(&str, &str); 9] = [("tokens", "Tokens"), ("atoms", "Atoms"), ("molecules", "Molecules"), ("organisms", "Organisms"), ("panels", "Panels"), ("menus", "Menus"), ("modals", "Modals"), ("pages", "Pages"), ("shortcuts", "Shortcut")];

pub struct DesignSystemPage {
    app: Entity<AppState>,
    scroll: gpui::ScrollHandle,
    input: Entity<TextInput>,
    folder: Entity<TextInput>,
    scale_slider: Entity<crate::ui::Slider>,
    sw: bool,
    cb: bool,
    sel: SharedString,
    tab: SharedString,
    menu: Option<gpui::Point<gpui::Pixels>>,
    dialog: bool,
    confirm: bool,
}

impl DesignSystemPage {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(cx, "Nama project"));
        let folder = cx.new(|cx| TextInput::new(cx, "Folder"));
        let cur = crate::core::settings::SettingsStore::global(cx).read(cx).num("uiScale") as f32;
        let scale_slider = cx.new(|_| crate::ui::Slider::new(cur * 100., UI_SCALES[0] * 100., UI_SCALES[UI_SCALES.len() - 1] * 100., 12.5, "Skala UI"));
        cx.subscribe(&scale_slider, |_, _, e: &crate::ui::SliderChanged, cx| {
            let v = UI_SCALES.iter().copied().find(|s| (s * 100. - e.0).abs() < 0.01).unwrap_or(1.);
            let st = crate::core::settings::SettingsStore::global(cx);
            let _ = st.update(cx, |s, cx| s.set("uiScale", serde_json::json!(v), cx));
        })
        .detach();
        Self { app, scroll: gpui::ScrollHandle::new(), input, folder, scale_slider, sw: true, cb: false, sel: "claude".into(), tab: "a".into(), menu: None, dialog: false, confirm: false }
    }
}

fn section(t: &Theme, id: &str, title: &str, body: impl IntoElement) -> AnyElement {
    div().id(SharedString::from(format!("ds-{id}"))).flex().flex_col().gap(t.space(3)).pb(t.space(6)).child(div().typo(t, H3).child(title.to_string())).child(body).into_any_element()
}

fn demo(t: &Theme, name: &str, body: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t.space(2))
        .p(t.space(3))
        .rounded(t.rem(rem::RADIUS_LG))
        .border_1()
        .border_color(t.colors.border_soft)
        .child(div().typo(t, H4).child(name.to_string()))
        .child(body)
        .into_any_element()
}

fn row(t: &Theme) -> gpui::Div {
    div().flex().flex_wrap().items_center().gap(t.space(2))
}

fn not_yet(t: &Theme, phase: &str, what: &str) -> AnyElement {
    div()
        .p(t.space(3))
        .rounded(t.rem(rem::RADIUS_MD))
        .border_1()
        .border_dashed()
        .border_color(t.colors.border)
        .child(div().typo(t, LABEL).child("Belum ada"))
        .child(div().small(t).text_color(t.colors.fg_muted).child(format!("{what} Diisi mulai {phase}. Tidak ada contoh palsu di sini.")))
        .into_any_element()
}

fn hex(c: gpui::Hsla) -> String {
    let r = gpui::Rgba::from(c);
    let b = |x: f32| (x.clamp(0., 1.) * 255.).round() as u8;
    if r.a < 0.999 {
        format!("rgba({}, {}, {}, {:.2})", b(r.r), b(r.g), b(r.b), r.a)
    } else {
        format!("#{:02x}{:02x}{:02x}", b(r.r), b(r.g), b(r.b))
    }
}

impl DesignSystemPage {
    fn tokens(&self, t: &Theme) -> AnyElement {
        let c = &t.colors;
        let colors: [(&str, gpui::Hsla); 16] = [
            ("bg", c.bg), ("surface", c.surface), ("raised", c.raised), ("hover", c.hover), ("active", c.active), ("border", c.border), ("border-strong", c.border_strong), ("fg", c.fg), ("fg-muted", c.fg_muted), ("fg-subtle", c.fg_subtle), ("accent", c.accent), ("on-accent", c.on_accent), ("focus", c.focus), ("danger", c.danger), ("success", c.success), ("warning", c.warning),
        ];
        let swatches = row(t).children(colors.iter().map(|(n, col)| {
            div()
                .w(t.rem(9.))
                .flex()
                .flex_col()
                .gap(t.space(1))
                .child(div().h(t.rem(2.5)).rounded(t.rem(rem::RADIUS_MD)).border_1().border_color(t.colors.border).bg(*col))
                .child(div().code(t).child(format!("--z-color-{n}")))
                .child(div().small(t).text_color(t.colors.fg_muted).child(hex(*col)))
        }));
        let types = [("h1", H1), ("h2", H2), ("h3", H3), ("h4", H4), ("p", P), ("small", SMALL), ("link", LINK), ("button", BUTTON), ("label", LABEL), ("code", CODE)];
        let type_rows = div().flex().flex_col().gap(t.space(2)).children(types.iter().map(|(n, s)| {
            let s2 = t.ty(*s);
            div()
                .flex()
                .items_baseline()
                .gap(t.space(3))
                .child(div().w(t.rem(12.)).flex_none().code(t).text_color(t.colors.fg_muted).child(format!("{n} · {}rem / {}", s2.size, s2.weight)))
                .child(div().typo(t, *s).when(*n == "link", |d| d.text_color(t.colors.accent).underline()).when(*n == "code", |d| d.font_family(t.font_code.clone())).child("Zandor memakai banyak AI sekaligus"))
        }));
        let spaces = div().flex().flex_col().gap(t.space(1)).children((1..=7).map(|n| {
            div()
                .flex()
                .items_center()
                .gap(t.space(3))
                .child(div().w(t.rem(9.)).code(t).child(format!("--z-space-{n}")))
                .child(div().h(t.rem(0.75)).w(t.space(n)).bg(t.colors.fg_muted).rounded(t.rem(rem::RADIUS_SM)))
                .child(div().small(t).text_color(t.colors.fg_muted).child(format!("{}rem", rem::SPACE[n])))
        }));
        let boxed = |label: String| div().size(t.rem(6.)).flex().items_center().justify_center().border_1().border_color(t.colors.border).bg(t.colors.surface).child(div().code(t).child(label));
        let radii = row(t)
            .gap(t.space(4))
            .children([("sm", rem::RADIUS_SM), ("md", rem::RADIUS_MD), ("lg", rem::RADIUS_LG), ("full", rem::RADIUS_FULL)].map(|(n, r)| boxed(format!("radius-{n}")).rounded(t.rem(r))))
            .child(boxed("shadow-1".into()).rounded(t.rem(rem::RADIUS_MD)).shadow(t.shadow_1()))
            .child(boxed("shadow-2".into()).rounded(t.rem(rem::RADIUS_MD)).shadow(t.shadow_2()))
            .child(boxed("shadow-3".into()).rounded(t.rem(rem::RADIUS_MD)).shadow(t.shadow_3()));
        let h = |s: &str| div().typo(t, H4).child(s.to_string());
        let note = |s: &str| div().small(t).text_color(t.colors.fg_muted).child(s.to_string());
        div()
            .flex()
            .flex_col()
            .gap(t.space(5))
            .child(div().flex().flex_col().gap(t.space(2)).child(h("Warna")).child(note("Monokrom tanpa warna primary. Merah/hijau/kuning hanya untuk status.")).child(swatches))
            .child(div().flex().flex_col().gap(t.space(2)).child(h("Tipografi per variabel")).child(note("Setiap variabel punya ukuran, ketebalan, tinggi baris, dan jarak huruf sendiri.")).child(type_rows))
            .child(div().flex().flex_col().gap(t.space(2)).child(h("Ruang")).child(spaces))
            .child(div().flex().flex_col().gap(t.space(2)).child(h("Radius dan bayangan")).child(radii))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(1))
                    .child(h("Font"))
                    .child(div().font_family(t.font_ui.clone()).child("UI — Plus Jakarta Sans Variable (OFL)"))
                    .child(div().font_family(t.font_code.clone()).code(t).child("Kode — JetBrains Mono Variable (OFL): const x = 42;"))
                    .child(note("Font terminal memakai font kode secara bawaan dan bisa diatur terpisah (--z-font-terminal).")),
            )
            .into_any_element()
    }

    fn atoms(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let me = cx.entity().downgrade();
        let me2 = me.clone();
        let me3 = me.clone();
        div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(demo(
                t,
                "Button",
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(2))
                    .child(
                        row(t)
                            .child(button("ds-b1").variant(ButtonVariant::Solid).label("Solid"))
                            .child(button("ds-b2").variant(ButtonVariant::Outline).label("Outline"))
                            .child(button("ds-b3").variant(ButtonVariant::Ghost).label("Ghost"))
                            .child(button("ds-b4").variant(ButtonVariant::Danger).label("Danger"))
                            .child(button("ds-b5").disabled(true).label("Nonaktif"))
                            .child(button("ds-b6").loading(true).label("Memuat")),
                    )
                    .child(row(t).child(button("ds-s1").size(ControlSize::Sm).label("Kecil")).child(button("ds-s2").size(ControlSize::Md).label("Sedang")).child(button("ds-s3").size(ControlSize::Lg).label("Besar"))),
            ))
            .child(demo(
                t,
                "IconButton",
                row(t)
                    .child(icon_button("ds-i1", "plus", "Tambah project"))
                    .child(icon_button("ds-i2", "plus", "Tambah (outline)").variant(ButtonVariant::Outline))
                    .child(icon_button("ds-i3", "plus", "Tambah (kecil)").size(ControlSize::Sm)),
            ))
            .child(demo(t, "Input", div().w(t.rem(20.)).child(self.input.clone())))
            .child(demo(
                t,
                "Select",
                div().w(t.rem(20.)).child(
                    select("ds-sel", vec![("claude".into(), "Claude".into()), ("gpt".into(), "GPT".into()), ("gemini".into(), "Gemini".into())], self.sel.clone()).on_change(move |v, _, cx| {
                        me.update(cx, |s, cx| {
                            s.sel = v;
                            cx.notify();
                        })
                        .ok();
                    }),
                ),
            ))
            .child(demo(
                t,
                "Switch · Checkbox",
                row(t)
                    .gap(t.space(4))
                    .child(switch("ds-sw", "Notifikasi", self.sw).on_change(move |v, _, cx| {
                        me2.update(cx, |s, cx| {
                            s.sw = v;
                            cx.notify();
                        })
                        .ok();
                    }))
                    .child(checkbox("ds-cb", "Ingat pilihan", self.cb).on_change(move |v, _, cx| {
                        me3.update(cx, |s, cx| {
                            s.cb = v;
                            cx.notify();
                        })
                        .ok();
                    })),
            ))
            .child(demo(
                t,
                "Badge",
                row(t)
                    .child(badge("Netral"))
                    .child(badge("Outline").tone(BadgeTone::Outline))
                    .child(badge("Aktif").tone(BadgeTone::Success))
                    .child(badge("Peringatan").tone(BadgeTone::Warning))
                    .child(badge("Galat").tone(BadgeTone::Danger)),
            ))
            .child(demo(t, "Avatar", row(t).child(avatar("Abdi Guna")).child(avatar("Zandor").size(ControlSize::Sm)).child(avatar("Opus").size(ControlSize::Lg))))
            .child(demo(t, "Kbd", row(t).child(kbd("Ctrl+K")).child(kbd("Ctrl+Shift+F")).child(kbd("Alt+ArrowLeft"))))
            .child(demo(t, "Spinner", row(t).child(spinner(IconSize::Sm)).child(spinner(IconSize::Md))))
            .child(demo(
                t,
                "Working",
                row(t).gap(t.space(4)).children([WorkingVariant::Wave, WorkingVariant::Ripple, WorkingVariant::Orbit, WorkingVariant::Grid].into_iter().enumerate().map(|(n, v)| working(format!("ds-w{n}")).variant(v))),
            ))
            .child(demo(t, "Skeleton", div().flex().flex_col().gap(t.space(2)).w(t.rem(20.)).child(skeleton("ds-sk1")).child(skeleton("ds-sk2"))))
            .into_any_element()
    }

    fn molecules(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let me = cx.entity().downgrade();
        let items = vec![
            TabItem { value: "a".into(), label: "Ringkasan".into(), icon: None, disabled: false },
            TabItem { value: "b".into(), label: "Detail".into(), icon: None, disabled: false },
            TabItem { value: "c".into(), label: "Nonaktif".into(), icon: None, disabled: true },
        ];
        div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(demo(
                t,
                "Tabs",
                tabs("ds-tabs", items, self.tab.clone()).on_change(move |v, _, cx| {
                    me.update(cx, |s, cx| {
                        s.tab = v;
                        cx.notify();
                    })
                    .ok();
                }),
            ))
            .child(demo(
                t,
                "Toast",
                row(t)
                    .child(button("ds-t1").label("Netral").on_click(|_, _, cx| { Toasts::show(ToastInput::new("Perubahan disimpan"), cx); }))
                    .child(button("ds-t2").label("Sukses").on_click(|_, _, cx| { Toasts::show(ToastInput { tone: ToastTone::Success, ..ToastInput::new("Project ditambahkan") }, cx); }))
                    .child(button("ds-t3").label("Galat").on_click(|_, _, cx| { Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new("Engine tidak merespons") }, cx); })),
            ))
            .child(demo(
                t,
                "Field",
                div()
                    .flex()
                    .flex_col()
                    .gap(t.space(3))
                    .w(t.rem(20.))
                    .child(crate::ui::field(t, "Nama project", self.input.clone(), Some("Huruf kecil, tanpa spasi.".into()), None))
                    .child(crate::ui::field(t, "Folder", self.folder.clone(), None, Some("Folder tidak ditemukan.".into()))),
            ))
            .into_any_element()
    }

    fn menus(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        demo(
            t,
            "Menu",
            button("ds-menu").label("Buka menu").on_click(cx.listener(|s, _, w, cx| {
                s.menu = Some(w.mouse_position());
                cx.notify();
            })),
        )
    }

    fn modals(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(t.space(3))
            .child(demo(t, "Dialog", button("ds-dlg").label("Buka dialog").on_click(cx.listener(|s, _, _, cx| {
                s.dialog = true;
                cx.notify();
            }))))
            .child(demo(t, "ConfirmDialog", button("ds-cfm").variant(ButtonVariant::Danger).label("Hapus…").on_click(cx.listener(|s, _, _, cx| {
                s.confirm = true;
                cx.notify();
            }))))
            .into_any_element()
    }

    fn shortcuts(&self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> AnyElement {
        let mut table = div().flex().flex_col().border_1().border_color(t.colors.border_soft).rounded(t.rem(rem::RADIUS_MD));
        table = table.child(
            div()
                .flex()
                .bg(t.colors.raised)
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .small(t)
                .child(div().w(t.rem(12.)).px(t.space(2)).py(t.space(1)).child("Pintasan"))
                .child(div().w(t.rem(8.)).px(t.space(2)).py(t.space(1)).child("Scope"))
                .child(div().flex_1().px(t.space(2)).py(t.space(1)).child("ID")),
        );
        let mut rows: Vec<(String, String, String)> = vec![];
        for c in super::commands::COMMANDS {
            let Some(combo) = super::commands::shortcut_of(c.id, cx) else { continue };
            rows.push((combo, "global".into(), format!("{} — {}", c.id, i.t(c.title_key))));
        }
        // pintasan per scope dari keymap GPUI (konteks fokus: TextInput, Palette, PdfView, …)
        for b in cx.key_bindings().borrow().bindings() {
            let Some(pred) = b.predicate() else { continue };
            let combo = b.keystrokes().iter().map(|k| combo_text(k.inner())).collect::<Vec<_>>().join(" ");
            rows.push((combo, pred.to_string(), b.action().name().to_string()));
        }
        for (combo, scope, id) in rows {
            table = table.child(
                div()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(t.colors.border_soft)
                    .child(div().w(t.rem(12.)).px(t.space(2)).py(t.space(1)).child(kbd(combo)))
                    .child(div().w(t.rem(8.)).px(t.space(2)).py(t.space(1)).small(t).child(scope))
                    .child(div().flex_1().px(t.space(2)).py(t.space(1)).code(t).child(id)),
            );
        }
        div().flex().flex_col().gap(t.space(2)).child(div().small(t).text_color(t.colors.fg_muted).child("Pintasan terdaftar saat ini (global dan per scope). Scope ditentukan dari panel/popup tempat fokus berada.")).child(table).into_any_element()
    }
}

/// Keystroke GPUI → teks Kbd ("Ctrl+Shift+K").
fn combo_text(k: &gpui::Keystroke) -> String {
    let m = &k.modifiers;
    let mut parts: Vec<String> = vec![];
    if m.control {
        parts.push("Ctrl".into());
    }
    if m.alt {
        parts.push("Alt".into());
    }
    if m.shift {
        parts.push("Shift".into());
    }
    if m.platform {
        parts.push("Win".into());
    }
    let key = match k.key.as_str() {
        "up" => "ArrowUp".to_string(),
        "down" => "ArrowDown".to_string(),
        "left" => "ArrowLeft".to_string(),
        "right" => "ArrowRight".to_string(),
        "escape" => "Esc".to_string(),
        "enter" => "Enter".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "tab" => "Tab".to_string(),
        "space" => "Space".to_string(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => {
            let mut c = k.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    };
    parts.push(key);
    parts.join("+")
}

impl Render for DesignSystemPage {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let settings = crate::core::settings::SettingsStore::global(cx);
        let theme_v = settings.read(cx).str("theme");
        let scale = settings.read(cx).num("uiScale") as f32;
        let set = |k: &'static str| {
            let st = settings.clone();
            move |v: serde_json::Value, cx: &mut gpui::App| {
                let _ = st.update(cx, |s, cx| s.set(k, v, cx));
            }
        };
        let set_theme = set("theme");
        let set_scale2 = set("uiScale");
        let theme_items = ["system", "light", "dark"].iter().map(|th| (SharedString::from(*th), SharedString::from(i.t(&format!("theme.{th}"))))).collect();
        if (self.scale_slider.read(cx).value - scale * 100.).abs() > 0.01 {
            self.scale_slider.update(cx, |s, cx| {
                s.value = scale * 100.;
                cx.notify();
            });
        }
        let app = self.app.clone();
        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(t.space(3))
            .px(t.space(5))
            .py(t.space(3))
            .border_b_1()
            .border_color(t.colors.border)
            .child(div().typo(&t, H2).flex_1().child("Design system Zandor"))
            .child(div().flex().items_center().gap(t.space(2)).child(div().typo(&t, LABEL).child(i.t("theme.label"))).child(div().w(t.rem(10.)).child(select("ds-theme", theme_items, theme_v).on_change(move |v, _, cx| set_theme(serde_json::json!(v.to_string()), cx)))))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(1))
                    .child(div().typo(&t, LABEL).child(format!("{}: {}%", i.t("scale.label"), (scale * 100.).round())))
                    .child(div().w(t.rem(10.)).child(self.scale_slider.clone())),
            )
            .child(button("ds-100").size(ControlSize::Sm).label("100%").on_click(move |_, _, cx| set_scale2(serde_json::json!(1.0), cx)))
            .child(button("ds-back").size(ControlSize::Sm).variant(ButtonVariant::Ghost).label("← Aplikasi").on_click(move |_, _, cx| app.update(cx, |a, cx| a.push("/", cx))));
        let scroll = self.scroll.clone();
        let nav = div().flex_none().w(t.rem(11.)).p(t.space(3)).flex().flex_col().gap(t.space(1)).border_r_1().border_color(t.colors.border_soft).children(SECTIONS.iter().enumerate().map(|(n, (id, label))| {
            let scroll = scroll.clone();
            div()
                .id(SharedString::from(format!("ds-nav-{id}")))
                .px(t.space(2))
                .py(t.space(1))
                .rounded(t.rem(rem::RADIUS_SM))
                .cursor_pointer()
                .hover(|s| s.bg(t.colors.hover))
                .child(label.to_string())
                .on_click(move |_, _, cx| {
                    scroll.scroll_to_item(n);
                    cx.refresh_windows();
                })
        }));
        let content = div()
            .id("ds-content")
            .flex_1()
            .min_w_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(t.space(5))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(section(&t, "tokens", "Tokens", self.tokens(&t)))
                    .child(section(&t, "atoms", "Atoms", self.atoms(&t, cx)))
                    .child(section(&t, "molecules", "Molecules", self.molecules(&t, cx)))
                    .child(section(&t, "organisms", "Organisms", not_yet(&t, "Fase 3", "Top bar, command palette, dan daftar project belum dibuat.")))
                    .child(section(&t, "panels", "Panels", not_yet(&t, "Fase 3", "Kelima panel belum dibuat.")))
                    .child(section(&t, "menus", "Menus", self.menus(&t, cx)))
                    .child(section(&t, "modals", "Modals", self.modals(&t, cx)))
                    .child(section(&t, "pages", "Pages", not_yet(&t, "Fase 3", "Halaman Pengaturan belum dibuat.")))
                    .child(section(&t, "shortcuts", "Shortcut", self.shortcuts(&t, &i, cx))),
            );
        let menu = self.menu.map(|pos| {
            let me = cx.entity().downgrade();
            let entries = vec![
                entry("Ganti nama", |_, cx| { Toasts::show(ToastInput::new("Ganti nama"), cx); }).icon("pencil"),
                entry("Duplikat", |_, cx| { Toasts::show(ToastInput::new("Duplikat"), cx); }).icon("copy"),
                crate::ui::MenuEntry::Separator,
                entry("Hapus", |_, cx| { Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new("Hapus") }, cx); }).icon("trash").danger(),
            ];
            crate::ui::overlay_deferred(gpui::anchored().position(pos).snap_to_window().child(menu_popup("ds-menu-pop", &t, entries, gpui::Anchor::TopLeft, move |_, cx| {
                me.update(cx, |s, cx| {
                    s.menu = None;
                    cx.notify();
                })
                .ok();
            })))
            .with_priority(3)
        });
        let me_c = cx.entity().downgrade();
        let close = move || {
            let me = me_c.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                me.update(cx, |s, cx| {
                    s.dialog = false;
                    s.confirm = false;
                    cx.notify();
                })
                .ok();
            }
        };
        let dialog = self.dialog.then(|| {
            crate::ui::dialog("Judul dialog")
                .description("Dialog menahan fokus dan menutup dengan Esc atau klik di luar.")
                .on_dismiss(close())
                .footer(button("ds-dlg-ok").variant(ButtonVariant::Solid).label("Mengerti").on_click(close()))
        });
        let confirm = self.confirm.then(|| {
            crate::ui::dialog("Hapus project?")
                .description("Project dilepas dari daftar; berkas di disk tidak dihapus.")
                .on_dismiss(close())
                .footer(div().flex().gap(t.space(2)).child(button("ds-cf-no").label("Batal").on_click(close())).child(button("ds-cf-yes").variant(ButtonVariant::Danger).label("Hapus").on_click(close())))
        });
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .child(div().flex().flex_1().min_h_0().child(nav).child(content))
            .children(menu)
            .children(dialog)
            .children(confirm)
            .text_size(px(f32::from(t.rem(t.ty(P).size))))
    }
}

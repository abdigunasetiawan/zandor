//! Top bar (frameless) — port shell/TitleBar.tsx + .titlebar/.omnibox di styles.css.
use super::state::{format_combo, AppState, PANEL_IDS, SMART_RESIZE_SHORTCUT};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, kbd, menu_item, popup, separator, ButtonVariant, ControlSize, IconSize};
use gpui::{
    anchored, div, prelude::*, px, App, Context, Anchor, Entity, IntoElement, MouseButton,
    SharedString, Window, WindowControlArea,
};

/// Isi menu bar: (kunci label menu, [id perintah | "-"]).
const MENUS: &[(&str, &[&str])] = &[
    ("menu.file", &["app.openSettings", "-", "window.close"]),
    (
        "menu.view",
        &[
            "view.toggleSidebar", "view.toggleChat", "view.toggleDocs", "view.toggleTools", "view.toggleRail",
            "view.smartResize", "view.resetLayout", "-", "view.zoomIn", "view.zoomOut", "view.zoomReset", "-",
            "view.toggleTheme", "view.langId", "view.langEn",
        ],
    ),
    ("menu.help", &["app.palette", "help.about"]),
    ("menu.dev", &["dev.designSystem", "dev.labs", "-", "dev.deleteChats"]),
];

pub struct TitleBar {
    app: Entity<AppState>,
    open_menu: Option<usize>,
    pub on_command: Option<std::rc::Rc<dyn Fn(&str, &mut Window, &mut App)>>,
}

impl TitleBar {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&app, |_, _, cx| cx.notify()).detach();
        Self { app, open_menu: None, on_command: None }
    }

    fn window_controls(&self, window: &Window, t: &Theme, i: &I18n) -> gpui::AnyElement {
        let maximized = window.is_maximized();
        let w = t.rem(rem::RAIL_WIDTH);
        let (hover, danger, on_accent, fg) = (t.colors.hover, t.colors.danger, t.colors.on_accent, t.colors.fg);
        let ctl = |id: &'static str, glyph: &'static str, label: String, close: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .h_full()
                .w(w)
                .text_color(fg)
                .tooltip(crate::ui::tooltip::text_tooltip(label.into()))
                .hover(move |s| if close { s.bg(danger).text_color(on_accent) } else { s.bg(hover) })
                .child(icon(glyph))
        };
        // Minimize dan Close sengaja tombol area klien biasa (klik ditangani aplikasi), BUKAN
        // `window_control_area(Min/Close)`: area kontrol non-klien (HTMINBUTTON/HTCLOSE) membuat Windows menampilkan
        // tooltip bawaannya sendiri ("Close") di luar kendali GPUI, yang bisa tersangkut di layar. Maximize tetap
        // `WindowControlArea::Max` karena Snap Layouts Windows 11 hanya muncul dari HTMAXBUTTON.
        div()
            .flex()
            .flex_none()
            .self_stretch()
            .child(ctl("win-min", "minimize", i.t("window.minimize"), false).on_click(|_, window, _| window.minimize_window()))
            .child(
                ctl("win-max", if maximized { "restore" } else { "maximize" }, i.t(if maximized { "window.restore" } else { "window.maximize" }), false)
                    .window_control_area(WindowControlArea::Max),
            )
            .child(ctl("win-close", "close", i.t("window.close"), true).on_click(|_, window, _| window.remove_window()))
            .into_any_element()
    }

    fn menu_bar(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let i = cx.global::<I18n>().clone();
        let i = &i;
        let mut bar = div().flex().flex_none().items_center().gap(t.space(1));
        // Menu Dev Tools hanya untuk build pengembangan; pengguna rilis tidak melihatnya.
        for (idx, (label_key, ids)) in MENUS.iter().filter(|(k, _)| cfg!(debug_assertions) || *k != "menu.dev").enumerate() {
            let open = self.open_menu == Some(idx);
            let mut wrap = div().relative().child(
                button(("menu", idx))
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .label(i.t(label_key))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_menu = if this.open_menu == Some(idx) { None } else { Some(idx) };
                        cx.notify();
                    })),
            );
            if open {
                let mut list = popup(t).min_w(t.rem(rem::SPACE[7] * 4.));
                for (n, id) in ids.iter().enumerate() {
                    if *id == "-" {
                        list = list.child(separator(t));
                        continue;
                    }
                    let id_s: SharedString = (*id).into();
                    let mut item = menu_item(("mi", idx * 100 + n), i.t(super::commands::find(id).map(|c| c.title_key).unwrap_or(id))).on_click(cx.listener(
                        move |this, _, w, cx| {
                            this.open_menu = None;
                            cx.notify();
                            if let Some(f) = this.on_command.clone() {
                                f(&id_s, w, cx);
                            }
                        },
                    ));
                    if let Some(s) = super::commands::shortcut_of(id, cx) {
                        item = item.shortcut(format_combo(&s));
                    }
                    list = list.child(item);
                }
                wrap = wrap.child(crate::ui::overlay_deferred(
                    anchored().anchor(Anchor::TopLeft).snap_to_window_with_margin(px(8.)).child(
                        div().mt(t.space(1)).child(list.on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.open_menu = None;
                            cx.notify();
                        }))),
                    ),
                ).with_priority(65));
            }
            bar = bar.child(wrap);
        }
        bar.into_any_element()
    }
}

impl Render for TitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let route = self.app.read(cx).current().to_string();
        let i18n = cx.global::<I18n>();
        let bar = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(t.space(2))
            .h(t.rem(rem::TITLEBAR_HEIGHT))
            .pl(t.space(2))
            .text_color(t.colors.fg);

        // Onboarding: hanya area seret + kontrol jendela.
        if route.trim_end_matches('/') == "/welcome" {
            let controls = self.window_controls(window, &t, i18n);
            return bar
                .child(div().flex_1().h_full().window_control_area(WindowControlArea::Drag))
                .child(controls)
                .into_any_element();
        }

        let on_settings = route.starts_with("/settings");
        let (can_back, can_forward) = {
            let a = self.app.read(cx);
            (a.can_back(), a.can_forward())
        };
        let collapsed: Vec<bool> = PANEL_IDS.iter().map(|p| self.app.read(cx).is_collapsed(*p)).collect();
        let app = self.app.clone();
        let i18n = cx.global::<I18n>();
        let back_l = i18n.t("nav.back");
        let fwd_l = i18n.t("nav.forward");
        let smart_l = format!("{} ({})", i18n.t("panel.smartResize"), format_combo(SMART_RESIZE_SHORTCUT));
        let toggle_labels: Vec<String> =
            PANEL_IDS.iter().map(|p| format!("{} ({})", i18n.t(p.label_key()), format_combo(p.shortcut()))).collect();
        let settings_l = i18n.t("settings.title");
        let palette_l = i18n.t("palette.placeholder");
        let controls = self.window_controls(window, &t, cx.global::<I18n>());
        let menus = self.menu_bar(&t, cx);

        let nav = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(t.space(1))
            .child(icon_button("nav-back", "arrow_left", back_l).disabled(!can_back).on_click({
                let app = app.clone();
                move |_, _, cx| app.update(cx, |a, cx| a.back(cx))
            }))
            .child(icon_button("nav-fwd", "arrow_right", fwd_l).disabled(!can_forward).on_click({
                let app = app.clone();
                move |_, _, cx| app.update(cx, |a, cx| a.forward(cx))
            }));

        let (raised, hover, border_soft, muted) = (t.colors.raised, t.colors.hover, t.colors.border_soft, t.colors.fg_muted);
        let mut center = div()
            .flex()
            .flex_1()
            .justify_center()
            .min_w_0()
            .px(t.space(2))
            .h_full()
            .items_center()
            .window_control_area(WindowControlArea::Drag);
        if !on_settings {
            let cb = self.on_command.clone();
            // Jendela sempit: kolom tengah lebih kecil dari ikon + chip pintasan, sehingga chip meluap ke toolbar.
            // Di bawah ambang ini chip disembunyikan (label sudah menyusut habis).
            let show_kbd = window.viewport_size().width >= t.rem(rem::SPACE[7] * 18.);
            center = center.child(
                div()
                    .id("omnibox")
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .w_full()
                    .max_w(t.rem(rem::SPACE[7] * 9.))
                    .overflow_hidden()
                    .h(t.rem(rem::CONTROL_MD))
                    .px(t.space(3))
                    .border_1()
                    .border_color(border_soft)
                    .rounded(t.rem(rem::RADIUS_MD))
                    .bg(raised)
                    .text_color(muted)
                    .text_size(t.rem(t.ty(crate::theme::BUTTON).size))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, w, cx| {
                        if let Some(f) = cb.clone() {
                            f("app.palette", w, cx);
                        }
                    })
                    .child(icon("search").size(IconSize::Sm).color(muted))
                    .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(palette_l))
                    .when(show_kbd, |d| d.child(kbd("Ctrl+K"))),
            );
        }

        let mut toggles = div().flex().flex_none().items_center().gap(t.space(1)).child(
            icon_button("smart", "smart_layout", smart_l).size(ControlSize::Sm).on_click({
                let cb = self.on_command.clone();
                move |_, w, cx| {
                    if let Some(f) = cb.clone() {
                        f("view.smartResize", w, cx);
                    }
                }
            }),
        );
        for (n, p) in PANEL_IDS.iter().enumerate() {
            let p = *p;
            let app = app.clone();
            let mut b = icon_button(("toggle", n), p.icon(), toggle_labels[n].clone())
                .size(ControlSize::Sm)
                .on_click(move |_, _, cx| app.update(cx, |a, cx| a.toggle(p, cx)));
            // aria-pressed: panel terbuka tampil aktif
            if !collapsed[n] {
                b = b.variant(ButtonVariant::Outline);
            }
            toggles = toggles.child(b);
        }

        let tools = div().flex().flex_none().items_center().gap(t.space(1)).child(toggles).child(
            icon_button("settings", "settings", settings_l).on_click(move |_, _, cx| {
                app.update(cx, |a, cx| a.push("/settings", cx));
            }),
        );

        bar.child(nav).child(menus).child(center).child(tools).child(controls).into_any_element()
    }
}

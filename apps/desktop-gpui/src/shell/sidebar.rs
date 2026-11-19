//! Sidebar — port shell/Sidebar.tsx: kepala (logo, nama, badge, cari), tab Chat/Berkas, aksi, daftar project & sesi,
//! kelompok Chat, dan kaki (profil + status engine).
use super::state::AppState;
use crate::core::engine::SessionInfo;
use crate::core::projects::{session_title, LoadStatus, ProjectStore, SessionsState, CHAT_KEY};
use crate::core::services::EngineStatus;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H4};
use crate::ui::{
    badge, button, dialog, entry, icon, icon_button, menu_popup, tabs, BadgeTone, ButtonVariant, ControlSize,
    IconSize, MenuEntry, TabItem, TextInput, ToastInput, ToastTone, Toasts, TypeExt,
};
use gpui::{
    div, prelude::*, px, Focusable, Anchor, AnyElement, App, ClipboardItem, Context, Entity, IntoElement, PathPromptOptions,
    SharedString, WeakEntity, Window,
};
use std::collections::HashSet;

#[derive(Clone)]
enum SbDialog {
    Trust(String),
    /// Tambah project dengan pilihan engine (tampil bila ada engine remote): "local" atau id engine remote.
    AddWithEngine(String),
    RemoveProject(String, String),
    DeleteSession(String, String, String),
    RenameProject(String),
    RenameSession(String, String),
}

pub struct Sidebar {
    app: Entity<AppState>,
    store: Entity<ProjectStore>,
    engine: Entity<EngineStatus>,
    search: Entity<TextInput>,
    rename: Entity<TextInput>,
    show_search: bool,
    tab: SharedString,
    collapsed: HashSet<String>,
    workspace_open: bool,
    open_menu: Option<String>,
    dialog: Option<SbDialog>,
    pstyle: Option<Entity<super::project_style_dialog::ProjectStyleDialog>>,
    files: Entity<super::files_tree::FilesTree>,
}


impl Sidebar {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let store = ProjectStore::global(cx);
        let engine = EngineStatus::global(cx);
        let i = cx.global::<I18n>();
        let (ph, ph2) = (i.t("sb.search"), i.t("sb.name"));
        let search = cx.new(|cx| TextInput::new(cx, ph));
        let rename = cx.new(|cx| TextInput::new(cx, ph2));
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        cx.observe(&crate::core::profile::Profile::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe(&crate::core::appearance::AppearanceStore::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe(&engine, |_, _, cx| cx.notify()).detach();
        cx.observe(&crate::core::settings::SettingsStore::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe_global::<crate::theme::Theme>(|_, cx| cx.notify()).detach();
        cx.observe_global::<crate::i18n::I18n>(|_, cx| cx.notify()).detach();
        // titik status sesi membaca ChatStore + Attention (panel di-cache: perlu notify eksplisit)
        cx.observe(&crate::core::chat_store::ChatStore::global(cx), |_, _, cx| cx.notify()).detach();
        if let Some(a) = super::notification_driver::Attention::global(cx) {
            cx.observe(&a, |_, _, cx| cx.notify()).detach();
        }
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&search, |_, _, _: &crate::ui::InputChanged, cx| cx.notify()).detach();
        Self {
            app,
            store,
            engine,
            search,
            rename,
            show_search: false,
            tab: "sessions".into(),
            collapsed: HashSet::new(),
            workspace_open: true,
            open_menu: None,
            dialog: None,
            pstyle: None,
            files: cx.new(super::files_tree::FilesTree::new),
        }
    }

    fn weak(cx: &Context<Self>) -> WeakEntity<Self> {
        cx.entity().downgrade()
    }

    fn pick_folder(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(p) = paths.first() {
                    let p = p.to_string_lossy().to_string();
                    this.update(cx, |s, cx| {
                        s.dialog = Some(SbDialog::Trust(p));
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn new_session(&mut self, key: String, cx: &mut Context<Self>) {
        let task = self.store.update(cx, |s, cx| s.create_session(&key, None, true, cx));
        cx.spawn(async move |_, cx| {
            if let Err(e) = task.await {
                cx.update(|cx| {
                    Toasts::show(
                        ToastInput { title: e.into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None },
                        cx,
                    );
                });
            }
        })
        .detach();
        self.app.update(cx, |a, cx| a.push("/", cx));
    }

    fn open_rename(&mut self, d: SbDialog, current: String, window: &mut Window, cx: &mut Context<Self>) {
        self.rename.update(cx, |r, cx| r.set_text(current, cx));
        self.dialog = Some(d);
        let fh = self.rename.read(cx).focus_handle(cx);
        window.focus(&fh, cx);
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.dialog.take() else { return };
        let name = self.rename.read(cx).text().trim().to_string();
        let store = self.store.clone();
        match d {
            SbDialog::Trust(path) => {
                let r = store.update(cx, |s, cx| s.add_project(&path, cx));
                match r {
                    Ok(p) => store.update(cx, |s, cx| s.select(Some(p.id), None, cx)),
                    Err(e) => toast_err(e, cx),
                }
            }
            SbDialog::AddWithEngine(eng) if eng == "local" => self.pick_folder(cx),
            SbDialog::AddWithEngine(eng) => {
                if let Err(e) = store.update(cx, |s, cx| s.add_remote_project(&name, &eng, cx)) {
                    toast_err(e, cx);
                }
            }
            SbDialog::RemoveProject(id, _) => store.update(cx, |s, cx| s.remove_project(&id, cx)),
            SbDialog::DeleteSession(key, sid, _) => store.update(cx, |s, cx| s.delete_session(&key, &sid, cx)),
            SbDialog::RenameProject(id) if !name.is_empty() => {
                if let Err(e) = store.update(cx, |s, cx| s.rename_project(&id, &name, cx)) {
                    toast_err(e, cx);
                }
            }
            SbDialog::RenameSession(key, sid) if !name.is_empty() => {
                store.update(cx, |s, cx| s.rename_session(&key, &sid, &name, cx))
            }
            _ => {}
        }
        cx.notify();
    }
}

fn toast_err(e: String, cx: &mut App) {
    Toasts::show(ToastInput { title: e.into(), description: None, tone: ToastTone::Danger, timeout_ms: None, action: None }, cx);
}

struct Ctx<'a> {
    t: &'a Theme,
    i: &'a I18n,
    me: WeakEntity<Sidebar>,
    open_menu: Option<String>,
    sel: (Option<String>, Option<String>),
    query: String,
    ind: std::collections::HashMap<String, crate::core::attention::Indicator>,
}

/// Lapisan latar baris project: gambar (posisi/zoom) > gradien, lalu overlay. Port rowStyle + CSS overlay (Sidebar.tsx).
/// Gradien dirasterisasi penuh (semua titik, sudut, radial) sesuai ukuran baris — lihat `gradient_layer`.
fn project_layers(ap: &crate::core::appearance::AppearanceStore, id: &str) -> Vec<gpui::AnyElement> {
    style_layers(&|k| ap.project_style(id, k))
}

/// Lapisan latar gaya project (gambar/gradien + overlay) dari pembaca nilai gaya — dipakai baris sidebar dan
/// pratinjau dialog Kustomisasi project.
pub(crate) fn style_layers(get: &dyn Fn(&str) -> Option<String>) -> Vec<gpui::AnyElement> {
    use crate::core::appearance::parse_color;
    use gpui::{linear_color_stop, linear_gradient, relative};
    let mut out = vec![];
    let num = |k: &str, d: f32| get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
    if let Some(img) = get("image").and_then(|u| crate::core::gradient::image_of(&u)) {
        let z = num("imageZoom", 100.) / 100.;
        let y = num("imageY", 50.) / 100.;
        out.push(
            gpui::img(img)
                .absolute()
                .w(relative(z))
                .h(relative(z))
                .left(relative(-(z - 1.) / 2.))
                .top(relative(-(z - 1.) * y))
                .object_fit(gpui::ObjectFit::Cover)
                .into_any_element(),
        );
    } else if let Some(spec) = get("gradient").and_then(|g| crate::core::gradient::parse_gradient(&g)) {
        let stops: Vec<(f32, [f32; 4])> = spec
            .stops
            .iter()
            .filter_map(|s| parse_color(&s.color).map(|c| (s.pos / 100., gpui::Rgba::from(c))))
            .map(|(p, c)| (p, [c.r, c.g, c.b, c.a]))
            .collect();
        if stops.len() == spec.stops.len() && !stops.is_empty() {
            out.push(gradient_layer(stops, spec.radial, spec.angle));
        }
    }
    if let Some(kind) = get("overlay") {
        let mut c = get("overlayColor").and_then(|v| parse_color(&v)).unwrap_or(gpui::black());
        c.a = num("overlayOpacity", 50.) / 100.;
        let clear = gpui::Hsla { a: 0., ..c };
        let layer = div().absolute().inset_0();
        let layer = match kind.as_str() {
            "full" => layer.bg(c),
            "gradient" => layer.bg(linear_gradient(90., linear_color_stop(c, 0.), linear_color_stop(clear, 1.))),
            "gradient-rtl" => layer.bg(linear_gradient(270., linear_color_stop(c, 0.), linear_color_stop(clear, 1.))),
            "gradient-ttb" => layer.bg(linear_gradient(180., linear_color_stop(c, 0.), linear_color_stop(clear, 1.))),
            _ => layer.bg(linear_gradient(0., linear_color_stop(c, 0.), linear_color_stop(clear, 1.))),
        };
        out.push(layer.into_any_element());
    }
    out
}

thread_local! {
    static GRAD_CACHE: std::cell::RefCell<std::collections::HashMap<String, std::sync::Arc<gpui::RenderImage>>> = Default::default();
}

/// Lapisan gradien: dirender ke bitmap seukuran elemen (dibulatkan ke 4 px) lalu digambar sebagai gambar.
fn gradient_layer(stops: Vec<(f32, [f32; 4])>, radial: bool, angle: f32) -> gpui::AnyElement {
    gpui::canvas(
        move |b, window, _| {
            let sf = window.scale_factor();
            let q = |v: f32| (((v * sf) / 4.).ceil() as u32 * 4).clamp(4, 4096);
            let (w, h) = (q(f32::from(b.size.width)), q(f32::from(b.size.height)));
            let key = format!("{stops:?}|{radial}|{angle}|{w}x{h}");
            if let Some(i) = GRAD_CACHE.with(|c| c.borrow().get(&key).cloned()) {
                return Some(i);
            }
            let buf = crate::core::gradient::raster(&stops, radial, angle, w, h);
            let frame = image::Frame::new(image::RgbaImage::from_raw(w, h, buf)?);
            let img = std::sync::Arc::new(gpui::RenderImage::new(smallvec::smallvec![frame]));
            GRAD_CACHE.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() > 128 {
                    c.clear();
                }
                c.insert(key, img.clone());
            });
            Some(img)
        },
        |b, img, window, _| {
            if let Some(img) = img {
                let _ = window.paint_image(b, b, gpui::Corners::default(), img, 0, false);
            }
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

fn row_base(t: &Theme, id: impl Into<SharedString>, active: bool) -> gpui::Stateful<gpui::Div> {
    let c = &t.colors;
    div()
        .id(gpui::ElementId::Name(id.into()))
        .group("row")
        .flex()
        .items_center()
        .gap(t.space(2))
        .h(t.rem(rem::CONTROL_MD))
        .px(t.space(2))
        .rounded(t.rem(rem::RADIUS_MD))
        .cursor_pointer()
        .text_color(if active { c.fg } else { c.fg_muted })
        .when(active, |d| d.bg(c.active))
        .when(!active, |d| d.hover(|s| s.bg(c.hover).text_color(c.fg)))
}

fn menu_trigger(cx: &Ctx, key: String, entries: Vec<MenuEntry>, label: String) -> AnyElement {
    let open = cx.open_menu.as_deref() == Some(key.as_str());
    let me = cx.me.clone();
    let me2 = cx.me.clone();
    let k2 = key.clone();
    div()
        .relative()
        .flex_none()
        .when(!open, |d| d.invisible().group_hover("row", |s| s.visible()))
        .child(
            icon_button(SharedString::from(format!("mt-{key}")), "more", label)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .pressed(open)
                .on_click(move |_, _, cx| {
                    let k = k2.clone();
                    me.update(cx, |s, cx| {
                        s.open_menu = if s.open_menu.as_deref() == Some(k.as_str()) { None } else { Some(k) };
                        cx.notify();
                    })
                    .ok();
                }),
        )
        .when(open, |d| {
            d.child(menu_popup(SharedString::from(format!("m-{key}")), cx.t, entries, Anchor::TopRight, move |_, cx| {
                me2.update(cx, |s, cx| {
                    s.open_menu = None;
                    cx.notify();
                })
                .ok();
            }))
        })
        .into_any_element()
}

/// Titik status sesi (port SessionDot): idle redup, bekerja = indikator grid, butuh aksi = titik peringatan berdenyut,
/// selesai belum dilihat = hijau, gagal = bahaya, batas = bahaya dengan cincin.
pub fn session_dot(t: &Theme, i: &I18n, st: crate::core::attention::Indicator, id: &str) -> AnyElement {
    use crate::core::attention::Indicator;
    use gpui::AnimationExt;
    let label: SharedString = i.t(&format!("sdot.{}", st.key())).into();
    let wrap = div().id(SharedString::from(format!("sdot-{id}"))).flex_none().size(t.space(3)).flex().items_center().justify_center().tooltip(crate::ui::tooltip::text_tooltip(label.clone()));
    if st == Indicator::Busy {
        return wrap.child(crate::ui::working(format!("sdotw-{id}")).variant(crate::ui::feedback::WorkingVariant::Grid).size(crate::ui::feedback::WorkingSize::Dot)).into_any_element();
    }
    let (big, color) = match st {
        Indicator::Done => (true, t.colors.success),
        Indicator::Action => (true, t.colors.warning),
        Indicator::Failed | Indicator::Limit => (true, t.colors.danger),
        _ => (false, t.colors.fg_subtle),
    };
    let sz = if big { t.space(1) * 1.5 } else { t.space(1) };
    let dot = div().size(sz).rounded_full().bg(color);
    let wrap = if st == Indicator::Limit { wrap.border_1().border_dashed().border_color(t.colors.danger).rounded_full() } else { wrap };
    if st == Indicator::Action {
        // sdot-ping: cincin peringatan yang mengembang dan memudar (1800 ms, berulang)
        if crate::theme::reduced_motion() {
            return wrap.child(dot).into_any_element();
        }
        let ring = div().absolute().size(sz).rounded_full().border_1().border_color(color).with_animation(
            SharedString::from(format!("sdotp-{id}")),
            gpui::Animation::new(std::time::Duration::from_millis(1800)).repeat(),
            move |el, d| {
                let k = (d / 0.7).min(1.);
                el.size(sz + t_px(k * 8.)).opacity(0.55 * (1. - k))
            },
        );
        return wrap.relative().child(ring).child(dot).into_any_element();
    }
    wrap.child(dot).into_any_element()
}
fn t_px(v: f32) -> gpui::Pixels {
    gpui::px(v)
}

fn session_row(cx: &Ctx, key: &str, s: &SessionInfo) -> AnyElement {
    let t = cx.t;
    let active = cx.sel.0.as_deref() == Some(key) && cx.sel.1.as_deref() == Some(s.id.as_str());
    let title = session_title(s).map(str::to_string).unwrap_or_else(|| cx.i.t("sb.untitled"));
    let mk = |f: fn(&mut Sidebar, String, String, String, &mut Window, &mut Context<Sidebar>)| {
        let me = cx.me.clone();
        let (k, id, ti) = (key.to_string(), s.id.clone(), title.clone());
        move |w: &mut Window, cx: &mut App| {
            me.update(cx, |sb, cx| f(sb, k.clone(), id.clone(), ti.clone(), w, cx)).ok();
        }
    };
    let entries = vec![
        entry(cx.i.t("sb.rename"), mk(|sb, k, id, ti, w, cx| sb.open_rename(SbDialog::RenameSession(k, id), ti, w, cx))).icon("edit"),
        entry(cx.i.t("sb.copyId"), {
            let id = s.id.clone();
            move |_, cx: &mut App| cx.write_to_clipboard(ClipboardItem::new_string(id.clone()))
        })
        .icon("copy"),
        MenuEntry::Separator,
        entry(cx.i.t("sb.deleteSession"), mk(|sb, k, id, ti, _, cx| {
            // pengaturan confirmDelete: tanpa konfirmasi → langsung hapus
            if crate::core::settings::SettingsStore::global(cx).read(cx).bool("confirmDelete") {
                sb.dialog = Some(SbDialog::DeleteSession(k, id, ti));
                cx.notify();
            } else {
                sb.store.update(cx, |s, cx| s.delete_session(&k, &id, cx));
            }
        }))
        .icon("trash")
        .danger(),
    ];
    let me = cx.me.clone();
    let (k, id) = (key.to_string(), s.id.clone());
    row_base(t, format!("s-{key}-{}", s.id), active)
        .text_color(t.colors.fg)
        .on_click(move |_, _, cx| {
            let (k, id) = (k.clone(), id.clone());
            me.update(cx, |sb, cx| {
                sb.store.update(cx, |st, cx| st.select(Some(k), Some(id), cx));
                sb.app.update(cx, |a, cx| a.push("/", cx));
            })
            .ok();
        })
        .child(session_dot(t, cx.i, cx.ind.get(&s.id).copied().unwrap_or(crate::core::attention::Indicator::Idle), &s.id))
        .child(div().flex_1().min_w_0().truncate().child(title.clone()))
        .child(menu_trigger(cx, format!("s:{key}:{}", s.id), entries, cx.i.tp("sb.menuFor", &[("name", &title)])))
        .into_any_element()
}

fn note(t: &Theme, text: String) -> AnyElement {
    div().pl(t.space(5)).pr(t.space(2)).py(t.space(1)).small(t).text_color(t.colors.fg_subtle).child(text).into_any_element()
}

fn sessions_list(cx: &Ctx, key: &str, st: Option<&SessionsState>, engine_ready: bool) -> Vec<AnyElement> {
    let t = cx.t;
    if !engine_ready {
        return vec![note(t, cx.i.t("sb.engineWait"))];
    }
    let Some(st) = st else { return vec![note(t, cx.i.t("sb.loading"))] };
    match st.status {
        LoadStatus::Error => return vec![note(t, st.error.clone().unwrap_or_else(|| cx.i.t("sb.loadError")))],
        LoadStatus::Loading | LoadStatus::Idle if st.items.is_empty() => return vec![note(t, cx.i.t("sb.loading"))],
        _ => {}
    }
    let q = cx.query.to_lowercase();
    let items: Vec<&SessionInfo> = st
        .items
        .iter()
        .filter(|s| q.is_empty() || session_title(s).unwrap_or("").to_lowercase().contains(&q))
        .collect();
    if items.is_empty() {
        return vec![note(t, cx.i.t(if q.is_empty() { "sb.noSessions" } else { "sb.noMatch" }))];
    }
    items.into_iter().map(|s| session_row(cx, key, s)).collect()
}

fn section_head(t: &Theme, (label, n): (String, usize), extra: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t.space(1))
        .h(t.rem(rem::CONTROL_MD))
        .px(t.space(2))
        .mt(t.space(2))
        .text_color(t.colors.fg_muted)
        .child(
            div()
                .flex_1()
                .flex()
                .gap(t.space(1))
                .typo(t, crate::theme::LABEL)
                .child(label)
                .child(div().text_color(t.colors.fg_subtle).font_weight(gpui::FontWeight::NORMAL).child(format!("({n})"))),
        )
        .children(extra)
        .into_any_element()
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>();
        let c = t.colors.clone();
        let me = Self::weak(cx);
        let store = self.store.read(cx);
        let projects = store.projects.clone();
        let sessions = store.sessions.clone();
        let engine_ready = store.engine_ready;
        let sel = (store.selection.key.clone(), store.selection.session_id.clone());
        let est = self.engine.read(cx).state.clone();
        let query = if self.show_search { self.search.read(cx).text().to_string() } else { String::new() };
        let ind = sessions
            .values()
            .flat_map(|g| g.items.iter())
            .map(|s| (s.id.clone(), super::notification_driver::Attention::indicator_of(&s.id, cx)))
            .collect();
        let ctx = Ctx { t: &t, i: &i, me: me.clone(), open_menu: self.open_menu.clone(), sel, query, ind };

        // ---- kepala ----
        let head = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .px(t.space(3))
            .pt(t.space(3))
            .pb(t.space(2))
            .child(
                div()
                    .flex_none()
                    .size(t.rem(rem::CONTROL_MD))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .bg(c.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("app_logo").size(IconSize::Md).color(c.on_accent)),
            )
            .child(div().typo(&t, H4).child("Zandor"))
            .child(if cfg!(debug_assertions) { badge("Dev").tone(BadgeTone::Warning) } else { badge("BETA").tone(BadgeTone::Outline) })
            .child(div().flex_1())
            .child(
                icon_button("sb-search", "search", i.t("sb.searchToggle"))
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .pressed(self.show_search)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.show_search = !s.show_search;
                        if s.show_search {
                            let fh = s.search.read(cx).focus_handle(cx);
                            w.focus(&fh, cx);
                        }
                        cx.notify();
                    })),
            );

        let search = self.show_search.then(|| div().px(t.space(3)).pb(t.space(2)).child(self.search.clone()));

        let tab_bar = div().mx(t.space(3)).child(
            tabs(
                "sb-tabs",
                vec![
                    TabItem { value: "sessions".into(), label: i.t("sb.tab.sessions").into(), icon: Some("chat"), disabled: false },
                    TabItem { value: "files".into(), label: i.t("sb.tab.files").into(), icon: Some("file"), disabled: false },
                ],
                self.tab.clone(),
            )
            .stretch(true)
            .on_change({
                let me = me.clone();
                move |v, _, cx| {
                    me.update(cx, |s, cx| {
                        s.tab = v;
                        cx.notify();
                    })
                    .ok();
                }
            }),
        );

        // ---- aksi ----
        let action = |id: &'static str, ic: &'static str, label: String, tail: Option<AnyElement>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(t.space(2))
                .w_full()
                .h(t.rem(rem::CONTROL_MD))
                .px(t.space(3))
                .border_1()
                .border_color(c.border)
                .rounded(t.rem(rem::RADIUS_MD))
                .bg(c.bg)
                .cursor_pointer()
                .text_color(c.fg)
                .hover(|s| s.bg(c.hover))
                .child(icon(ic).size(IconSize::Md))
                .child(div().flex_1().truncate().child(label))
                .children(tail)
        };
        let actions = div()
            .flex()
            .flex_col()
            .gap(t.space(1))
            .mx(t.space(3))
            .mt(t.space(2))
            .child(
                action("sb-newchat", "plus", i.t("sb.newChat"), Some(div().small(&t).code(&t).text_color(c.fg_muted).child("Ctrl N").into_any_element()))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.store.update(cx, |st, cx| st.select(Some(CHAT_KEY.into()), None, cx));
                        s.app.update(cx, |a, cx| a.push("/", cx));
                    })),
            )
            .child(
                action("sb-auto", "clock", i.t("auto.manage"), None)
                    .on_click(cx.listener(|s, _, _, cx| s.app.update(cx, |a, cx| a.push("/automation", cx)))),
            );

        // ---- daftar ----
        let mut list = div().id("sb-list").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().px(t.space(3)).pb(t.space(3));
        if self.tab.as_ref() == "files" {
            list = list.child(self.files.clone());
        } else {
            list = list.child(
                div()
                    .id("sb-ws")
                    .flex()
                    .items_center()
                    .justify_between()
                    .mt(t.space(1))
                    .mx(-t.space(3))
                    .px(t.space(3))
                    .py(t.space(2))
                    .border_t_1()
                    .border_b_1()
                    .border_color(c.border_soft)
                    .cursor_pointer()
                    .text_color(c.fg_muted)
                    .child(div().typo(&t, crate::theme::LABEL).child(i.t("sb.workspace")))
                    .child(icon(if self.workspace_open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.workspace_open = !s.workspace_open;
                        cx.notify();
                    })),
            );
            if self.workspace_open {
                let all_collapsed = !projects.is_empty() && projects.iter().all(|p| self.collapsed.contains(&p.id));
                list = list.child(section_head(
                    &t,
                    (i.t("sb.section.projects"), projects.len()),
                    vec![
                        icon_button("sb-add", "plus", i.t("sb.addProject"))
                            .variant(ButtonVariant::Ghost)
                            .size(ControlSize::Sm)
                            .on_click(cx.listener(|s, _, _, cx| {
                                if s.store.read(cx).remotes.is_empty() {
                                    s.pick_folder(cx);
                                } else {
                                    s.rename.update(cx, |r, cx| r.set_text("", cx));
                                    s.dialog = Some(SbDialog::AddWithEngine("local".into()));
                                    cx.notify();
                                }
                            }))
                            .into_any_element(),
                        icon_button(
                            "sb-collapse",
                            if all_collapsed { "expand_all" } else { "collapse_all" },
                            i.t(if all_collapsed { "sb.restoreProjects" } else { "sb.collapseProjects" }),
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .on_click(cx.listener(move |s, _, _, cx| {
                            let ids: Vec<String> = s.store.read(cx).projects.iter().map(|p| p.id.clone()).collect();
                            if all_collapsed { s.collapsed.clear() } else { s.collapsed.extend(ids) }
                            cx.notify();
                        }))
                        .into_any_element(),
                    ],
                ));
                if projects.is_empty() {
                    list = list.child(note(&t, i.t("sb.noProjects")));
                }
                for p in &projects {
                    let open = !self.collapsed.contains(&p.id);
                    let active = ctx.sel.0.as_deref() == Some(p.id.as_str());
                    let pid = p.id.clone();
                    let mkp = |f: fn(&mut Sidebar, String, String, &mut Window, &mut Context<Sidebar>)| {
                        let me = me.clone();
                        let (id, name) = (p.id.clone(), p.name.clone());
                        move |w: &mut Window, cx: &mut App| {
                            me.update(cx, |sb, cx| f(sb, id.clone(), name.clone(), w, cx)).ok();
                        }
                    };
                    let entries = vec![
                        entry(i.t("sb.newSession"), mkp(|sb, id, _, _, cx| sb.new_session(id, cx))).icon("plus"),
                        entry(i.t("sb.rename"), mkp(|sb, id, n, w, cx| sb.open_rename(SbDialog::RenameProject(id), n, w, cx))).icon("edit"),
                        entry(i.t("sb.customize"), mkp(|sb, id, n, _, cx| {
                            let d = cx.new(|cx| super::project_style_dialog::ProjectStyleDialog::new(id, n, cx));
                            cx.subscribe(&d, |sb, _, _: &super::project_style_dialog::StyleClosed, cx| {
                                sb.pstyle = None;
                                cx.notify();
                            })
                            .detach();
                            sb.pstyle = Some(d);
                            cx.notify();
                        }))
                        .icon("image"),
                        entry(i.t("sb.refresh"), mkp(|sb, id, _, _, cx| sb.store.update(cx, |s, cx| s.refresh(&id, cx)))).icon("refresh"),
                        MenuEntry::Separator,
                        entry(i.t("sb.removeProject"), mkp(|sb, id, n, _, cx| {
                            sb.dialog = Some(SbDialog::RemoveProject(id, n));
                            cx.notify();
                        }))
                        .icon("trash")
                        .danger(),
                    ];
                    let pid2 = pid.clone();
                    let (logo, logo_img, pborder, pbg, pfg, layers) = {
                        let ap = crate::core::appearance::AppearanceStore::global(cx).read(cx);
                        let col = |k: &str| ap.project_style(&p.id, k).and_then(|v| crate::core::appearance::parse_color(&v));
                        let logo_img = ap.project_style(&p.id, "logoImage").and_then(|u| crate::core::gradient::image_of(&u));
                        (ap.project_style(&p.id, "logo"), logo_img, col("border"), col("background"), col("fg"), project_layers(&ap, &p.id))
                    };
                    let row = row_base(&t, format!("p-{}", p.id), active && ctx.sel.1.is_none())
                        .when(!layers.is_empty(), |d| d.relative().overflow_hidden().children(layers))
                        .when_some(pbg, |d, c| d.bg(c))
                        .when_some(pborder, |d, c| d.border_1().border_color(c))
                        .when_some(pfg, |d, c| d.text_color(c))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            if !s.collapsed.remove(&pid) {
                                s.collapsed.insert(pid.clone());
                            }
                            cx.notify();
                        }))
                        .child(match (logo_img, logo) {
                            (Some(img), _) => gpui::img(img).flex_none().size(t.rem(rem::ICON_MD)).rounded(t.rem(rem::RADIUS_SM)).object_fit(gpui::ObjectFit::Cover).into_any_element(),
                            (None, Some(l)) => div().flex_none().w(t.rem(rem::ICON_MD)).flex().justify_center().child(l).into_any_element(),
                            (None, None) => icon(if open { "folder_open" } else { "folder" }).size(IconSize::Md).into_any_element(),
                        })
                        .child(div().flex_1().min_w_0().truncate().child(p.name.clone()))
                        .children(p.engine.as_ref().map(|eid| {
                            use crate::core::projects::RemoteStatus;
                            let st = ProjectStore::global(cx).read(cx);
                            let name = st.remote_by_id(eid).map(|r| r.name.clone()).unwrap_or_else(|| eid.clone());
                            let (status, color) = match st.remote_status.get(eid) {
                                Some(RemoteStatus::Ready(v)) => (i.tp("remote.ready", &[("version", if v.is_empty() { "?" } else { v })]), t.colors.success),
                                Some(RemoteStatus::Error(e)) => (format!("{}: {e}", i.t("remote.failed")), t.colors.danger),
                                _ => (i.t("remote.checking"), t.colors.fg_subtle),
                            };
                            let tip = format!("{} · {status}", i.tp("remote.badge", &[("name", &name)]));
                            div()
                                .id(SharedString::from(format!("prm-{}", p.id)))
                                .flex_none()
                                .text_color(color)
                                .tooltip(crate::ui::tooltip::text_tooltip(tip.into()))
                                .child(icon("globe").size(IconSize::Sm))
                        }))
                        .child(
                            div().flex_none().invisible().group_hover("row", |s| s.visible()).child(
                                icon_button(SharedString::from(format!("pn-{}", p.id)), "plus", i.tp("sb.newSessionIn", &[("name", &p.name)]))
                                    .variant(ButtonVariant::Ghost)
                                    .size(ControlSize::Sm)
                                    .on_click(cx.listener(move |s, _, _, cx| s.new_session(pid2.clone(), cx))),
                            ),
                        )
                        .child(menu_trigger(&ctx, format!("p:{}", p.id), entries, i.tp("sb.menuFor", &[("name", &p.name)])));
                    list = list.child(row);
                    if open {
                        list = list.children(sessions_list(&ctx, &p.id, sessions.get(&p.id), engine_ready));
                    }
                }
                list = list.child(section_head(
                    &t,
                    (i.t("sb.section.chats"), sessions.get(CHAT_KEY).map(|g| g.items.len()).unwrap_or(0)),
                    vec![icon_button("sb-chat-new", "plus", i.t("sb.newChat"))
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .on_click(cx.listener(|s, _, _, cx| s.new_session(CHAT_KEY.into(), cx)))
                        .into_any_element()],
                ));
                // Sesi demo bawaan (pengaturan showDemos): disematkan, permanen, hanya-baca (port DemoRows).
                if crate::core::settings::SettingsStore::global(cx).read(cx).bool("showDemos") {
                    for d in crate::core::demo::demos() {
                        let active = ctx.sel.0.as_deref() == Some(crate::core::demo::DEMO_KEY) && ctx.sel.1.as_deref() == Some(d.id.as_str());
                        let id = d.id.clone();
                        list = list.child(
                            row_base(&t, format!("demo-{}", d.id), active)
                                .text_color(c.fg)
                                .tooltip(crate::ui::tooltip::text_tooltip(i.t(&d.subtitle_key).into()))
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    let id = id.clone();
                                    s.store.update(cx, |st, cx| st.select(Some(crate::core::demo::DEMO_KEY.into()), Some(id), cx));
                                    s.app.update(cx, |a, cx| a.push("/", cx));
                                }))
                                .child(session_dot(&t, &i, crate::core::attention::Indicator::Idle, &d.id))
                                .child(div().flex_1().min_w_0().truncate().child(i.t(&d.title_key)))
                                .child(div().flex_none().text_color(c.fg_subtle).child(icon("pin").size(IconSize::Sm))),
                        );
                    }
                }
                list = list.children(sessions_list(&ctx, CHAT_KEY, sessions.get(CHAT_KEY), engine_ready));
            }
        }

        // ---- kaki ----
        let (dot, failed) = match est.status.as_str() {
            "ready" => (c.success, false),
            "error" | "stopped" => (c.danger, true),
            _ => (c.warning, false),
        };
        let name = crate::core::profile::Profile::global(cx).read(cx).display_name();
        let menu_open = self.open_menu.as_deref() == Some("user");
        let nav = |route: &'static str| {
            let me = me.clone();
            move |_: &mut Window, cx: &mut App| {
                me.update(cx, |s, cx| s.app.update(cx, |a, cx| a.push(route, cx))).ok();
            }
        };
        let user_entries = vec![
            entry(i.t("profile.manage"), nav("/settings/profile")).icon("user"),
            entry(i.t("settings.title"), nav("/settings")).icon("settings"),
            entry(i.t("pv.manage"), nav("/settings/models")).icon("globe"),
            entry(i.t("mem.manage"), nav("/memory")).icon("brain"),
            entry(i.t("cmd.appearance"), nav("/appearance")).icon("eye"),
        ];
        let foot = div()
            .flex()
            .items_center()
            .gap(t.space(2))
            .px(t.space(3))
            .py(t.space(2))
            .border_t_1()
            .border_color(c.border)
            .child(
                div()
                    .relative()
                    .id("sb-user")
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(t.space(2))
                    .child(crate::core::profile::profile_avatar(cx).size(ControlSize::Md))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(div().truncate().font_weight(gpui::FontWeight::MEDIUM).child(name))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(t.space(1))
                                    .small(&t)
                                    .text_color(c.fg_subtle)
                                    .child(div().code(&t).child(i.tp("sb.versionShort", &[("version", env!("CARGO_PKG_VERSION"))])))
                                    .when(est.status != "ready", |d| d.child(div().size(px(6.)).rounded_full().bg(dot)).child(i.t(&format!("engine.{}", est.status)))),
                            ),
                    )
,
            )
            .when(failed, |d| {
                d.child(
                    icon_button("sb-retry", "refresh", i.t("engine.retry"))
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .on_click(cx.listener(|s, _, _, cx| s.engine.read(cx).restart())),
                )
            })
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(
                        icon_button("sb-user-menu", "more", i.t("sb.user.menu"))
                            .variant(ButtonVariant::Ghost)
                            .size(ControlSize::Sm)
                            .pressed(menu_open)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.open_menu = if s.open_menu.as_deref() == Some("user") { None } else { Some("user".into()) };
                                cx.notify();
                            })),
                    )
                    .when(menu_open, |d| {
                        let me = me.clone();
                        d.child(menu_popup("m-user", &t, user_entries, Anchor::BottomLeft, move |_, cx| {
                            me.update(cx, |s, cx| {
                                s.open_menu = None;
                                cx.notify();
                            })
                            .ok();
                        }))
                    }),
            );

        // ---- dialog ----
        let dlg = self.dialog.clone().map(|d| {
            let (title, body, input, danger, ok) = match &d {
                SbDialog::Trust(p) => (i.t("sb.trustTitle"), Some(i.tp("sb.trustBody", &[("path", p)])), false, false, i.t("sb.trustYes")),
                SbDialog::AddWithEngine(e) if e == "local" => (i.t("sb.addByPathTitle"), None, false, false, i.t("sb.addProject")),
                SbDialog::AddWithEngine(_) => (i.t("sb.addByPathTitle"), Some(i.t("remote.pathHint")), true, false, i.t("sb.add")),
                SbDialog::RemoveProject(_, n) => {
                    (i.tp("sb.confirmRemoveTitle", &[("name", n)]), Some(i.t("sb.confirmRemoveBody")), false, true, i.t("sb.removeProject"))
                }
                SbDialog::DeleteSession(_, _, n) => {
                    (i.tp("sb.confirmDeleteTitle", &[("name", n)]), Some(i.t("sb.confirmDeleteBody")), false, true, i.t("sb.deleteSession"))
                }
                SbDialog::RenameProject(_) => (i.t("sb.renameProjectTitle"), None, true, false, i.t("sb.save")),
                SbDialog::RenameSession(..) => (i.t("sb.renameSessionTitle"), None, true, false, i.t("sb.save")),
            };
            let mut dl = dialog(title).on_dismiss(cx.listener(|s, _, _, cx| {
                s.dialog = None;
                cx.notify();
            }));
            if let Some(b) = body {
                dl = dl.description(b);
            }
            if let SbDialog::AddWithEngine(cur) = &d {
                let mut items: Vec<(SharedString, SharedString)> = vec![("local".into(), i.t("remote.local").into())];
                items.extend(self.store.read(cx).remotes.iter().map(|r| (SharedString::from(r.id.clone()), SharedString::from(r.name.clone()))));
                let me = cx.entity();
                dl = dl.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(2))
                        .child(div().flex_1().child(i.t("remote.engine")))
                        .child(crate::ui::select("sb-engine", items, cur.clone()).on_change(move |v, _, cx| {
                            me.update(cx, |s, cx| {
                                s.dialog = Some(SbDialog::AddWithEngine(v.to_string()));
                                cx.notify();
                            })
                        })),
                );
                if cur != "local" {
                    let name = self.store.read(cx).remote_by_id(cur).map(|r| r.name.clone()).unwrap_or_default();
                    dl = dl.child(crate::ui::badge(i.tp("remote.badge", &[("name", &name)]))).child(div().small(&t).text_color(c.fg_muted).child(i.t("remote.noMemory")));
                }
            }
            if input {
                dl = dl.child(self.rename.clone());
            }
            dl.footer(
                div()
                    .flex()
                    .justify_end()
                    .gap(t.space(2))
                    .child(button("dlg-cancel").label(i.t("ui.cancel")).variant(ButtonVariant::Ghost).on_click(cx.listener(|s, _, _, cx| {
                        s.dialog = None;
                        cx.notify();
                    })))
                    .child(
                        button("dlg-ok")
                            .label(ok)
                            .variant(if danger { ButtonVariant::Danger } else { ButtonVariant::Solid })
                            .on_click(cx.listener(|s, _, _, cx| s.confirm(cx))),
                    ),
            )
        });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(c.bg)
            .child(head)
            .children(search)
            .child(tab_bar)
            .child(actions)
            .child(list)
            .child(foot)
            .children(dlg)
            .children(self.pstyle.clone())
    }
}

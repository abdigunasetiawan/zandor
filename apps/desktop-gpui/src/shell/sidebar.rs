//! Sidebar — port shell/Sidebar.tsx: kepala (logo, nama, badge, cari), tab Chat/Berkas, aksi, daftar project & sesi,
//! kelompok Chat, dan kaki (profil + status engine).
use super::state::AppState;
use crate::core::engine::SessionInfo;
use crate::core::projects::{session_title, LoadStatus, ProjectStore, SessionsState, CHAT_KEY};
use crate::core::services::EngineStatus;
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H4};
use crate::ui::{
    badge, button, dialog, entry, icon, icon_button, kbd, menu_popup, tabs, BadgeTone, ButtonVariant, ControlSize,
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
    files: Entity<super::tools::ToolsHost>,
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
            files: cx.new(|cx| {
                let mut h = super::tools::ToolsHost::new(cx);
                h.tool = super::layout::ToolId::Files;
                h
            }),
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
}

/// Lapisan latar baris project: gambar (posisi/zoom) > gradien, lalu overlay. Port rowStyle + CSS overlay (Sidebar.tsx).
/// GPUI hanya mendukung gradien linear 2 titik: titik pertama & terakhir dipakai; gradien radial digambar linear.
fn project_layers(ap: &crate::core::appearance::AppearanceStore, id: &str) -> Vec<gpui::AnyElement> {
    use crate::core::appearance::parse_color;
    use gpui::{linear_color_stop, linear_gradient, relative};
    let mut out = vec![];
    let num = |k: &str, d: f32| ap.project_style(id, k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
    if let Some(img) = ap.project_style(id, "image").and_then(|u| crate::core::gradient::image_of(&u)) {
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
    } else if let Some(spec) = ap.project_style(id, "gradient").and_then(|g| crate::core::gradient::parse_gradient(&g)) {
        let (a, b) = (spec.stops.first(), spec.stops.last());
        if let (Some(a), Some(b)) = (a, b) {
            if let (Some(ca), Some(cb)) = (parse_color(&a.color), parse_color(&b.color)) {
                let angle = if spec.radial { 180. } else { spec.angle };
                out.push(div().absolute().inset_0().bg(linear_gradient(angle, linear_color_stop(ca, a.pos / 100.), linear_color_stop(cb, b.pos / 100.))).into_any_element());
            }
        }
    }
    if let Some(kind) = ap.project_style(id, "overlay") {
        let mut c = ap.project_style(id, "overlayColor").and_then(|v| parse_color(&v)).unwrap_or(gpui::black());
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
            sb.dialog = Some(SbDialog::DeleteSession(k, id, ti));
            cx.notify();
        }))
        .icon("trash")
        .danger(),
    ];
    let me = cx.me.clone();
    let (k, id) = (key.to_string(), s.id.clone());
    row_base(t, format!("s-{key}-{}", s.id), active)
        .pl(t.space(5))
        .on_click(move |_, _, cx| {
            let (k, id) = (k.clone(), id.clone());
            me.update(cx, |sb, cx| {
                sb.store.update(cx, |st, cx| st.select(Some(k), Some(id), cx));
                sb.app.update(cx, |a, cx| a.push("/", cx));
            })
            .ok();
        })
        .child(
            div().flex_none().size(t.rem(0.75)).flex().items_center().justify_center().child(
                div().size(t.rem(0.25)).rounded_full().bg(if active { t.colors.accent } else { t.colors.fg_subtle }),
            ),
        )
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

fn section_head(t: &Theme, label: String, extra: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(t.space(1))
        .h(t.rem(rem::CONTROL_MD))
        .px(t.space(2))
        .mt(t.space(2))
        .child(div().flex_1().small(t).font_weight(gpui::FontWeight::SEMIBOLD).text_color(t.colors.fg_subtle).child(label))
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
        let ctx = Ctx { t: &t, i: &i, me: me.clone(), open_menu: self.open_menu.clone(), sel, query };

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
                    TabItem { value: "sessions".into(), label: i.t("sb.tab.sessions").into(), icon: None, disabled: false },
                    TabItem { value: "files".into(), label: i.t("sb.tab.files").into(), icon: None, disabled: false },
                ],
                self.tab.clone(),
            )
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
                .h(t.rem(rem::CONTROL_MD))
                .px(t.space(2))
                .rounded(t.rem(rem::RADIUS_MD))
                .cursor_pointer()
                .text_color(c.fg_muted)
                .hover(|s| s.bg(c.hover).text_color(c.fg))
                .child(icon(ic).size(IconSize::Md))
                .child(div().flex_1().child(label))
                .children(tail)
        };
        let actions = div()
            .flex()
            .flex_col()
            .gap(t.space(1))
            .mx(t.space(3))
            .my(t.space(2))
            .p(t.space(1))
            .border_1()
            .border_color(c.border)
            .rounded(t.rem(rem::RADIUS_LG))
            .bg(c.bg)
            .child(
                action("sb-newchat", "chat", i.t("sb.newChat"), Some(kbd("Ctrl+N").into_any_element()))
                    .on_click(cx.listener(|s, _, _, cx| s.new_session(CHAT_KEY.into(), cx))),
            )
            .child(
                action("sb-auto", "clock", i.t("auto.title"), None)
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
                    .gap(t.space(1))
                    .h(t.rem(rem::CONTROL_MD))
                    .px(t.space(2))
                    .border_t_1()
                    .border_b_1()
                    .border_color(c.border_soft)
                    .cursor_pointer()
                    .text_color(c.fg_muted)
                    .child(icon(if self.workspace_open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm))
                    .child(div().small(&t).child(i.t("sb.workspace")))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.workspace_open = !s.workspace_open;
                        cx.notify();
                    })),
            );
            if self.workspace_open {
                let all_collapsed = !projects.is_empty() && projects.iter().all(|p| self.collapsed.contains(&p.id));
                list = list.child(section_head(
                    &t,
                    format!("{} · {}", i.t("sb.projects"), projects.len()),
                    vec![
                        icon_button("sb-add", "plus", i.t("sb.addProject"))
                            .variant(ButtonVariant::Ghost)
                            .size(ControlSize::Sm)
                            .on_click(cx.listener(|s, _, _, cx| s.pick_folder(cx)))
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
                    i.t("sb.chatGroup"),
                    vec![icon_button("sb-chat-new", "plus", i.t("sb.newChat"))
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .on_click(cx.listener(|s, _, _, cx| s.new_session(CHAT_KEY.into(), cx)))
                        .into_any_element()],
                ));
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
            entry(i.t("cmd.openSettings"), nav("/settings")).icon("settings"),
            entry(i.t("pv.manage"), nav("/settings/models")).icon("server"),
            entry(i.t("mem.manage"), nav("/memory")).icon("brain"),
            entry(i.t("cmd.appearance"), nav("/appearance")).icon("customize"),
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
                    .p(t.space(1))
                    .rounded(t.rem(rem::RADIUS_MD))
                    .cursor_pointer()
                    .hover(|s| s.bg(c.hover))
                    .when(menu_open, |d| d.bg(c.active))
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.open_menu = if s.open_menu.as_deref() == Some("user") { None } else { Some("user".into()) };
                        cx.notify();
                    }))
                    .child(crate::core::profile::profile_avatar(cx).size(ControlSize::Sm))
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
                                    .child(div().size(px(6.)).rounded_full().bg(dot))
                                    .child(i.t(&format!("engine.{}", est.status))),
                            ),
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
            )
            .when(failed, |d| {
                d.child(
                    icon_button("sb-retry", "refresh", i.t("engine.retry"))
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .on_click(cx.listener(|s, _, _, cx| s.engine.read(cx).restart())),
                )
            });

        // ---- dialog ----
        let dlg = self.dialog.clone().map(|d| {
            let (title, body, input, danger, ok) = match &d {
                SbDialog::Trust(p) => (i.t("sb.trustTitle"), Some(i.tp("sb.trustBody", &[("path", p)])), false, false, i.t("sb.trustYes")),
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
            .bg(c.surface)
            .child(head)
            .children(search)
            .child(tab_bar)
            .child(actions)
            .child(list)
            .child(foot)
            .children(dlg)
    }
}

//! Tab Files — port FilesTree.tsx: pohon berkas nyata dari folder kerja aktif (project atau folder sesi Chat), dimuat
//! malas per folder lewat engine (GET /file?path=). Kepala: nama project (ganti project), berkas/folder baru, tutup/buka
//! semua, muat ulang; kolom cari (membuka semua folder sekali, dibatasi MAX_EXPAND). Klik berkas membukanya di Panel 3.
use crate::core::engine::FileNode;
use crate::core::projects::ProjectStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, entry, icon, icon_button, menu_popup, ButtonVariant, ControlSize, IconSize, InputBlur, InputChanged, InputSubmit, MenuEntry, TextInput, Toasts, ToastInput, ToastTone, TypeExt};
use gpui::{anchored, div, prelude::*, AnyElement, Anchor, Context, Entity, Focusable, IntoElement, SharedString, Window};
use std::collections::{HashMap, HashSet};

/// Batas folder yang dibuka sekaligus oleh "Buka semua" dan pencarian (melindungi repo besar).
pub const MAX_EXPAND: usize = 150;

#[derive(Clone)]
enum Listing {
    Loading(Vec<FileNode>),
    Ready(Vec<FileNode>),
    Error(String),
}

fn sort_nodes(v: &mut [FileNode]) {
    v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() { name.to_string() } else { format!("{parent}/{name}") }
}

pub struct FilesTree {
    root: Option<String>,
    engine_ready: bool,
    listings: HashMap<String, Listing>,
    open: HashSet<String>,
    query: Entity<TextInput>,
    searched: bool,
    capped: bool,
    /// Pembuatan entri: (folder induk, folder?).
    creating: Option<(String, bool)>,
    name: Entity<TextInput>,
    project_menu: bool,
}

impl FilesTree {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let i = cx.global::<I18n>().clone();
        let query = cx.new(|cx| TextInput::new(cx, i.t("files.search")));
        let name = cx.new(|cx| TextInput::new(cx, i.t("files.nameFile")));
        cx.subscribe(&query, |s, _, e: &InputChanged, cx| {
            let q = e.0.trim().to_string();
            if !q.is_empty() && !s.searched && s.root.is_some() {
                s.searched = true;
                s.expand_all(cx);
            }
            if q.is_empty() {
                s.searched = false;
            }
            cx.notify();
        })
        .detach();
        cx.subscribe(&name, |s, _, _: &InputSubmit, cx| s.confirm_create(cx)).detach();
        cx.subscribe(&name, |s, _, _: &InputBlur, cx| {
            s.creating = None;
            cx.notify();
        })
        .detach();
        let store = ProjectStore::global(cx);
        cx.observe(&store, |s, _, cx| s.sync(cx)).detach();
        let mut me = Self { root: None, engine_ready: false, listings: HashMap::new(), open: HashSet::new(), query, searched: false, capped: false, creating: None, name, project_menu: false };
        me.sync(cx);
        me
    }

    /// Folder kerja aktif: sesi terpilih → direktorinya; atau project terpilih.
    fn dir(cx: &gpui::App) -> Option<String> {
        let st = ProjectStore::global(cx).read(cx);
        if let Some((_, s, p)) = st.selected() {
            if !s.directory.is_empty() {
                return Some(s.directory);
            }
            return p.map(|p| p.path);
        }
        let key = st.selection.key.clone()?;
        st.projects.iter().find(|p| p.id == key).map(|p| p.path.clone())
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let dir = Self::dir(cx);
        let ready = ProjectStore::global(cx).read(cx).engine_ready;
        if dir == self.root && ready == self.engine_ready {
            return;
        }
        // ganti project atau status engine → mulai dari nol
        self.root = dir;
        self.engine_ready = ready;
        self.listings.clear();
        self.open.clear();
        self.query.update(cx, |q, cx| q.set_text("", cx));
        self.searched = false;
        self.creating = None;
        self.capped = false;
        if self.root.is_some() && ready {
            self.fetch(String::new(), true, cx);
        }
        cx.notify();
    }

    fn fetch(&mut self, rel: String, force: bool, cx: &mut Context<Self>) {
        let (Some(root), Some(api)) = (self.root.clone(), ProjectStore::global(cx).read(cx).api()) else { return };
        if !force && matches!(self.listings.get(&rel), Some(Listing::Ready(_))) {
            return;
        }
        let prev = match self.listings.get(&rel) {
            Some(Listing::Ready(v) | Listing::Loading(v)) => v.clone(),
            _ => vec![],
        };
        self.listings.insert(rel.clone(), Listing::Loading(prev));
        let r2 = rel.clone();
        let task = cx.background_spawn(async move { api.list_files(&root, &r2).map_err(|e| e.0) });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                s.listings.insert(rel, match r {
                    Ok(mut v) => {
                        sort_nodes(&mut v);
                        Listing::Ready(v)
                    }
                    Err(e) => Listing::Error(e),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Buka semua folder (kecuali yang diabaikan git) secara bertingkat, dibatasi MAX_EXPAND.
    fn expand_all(&mut self, cx: &mut Context<Self>) {
        let (Some(root), Some(api)) = (self.root.clone(), ProjectStore::global(cx).read(cx).api()) else { return };
        let cached: HashMap<String, Vec<FileNode>> = self.listings.iter().filter_map(|(k, l)| if let Listing::Ready(v) = l { Some((k.clone(), v.clone())) } else { None }).collect();
        let task = cx.background_spawn(async move {
            let mut got: HashMap<String, Result<Vec<FileNode>, String>> = HashMap::new();
            let mut opened: Vec<String> = vec![];
            let mut frontier = vec![String::new()];
            let mut cap = false;
            while !frontier.is_empty() && !cap {
                let mut next = vec![];
                for p in frontier {
                    let nodes = match cached.get(&p) {
                        Some(v) => v.clone(),
                        None => {
                            let r = api.list_files(&root, &p).map_err(|e| e.0);
                            let v = r.clone().unwrap_or_default();
                            got.insert(p.clone(), r);
                            v
                        }
                    };
                    for n in nodes {
                        if !n.is_dir || n.ignored {
                            continue;
                        }
                        if opened.len() >= MAX_EXPAND {
                            cap = true;
                            break;
                        }
                        opened.push(n.path.clone());
                        next.push(n.path);
                    }
                    if cap {
                        break;
                    }
                }
                frontier = next;
            }
            (got, opened, cap)
        });
        cx.spawn(async move |this, cx| {
            let (got, opened, cap) = task.await;
            this.update(cx, |s, cx| {
                for (k, r) in got {
                    s.listings.insert(k, match r {
                        Ok(mut v) => {
                            sort_nodes(&mut v);
                            Listing::Ready(v)
                        }
                        Err(e) => Listing::Error(e),
                    });
                }
                s.open = opened.into_iter().collect();
                s.capped = cap;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let dirs: Vec<String> = std::iter::once(String::new()).chain(self.open.iter().cloned()).collect();
        for d in dirs {
            self.fetch(d, true, cx);
        }
    }

    fn toggle(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.open.remove(&path) {
            self.open.insert(path.clone());
            self.fetch(path, false, cx);
        }
        cx.notify();
    }

    fn open_file(&self, rel: &str, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        let abs = std::path::Path::new(&root).join(rel).to_string_lossy().into_owned();
        // membuka tab dokumen dan memunculkan Panel 3 bila terlipat (seperti Tauri: panels.reveal("docs"))
        super::agent_control::open_documents(vec![abs], cx);
    }

    fn start_create(&mut self, parent: String, dir: bool, window: &mut Window, cx: &mut Context<Self>) {
        let ph = cx.global::<I18n>().t(if dir { "files.nameFolder" } else { "files.nameFile" });
        self.name.update(cx, |n, cx| {
            n.set_placeholder(ph);
            n.set_text("", cx);
        });
        if !parent.is_empty() {
            self.open.insert(parent.clone());
            self.fetch(parent.clone(), false, cx);
        }
        self.creating = Some((parent, dir));
        let fh = self.name.read(cx).focus_handle(cx);
        window.focus(&fh, cx);
        cx.notify();
    }

    fn confirm_create(&mut self, cx: &mut Context<Self>) {
        let (Some((parent, dir)), Some(root)) = (self.creating.clone(), self.root.clone()) else { return };
        let value = self.name.read(cx).text().trim().to_string();
        if value.is_empty() {
            self.creating = None;
            cx.notify();
            return;
        }
        match crate::files::create_entry(&std::path::Path::new(&root).join(&parent), &value, dir) {
            Ok(_) => {
                self.fetch(parent.clone(), true, cx);
                let made = join(&parent, &value);
                self.creating = None;
                if dir {
                    self.open.insert(made);
                } else {
                    self.open_file(&made, cx);
                }
            }
            Err(e) => {
                let title = cx.global::<I18n>().tp("files.createFail", &[("error", &e)]);
                Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(title) }, cx);
            }
        }
        cx.notify();
    }

    fn note(t: &Theme, text: String, pad: f32) -> AnyElement {
        div().pl(t.rem(pad)).py(t.space(1)).small(t).text_color(t.colors.fg_subtle).child(text).into_any_element()
    }

    fn create_row(&self, t: &Theme, parent: &str, depth: usize) -> Option<AnyElement> {
        match &self.creating {
            Some((p, _)) if p == parent => Some(div().pl(t.rem(0.75 * depth as f32 + 0.25 + 1.))).map(|d| d.pr(t.space(2)).py(t.space(0)).child(self.name.clone()).into_any_element()),
            _ => None,
        }
    }

    fn dir_rows(&self, t: &Theme, i: &I18n, path: &str, depth: usize, out: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let pad = 0.75 * depth as f32 + 0.5;
        let nodes = match self.listings.get(path) {
            None => {
                out.push(Self::note(t, format!("… {}", i.t("sb.loading")), pad));
                return;
            }
            Some(Listing::Loading(v)) if v.is_empty() => {
                out.push(Self::note(t, format!("… {}", i.t("sb.loading")), pad));
                return;
            }
            Some(Listing::Error(e)) => {
                out.push(div().pl(t.rem(pad)).py(t.space(1)).small(t).text_color(t.colors.danger).child(i.tp("files.error", &[("error", e)])).into_any_element());
                return;
            }
            Some(Listing::Loading(v) | Listing::Ready(v)) => v.clone(),
        };
        let creating_here = matches!(&self.creating, Some((p, _)) if p == path);
        if nodes.is_empty() && !creating_here {
            out.push(Self::note(t, i.t("files.empty"), pad));
            return;
        }
        out.extend(self.create_row(t, path, depth));
        for n in nodes {
            let open = self.open.contains(&n.path);
            let p = n.path.clone();
            let is_dir = n.is_dir;
            let mut row = div()
                .id(SharedString::from(format!("fn-{}", n.path)))
                .group("frow")
                .flex()
                .items_center()
                .gap(t.space(1))
                .h(t.rem(rem::CONTROL_SM))
                .pl(t.rem(0.25))
                .pr(t.space(1))
                .rounded(t.rem(rem::RADIUS_SM))
                .cursor_pointer()
                .when(n.ignored, |d| d.opacity(0.6))
                .hover(|s| s.bg(t.colors.hover))
                .on_click(cx.listener(move |s, _, _, cx| {
                    if is_dir {
                        s.toggle(p.clone(), cx);
                    } else {
                        s.open_file(&p, cx);
                    }
                }))
                // garis panduan: satu segmen per tingkat, tampak hanya saat pohon di-hover (.ftree__tree:hover)
                .children((0..depth).map(|_| div().flex_none().self_stretch().w(t.rem(0.75)).border_l_1().border_color(gpui::transparent_black()).group_hover("ftree-box", |s| s.border_color(t.colors.border))))
                .child(if is_dir {
                    icon(if open { "chevron_down" } else { "chevron_right" }).size(IconSize::Sm).color(t.colors.fg_subtle).into_any_element()
                } else {
                    div().w(t.rem(rem::ICON_SM)).into_any_element()
                })
                .child(super::file_icons::file_type_icon(t, &n.name, is_dir, open, rem::ICON_SM))
                .child(div().flex_1().min_w_0().truncate().child(n.name.clone()));
            if !is_dir {
                row = row.tooltip(crate::ui::tooltip::text_tooltip(n.path.clone().into()));
            } else {
                let (p1, p2) = (n.path.clone(), n.path.clone());
                row = row.child(
                    div()
                        .flex()
                        .invisible()
                        .group_hover("frow", |s| s.visible())
                        .child(icon_button(SharedString::from(format!("fnf-{}", n.path)), "file_plus", i.tp("files.newFileIn", &[("name", &n.name)])).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, w, cx| {
                            cx.stop_propagation();
                            s.start_create(p1.clone(), false, w, cx);
                        })))
                        .child(icon_button(SharedString::from(format!("fnd-{}", n.path)), "folder_plus", i.tp("files.newFolderIn", &[("name", &n.name)])).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, w, cx| {
                            cx.stop_propagation();
                            s.start_create(p2.clone(), true, w, cx);
                        }))),
                );
            }
            out.push(row.into_any_element());
            if is_dir && open {
                self.dir_rows(t, i, &n.path, depth + 1, out, cx);
            }
        }
    }
}

impl Render for FilesTree {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let (projects, sel_key, sel_sid, cur_name) = {
            let st = ProjectStore::global(cx).read(cx);
            let name = st.selection.key.as_ref().and_then(|k| st.projects.iter().find(|p| &p.id == k)).map(|p| p.name.clone());
            (st.projects.clone(), st.selection.key.clone(), st.selection.session_id.clone(), name)
        };
        let has = self.root.is_some();

        // ---- kepala ----
        let pmenu = self.project_menu.then(|| {
            let store = ProjectStore::global(cx);
            let entries: Vec<MenuEntry> = if projects.is_empty() {
                vec![entry(i.t("files.noneProject"), |_, _| {}).disabled(true)]
            } else {
                projects
                    .iter()
                    .map(|p| {
                        let (store, id) = (store.clone(), p.id.clone());
                        let sid = if sel_key.as_deref() == Some(p.id.as_str()) { sel_sid.clone() } else { None };
                        entry(p.name.clone(), move |_, cx| store.update(cx, |s, cx| s.select(Some(id.clone()), sid.clone(), cx))).checked(sel_key.as_deref() == Some(p.id.as_str()))
                    })
                    .collect()
            };
            let me = cx.entity().downgrade();
            div()
                .absolute()
                .top_full()
                .left_0()
                .child(anchored().child(menu_popup("ftree-pm", &t, entries, Anchor::TopLeft, move |_, cx| {
                    me.update(cx, |s, cx| {
                        s.project_menu = false;
                        cx.notify();
                    })
                    .ok();
                })))
        });
        let head = div()
            .flex()
            .items_center()
            .gap(t.space(1))
            .px(t.space(2))
            .py(t.space(1))
            .child(
                div().relative().flex_1().min_w_0().child(
                    button("ftree-project")
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .tooltip_text(i.t("files.switchProject"))
                        .child(icon("folder").size(IconSize::Sm))
                        .child(div().min_w_0().truncate().child(cur_name.unwrap_or_else(|| i.t("files.pickProject"))))
                        .child(icon("chevron_down").size(IconSize::Sm))
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.project_menu = !s.project_menu;
                            cx.notify();
                        })),
                )
                .children(pmenu),
            )
            .child(icon_button("ftree-newfile", "file_plus", i.t("files.newFile")).size(ControlSize::Sm).disabled(!has).on_click(cx.listener(|s, _, w, cx| s.start_create(String::new(), false, w, cx))))
            .child(icon_button("ftree-newdir", "folder_plus", i.t("files.newFolder")).size(ControlSize::Sm).disabled(!has).on_click(cx.listener(|s, _, w, cx| s.start_create(String::new(), true, w, cx))))
            .child(icon_button("ftree-collapse", "collapse_all", i.t("files.collapseAll")).size(ControlSize::Sm).disabled(!has || self.open.is_empty()).on_click(cx.listener(|s, _, _, cx| {
                s.open.clear();
                s.capped = false;
                cx.notify();
            })))
            .child(icon_button("ftree-expand", "expand_all", i.t("files.expandAll")).size(ControlSize::Sm).disabled(!has).on_click(cx.listener(|s, _, _, cx| s.expand_all(cx))))
            .child(icon_button("ftree-refresh", "refresh", i.t("files.refresh")).size(ControlSize::Sm).disabled(!has).on_click(cx.listener(|s, _, _, cx| s.refresh(cx))));

        let mut col = div().flex().flex_col().gap(t.space(1)).size_full().min_h_0().child(head);
        if !has {
            return col.child(Self::note(&t, i.t("files.noProject"), 0.75)).into_any_element();
        }
        col = col.child(div().px(t.space(2)).pb(t.space(1)).child(self.query.clone()));

        let q = self.query.read(cx).text().trim().to_lowercase();
        let mut rows: Vec<AnyElement> = vec![];
        if !q.is_empty() {
            let mut matches: Vec<(FileNode, String)> = vec![];
            for (dir, l) in &self.listings {
                if let Listing::Ready(v) | Listing::Loading(v) = l {
                    for n in v {
                        if n.name.to_lowercase().contains(&q) {
                            matches.push((n.clone(), dir.clone()));
                        }
                    }
                }
            }
            matches.sort_by(|a, b| a.0.path.cmp(&b.0.path));
            matches.truncate(200);
            if matches.is_empty() {
                rows.push(Self::note(&t, i.t("files.noMatch"), 0.5));
            }
            for (n, dir) in matches {
                let (p, is_dir) = (n.path.clone(), n.is_dir);
                rows.push(
                    div()
                        .id(SharedString::from(format!("fm-{}", n.path)))
                        .flex()
                        .items_center()
                        .gap(t.space(1))
                        .h(t.rem(rem::CONTROL_SM))
                        .px(t.space(1))
                        .rounded(t.rem(rem::RADIUS_SM))
                        .cursor_pointer()
                        .hover(|s| s.bg(t.colors.hover))
                        .tooltip(crate::ui::tooltip::text_tooltip(n.path.clone().into()))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            if !is_dir {
                                return s.open_file(&p, cx);
                            }
                            s.query.update(cx, |q, cx| q.set_text("", cx));
                            s.searched = false;
                            s.open.insert(p.clone());
                            s.fetch(p.clone(), false, cx);
                            cx.notify();
                        }))
                        .child(super::file_icons::file_type_icon(&t, &n.name, is_dir, false, rem::ICON_SM))
                        .child(div().min_w_0().truncate().child(n.name.clone()))
                        .when(!dir.is_empty(), |d| d.child(div().flex_1().min_w_0().truncate().small(&t).text_color(t.colors.fg_subtle).child(dir.clone())))
                        .into_any_element(),
                );
            }
        } else {
            self.dir_rows(&t, &i, "", 0, &mut rows, cx);
        }
        // kotak pohon: border lembut + sudut besar, mengisi sisa tinggi dan menggulir sendiri (.ftree__tree)
        col = col.child(div().group("ftree-box").flex().flex_col().flex_1().min_h_0().mx(t.space(2)).mb(t.space(2)).border_1().border_color(t.colors.border_soft).rounded(t.rem(rem::RADIUS_LG)).child(div().id("ftree-tree").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().p(t.space(2)).children(rows)));
        if self.capped {
            col = col.child(Self::note(&t, i.tp("files.partial", &[("max", &MAX_EXPAND.to_string())]), 0.75));
        }
        col.into_any_element()
    }
}

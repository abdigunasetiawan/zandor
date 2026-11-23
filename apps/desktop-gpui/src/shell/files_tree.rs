//! Tab Files — port FilesTree.tsx: pohon berkas nyata dari folder kerja aktif (project atau folder sesi Chat), dimuat
//! malas per folder lewat engine (GET /file?path=). Kepala: nama project (ganti project), berkas/folder baru, tutup/buka
//! semua, muat ulang; kolom cari (membuka semua folder sekali, dibatasi MAX_EXPAND). Klik berkas membukanya di Panel 3.
use crate::core::engine::FileNode;
use crate::core::projects::ProjectStore;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, entry, icon, icon_button, menu_popup, ButtonVariant, ControlSize, IconSize, InputBlur, InputChanged, InputSubmit, MenuEntry, TextInput, Toasts, ToastInput, ToastTone, TypeExt};
use crate::core::fs_ops;
use gpui::{anchored, div, prelude::*, AnyElement, Anchor, ClipboardEntry, ClipboardItem, Context, Entity, ExternalPaths, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point, SharedString, Window};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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

/// Entri yang sedang diseret di dalam pohon (juga jadi pratinjau seret).
#[derive(Clone)]
struct DragEntry {
    abs: PathBuf,
    label: SharedString,
}

impl Render for DragEntry {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        div().max_w(t.rem(20.)).truncate().px(t.space(2)).py(t.space(1)).rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.raised).border_1().border_color(t.colors.border).shadow(t.shadow_2()).child(self.label.clone())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Copy,
    Move,
}

/// Papan klip internal pohon: jalur mutlak + apakah operasinya "potong".
struct Clip {
    paths: Vec<PathBuf>,
    cut: bool,
}

/// Bentuk baku jalur relatif untuk dibandingkan: pemisah "/", tanpa pemisah di ujung.
fn norm(rel: &str) -> String {
    rel.replace('\\', "/").trim_end_matches('/').to_string()
}

fn parent_of(rel: &str) -> String {
    norm(rel).rsplit_once('/').map(|(a, _)| a.to_string()).unwrap_or_default()
}

/// Berkas yang ada di papan klip sistem (mis. disalin dari File Explorer).
fn clipboard_files(cx: &gpui::App) -> Vec<PathBuf> {
    cx.read_from_clipboard()
        .map(|c| c.entries().iter().filter_map(|e| if let ClipboardEntry::ExternalPaths(p) = e { Some(p.paths().to_vec()) } else { None }).flatten().collect())
        .unwrap_or_default()
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
    /// Entri terpilih (jalur relatif, folder?) — sasaran salin/potong dan folder tujuan tempel/unggah.
    selected: Option<(String, bool)>,
    focus: FocusHandle,
    clip: Option<Clip>,
    /// Menu klik-kanan terbuka di posisi ini; menu unggah (tombol kepala).
    ctx: Option<Point<Pixels>>,
    upload_menu: bool,
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
        let mut me = Self { root: None, engine_ready: false, listings: HashMap::new(), open: HashSet::new(), query, searched: false, capped: false, creating: None, name, project_menu: false, selected: None, focus: cx.focus_handle(), clip: None, ctx: None, upload_menu: false };
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
        self.selected = None;
        self.ctx = None;
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

    /// Kunci listing yang sudah ada untuk `rel` (bentuk aslinya dari engine), atau bentuk bakunya bila belum ada.
    fn key_for(&self, rel: &str) -> String {
        let n = norm(rel);
        if n.is_empty() {
            return String::new();
        }
        self.listings.keys().chain(self.open.iter()).find(|k| norm(k) == n).cloned().unwrap_or(n)
    }

    fn abs_of(&self, rel: &str) -> Option<PathBuf> {
        if !fs_ops::safe_rel(rel) {
            return None;
        }
        Some(Path::new(self.root.as_ref()?).join(rel))
    }

    /// Folder tujuan tempel/unggah: folder terpilih, induk berkas terpilih, atau akar project.
    fn target_dir(&self) -> String {
        match &self.selected {
            Some((p, true)) => self.key_for(p),
            Some((p, false)) => self.key_for(&parent_of(p)),
            None => String::new(),
        }
    }

    fn select(&mut self, rel: String, is_dir: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some((rel, is_dir));
        self.ctx = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn copy_selected(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Some(abs) = self.selected.as_ref().and_then(|(rel, _)| self.abs_of(rel)) else { return };
        // Jalur ditulis juga ke papan klip sistem sebagai teks: ini menimpa berkas Explorer yang disalin sebelumnya,
        // sehingga "Tempel" tidak salah memakai papan klip yang lebih lama.
        cx.write_to_clipboard(ClipboardItem::new_string(abs.to_string_lossy().into_owned()));
        self.clip = Some(Clip { paths: vec![abs], cut });
        self.ctx = None;
        cx.notify();
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        self.ctx = None;
        let dest = self.target_dir();
        let ext = clipboard_files(cx);
        if !ext.is_empty() {
            return self.transfer(ext, dest, Mode::Copy, cx);
        }
        if let Some(c) = self.clip.as_ref() {
            let (paths, cut) = (c.paths.clone(), c.cut);
            if cut {
                self.clip = None; // potong → tempel hanya sekali
            }
            return self.transfer(paths, dest, if cut { Mode::Move } else { Mode::Copy }, cx);
        }
        let title = cx.global::<I18n>().t("files.nothingToPaste");
        Toasts::show(ToastInput::new(title), cx);
    }

    /// Dialog pilih berkas/folder dari komputer lalu salin ke folder tujuan saat ini.
    fn pick(&mut self, folder: bool, cx: &mut Context<Self>) {
        self.upload_menu = false;
        self.ctx = None;
        let dest = self.target_dir();
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: !folder, directories: folder, multiple: true, prompt: None });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            this.update(cx, |s, cx| s.transfer(paths, dest, Mode::Copy, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    /// Salin/pindahkan `items` ke folder `dest_rel` (relatif ke akar) di thread latar, lalu segarkan pohon.
    fn transfer(&mut self, items: Vec<PathBuf>, dest_rel: String, mode: Mode, cx: &mut Context<Self>) {
        let Some(dest_abs) = self.abs_of(&dest_rel) else { return };
        if items.is_empty() {
            return;
        }
        let task = cx.background_spawn(async move {
            items
                .into_iter()
                .map(|src| match mode {
                    Mode::Copy => fs_ops::copy_entry(&src, &dest_abs),
                    Mode::Move => fs_ops::move_entry(&src, &dest_abs),
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let results = task.await;
            this.update(cx, |s, cx| s.finish_transfer(results, dest_rel, mode, cx)).ok();
        })
        .detach();
    }

    fn finish_transfer(&mut self, results: Vec<fs_ops::Outcome>, dest_rel: String, mode: Mode, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        let done = results.iter().filter(|r| matches!(r, Ok(Some(_)))).count();
        if let Some(e) = results.iter().find_map(|r| r.as_ref().err()) {
            Toasts::show(ToastInput { tone: ToastTone::Danger, ..ToastInput::new(i.tp("files.transferFail", &[("error", e)])) }, cx);
        }
        if done > 0 {
            let dest = if dest_rel.is_empty() {
                self.root.as_deref().and_then(|r| Path::new(r).file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            } else {
                norm(&dest_rel).rsplit('/').next().unwrap_or_default().to_string()
            };
            let key = if mode == Mode::Move { "files.moved" } else { "files.copied" };
            Toasts::show(ToastInput { tone: ToastTone::Success, ..ToastInput::new(i.tp(key, &[("n", &done.to_string()), ("dest", &dest)])) }, cx);
            if !dest_rel.is_empty() {
                self.open.insert(dest_rel.clone());
            }
            if mode == Mode::Move {
                self.refresh(cx); // sumber dan tujuan berubah
            } else {
                self.fetch(dest_rel, true, cx);
            }
        }
        cx.notify();
    }

    fn context_menu(&self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pos = self.ctx?;
        let me = cx.entity().downgrade();
        let act = |f: fn(&mut Self, &mut Context<Self>)| {
            let me = me.clone();
            move |_: &mut Window, cx: &mut gpui::App| {
                me.update(cx, |s, cx| f(s, cx)).ok();
            }
        };
        let has_sel = self.selected.is_some();
        let can_paste = self.clip.is_some() || !clipboard_files(cx).is_empty();
        let entries = vec![
            entry(i.t("files.copy"), act(|s, cx| s.copy_selected(false, cx))).icon("copy").disabled(!has_sel),
            entry(i.t("files.cut"), act(|s, cx| s.copy_selected(true, cx))).icon("scissors").disabled(!has_sel),
            entry(i.t("files.paste"), act(|s, cx| s.paste(cx))).icon("clipboard_paste").disabled(!can_paste),
            MenuEntry::Separator,
            entry(i.t("files.uploadFiles"), act(|s, cx| s.pick(false, cx))).icon("upload"),
            entry(i.t("files.uploadFolder"), act(|s, cx| s.pick(true, cx))).icon("upload"),
        ];
        let me2 = cx.entity().downgrade();
        Some(
            div()
                .absolute()
                .child(anchored().position(pos).child(menu_popup("ftree-ctx", t, entries, Anchor::TopLeft, move |_, cx| {
                    me2.update(cx, |s, cx| {
                        s.ctx = None;
                        cx.notify();
                    })
                    .ok();
                })))
                .into_any_element(),
        )
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
        let active_bg = t.colors.active;
        for n in nodes {
            let open = self.open.contains(&n.path);
            let p = n.path.clone();
            let is_dir = n.is_dir;
            let selected = self.selected.as_ref().is_some_and(|(sp, _)| *sp == n.path);
            let abs = self.abs_of(&n.path).unwrap_or_default();
            let is_cut = self.clip.as_ref().is_some_and(|c| c.cut && c.paths.contains(&abs));
            // Lepas di folder → ke folder itu; lepas di berkas → ke folder induknya.
            let drop_to = if is_dir { n.path.clone() } else { self.key_for(&parent_of(&n.path)) };
            let (drop_a, drop_b, ctx_p) = (drop_to.clone(), drop_to.clone(), n.path.clone());
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
                .when(n.ignored || is_cut, |d| d.opacity(if is_cut { 0.5 } else { 0.6 }))
                .when(selected, |d| d.bg(t.colors.active))
                .hover(|s| s.bg(t.colors.hover))
                .on_click(cx.listener(move |s, _, w, cx| {
                    s.select(p.clone(), is_dir, w, cx);
                    if is_dir {
                        s.toggle(p.clone(), cx);
                    } else {
                        s.open_file(&p, cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |s, e: &MouseDownEvent, w, cx| {
                        s.select(ctx_p.clone(), is_dir, w, cx);
                        s.ctx = Some(e.position);
                        cx.stop_propagation();
                    }),
                )
                .on_drag(DragEntry { abs: abs.clone(), label: n.name.clone().into() }, |d, _, _, cx| cx.new(|_| d.clone()))
                .when(is_dir, |d| d.drag_over::<DragEntry>(move |st, _, _, _| st.bg(active_bg)).drag_over::<ExternalPaths>(move |st, _, _, _| st.bg(active_bg)))
                .on_drop(cx.listener(move |s, d: &DragEntry, w, cx| {
                    let mode = if w.modifiers().control { Mode::Copy } else { Mode::Move };
                    s.transfer(vec![d.abs.clone()], drop_a.clone(), mode, cx);
                }))
                .on_drop(cx.listener(move |s, d: &ExternalPaths, _, cx| s.transfer(d.paths().to_vec(), drop_b.clone(), Mode::Copy, cx)))
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
        let umenu = self.upload_menu.then(|| {
            let (a, b, me) = (cx.entity().downgrade(), cx.entity().downgrade(), cx.entity().downgrade());
            let entries = vec![
                entry(i.t("files.uploadFiles"), move |_, cx| {
                    a.update(cx, |s, cx| s.pick(false, cx)).ok();
                })
                .icon("upload"),
                entry(i.t("files.uploadFolder"), move |_, cx| {
                    b.update(cx, |s, cx| s.pick(true, cx)).ok();
                })
                .icon("upload"),
            ];
            div().absolute().top_full().right_0().child(anchored().child(menu_popup("ftree-um", &t, entries, Anchor::TopRight, move |_, cx| {
                me.update(cx, |s, cx| {
                    s.upload_menu = false;
                    cx.notify();
                })
                .ok();
            })))
        });
        let head = div()
            .flex()
            .items_center()
            .gap(t.space(1))
            .px(t.space(3))
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
            .child(
                div()
                    .relative()
                    .child(icon_button("ftree-upload", "upload", i.t("files.upload")).size(ControlSize::Sm).disabled(!has).on_click(cx.listener(|s, _, _, cx| {
                        s.upload_menu = !s.upload_menu;
                        cx.notify();
                    })))
                    .children(umenu),
            )
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
        col = col.child(div().px(t.space(3)).pb(t.space(1)).child(self.query.clone()));

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
        let focus_c = t.colors.focus;
        col = col.child(div()
            .group("ftree-box")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, _, cx| {
                let m = &e.keystroke.modifiers;
                if s.creating.is_some() || !(m.control || m.platform) {
                    return;
                }
                match e.keystroke.key.as_str() {
                    "c" => s.copy_selected(false, cx),
                    "x" => s.copy_selected(true, cx),
                    "v" => s.paste(cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .on_mouse_down(MouseButton::Right, cx.listener(|s, e: &MouseDownEvent, _, cx| {
                s.selected = None;
                s.ctx = Some(e.position);
                cx.notify();
            }))
            .drag_over::<ExternalPaths>(move |st, _, _, _| st.border_color(focus_c))
            .drag_over::<DragEntry>(move |st, _, _, _| st.border_color(focus_c))
            .on_drop(cx.listener(|s, d: &ExternalPaths, _, cx| s.transfer(d.paths().to_vec(), String::new(), Mode::Copy, cx)))
            .on_drop(cx.listener(|s, d: &DragEntry, w, cx| {
                let mode = if w.modifiers().control { Mode::Copy } else { Mode::Move };
                s.transfer(vec![d.abs.clone()], String::new(), mode, cx);
            }))
            .flex().flex_col().flex_1().min_h_0().mx(t.space(3)).mb(t.space(3)).border_1().border_color(t.colors.border_soft).rounded(t.rem(rem::RADIUS_LG)).child(div().id("ftree-tree").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().p(t.space(1)).children(rows)));
        if self.capped {
            col = col.child(Self::note(&t, i.tp("files.partial", &[("max", &MAX_EXPAND.to_string())]), 0.75));
        }
        col.children(self.context_menu(&t, &i, cx)).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jalur_engine_windows_dinormalkan() {
        assert_eq!(norm("dirB\\"), "dirB");
        assert_eq!(norm("a/b/"), "a/b");
        assert_eq!(parent_of("dirB\\a.txt"), "dirB");
        assert_eq!(parent_of("a/b/c.txt"), "a/b");
        assert_eq!(parent_of("a.txt"), "");
    }
}

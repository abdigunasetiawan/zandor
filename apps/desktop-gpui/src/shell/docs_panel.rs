//! Panel Dokumen — port DocPanel.tsx + simple-viewers.tsx + office-viewers.tsx (bagian yang bisa dirender GPUI): tab
//! berkas terbuka, toolbar (render/mentah, bungkus baris, salin isi/path, muat ulang, buka di aplikasi bawaan, tampilkan
//! di Explorer), dan penampil per jenis: Markdown, kode bernomor, gambar/SVG, CSV/TSV (tabel), JSON (diformat), HTML
//! (sumber + buka di peramban), DOCX/XLSX/PPTX/ipynb (pembaca best-effort), PDF/media/biner (info + buka di aplikasi
//! lain — GPUI tidak punya mesin PDF/media).
use crate::core::doc_kinds::{classify, decode_text, default_render, file_name, always_raw, has_raw_mode, is_text_kind, parse_notebook, DocKind, NbOutput, MAX_TEXT_BYTES};
use crate::core::patch::{drop_hunk, format_hunks, parse_patch, reverse_apply, revert_hunk, Hunk};
use crate::core::office::{parse_docx, parse_pptx, parse_xlsx, DocxBlock};
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{button, icon, icon_button, markdown, ButtonVariant, ControlSize, IconSize, TypeExt};
use gpui::{div, prelude::*, AnyElement, App, AppContext, ClipboardItem, Context, Entity, FontWeight, Global, IntoElement, SharedString, Window};
use std::collections::{HashMap, HashSet};

const MAX_BYTES: u64 = 40 * 1024 * 1024;
const MAX_TABLE_ROWS: usize = 2000;

pub struct DocsState {
    pub open: Vec<String>,
    pub active: Option<String>,
    /// Diff tinjauan per berkas (ADR-075): ada → panel menampilkan DiffViewer.
    pub diffs: HashMap<String, String>,
}
struct Handle(Entity<DocsState>);
impl Global for Handle {}

impl DocsState {
    pub fn init(cx: &mut App) -> Entity<Self> {
        let e = cx.new(|_| Self { open: vec![], active: None, diffs: HashMap::new() });
        cx.set_global(Handle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<Handle>().0.clone()
    }
    pub fn open_file(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.open.contains(&path) {
            self.open.push(path.clone());
        }
        self.active = Some(path);
        cx.notify();
    }
    pub fn open_diff(&mut self, path: String, patch: String, cx: &mut Context<Self>) {
        self.diffs.insert(path.clone(), patch);
        self.open_file(path, cx);
    }
    pub fn set_diff(&mut self, path: &str, patch: Option<String>, cx: &mut Context<Self>) {
        match patch {
            Some(p) => self.diffs.insert(path.to_string(), p),
            None => self.diffs.remove(path),
        };
        cx.notify();
    }
    fn close(&mut self, path: &str, cx: &mut Context<Self>) {
        self.diffs.remove(path);
        self.open.retain(|p| p != path);
        if self.active.as_deref() == Some(path) {
            self.active = self.open.last().cloned();
        }
        cx.notify();
    }
}

struct Loaded {
    kind: DocKind,
    bytes: Result<std::sync::Arc<Vec<u8>>, String>,
    text: Option<(String, bool)>,
}

pub struct DocsPanel {
    state: Entity<DocsState>,
    cache: HashMap<String, Loaded>,
    raw: HashSet<String>,
    nowrap: HashSet<String>,
    sheet: HashMap<String, usize>,
    diff_at: HashMap<String, usize>,
    confirm_all: bool,
    /// Pratinjau lampiran (AttachmentPreview): hanya isi, tanpa tab dan toolbar.
    preview: bool,
    /// Editor per berkas yang sedang diedit (port docs.beginEdit).
    editors: HashMap<String, Entity<crate::ui::TextInput>>,
    /// Berkas yang baru dicoba diubah saat belum mode edit (indikator baca-saja) + fokus area tampilan.
    hint: Option<String>,
    view_focus: gpui::FocusHandle,
    /// Penampil PDF per berkas (perender bawaan Windows).
    pdfs: HashMap<String, Entity<super::pdf_view::PdfView>>,
    /// Mode pratinjau lampiran: nama berkas → path berkas sementara berisi byte-nya (untuk gambar/PDF/media).
    temp: HashMap<String, String>,
    /// Pemutar media untuk tab aktif saja (webview dibuang saat berpindah tab).
    media: HashMap<String, Entity<super::media_view::MediaView>>,
    /// Waktu ubah berkas saat dimuat (deteksi konflik saat menyimpan).
    mtimes: HashMap<String, Option<std::time::SystemTime>>,
    ask: Option<Ask>,
    tab_menu: Option<(String, gpui::Point<gpui::Pixels>)>,
    /// Zoom gambar per berkas (1.0 = pas); `actual` = ukuran asli.
    zoom: HashMap<String, f32>,
    actual: HashSet<String>,
    /// Bilah cari (port find widget Monaco): input + indeks temuan aktif.
    find: Option<Entity<crate::ui::TextInput>>,
    find_at: usize,
    body_scroll: gpui::ScrollHandle,
    /// Popover "Buka dengan": posisi + status daftar aplikasi.
    open_with: Option<(gpui::Point<gpui::Pixels>, OwLoad)>,
    /// Popover Kustomisasi (DocCustomize): kontrol gaya untuk penampil aktif.
    doc_style: Option<Entity<super::doc_style_section::DocStyleControls>>,
    customize: bool,
    look: DocLook,
}

/// Gaya Panel 3 yang sudah dihitung dari appearance.docs (port variabel CSS --z-doc-*).
#[derive(Clone, Default)]
struct DocLook {
    code_size: Option<f32>,
    code_lh: Option<f32>,
    code_font: Option<SharedString>,
    line_numbers: bool,
    whitespace: bool,
    minimap: bool,
    ligatures: bool,
    /// (latar, teks, teks redup) tema baca.
    theme: Option<(gpui::Hsla, gpui::Hsla, gpui::Hsla)>,
    prose_font: Option<SharedString>,
    prose_size: Option<f32>,
    prose_line: Option<f32>,
    prose_width: f32,
    table_size: Option<f32>,
    image_bg: String,
}

fn hex(s: &str) -> gpui::Hsla {
    crate::core::appearance::parse_color(s).unwrap_or(gpui::black())
}

impl DocLook {
    fn from(d: &serde_json::Map<String, serde_json::Value>) -> Self {
        use super::doc_style_section::doc_num;
        let has = |k: &str| d.contains_key(k);
        let font = |k: &str| d.get(k).and_then(|v| v.as_str()).map(|f| SharedString::from(crate::core::appearance::first_family(f)));
        let theme = match d.get("theme").and_then(|v| v.as_str()) {
            Some("paper") => Some((hex("#ffffff"), hex("#1f1f1f"), hex("#5c5c5c"))),
            Some("sepia") => Some((hex("#f4ecd8"), hex("#3b2f1e"), hex("#6b5a41"))),
            Some("night") => Some((hex("#0f1115"), hex("#d8dbe0"), hex("#9aa0a8"))),
            _ => None,
        };
        Self {
            code_size: has("codeSize").then(|| doc_num(d, "codeSize", 13.)),
            code_lh: has("codeLineHeight").then(|| doc_num(d, "codeLineHeight", 1.5)),
            code_font: font("codeFont"),
            line_numbers: d.get("lineNumbers").and_then(|v| v.as_bool()).unwrap_or(true),
            whitespace: d.get("whitespace").and_then(|v| v.as_bool()).unwrap_or(false),
            minimap: d.get("minimap").and_then(|v| v.as_bool()).unwrap_or(false),
            ligatures: d.get("ligatures").and_then(|v| v.as_bool()).unwrap_or(false),
            theme,
            prose_font: font("proseFont"),
            prose_size: has("proseSize").then(|| doc_num(d, "proseSize", 0.875)),
            prose_line: has("proseLine").then(|| doc_num(d, "proseLine", 1.55)),
            prose_width: doc_num(d, "proseWidth", 48.),
            table_size: has("tableSize").then(|| doc_num(d, "tableSize", 0.8125)),
            image_bg: d.get("imageBg").and_then(|v| v.as_str()).unwrap_or("auto").to_string(),
        }
    }

    /// Tema dengan warna teks tema baca (dipakai penampil Markdown/Office).
    fn themed(&self, t: &Theme) -> Theme {
        let mut t2 = t.clone();
        if let Some((_, fg, muted)) = self.theme {
            t2.colors.fg = fg;
            t2.colors.fg_muted = muted;
        }
        t2
    }

    /// Bungkus penampil prosa: lebar kolom baca, huruf, ukuran, tinggi baris, latar tema.
    fn prose(&self, t: &Theme, el: AnyElement) -> AnyElement {
        let inner = div()
            .w_full()
            .max_w(t.rem(self.prose_width))
            .mx_auto()
            .when_some(self.prose_font.clone(), |d, f| d.font_family(f))
            .when_some(self.prose_size, |d, s| d.text_size(t.rem(s)))
            .when_some(self.prose_line, |d, l| d.line_height(gpui::relative(l)))
            .child(el);
        div()
            .w_full()
            .when_some(self.theme, |d, (bg, fg, _)| d.bg(bg).text_color(fg))
            .child(inner)
            .into_any_element()
    }
}

#[derive(Clone)]
enum OwLoad {
    Loading,
    Ready(Vec<crate::openwith::OpenApp>),
    Error(String),
}

/// Dialog konfirmasi yang menunggu jawaban.
enum Ask {
    /// Buang perubahan lalu jalankan aksi (tutup/muat ulang/selesai edit).
    Discard { names: Vec<String>, then: DiscardThen },
    Conflict { path: String },
}
#[derive(Clone)]
enum DiscardThen {
    Close(Vec<String>),
    Reload(String),
    EndEdit(String),
}

impl DocsPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = DocsState::global(cx);
        // Muat ulang otomatis berkas aktif bila berubah di disk (mis. diedit agent) dan tidak sedang diedit.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(800)).await;
            // 1) tentukan berkas yang perlu dicek (murah, di thread UI)
            let target = this.update(cx, |s, cx| {
                let path = s.state.read(cx).active.clone()?;
                if s.preview || s.editors.contains_key(&path) || s.state.read(cx).diffs.contains_key(&path) {
                    return None;
                }
                let known = s.mtimes.get(&path).cloned().flatten()?;
                Some((path, known))
            });
            let Ok(target) = target else { break };
            let Some((path, known)) = target else { continue };
            // 2) syscall `metadata` di thread latar (drive jaringan/antivirus bisa lambat)
            let p2 = path.clone();
            let now = cx.background_spawn(async move { mtime(&p2) }).await;
            let Some(now) = now.filter(|n| *n != known) else { continue };
            // 3) berubah: buang cache lalu render ulang (render memuat ulang dan memperbarui mtimes)
            let ok = this
                .update(cx, |s, cx| {
                    if s.state.read(cx).active.as_deref() == Some(path.as_str()) && s.mtimes.get(&path).cloned().flatten() == Some(known) {
                        s.cache.remove(&path);
                        s.pdfs.remove(&path);
                        let _ = now;
                        cx.notify();
                    }
                })
                .is_ok();
            if !ok {
                break;
            }
        })
        .detach();
        cx.observe_global::<crate::theme::Theme>(|_, cx| cx.notify()).detach();
        cx.observe_global::<crate::i18n::I18n>(|_, cx| cx.notify()).detach();
        cx.observe(&crate::core::appearance::AppearanceStore::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe(&state, |s, st, cx| {
            let open = st.read(cx).open.clone();
            s.cache.retain(|k, _| open.contains(k));
            s.editors.retain(|k, _| open.contains(k));
            s.pdfs.retain(|k, _| open.contains(k));
            cx.notify();
        })
        .detach();
        Self { temp: HashMap::new(), state, cache: HashMap::new(), raw: HashSet::new(), nowrap: HashSet::new(), sheet: HashMap::new(), diff_at: HashMap::new(), confirm_all: false, preview: false, editors: HashMap::new(), hint: None, view_focus: cx.focus_handle(), pdfs: HashMap::new(), media: HashMap::new(), mtimes: HashMap::new(), ask: None, tab_menu: None, zoom: HashMap::new(), actual: HashSet::new(), find: None, find_at: 0, body_scroll: gpui::ScrollHandle::new(), open_with: None, doc_style: None, customize: false, look: DocLook::default() }
    }

    /// Penampil mandiri untuk byte lampiran (port AttachmentPreview.tsx → PreviewBody).
    pub fn preview(name: String, bytes: Vec<u8>, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| DocsState { open: vec![name.clone()], active: Some(name.clone()), diffs: HashMap::new() });
        let kind = classify(&name, Some(&bytes));
        let text = is_text_kind(kind).then(|| {
            let d = decode_text(&bytes, MAX_TEXT_BYTES);
            (d.text, d.truncated)
        });
        // Gambar/PDF/media dibaca dari path oleh penampilnya; `name` hanya nama berkas, bukan path nyata.
        // Tulis byte ke berkas sementara agar penampil punya path yang valid.
        let mut temp = HashMap::new();
        if matches!(kind, DocKind::Image | DocKind::Svg | DocKind::Pdf | DocKind::Audio | DocKind::Video) {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            name.hash(&mut h);
            bytes.len().hash(&mut h);
            let ext = std::path::Path::new(&name).extension().and_then(|e| e.to_str()).filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or("bin");
            let dir = std::env::temp_dir().join("zandor-preview");
            let file = dir.join(format!("{:016x}.{ext}", h.finish()));
            if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&file, &bytes).is_ok() {
                temp.insert(name.clone(), file.to_string_lossy().into_owned());
            }
        }
        let mut cache = HashMap::new();
        cache.insert(name, Loaded { kind, bytes: Ok(std::sync::Arc::new(bytes)), text });
        Self { temp, state, cache, raw: HashSet::new(), nowrap: HashSet::new(), sheet: HashMap::new(), diff_at: HashMap::new(), confirm_all: false, preview: true, editors: HashMap::new(), hint: None, view_focus: cx.focus_handle(), pdfs: HashMap::new(), media: HashMap::new(), mtimes: HashMap::new(), ask: None, tab_menu: None, zoom: HashMap::new(), actual: HashSet::new(), find: None, find_at: 0, body_scroll: gpui::ScrollHandle::new(), open_with: None, doc_style: None, customize: false, look: DocLook::default() }
    }

    fn load(path: &str) -> Loaded {
        let bytes = crate::files::read_limited(std::path::Path::new(path), MAX_BYTES).map(std::sync::Arc::new);
        let kind = classify(path, bytes.as_ref().ok().map(|b| b.as_slice()));
        let text = match &bytes {
            Ok(b) if is_text_kind(kind) => {
                let d = decode_text(b, MAX_TEXT_BYTES);
                Some((d.text, d.truncated))
            }
            _ => None,
        };
        Loaded { kind, bytes, text }
    }
}

fn mtime(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn toast_ok(cx: &mut App, title: String) {
    crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Success, timeout_ms: Some(1500), ..crate::ui::ToastInput::new(title) }, cx);
}
fn toast_err(cx: &mut App, title: String) {
    crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(title) }, cx);
}

impl DocsPanel {
    /// Alasan berkas tidak bisa diedit (kunci doc.noEdit.*), atau None.
    fn edit_block(&self, path: &str) -> Option<&'static str> {
        let Some(l) = self.cache.get(path) else { return Some("notLoaded") };
        if !is_text_kind(l.kind) {
            return Some("binary");
        }
        let Ok(b) = &l.bytes else { return Some("notLoaded") };
        if l.text.as_ref().is_some_and(|x| x.1) {
            return Some("truncated");
        }
        if std::str::from_utf8(b).is_err() {
            return Some("encoding");
        }
        None
    }

    pub fn is_dirty(&self, path: &str, cx: &App) -> bool {
        let Some(e) = self.editors.get(path) else { return false };
        let orig = self.cache.get(path).and_then(|l| l.text.as_ref()).map(|x| x.0.as_str()).unwrap_or("");
        e.read(cx).text().as_ref() != orig
    }

    fn begin_edit(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(why) = self.edit_block(path) {
            let i = cx.global::<I18n>().clone();
            toast_err(cx, i.t(&format!("doc.noEdit.{why}")));
            return;
        }
        let text = self.cache[path].text.clone().map(|x| x.0).unwrap_or_default();
        let lang_path = path.to_string();
        let ed = cx.new(|cx| {
            let mut ti = crate::ui::TextInput::new(cx, "");
            ti.multiline = true;
            ti.enter_newline = true;
            ti.bare = true;
            ti.lang = Some(crate::ui::highlight::lang_of_path(&lang_path));
            ti.set_text(text, cx);
            ti
        });
        cx.observe(&ed, |_, _, cx| cx.notify()).detach();
        self.editors.insert(path.to_string(), ed);
        self.hint = None;
        let p = path.to_string();
        self.state.update(cx, |d, cx| {
            d.diffs.remove(&p);
            cx.notify();
        });
        cx.notify();
    }

    /// Tampilkan indikator "baca saja" beberapa detik (pengguna mencoba mengubah di luar mode edit).
    fn show_hint(&mut self, path: &str, cx: &mut Context<Self>) {
        self.hint = Some(path.to_string());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(5)).await;
            this.update(cx, |s, cx| {
                s.hint = None;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn end_edit(&mut self, path: &str, cx: &mut Context<Self>) {
        self.editors.remove(path);
        cx.notify();
    }

    /// Simpan isi editor; `force` = timpa walau berkas berubah di disk sejak dibuka.
    fn save(&mut self, path: &str, force: bool, cx: &mut Context<Self>) {
        let Some(ed) = self.editors.get(path) else { return };
        let text = ed.read(cx).text().to_string();
        let i = cx.global::<I18n>().clone();
        let known = self.mtimes.get(path).cloned().flatten();
        if !force && known.is_some() && mtime(path) != known {
            self.ask = Some(Ask::Conflict { path: path.to_string() });
            cx.notify();
            return;
        }
        match std::fs::write(path, text.as_bytes()) {
            Ok(_) => {
                if let Some(l) = self.cache.get_mut(path) {
                    l.text = Some((text.clone(), false));
                    l.bytes = Ok(std::sync::Arc::new(text.into_bytes()));
                }
                self.mtimes.insert(path.to_string(), mtime(path));
                toast_ok(cx, i.t("doc.savedEdit"));
            }
            Err(e) => toast_err(cx, i.tp("sb.error", &[("error", &e.to_string())])),
        }
        cx.notify();
    }

    /// Jalankan aksi; bila ada perubahan belum disimpan, minta konfirmasi dulu.
    fn guarded(&mut self, then: DiscardThen, cx: &mut Context<Self>) {
        let paths = match &then {
            DiscardThen::Close(p) => p.clone(),
            DiscardThen::Reload(p) | DiscardThen::EndEdit(p) => vec![p.clone()],
        };
        let names: Vec<String> = paths.iter().filter(|p| self.is_dirty(p, cx)).map(|p| file_name(p).to_string()).collect();
        if names.is_empty() {
            self.run(then, cx);
        } else {
            self.ask = Some(Ask::Discard { names, then });
            cx.notify();
        }
    }

    fn run(&mut self, then: DiscardThen, cx: &mut Context<Self>) {
        match then {
            DiscardThen::Close(paths) => {
                for p in paths {
                    self.editors.remove(&p);
                    self.state.update(cx, |d, cx| d.close(&p, cx));
                }
            }
            DiscardThen::Reload(p) => {
                self.editors.remove(&p);
                self.pdfs.remove(&p);
                self.cache.remove(&p);
            }
            DiscardThen::EndEdit(p) => self.end_edit(&p, cx),
        }
        cx.notify();
    }

    /// Tutup beberapa tab (one/others/right/left/all) relatif ke `anchor`.
    fn close_many(&mut self, mode: &str, anchor: &str, cx: &mut Context<Self>) {
        let open = self.state.read(cx).open.clone();
        let at = open.iter().position(|p| p == anchor).unwrap_or(0);
        let targets: Vec<String> = open
            .iter()
            .enumerate()
            .filter(|(n, p)| match mode {
                "one" => p.as_str() == anchor,
                "others" => p.as_str() != anchor,
                "right" => *n > at,
                "left" => *n < at,
                _ => true,
            })
            .map(|(_, p)| p.clone())
            .collect();
        self.guarded(DiscardThen::Close(targets), cx);
        // tab yang diklik kanan tetap aktif bila masih ada
        if mode != "one" && mode != "all" && self.ask.is_none() && self.state.read(cx).open.iter().any(|p| p == anchor) {
            let a = anchor.to_string();
            self.state.update(cx, |d, cx| d.open_file(a, cx));
        }
    }

    fn save_copy(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(Ok(bytes)) = self.cache.get(path).map(|l| l.bytes.clone()) else { return };
        let dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let rx = cx.prompt_for_new_path(&dir, Some(file_name(path)));
        cx.spawn(async move |_, cx| {
            let Ok(Ok(Some(dest))) = rx.await else { return };
            let r = std::fs::write(&dest, bytes.as_slice());
            cx.update(|cx| {
                let i = cx.global::<I18n>().clone();
                match r {
                    Ok(_) => toast_ok(cx, i.t("doc.saved")),
                    Err(e) => toast_err(cx, i.tp("sb.error", &[("error", &e.to_string())])),
                }
            })
            ;
        })
        .detach();
    }

    fn ask_dialog(&mut self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dismiss = cx.listener(|s: &mut Self, _: &gpui::ClickEvent, _, cx| {
            s.ask = None;
            cx.notify();
        });
        let cancel = button("doc-ask-no").label(i.t("ui.cancel")).on_click(cx.listener(|s, _, _, cx| {
            s.ask = None;
            cx.notify();
        }));
        let el = match self.ask.as_ref()? {
            Ask::Discard { names, then } => {
                let title = if names.len() == 1 { i.tp("doc.discardTitle", &[("name", &names[0])]) } else { i.tp("doc.tabsCloseConfirmTitle", &[("n", &names.len().to_string())]) };
                let body = if names.len() == 1 { i.t("doc.discardBody") } else { i.t("doc.tabsCloseConfirmBody") };
                let then = then.clone();
                crate::ui::dialog(title)
                    .description(body)
                    .on_dismiss(dismiss)
                    .footer(div().flex().gap(t.space(2)).child(cancel).child(button("doc-ask-yes").variant(ButtonVariant::Danger).label(i.t("doc.discard")).on_click(cx.listener(move |s, _, _, cx| {
                        s.ask = None;
                        s.run(then.clone(), cx);
                    }))))
                    .into_any_element()
            }
            Ask::Conflict { path } => {
                let p = path.clone();
                crate::ui::dialog(i.tp("doc.conflictTitle", &[("name", file_name(path))]))
                    .description(i.t("doc.conflictBody"))
                    .on_dismiss(dismiss)
                    .footer(div().flex().gap(t.space(2)).child(cancel).child(button("doc-cf-yes").variant(ButtonVariant::Danger).label(i.t("doc.overwrite")).on_click(cx.listener(move |s, _, _, cx| {
                        s.ask = None;
                        s.save(&p, true, cx);
                    }))))
                    .into_any_element()
            }
        };
        Some(el)
    }
}

fn note(t: &Theme, s: String) -> AnyElement {
    div().p(t.space(3)).small(t).text_color(t.colors.fg_muted).child(s).into_any_element()
}

fn code_view(t: &Theme, text: &str, wrap: bool, lang: &str) -> AnyElement {
    code_view_find(t, &DocLook { line_numbers: true, prose_width: 48., ..Default::default() }, text, wrap, "", usize::MAX, lang)
}

fn code_view_look(t: &Theme, look: &DocLook, text: &str, wrap: bool, lang: &str) -> AnyElement {
    code_view_find(t, look, text, wrap, "", usize::MAX, lang)
}

/// Posisi temuan (baris, kolom byte) untuk kueri tanpa membedakan huruf besar/kecil.
pub fn find_matches(text: &str, q: &str) -> Vec<(usize, usize)> {
    if q.is_empty() {
        return vec![];
    }
    let ql = q.to_lowercase();
    let mut out = vec![];
    for (n, l) in text.lines().take(50_000).enumerate() {
        let line = l.replace('\t', "    ").to_lowercase();
        let mut from = 0;
        while let Some(k) = line[from..].find(&ql) {
            out.push((n, from + k));
            from += k + ql.len().max(1);
        }
    }
    out
}

fn code_view_find(t: &Theme, look: &DocLook, text: &str, wrap: bool, q: &str, current: usize, lang: &str) -> AnyElement {
    let mut carry = crate::ui::highlight::Carry::None;
    let hits = find_matches(text, q);
    let ql = q.len();
    let lines: Vec<&str> = text.lines().collect();
    let w = lines.len().to_string().len().max(2);
    // minimap: ringkasan berkas berupa batang per baris (lebar ~ panjang baris, indentasi dipertahankan)
    let minimap = look.minimap.then(|| {
        let n = lines.len().max(1);
        let step = n.div_ceil(1500);
        div()
            .flex_none()
            .w(gpui::px(72.))
            .pt(t.space(3))
            .border_l_1()
            .border_color(t.colors.border)
            .flex()
            .flex_col()
            .children(lines.iter().step_by(step).map(|l| {
                let ind = l.len() - l.trim_start().len();
                let len = l.trim().chars().count();
                div().h(gpui::px(2.)).mb(gpui::px(1.)).pl(gpui::px((ind.min(40) as f32) * 1.2)).child(div().h_full().w(gpui::px((len.min(80) as f32) * 0.8)).bg(t.colors.fg_subtle.opacity(0.45)))
            }))
    });
    let body = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .p(t.space(3))
        .font_features(if look.ligatures { gpui::FontFeatures(std::sync::Arc::new(vec![("calt".into(), 1), ("liga".into(), 1)])) } else { gpui::FontFeatures::disable_ligatures() })
        .font_family(t.font_code.clone())
        .code(t)
        .when_some(look.code_font.clone(), |d, f| d.font_family(f))
        .when_some(look.code_size, |d, s| d.text_size(gpui::px(s * t.scale)))
        .when_some(look.code_lh, |d, l| d.line_height(gpui::relative(l)))
        .children(lines.into_iter().take(50_000).enumerate().map(|(n, l)| {
            let mut line = if l.is_empty() { " ".to_string() } else { l.replace('\t', "    ") };
            if look.whitespace && q.is_empty() {
                line = line.replace(' ', "\u{b7}");
            }
            let hl: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> = hits
                .iter()
                .enumerate()
                .filter(|(_, h)| h.0 == n && h.1 + ql <= line.len() && line.is_char_boundary(h.1) && line.is_char_boundary(h.1 + ql))
                .map(|(k, h)| (h.1..h.1 + ql, gpui::HighlightStyle { background_color: Some(if k == current { t.colors.warning.opacity(0.6) } else { t.colors.warning.opacity(0.25) }), ..Default::default() }))
                .collect();
            let syn = crate::ui::highlight::highlight_line(lang, &line, &mut carry, t);
            let hl = if hl.is_empty() { syn } else { hl };
            let content: AnyElement = if hl.is_empty() { line.into_any_element() } else { gpui::StyledText::new(line).with_highlights(hl).into_any_element() };
            div()
                .flex()
                .gap(t.space(3))
                .when(look.line_numbers, |d| d.child(div().flex_none().text_color(t.colors.code_ln.unwrap_or(t.colors.fg_subtle)).child(format!("{:>w$}", n + 1, w = w))))
                .child(if wrap { div().flex_1().min_w_0().child(content) } else { div().whitespace_nowrap().child(content) })
        }));
    div().flex().items_start().w_full().child(body).children(minimap).into_any_element()
}

/// Parse CSV/TSV sederhana (tanda kutip ganda, pemisah koma/tab).
pub fn parse_delimited(text: &str, sep: char) -> Vec<Vec<String>> {
    let mut rows = vec![];
    let mut row = vec![];
    let mut cell = String::new();
    let mut quoted = false;
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if quoted {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    cell.push('"');
                    it.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() => quoted = true,
            c if c == sep => row.push(std::mem::take(&mut cell)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            c => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn table(t: &Theme, rows: &[Vec<String>], header: bool) -> AnyElement {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    // Lebar kolom mengikuti isi (sampel 60 baris pertama), dibatasi agar tabel tetap rapi.
    let widths: Vec<f32> = (0..cols)
        .map(|ci| {
            let chars = rows.iter().take(60).filter_map(|r| r.get(ci)).map(|c| c.chars().count()).max().unwrap_or(0) as f32;
            (chars * 0.5 + 1.5).clamp(5., 22.)
        })
        .collect();
    let mut grid = div().flex().flex_col().border_1().border_color(t.colors.border).rounded(t.rem(rem::RADIUS_MD)).small(t);
    for (ri, r) in rows.iter().take(MAX_TABLE_ROWS).enumerate() {
        let mut line = div().flex().when(ri > 0, |d| d.border_t_1().border_color(t.colors.border)).when(header && ri == 0, |d| d.bg(t.colors.segment).font_weight(FontWeight::SEMIBOLD));
        for ci in 0..cols {
            line = line.child(div().w(t.rem(widths[ci])).flex_none().px(t.space(2)).py(t.space(1)).when(ci > 0, |d| d.border_l_1().border_color(t.colors.border)).truncate().child(r.get(ci).cloned().unwrap_or_default()));
        }
        grid = grid.child(line);
    }
    div().p(t.space(3)).child(grid).into_any_element()
}

fn data_img(url: &str, t: &Theme) -> Option<AnyElement> {
    crate::core::gradient::image_of(url).map(|img| gpui::img(img).max_w_full().max_h(t.rem(16.)).into_any_element())
}

/// 1 pt dokumen = 1/12 rem (halaman Letter 612 pt ≈ 51 rem).
const PT: f32 = 1. / 12.;

fn hex_color(h: &str) -> gpui::Hsla {
    gpui::rgb(u32::from_str_radix(h, 16).unwrap_or(0)).into()
}

/// Kertas dokumen: latar putih + teks hitam apa adanya (warna dokumen tidak mengikuti tema gelap).
fn docx_paper(t: &Theme, r: &crate::core::office::DocxResult) -> AnyElement {
    let pg = r.page.clone().unwrap_or(crate::core::office::DocxPage { width: 612., height: 792., left: 72., right: 72., top: 72., bottom: 72. });
    div()
        .w_full()
        .max_w(t.rem(pg.width * PT))
        .mx_auto()
        .flex()
        .flex_col()
        .rounded(t.rem(rem::RADIUS_SM))
        .bg(gpui::white())
        .text_color(gpui::black())
        .pl(t.rem(pg.left * PT))
        .pr(t.rem(pg.right * PT))
        .pt(t.rem(pg.top * PT))
        .pb(t.rem(pg.bottom * PT))
        .children(docx_view(t, &r.blocks))
        .into_any_element()
}

fn docx_view(t: &Theme, blocks: &[DocxBlock]) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = Vec::with_capacity(blocks.len());
    let mut counters: Vec<u32> = vec![];
    for b in blocks {
        match b {
            DocxBlock::P { style, list, runs, fmt } => {
                let first = runs.iter().find(|r| !r.text.trim().is_empty());
                let size = first.and_then(|r| r.size);
                let sz_pt = size.unwrap_or(11.);
                let has_image = runs.iter().any(|r| r.image.is_some());
                // teks digabung jadi satu StyledText (pembungkusan kata benar) dengan gaya per rentang
                let mut segs: Vec<AnyElement> = vec![];
                let mut text = String::new();
                let mut hl: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> = vec![];
                if let Some(f) = fmt.first.filter(|f| *f > 0.) {
                    text.push_str(&"\u{2003}".repeat((f / sz_pt).round().max(1.) as usize));
                }
                let flush = |text: &mut String, hl: &mut Vec<(std::ops::Range<usize>, gpui::HighlightStyle)>, segs: &mut Vec<AnyElement>| {
                    if !text.is_empty() {
                        segs.push(gpui::StyledText::new(std::mem::take(text)).with_highlights(std::mem::take(hl)).into_any_element());
                    }
                };
                for r in runs {
                    let s = r.text.replace('\t', "    ");
                    if !s.is_empty() {
                        let start = text.len();
                        text.push_str(&s);
                        let link = r.href.is_some();
                        hl.push((
                            start..text.len(),
                            gpui::HighlightStyle {
                                font_weight: r.bold.then_some(FontWeight::BOLD),
                                font_style: r.italic.then_some(gpui::FontStyle::Italic),
                                color: r.color.as_deref().map(hex_color).or_else(|| link.then(|| hex_color("0563C1"))),
                                background_color: r.highlight.as_deref().map(hex_color),
                                underline: (r.underline || link).then(|| gpui::UnderlineStyle { thickness: gpui::px(1.), color: None, wavy: false }),
                                strikethrough: r.strike.then(|| gpui::StrikethroughStyle { thickness: gpui::px(1.), color: None }),
                                ..Default::default()
                            },
                        ));
                    }
                    if let Some(img) = r.image.as_deref().and_then(|u| crate::core::gradient::image_of(u)) {
                        flush(&mut text, &mut hl, &mut segs);
                        let el = gpui::img(img);
                        segs.push(match r.image_size {
                            Some((w, h)) => el.w(t.rem(w * PT)).h(t.rem(h * PT)).max_w_full().flex_none().into_any_element(),
                            None => el.max_w_full().max_h(t.rem(16.)).into_any_element(),
                        });
                    }
                }
                flush(&mut text, &mut hl, &mut segs);

                let mut p = div().w_full().min_w_0();
                p = match (size, style.as_deref()) {
                    (None, Some("title")) => p.typo(t, crate::theme::H1),
                    (None, Some("h1")) => p.typo(t, crate::theme::H2),
                    (None, Some("h2")) => p.typo(t, crate::theme::H3),
                    (None, Some(_)) => p.typo(t, crate::theme::H4),
                    _ => p.text_size(t.rem(sz_pt * PT)),
                };
                if let Some(f) = first.and_then(|r| r.font.clone()) {
                    p = p.font_family(f);
                }
                p = p.line_height(gpui::relative(fmt.line.unwrap_or(1.) * 1.2));
                p = match fmt.align.as_deref() {
                    Some("center") => p.text_center(),
                    Some("right") => p.text_right(),
                    _ => p,
                };
                if let Some(v) = fmt.before {
                    p = p.mt(t.rem(v * PT));
                }
                if let Some(v) = fmt.after {
                    p = p.mb(t.rem(v * PT));
                }
                if let Some(c) = fmt.fill.as_deref() {
                    p = p.bg(hex_color(c));
                }
                if segs.is_empty() {
                    p = p.min_h(t.rem(sz_pt * PT * 1.2));
                } else if has_image {
                    p = p.flex().flex_wrap().items_end().when(fmt.align.as_deref() == Some("center"), |d| d.justify_center()).when(fmt.align.as_deref() == Some("right"), |d| d.justify_end()).children(segs);
                } else {
                    p = p.children(segs);
                }
                match list {
                    Some((lvl, ordered)) => {
                        let l = *lvl as usize;
                        let marker = if *ordered {
                            counters.truncate(l + 1);
                            counters.resize(l + 1, 0);
                            counters[l] += 1;
                            format!("{}.", counters[l])
                        } else {
                            counters.clear();
                            ["\u{2022}", "\u{25E6}", "\u{25AA}"][l.min(2)].to_string()
                        };
                        let left = fmt.left.unwrap_or(18. * (*lvl as f32 + 1.));
                        out.push(
                            div()
                                .w_full()
                                .flex()
                                .pl(t.rem(left * PT))
                                .text_size(t.rem(sz_pt * PT))
                                .child(div().flex_none().w(t.rem(18. * PT)).child(marker))
                                .child(div().flex_1().min_w_0().child(p))
                                .into_any_element(),
                        );
                    }
                    None => {
                        counters.clear();
                        out.push(p.when_some(fmt.left, |d, v| d.pl(t.rem(v * PT))).when_some(fmt.right, |d, v| d.pr(t.rem(v * PT))).into_any_element());
                    }
                }
            }
            DocxBlock::Table { rows, widths, grid } => {
                counters.clear();
                let total: f32 = widths.iter().sum();
                let bc = hex_color("8C8C8C");
                let mut g = div().w_full().flex().flex_col().when(*grid, |d| d.border_t_1().border_l_1().border_color(bc));
                for r in rows {
                    let mut line = div().flex().w_full();
                    let mut col = 0usize;
                    for c in r {
                        let span = c.span.max(1);
                        let w = if total > 0. { widths.iter().skip(col).take(span).sum::<f32>() / total } else { 0. };
                        col += span;
                        let mut cell = div().min_w_0().overflow_hidden().p(t.rem(4. * PT)).flex().flex_col().when(*grid, |d| d.border_r_1().border_b_1().border_color(bc)).when_some(c.fill.as_deref(), |d, f| d.bg(hex_color(f)));
                        cell = if w > 0. { cell.w(gpui::relative(w)).flex_none() } else { cell.flex_1() };
                        line = line.child(cell.children(docx_view(t, &c.blocks)));
                    }
                    g = g.child(line);
                }
                out.push(g.into_any_element());
            }
        }
    }
    out
}

fn human(n: usize) -> String {
    if n >= 1 << 20 {
        format!("{:.1} MB", n as f64 / (1 << 20) as f64)
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.)
    } else {
        format!("{n} B")
    }
}

impl DocsPanel {
    fn body(&mut self, t: &Theme, i: &I18n, path: &str, cx: &mut Context<Self>) -> AnyElement {
        let raw = self.raw.contains(path);
        let wrap = !self.nowrap.contains(path);
        let l = &self.cache[path];
        let bytes = match &l.bytes {
            Err(e) => return note(t, i.tp("doc.error", &[("error", e)])),
            Ok(b) => b.clone(),
        };
        let kind = l.kind;
        let truncated = l.text.as_ref().is_some_and(|x| x.1);
        let text = l.text.as_ref().map(|x| x.0.clone()).unwrap_or_default();
        let trunc = truncated.then(|| note(t, i.t("doc.truncated")));
        let el: AnyElement = if (raw || always_raw(kind)) && is_text_kind(kind) {
            let q = self.find.as_ref().map(|f| f.read(cx).text().to_string()).unwrap_or_default();
            code_view_find(t, &self.look, &text, wrap, &q, self.find_at, &crate::ui::highlight::lang_of_path(path))
        } else {
            match kind {
                DocKind::Markdown => {
                    let t2 = self.look.themed(t);
                    self.look.prose(t, div().p(t.space(3)).child(markdown(&text, &t2, i)).into_any_element())
                }
                DocKind::Image | DocKind::Svg => div()
                    .relative()
                    .p(t.space(3))
                    .when(self.look.image_bg == "light", |d| d.bg(hex("#ffffff")))
                    .when(self.look.image_bg == "dark", |d| d.bg(hex("#0f1115")))
                    .when(self.look.image_bg == "checker", |d| d.child(checker(t)))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(t.space(2))
                    .child({
                        let z = self.zoom.get(path).copied().unwrap_or(1.);
                        let actual = self.actual.contains(path);
                        let p = path.to_string();
                        let err = i.t("doc.imageError");
                        let im = gpui::img(std::path::PathBuf::from(self.temp.get(path).map(String::as_str).unwrap_or(path))).with_fallback(move || div().child(err.clone()).into_any_element());
                        let im = if actual || z != 1. {
                            // ukuran asli dikali zoom (piksel gambar → px logis)
                            match image_dims(&bytes) {
                                Some((w, h)) => im.w(gpui::px(w as f32 * z)).h(gpui::px(h as f32 * z)),
                                None => im.max_w_full(),
                            }
                        } else {
                            im.max_w_full().max_h(t.rem(40.))
                        };
                        div()
                            .id("doc-img")
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip::text_tooltip(i.t("doc.imageToggle").into()))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                if !s.actual.remove(&p) {
                                    s.actual.insert(p.clone());
                                }
                                s.zoom.remove(&p);
                                cx.notify();
                            }))
                            .child(im)
                    })
                    .child(div().small(t).text_color(t.colors.fg_subtle).child(i.tp("doc.size", &[("size", &human(bytes.len()))])))
                    .into_any_element(),
                DocKind::Csv | DocKind::Tsv => {
                    let rows = parse_delimited(&text, if kind == DocKind::Tsv { '\t' } else { ',' });
                    if rows.is_empty() {
                        note(t, i.t("doc.tableEmpty"))
                    } else {
                        let info = i.tp("doc.tableInfo", &[("rows", &rows.len().to_string()), ("cols", &rows.iter().map(|r| r.len()).max().unwrap_or(0).to_string())]);
                        let clipped = rows.len() > MAX_TABLE_ROWS;
                        div().flex().flex_col().child(note(t, info)).child(div().when_some(self.look.table_size, |d, sz| d.text_size(t.rem(sz))).child(table(t, &rows, true))).when(clipped, |d| d.child(note(t, i.tp("doc.tableClipped", &[("max", &MAX_TABLE_ROWS.to_string())])))).into_any_element()
                    }
                }
                DocKind::Json => match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(v) => div().when_some(self.look.table_size, |d, sz| d.text_size(t.rem(sz))).child(code_view_look(t, &self.look, &serde_json::to_string_pretty(&v).unwrap_or_default(), wrap, "json")).into_any_element(),
                    Err(e) => div().child(note(t, i.tp("doc.jsonInvalid", &[("error", &e.to_string())]))).child(code_view(t, &text, wrap, "json")).into_any_element(),
                },
                DocKind::Html => {
                    div()
                        .flex()
                        .flex_col()
                        .child(code_view_look(t, &self.look, &text, wrap, &crate::ui::highlight::lang_of_path(path)))
                        .into_any_element()
                }
                DocKind::Notebook => match parse_notebook(&text) {
                    Err(e) => note(t, e),
                    Ok(nb) => {
                        let mut col = div().flex().flex_col().gap(t.space(3)).p(t.space(3)).child(div().small(t).text_color(t.colors.fg_muted).child(i.t("doc.nbNote")));
                        for c in nb.cells {
                            let mut cell = div().flex().flex_col().gap(t.space(1));
                            cell = match c.kind {
                                "markdown" => cell.child(markdown(&c.source, t, i)),
                                _ => cell
                                    .child(div().small(t).text_color(t.colors.fg_subtle).child(format!("[{}]", c.execution_count.map(|n| n.to_string()).unwrap_or_else(|| " ".into()))))
                                    .child(div().rounded(t.rem(rem::RADIUS_MD)).bg(t.colors.segment).child(code_view(t, &c.source, true, "python"))),
                            };
                            for o in c.outputs {
                                cell = cell.child(match o {
                                    NbOutput::Text { text, stderr } => div().code(t).font_family(t.font_code.clone()).when(stderr, |d| d.text_color(t.colors.danger)).child(text).into_any_element(),
                                    NbOutput::Image { src } => data_img(&src, t).unwrap_or_else(|| div().into_any_element()),
                                    NbOutput::Error { name, message, traceback } => div().code(t).font_family(t.font_code.clone()).text_color(t.colors.danger).child(format!("{name}: {message}\n{traceback}")).into_any_element(),
                                    NbOutput::Skipped { reason } => note(t, reason),
                                });
                            }
                            col = col.child(cell);
                        }
                        if nb.truncated {
                            col = col.child(note(t, i.t("doc.truncated")));
                        }
                        col.into_any_element()
                    }
                },
                DocKind::Docx => match parse_docx(&bytes) {
                    Err(e) => note(t, i.tp("doc.officeFailed", &[("name", file_name(path)), ("error", &e)])),
                    Ok(r) => {
                        div()
                            .flex()
                            .flex_col()
                            .gap(t.space(2))
                            .p(t.space(3))
                            .child(div().small(t).text_color(t.colors.fg_muted).child(i.t("doc.officeBestEffort")))
                            .child(docx_paper(t, &r))
                            .when(r.truncated, |d| d.child(note(t, i.t("doc.officeTruncated"))))
                            .into_any_element()
                    }
                },
                DocKind::Xlsx => match parse_xlsx(&bytes) {
                    Err(e) => note(t, i.tp("doc.officeFailed", &[("name", file_name(path)), ("error", &e)])),
                    Ok(sheets) => {
                        let cur = self.sheet.get(path).copied().unwrap_or(0).min(sheets.len().saturating_sub(1));
                        let mut tabs = div().flex().flex_wrap().gap(t.space(1)).p(t.space(2));
                        for (n, s) in sheets.iter().enumerate() {
                            let p = path.to_string();
                            tabs = tabs.child(button(SharedString::from(format!("sheet-{n}"))).size(ControlSize::Sm).pressed(n == cur).label(s.name.clone()).on_click(cx.listener(move |me, _, _, cx| {
                                me.sheet.insert(p.clone(), n);
                                cx.notify();
                            })));
                        }
                        let body = match sheets.get(cur) {
                            Some(s) if !s.rows.is_empty() => div().child(table(t, &s.rows, false)).when(s.truncated, |d| d.child(note(t, i.t("doc.sheetClipped")))).into_any_element(),
                            _ => note(t, i.t("doc.sheetEmpty")),
                        };
                        div().flex().flex_col().child(note(t, i.t("doc.xlsxNote"))).child(tabs).child(body).into_any_element()
                    }
                },
                DocKind::Pptx => match parse_pptx(&bytes) {
                    Err(e) => note(t, i.tp("doc.officeFailed", &[("name", file_name(path)), ("error", &e)])),
                    Ok((slides, trunc)) => {
                        let mut col = div().flex().flex_col().gap(t.space(3)).p(t.space(3)).child(div().small(t).text_color(t.colors.fg_muted).child(i.t("doc.pptxNote")));
                        for s in slides {
                            let mut card = div().flex().flex_col().gap(t.space(1)).p(t.space(3)).border_1().border_color(t.colors.border).rounded(t.rem(rem::RADIUS_LG));
                            card = card.child(div().small(t).text_color(t.colors.fg_subtle).child(i.tp("doc.slideLabel", &[("n", &s.index.to_string())])));
                            if !s.title.is_empty() {
                                card = card.child(div().typo(t, crate::theme::H4).child(s.title));
                            }
                            for p in s.paragraphs {
                                card = card.child(div().child(p));
                            }
                            for tb in s.tables {
                                card = card.child(table(t, &tb, false));
                            }
                            for u in s.images {
                                if let Some(img) = data_img(&u, t) {
                                    card = card.child(img);
                                }
                            }
                            col = col.child(card);
                        }
                        if trunc {
                            col = col.child(note(t, i.t("doc.officeTruncated")));
                        }
                        col.into_any_element()
                    }
                },
                DocKind::Audio | DocKind::Video => {
                    let p = path.to_string();
                    self.media.retain(|k, _| *k == p);
                    let src = self.temp.get(path).cloned().unwrap_or_else(|| p.clone());
                    self.media.entry(p).or_insert_with(|| cx.new(|cx| super::media_view::MediaView::new(&src, cx))).clone().into_any_element()
                }
                DocKind::Pdf => {
                    let p = path.to_string();
                    let src = self.temp.get(path).cloned().unwrap_or_else(|| p.clone());
                    self.pdfs.entry(p).or_insert_with(|| cx.new(|cx| super::pdf_view::PdfView::new(src, cx))).clone().into_any_element()
                }
                _ => {
                    let p = path.to_string();
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(t.space(2))
                        .p(t.space(5))
                        .child(icon("file").size(IconSize::Lg))
                        .child(div().text_color(t.colors.fg_muted).child(i.tp("doc.binary", &[("name", file_name(path))])))
                        .child(button("doc-open-ext").size(ControlSize::Sm).label(i.t("doc.openExternal")).on_click(move |_, _, cx| cx.open_with_system(std::path::Path::new(&p))))
                        .into_any_element()
                }
            }
        };
        div().flex().flex_col().flex_1().min_h_0().children(trunc).child(el).into_any_element()
    }
}

/// Tombol tutup Panel 3 (Dokumen) di ujung kanan bilah atas — sama dengan Chat dan Tools.
fn close_panel_button(i: &I18n) -> gpui::AnyElement {
    icon_button("docs-close-panel", "close", i.t("panel.close.docs"))
        .size(ControlSize::Sm)
        .on_click(|_, _, cx| {
            super::state::AppState::global(cx).update(cx, |a, cx| a.toggle(super::state::PanelId::Docs, cx));
        })
        .into_any_element()
}

impl Render for DocsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i = cx.global::<I18n>().clone();
        let st = self.state.read(cx);
        let (open, active) = (st.open.clone(), st.active.clone());
        let Some(active) = active else {
            return div()
                .flex()
                .flex_col()
                .size_full()
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_end()
                        .h(t.rem(rem::TITLEBAR_HEIGHT))
                        .px(t.space(2))
                        .border_b_1()
                        .border_color(t.colors.border)
                        .child(close_panel_button(&i)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap(t.space(1))
                        .p(t.space(4))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("empty.p3.title")))
                        .child(div().text_color(t.colors.fg_muted).child(i.t("empty.p3.body"))),
                )
                .into_any_element();
        };
        if !self.cache.contains_key(&active) {
            let l = Self::load(&active);
            self.mtimes.insert(active.clone(), mtime(&active));
            if !default_render(l.kind) {
                self.raw.remove(&active);
            }
            self.cache.insert(active.clone(), l);
        }
        let kind = self.cache[&active].kind;
        self.media.retain(|k, _| *k == active);
        self.look = DocLook::from(&crate::core::appearance::AppearanceStore::global(cx).read(cx).docs);
        // Tampilan kode/editor memakai latar + teks tema editor (editor.background / editor.foreground), seperti VS Code.
        let code_surface = t.colors.code_bg.is_some()
            && (self.editors.contains_key(&active) || ((self.raw.contains(&active) || always_raw(kind)) && is_text_kind(kind)) || matches!(kind, DocKind::Json | DocKind::Html));
        let (surf_bg, surf_fg) = (t.colors.code_bg, t.colors.code_fg);
        if self.preview {
            let body = self.body(&t, &i, &active, cx);
            return div().w_full().when(code_surface, |d| d.when_some(surf_bg, |d, c| d.bg(c)).when_some(surf_fg, |d, c| d.text_color(c))).child(body).into_any_element();
        }

        let mut tabs = div().id("doc-tabs").flex().flex_none().items_center().gap(t.space(1)).px(t.space(2)).h(t.rem(rem::TITLEBAR_HEIGHT)).border_b_1().border_color(t.colors.border).overflow_x_scroll();
        for p in &open {
            let name = file_name(p).to_string();
            let cur = *p == active;
            let dirty = self.is_dirty(p, cx);
            let (p1, p2, p3) = (p.clone(), p.clone(), p.clone());
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("doc-{p}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(t.space(1))
                    .pl(t.space(2))
                    .h(t.rem(rem::CONTROL_SM))
                    .rounded(t.rem(rem::RADIUS_SM))
                    .cursor_pointer()
                    .text_color(if cur { t.colors.fg } else { t.colors.fg_muted })
                    .when(cur, |d| d.bg(t.colors.active))
                    .hover(|s| s.bg(t.colors.hover))
                    .tooltip(crate::ui::tooltip::text_tooltip(p.clone().into()))
                    .on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.open_file(p1.clone(), cx))))
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(move |s, e: &gpui::MouseDownEvent, _, cx| {
                            s.tab_menu = Some((p3.clone(), e.position));
                            cx.notify();
                        }),
                    )
                    .child(crate::shell::file_icons::file_type_icon(&t, &name, false, false, rem::ICON_SM))
                    .when(dirty, |d| d.child(div().id(SharedString::from(format!("docd-{p}"))).text_color(t.colors.accent).tooltip(crate::ui::tooltip::text_tooltip(i.t("doc.unsaved").into())).child("●")))
                    .child(div().max_w(t.rem(10.)).truncate().small(&t).child(name.clone()))
                    .child(
                        icon_button(SharedString::from(format!("docx-{p}")), "close", i.tp("doc.closeTab", &[("name", &name)]))
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |s, _, _, cx| {
                                cx.stop_propagation();
                                s.guarded(DiscardThen::Close(vec![p2.clone()]), cx);
                            })),
                    ),
            );
        }

        let viewer = viewer_name(kind, self.raw.contains(&active), self.state.read(cx).diffs.contains_key(&active), self.editors.contains_key(&active));
        let tabs = div()
            .flex()
            .flex_none()
            .items_center()
            .border_b_1()
            .border_color(t.colors.border)
            .child(div().flex_1().min_w_0().child(tabs.border_b_0()))
            .child(div().flex_none().px(t.space(1)).child(icon_button("doc-customize", "customize", i.t("docStyle.open")).size(ControlSize::Sm).pressed(self.customize).on_click(cx.listener(|s, _, _, cx| {
                s.customize = !s.customize;
                cx.notify();
            }))))
            .child(div().flex_none().pr(t.space(2)).child(close_panel_button(&i)));
        let raw = self.raw.contains(&active);
        let wrap = !self.nowrap.contains(&active);
        let (a1, a2, a3, a4, a5, a6, a7) = (active.clone(), active.clone(), active.clone(), active.clone(), active.clone(), active.clone(), active.clone());
        let mut bar = div().flex().flex_none().items_center().gap(t.space(1)).px(t.space(2)).py(t.space(1)).border_b_1().border_color(t.colors.border);
        if has_raw_mode(kind) {
            bar = bar
                .child(button("doc-render").size(ControlSize::Sm).variant(ButtonVariant::Ghost).pressed(!raw).label(i.t("doc.render")).on_click(cx.listener(move |s, _, _, cx| {
                    s.raw.remove(&a1);
                    cx.notify();
                })))
                .child(button("doc-raw").size(ControlSize::Sm).variant(ButtonVariant::Ghost).pressed(raw).label(i.t("doc.raw")).on_click(cx.listener(move |s, _, _, cx| {
                    s.raw.insert(a2.clone());
                    cx.notify();
                })));
        }
        let editing = self.editors.contains_key(&active);
        let dirty = self.is_dirty(&active, cx);
        let textual = is_text_kind(kind) && (raw || always_raw(kind));
        let can_find = textual && !editing;
        if kind == DocKind::Html {
            let p = active.clone();
            bar = bar.child(icon_button("doc-live", "globe", i.t("doc.openInBrowser")).size(ControlSize::Sm).on_click(move |_, _, cx| {
                // live server (aset relatif, skrip, muat ulang otomatis) di Browser Panel 4
                let file = std::path::PathBuf::from(&p);
                let project = crate::core::projects::ProjectStore::global(cx).read(cx).projects.iter().map(|x| std::path::PathBuf::from(&x.path)).find(|d| file.starts_with(d));
                let dir = project.or_else(|| file.parent().map(|d| d.to_path_buf())).unwrap_or_default();
                let rel = file.strip_prefix(&dir).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
                match crate::liveserver::start(&dir, &rel) {
                    Ok(url) => super::layout::open_in_zandor_browser(url, cx),
                    Err(e) => {
                        let i = cx.global::<I18n>().clone();
                        toast_err(cx, i.tp("sb.error", &[("error", &e)]));
                    }
                }
            }));
        }
        bar = bar
            .child(icon_button("doc-find", "search", i.t(if can_find { "doc.find" } else { "doc.findUnavailable" })).size(ControlSize::Sm).disabled(!can_find).pressed(self.find.is_some()).on_click(cx.listener(|s, _, w, cx| s.toggle_find(w, cx))))
            .child(icon_button("doc-wrap", "wrap", i.t("doc.wrap")).size(ControlSize::Sm).disabled(!textual).pressed(wrap).on_click(cx.listener(move |s, _, _, cx| {
                if !s.nowrap.remove(&a3) {
                    s.nowrap.insert(a3.clone());
                }
                cx.notify();
            })))
            .child(icon_button("doc-copy", "copy", i.t("doc.copy")).size(ControlSize::Sm).disabled(!is_text_kind(kind)).on_click(cx.listener(move |s, _, _, cx| {
                if let Some((txt, _)) = s.cache.get(&a4).and_then(|l| l.text.clone()) {
                    cx.write_to_clipboard(ClipboardItem::new_string(txt));
                    let i = cx.global::<I18n>().clone();
                    toast_ok(cx, i.t("doc.copied"));
                }
            })))
            .child(button("doc-copypath").size(ControlSize::Sm).label(i.t("doc.copyPath")).on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(a6.clone()));
                let i = cx.global::<I18n>().clone();
                toast_ok(cx, i.t("doc.copied"));
            }));
        let a_dl = active.clone();
        bar = bar
            .child(icon_button("doc-dl", "download", i.t("doc.download")).size(ControlSize::Sm).disabled(self.cache.get(&active).is_none_or(|l| l.bytes.is_err())).on_click(cx.listener(move |s, _, _, cx| s.save_copy(&a_dl, cx))))
            .child(icon_button("doc-ext", "external_link", i.t("doc.openExternal")).size(ControlSize::Sm).pressed(self.open_with.is_some()).on_click(cx.listener(move |s, e: &gpui::ClickEvent, _, cx| {
                let _ = &a7;
                s.toggle_open_with(e.position(), cx);
            })));
        if textual && !editing {
            let block = self.edit_block(&active);
            let a_ed = active.clone();
            bar = bar.child(icon_button("doc-edit", "edit", block.map(|b| i.t(&format!("doc.noEdit.{b}"))).unwrap_or_else(|| i.t("doc.edit"))).size(ControlSize::Sm).disabled(block.is_some()).on_click(cx.listener(move |s, _, _, cx| s.begin_edit(&a_ed, cx))));
        }
        if editing {
            let (a_sv, a_done) = (active.clone(), active.clone());
            bar = bar
                .child(icon_button("doc-save", "save", i.t("doc.save")).size(ControlSize::Sm).disabled(!dirty).on_click(cx.listener(move |s, _, _, cx| s.save(&a_sv, false, cx))))
                .child(button("doc-done").size(ControlSize::Sm).label(i.t("doc.doneEdit")).on_click(cx.listener(move |s, _, _, cx| s.guarded(DiscardThen::EndEdit(a_done.clone()), cx))));
        }
        let a_close = active.clone();
        bar = bar
            .child(icon_button("doc-reload", "refresh", i.t("doc.reload")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.guarded(DiscardThen::Reload(a5.clone()), cx))))
            .child(icon_button("doc-close", "close", i.t("doc.close")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.guarded(DiscardThen::Close(vec![a_close.clone()]), cx))));

        let diff = self.state.read(cx).diffs.get(&active).cloned();
        let (bar, body) = match diff {
            Some(d) => {
                let (b, body) = self.diff_view(&t, &i, &active, &d, cx);
                (b.into_any_element(), body)
            }
            None => {
                let body = match self.editors.get(&active).cloned() {
                    Some(ed) => {
                        let a_ks = active.clone();
                        div()
                            .p(t.space(2))
                            .font_family(t.font_code.clone())
                            .code(&t)
                            .on_key_down(cx.listener(move |s, e: &gpui::KeyDownEvent, _, cx| {
                                let k = &e.keystroke;
                                if k.key == "s" && (k.modifiers.control || k.modifiers.platform) {
                                    s.save(&a_ks, false, cx);
                                    cx.stop_propagation();
                                }
                            }))
                            .child(ed)
                            .into_any_element()
                    }
                    None => self.body(&t, &i, &active, cx),
                };
                (bar.into_any_element(), body)
            }
        };
        let ask = self.ask_dialog(&t, &i, cx);
        let custom = self.customize_popup(&t, &i, super::doc_style_section::groups_for_viewer(viewer), cx);
        let ow = self.open_with_popup(&t, &i, &active, kind, cx);
        let find_bar = self.find.clone().filter(|_| can_find).map(|f| {
            let text = self.cache.get(&active).and_then(|l| l.text.as_ref()).map(|x| x.0.clone()).unwrap_or_default();
            let n = find_matches(&text, f.read(cx).text().as_ref()).len();
            let pos = if n == 0 { "0/0".to_string() } else { format!("{}/{}", self.find_at.min(n - 1) + 1, n) };
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(t.space(1))
                .px(t.space(2))
                .py(t.space(1))
                .border_b_1()
                .border_color(t.colors.border)
                .on_key_down(cx.listener(move |s, e: &gpui::KeyDownEvent, w, cx| match e.keystroke.key.as_str() {
                    "enter" => s.find_step(if e.keystroke.modifiers.shift { -1 } else { 1 }, n, cx),
                    "escape" => s.toggle_find(w, cx),
                    _ => {}
                }))
                .child(div().flex_1().child(f))
                .child(div().small(&t).text_color(t.colors.fg_muted).child(pos))
                .child(icon_button("doc-find-prev", "chevron_up", i.t("diff.prev")).size(ControlSize::Sm).disabled(n == 0).on_click(cx.listener(move |s, _, _, cx| s.find_step(-1, n, cx))))
                .child(icon_button("doc-find-next", "chevron_down", i.t("diff.next")).size(ControlSize::Sm).disabled(n == 0).on_click(cx.listener(move |s, _, _, cx| s.find_step(1, n, cx))))
                .child(icon_button("doc-find-close", "close", i.t("ui.close")).size(ControlSize::Sm).on_click(cx.listener(|s, _, w, cx| s.toggle_find(w, cx))))
        });
        let hint_bar = self.hint.clone().filter(|h| *h == active && textual && !editing).map(|_| {
            let block = self.edit_block(&active);
            let a_he = active.clone();
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(t.space(2))
                .px(t.space(3))
                .py(t.space(1))
                .bg(t.colors.hover)
                .child(icon("eye").size(IconSize::Sm).color(t.colors.fg_muted))
                .child(div().flex_1().small(&t).text_color(t.colors.fg_muted).child(match block {
                    None => i.t("doc.readOnly"),
                    Some(b) => format!("{} {}", i.t("doc.readOnly"), i.t(&format!("doc.noEdit.{b}"))),
                }))
                .when(block.is_none(), |d| d.child(button("doc-hint-edit").size(ControlSize::Sm).label(i.t("doc.edit")).on_click(cx.listener(move |s, _, _, cx| s.begin_edit(&a_he, cx)))))
        });
        let ro_path = active.clone();
        let ro_path2 = active.clone();
        let read_only = textual && !editing;
        let menu = self.tab_menu.clone().map(|(anchor, pos)| {
            let idx = open.iter().position(|p| *p == anchor).unwrap_or(0);
            let last = open.len().saturating_sub(1);
            let me = cx.entity().downgrade();
            let by = |mode: &'static str| {
                let (me, a) = (me.clone(), anchor.clone());
                move |_: &mut Window, cx: &mut App| {
                    me.update(cx, |s, cx| s.close_many(mode, &a, cx)).ok();
                }
            };
            let entries = vec![
                crate::ui::entry(i.t("tabs.close"), by("one")),
                crate::ui::entry(i.t("tabs.closeOthers"), by("others")).disabled(open.len() < 2),
                crate::ui::entry(i.t("tabs.closeRight"), by("right")).disabled(idx == last),
                crate::ui::entry(i.t("tabs.closeLeft"), by("left")).disabled(idx == 0),
                crate::ui::MenuEntry::Separator,
                crate::ui::entry(i.t("tabs.closeAll"), by("all")),
            ];
            div()
                .absolute()
                .child(gpui::anchored().position(pos).child(crate::ui::menu_popup("doc-tab-menu", &t, entries, gpui::Anchor::TopLeft, move |_, cx| {
                    me.update(cx, |s, cx| {
                        s.tab_menu = None;
                        cx.notify();
                    })
                    .ok();
                })))
                .into_any_element()
        });
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(tabs)
            .child(bar)
            .children(find_bar)
            .children(hint_bar)
            .child(if viewer == "pdf" || viewer == "media" {
                // PDF menggulir sendiri (memuat halaman sesuai viewport)
                div().id("doc-body").flex_1().min_h_0().flex().flex_col().child(body)
            } else {
                div()
                    .id("doc-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .track_scroll(&self.body_scroll)
                    .when(read_only, |d| {
                        d.track_focus(&self.view_focus)
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |s, _, w, cx| {
                                    s.view_focus.focus(w, cx);
                                    s.show_hint(&ro_path, cx);
                                }),
                            )
                            .on_key_down(cx.listener(move |s, e: &gpui::KeyDownEvent, _, cx| {
                                let k = &e.keystroke;
                                let m = &k.modifiers;
                                let typing = (k.key.chars().count() == 1 && !m.control && !m.platform && !m.alt) || matches!(k.key.as_str(), "enter" | "backspace" | "delete" | "tab") || ((m.control || m.platform) && matches!(k.key.as_str(), "v" | "x"));
                                if typing {
                                    s.show_hint(&ro_path2, cx);
                                }
                            }))
                    })
                    .when(code_surface, |d| d.when_some(surf_bg, |d, c| d.bg(c)).when_some(surf_fg, |d, c| d.text_color(c)))
                    .child(body)
            })
            .children(menu)
            .children(custom)
            .children(ow)
            .children(ask)
            .into_any_element()
    }
}

// ---- DiffViewer (port DiffViewer.tsx): tinjau perubahan agent per bagian. "Terima" hanya menandai selesai;
// "Tolak" mengembalikan bagian itu ke isi semula dan langsung menyimpan berkas (bisa diurungkan dari toast). Sisi
// "sebelum" dihitung dengan membalik diff terhadap isi berkas saat ini. GPUI tidak punya editor diff Monaco, jadi
// tiap bagian ditampilkan sebagai blok unified (bagian aktif ditandai) — isinya sama, tata letaknya tidak berdampingan.
impl DocsPanel {
    /// Isi berkas saat ini bila aman ditulis ulang (UTF-8 utuh, tidak terpotong).
    fn writable_text(&self, path: &str) -> Option<String> {
        let l = self.cache.get(path)?;
        let (txt, trunc) = l.text.clone()?;
        let bytes = l.bytes.as_ref().ok()?;
        (!trunc && std::str::from_utf8(bytes).is_ok()).then_some(txt)
    }

    fn write_file(&mut self, path: &str, text: &str, cx: &mut Context<Self>) -> bool {
        let i = cx.global::<I18n>().clone();
        // konflik: berkas berubah di disk sejak dimuat
        let disk = std::fs::read(path).ok();
        let loaded = self.cache.get(path).and_then(|l| l.bytes.as_ref().ok().cloned());
        if disk.as_deref() != loaded.as_deref().map(|b| b.as_slice()) {
            crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(i.t("diff.conflict")) }, cx);
            return false;
        }
        match std::fs::write(path, text.as_bytes()) {
            Ok(()) => {
                self.cache.insert(path.to_string(), Self::load(path));
                true
            }
            Err(e) => {
                crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(i.tp("diff.saveError", &[("error", &e.to_string())])) }, cx);
                false
            }
        }
    }

    fn finish(&mut self, path: &str, rest: Vec<Hunk>, cx: &mut Context<Self>) {
        let p = path.to_string();
        if rest.is_empty() {
            self.state.update(cx, |d, cx| d.set_diff(&p, None, cx));
            let title = cx.global::<I18n>().t("diff.done");
            crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Success, ..crate::ui::ToastInput::new(title) }, cx);
        } else {
            let f = format_hunks(&rest);
            self.state.update(cx, |d, cx| d.set_diff(&p, Some(f), cx));
        }
    }

    fn hunks_of(&self, path: &str, cx: &App) -> Vec<Hunk> {
        self.state.read(cx).diffs.get(path).map(|d| parse_patch(d)).unwrap_or_default()
    }

    fn accept_hunk(&mut self, path: &str, cx: &mut Context<Self>) {
        let hunks = self.hunks_of(path, cx);
        let at = self.diff_at.get(path).copied().unwrap_or(0).min(hunks.len().saturating_sub(1));
        if hunks.is_empty() {
            return;
        }
        self.finish(path, drop_hunk(&hunks, at, false), cx);
    }

    fn reject_hunk(&mut self, path: &str, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>().clone();
        let hunks = self.hunks_of(path, cx);
        let at = self.diff_at.get(path).copied().unwrap_or(0).min(hunks.len().saturating_sub(1));
        let (Some(h), Some(cur)) = (hunks.get(at), self.writable_text(path)) else { return };
        let Some(next) = revert_hunk(&cur, h) else {
            crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Warning, ..crate::ui::ToastInput::new(i.t("diff.stale")) }, cx);
            return;
        };
        let prev_patch = self.state.read(cx).diffs.get(path).cloned();
        if !self.write_file(path, &next, cx) {
            return;
        }
        self.finish(path, drop_hunk(&hunks, at, true), cx);
        let me = cx.entity().downgrade();
        let p = path.to_string();
        crate::ui::Toasts::show(
            crate::ui::ToastInput {
                action: Some(crate::ui::ToastAction {
                    label: i.t("diff.undo").into(),
                    on_click: std::rc::Rc::new(move |_, cx| {
                        let (p, cur, pp) = (p.clone(), cur.clone(), prev_patch.clone());
                        me.update(cx, |s, cx| {
                            if s.write_file(&p, &cur, cx) {
                                s.state.update(cx, |d, cx| d.set_diff(&p, pp, cx));
                            }
                        })
                        .ok();
                    }),
                }),
                ..crate::ui::ToastInput::new(i.t("diff.rejected"))
            },
            cx,
        );
    }

    fn reject_all(&mut self, path: &str, cx: &mut Context<Self>) {
        self.confirm_all = false;
        let hunks = self.hunks_of(path, cx);
        let Some(before) = self.writable_text(path).and_then(|c| reverse_apply(&c, &hunks)) else { return };
        if self.write_file(path, &before, cx) {
            self.state.update(cx, |d, cx| d.set_diff(path, None, cx));
            let title = cx.global::<I18n>().t("diff.rejectedAll");
            crate::ui::Toasts::show(crate::ui::ToastInput::new(title), cx);
        }
        cx.notify();
    }

    fn diff_view(&mut self, t: &Theme, i: &I18n, path: &str, patch: &str, cx: &mut Context<Self>) -> (gpui::Div, AnyElement) {
        let hunks = parse_patch(patch);
        let current = self.cache.get(path).and_then(|l| l.text.as_ref().map(|x| x.0.clone())).unwrap_or_default();
        let before = reverse_apply(&current, &hunks);
        let at = self.diff_at.get(path).copied().unwrap_or(0).min(hunks.len().saturating_sub(1));
        let writable = self.writable_text(path).is_some();
        let reject_reason = (!writable).then(|| i.t("diff.cantWrite"));
        let can_reject = before.is_some() && writable;
        let name = file_name(path).to_string();
        let p = |s: &str| -> SharedString { SharedString::from(format!("{s}-{path}")) };
        let (p1, p2, p3, p4, p5, p6, p7) = (path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string());
        let n = hunks.len();
        let bar = div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap(t.space(1))
            .px(t.space(2))
            .py(t.space(1))
            .border_b_1()
            .border_color(t.colors.border)
            .child(div().small(t).text_color(t.colors.fg_muted).child(if n > 0 { i.tp("diff.pos", &[("i", &(at + 1).to_string()), ("n", &n.to_string())]) } else { i.t("diff.none") }))
            .child(icon_button(p("dprev"), "chevron_up", i.t("diff.prev")).size(ControlSize::Sm).disabled(at == 0).on_click(cx.listener(move |s, _, _, cx| {
                let e = s.diff_at.entry(p1.clone()).or_insert(0);
                *e = e.saturating_sub(1);
                cx.notify();
            })))
            .child(icon_button(p("dnext"), "chevron_down", i.t("diff.next")).size(ControlSize::Sm).disabled(at + 1 >= n).on_click(cx.listener(move |s, _, _, cx| {
                s.diff_at.insert(p2.clone(), at + 1);
                cx.notify();
            })))
            .child(div().flex_1())
            .child(button(p("dacc")).size(ControlSize::Sm).variant(ButtonVariant::Solid).disabled(n == 0).leading_icon("check").label(i.t("diff.accept")).on_click(cx.listener(move |s, _, _, cx| s.accept_hunk(&p3, cx))))
            .child(button(p("drej")).size(ControlSize::Sm).disabled(n == 0 || !can_reject).child(icon("undo").size(IconSize::Sm)).label(i.t("diff.reject")).on_click(cx.listener(move |s, _, _, cx| s.reject_hunk(&p4, cx))))
            .child(button(p("daccall")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("diff.acceptAll")).on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.set_diff(&p5, None, cx)))))
            .child(button(p("drejall")).size(ControlSize::Sm).variant(ButtonVariant::Ghost).disabled(!can_reject).label(i.t("diff.rejectAll")).on_click(cx.listener(|s, _, _, cx| {
                s.confirm_all = true;
                cx.notify();
            })))
            .child(icon_button(p("dclose"), "close", i.t("diff.close")).size(ControlSize::Sm).on_click(cx.listener(move |s, _, _, cx| s.state.update(cx, |d, cx| d.set_diff(&p6, None, cx)))));

        let mut body = div().flex().flex_col().gap(t.space(2)).p(t.space(3));
        if let (Some(r), true) = (&reject_reason, before.is_some()) {
            body = body.child(note(t, r.clone()));
        }
        if before.is_none() {
            body = body.child(note(t, i.t("diff.mismatch")));
        } else {
            for (k, h) in hunks.iter().enumerate() {
                let rows = crate::core::tool_view::parse_diff(&format_hunks(std::slice::from_ref(h)));
                let pp = path.to_string();
                body = body.child(
                    div()
                        .id(SharedString::from(format!("dh-{k}-{path}")))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .border_1()
                        .border_color(if k == at { t.colors.accent } else { t.colors.border_soft })
                        .cursor_pointer()
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.diff_at.insert(pp.clone(), k);
                            cx.notify();
                        }))
                        .child(super::chat_panel::chat_parts::diff_block(t, i, SharedString::from(format!("dhb-{k}-{path}")), &rows)),
                );
            }
        }
        if self.confirm_all {
            body = body.child(
                crate::ui::dialog(i.t("diff.rejectAll.title"))
                    .description(i.tp("diff.rejectAll.body", &[("n", &n.to_string()), ("name", &name)]))
                    .on_dismiss(cx.listener(|s, _, _, cx| {
                        s.confirm_all = false;
                        cx.notify();
                    }))
                    .footer(
                        div()
                            .flex()
                            .justify_end()
                            .gap(t.space(2))
                            .child(button("drejall-no").label(i.t("ui.cancel")).on_click(cx.listener(|s, _, _, cx| {
                                s.confirm_all = false;
                                cx.notify();
                            })))
                            .child(button("drejall-yes").variant(ButtonVariant::Danger).label(i.t("diff.rejectAll")).on_click(cx.listener(move |s, _, _, cx| s.reject_all(&p7, cx)))),
                    ),
            );
        }
        (bar, body.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cari_temuan_tanpa_huruf_besar() {
        let m = super::find_matches("Satu dua
DUA tiga dua", "dua");
        assert_eq!(m, vec![(0, 5), (1, 0), (1, 9)]);
        assert!(super::find_matches("abc", "").is_empty());
    }

    #[test]
    fn csv() {
        let r = super::parse_delimited("a,\"b,c\",\"d\"\"e\"\r\n1,2\n", ',');
        assert_eq!(r, vec![vec!["a", "b,c", "d\"e"], vec!["1", "2"]]);
        assert_eq!(super::human(2048), "2.0 KB");
    }
}

/// Ukuran piksel gambar raster (PNG/JPEG/GIF/WebP) dari header-nya.
fn image_dims(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?.into_dimensions().ok()
}

impl DocsPanel {
    fn toggle_find(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.find.take().is_none() {
            let ph = cx.global::<I18n>().t("doc.find");
            let f = cx.new(|cx| crate::ui::TextInput::new(cx, ph));
            cx.observe(&f, |s, _, cx| {
                s.find_at = 0;
                s.scroll_to_hit(cx);
                cx.notify();
            })
            .detach();
            let fh = gpui::Focusable::focus_handle(f.read(cx), cx);
            fh.focus(w, cx);
            self.find = Some(f);
            self.find_at = 0;
        }
        cx.notify();
    }

    fn find_step(&mut self, d: isize, n: usize, cx: &mut Context<Self>) {
        if n == 0 {
            return;
        }
        self.find_at = ((self.find_at.min(n - 1) as isize + d).rem_euclid(n as isize)) as usize;
        self.scroll_to_hit(cx);
        cx.notify();
    }

    /// Gulir ke baris temuan aktif (tinggi baris kode tetap; tepat untuk tampilan tanpa bungkus baris).
    fn scroll_to_hit(&mut self, cx: &mut Context<Self>) {
        let Some(active) = self.state.read(cx).active.clone() else { return };
        let Some(f) = &self.find else { return };
        let text = self.cache.get(&active).and_then(|l| l.text.as_ref()).map(|x| x.0.clone()).unwrap_or_default();
        let hits = find_matches(&text, f.read(cx).text().as_ref());
        let Some((line, _)) = hits.get(self.find_at).copied() else { return };
        let t = cx.global::<Theme>();
        let ty = t.ty(crate::theme::CODE);
        let lh = t.rem(ty.size) * ty.lh;
        let y = t.space(3) + lh * line as f32 - gpui::px(80.);
        self.body_scroll.set_offset(gpui::point(self.body_scroll.offset().x, -y.max(gpui::px(0.))));
    }
}

impl DocsPanel {
    fn toggle_open_with(&mut self, pos: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        if self.open_with.take().is_some() {
            cx.notify();
            return;
        }
        self.open_with = Some((pos, OwLoad::Loading));
        self.fetch_apps(false, cx);
    }

    fn fetch_apps(&mut self, refresh: bool, cx: &mut Context<Self>) {
        let Some(active) = self.state.read(cx).active.clone() else { return };
        let text_like = self.cache.get(&active).is_some_and(|l| is_text_kind(l.kind));
        let ext = std::path::Path::new(&active).extension().and_then(|e| e.to_str()).map(str::to_string);
        if let Some((_, l)) = &mut self.open_with {
            *l = OwLoad::Loading;
        }
        let task = cx.background_spawn(async move {
            match ext {
                Some(e) => crate::openwith::apps_for(&e, text_like, refresh),
                None => Err("berkas tanpa ekstensi".to_string()),
            }
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                if let Some((_, l)) = &mut s.open_with {
                    *l = match r {
                        Ok(a) => OwLoad::Ready(a),
                        Err(e) => OwLoad::Error(e),
                    };
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn open_with_popup(&mut self, t: &Theme, i: &I18n, path: &str, kind: DocKind, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (pos, load) = self.open_with.clone()?;
        let text_like = is_text_kind(kind);
        let item = |id: SharedString, lead: AnyElement, name: String| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(t.space(2))
                .px(t.space(2))
                .h(t.rem(rem::CONTROL_MD))
                .rounded(t.rem(rem::RADIUS_SM))
                .cursor_pointer()
                .hover(|s| s.bg(t.colors.hover))
                .child(div().flex_none().size(t.rem(1.)).flex().items_center().justify_center().child(lead))
                .child(div().flex_1().min_w_0().truncate().child(name))
        };
        let generic = || icon("external_link").size(IconSize::Sm).into_any_element();
        let close = cx.listener(|s: &mut Self, _: &gpui::MouseDownEvent, _, cx| {
            s.open_with = None;
            cx.notify();
        });
        let p0 = path.to_string();
        let mut list = crate::ui::popup(t)
            .w(t.rem(18.))
            .child(div().px(t.space(2)).py(t.space(1)).small(t).font_weight(FontWeight::SEMIBOLD).child(i.t("doc.openWith.title")))
            .child(item("ow-system".into(), generic(), i.t("doc.openWith.system")).on_click(cx.listener(move |s, _, _, cx| {
                s.open_with = None;
                cx.open_with_system(std::path::Path::new(&p0));
                cx.notify();
            })));
        match load {
            OwLoad::Loading => list = list.child(div().flex().items_center().gap(t.space(1)).px(t.space(2)).py(t.space(1)).small(t).child(crate::ui::spinner(IconSize::Sm)).child(i.t("doc.openWith.loading"))),
            OwLoad::Error(e) => {
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(1))
                        .px(t.space(2))
                        .py(t.space(1))
                        .small(t)
                        .text_color(t.colors.danger)
                        .child(i.tp("doc.openWith.error", &[("error", &e)]))
                        .child(button("ow-retry").size(ControlSize::Sm).variant(ButtonVariant::Ghost).label(i.t("doc.reload")).on_click(cx.listener(|s, _, _, cx| s.fetch_apps(true, cx)))),
                )
            }
            OwLoad::Ready(apps) => {
                if apps.is_empty() {
                    list = list.child(div().px(t.space(2)).py(t.space(1)).small(t).child(i.t("doc.openWith.none")));
                }
                for (n, a) in apps.into_iter().enumerate() {
                    let lead = if a.icon.is_empty() {
                        generic()
                    } else {
                        crate::core::gradient::image_of(&format!("data:image/png;base64,{}", a.icon)).map(|img| gpui::img(img).size(t.rem(1.)).into_any_element()).unwrap_or_else(generic)
                    };
                    let (app, file) = (a.path.clone(), path.to_string());
                    let mut row = item(SharedString::from(format!("ow-{n}")), lead, a.name.clone()).tooltip(crate::ui::tooltip::text_tooltip(a.path.clone().into()));
                    if a.is_default {
                        row = row.child(crate::ui::badge(i.t("doc.openWith.default")));
                    }
                    list = list.child(row.on_click(cx.listener(move |s, _, _, cx| {
                        s.open_with = None;
                        let (app, file) = (app.clone(), file.clone());
                        let r = (|| {
                            if crate::files::is_executable_ext(std::path::Path::new(&file)) {
                                return Err("berkas yang dapat dieksekusi tidak dibuka dari Zandor".to_string());
                            }
                            let ext = std::path::Path::new(&file).extension().and_then(|e| e.to_str()).ok_or("berkas tanpa ekstensi")?.to_string();
                            let apps = crate::openwith::apps_for(&ext, text_like, false)?;
                            if !crate::openwith::app_allowed(&apps, &app) {
                                return Err("aplikasi ini tidak terdaftar untuk jenis berkas tersebut".to_string());
                            }
                            crate::openwith::launch(&app, std::path::Path::new(&file))
                        })();
                        if let Err(e) = r {
                            let i = cx.global::<I18n>().clone();
                            toast_err(cx, i.tp("sb.error", &[("error", &e)]));
                        }
                        cx.notify();
                    })));
                }
            }
        }
        let p1 = path.to_string();
        list = list.child(crate::ui::separator(t)).child(item("ow-reveal".into(), icon("folder_open").size(IconSize::Sm).into_any_element(), i.t("doc.openWith.reveal")).on_click(cx.listener(move |s, _, _, cx| {
            s.open_with = None;
            cx.reveal_path(std::path::Path::new(&p1));
            cx.notify();
        })));
        Some(
            crate::ui::overlay_deferred(gpui::anchored().position(pos).anchor(gpui::Anchor::TopRight).snap_to_window_with_margin(gpui::px(8.)).child(div().mt(t.space(2)).child(list.occlude().on_mouse_down_out(close))))
                .with_priority(65)
                .into_any_element(),
        )
    }
}

/// Latar kotak-kotak (transparansi) untuk penampil gambar: --z-doc-checker-a/b, sel space-4 / 2.
fn checker(t: &Theme) -> AnyElement {
    let cell = t.space(4) / 2.;
    let (a, b) = (hex("#cfcfcf"), hex("#f2f2f2"));
    gpui::canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            window.paint_quad(gpui::fill(bounds, b));
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let c = f32::from(cell).max(2.);
            let (cols, rows) = ((w / c).ceil() as i32, (h / c).ceil() as i32);
            for r in 0..rows.min(400) {
                for k in 0..cols.min(400) {
                    if (r + k) % 2 == 0 {
                        let o = bounds.origin + gpui::point(gpui::px(k as f32 * c), gpui::px(r as f32 * c));
                        window.paint_quad(gpui::fill(gpui::Bounds::new(o, gpui::size(gpui::px(c), gpui::px(c))), a));
                    }
                }
            }
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

/// Nama penampil untuk kelompok kustomisasi (port viewerFor).
fn viewer_name(kind: DocKind, raw: bool, diff: bool, editing: bool) -> &'static str {
    if diff && !editing && is_text_kind(kind) {
        return "diff";
    }
    if always_raw(kind) || (raw && has_raw_mode(kind)) {
        return "code";
    }
    match kind {
        DocKind::Markdown => "markdown",
        DocKind::Svg | DocKind::Image => "image",
        DocKind::Pdf => "pdf",
        DocKind::Csv | DocKind::Tsv => "table",
        DocKind::Json => "json",
        DocKind::Html => "html",
        DocKind::Audio | DocKind::Video => "media",
        DocKind::Docx => "docx",
        DocKind::Xlsx => "xlsx",
        DocKind::Pptx => "pptx",
        DocKind::Notebook => "notebook",
        _ => "binary",
    }
}

impl DocsPanel {
    fn customize_popup(&mut self, t: &Theme, i: &I18n, groups: Vec<&'static str>, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.customize {
            return None;
        }
        let ds = self.doc_style.get_or_insert_with(|| cx.new(|cx| super::doc_style_section::DocStyleControls::new(vec![], cx))).clone();
        ds.update(cx, |d, cx| {
            if d.groups != groups {
                d.groups = groups;
                cx.notify();
            }
        });
        let me = cx.entity().downgrade();
        let me2 = me.clone();
        Some(
            crate::ui::overlay_deferred(
                gpui::anchored().anchor(gpui::Anchor::TopRight).snap_to_window_with_margin(gpui::px(8.)).child(
                    crate::ui::popup(t)
                        .occlude()
                        .mt(t.space(1))
                        .w(t.rem(22.))
                        .max_h(t.rem(32.))
                        .flex()
                        .flex_col()
                        .on_mouse_down_out(move |_, _, cx| {
                            me.update(cx, |s, cx| {
                                s.customize = false;
                                cx.notify();
                            })
                            .ok();
                        })
                        .child(div().px(t.space(3)).pt(t.space(3)).pb(t.space(2)).font_weight(FontWeight::SEMIBOLD).child(i.t("docStyle.title")))
                        .child(div().id("dstyle-pop-body").flex_1().min_h_0().overflow_y_scroll().px(t.space(3)).pb(t.space(3)).child(ds))
                        .child(
                            div().flex().border_t_1().border_color(t.colors.border).p(t.space(2)).child(
                                button("dstyle-all")
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .child(icon("settings").size(IconSize::Sm))
                                    .label(i.t("docStyle.allSettings"))
                                    .on_click(move |_, _, cx| {
                                        me2.update(cx, |s, cx| {
                                            s.customize = false;
                                            cx.notify();
                                        })
                                        .ok();
                                        let app = super::state::AppState::global(cx);
                                        app.update(cx, |a, cx| a.push("/settings/documents", cx));
                                    }),
                            ),
                        ),
                ),
            )
            .with_priority(50)
            .into_any_element(),
        )
    }
}

//! Lima panel (ADR-015/025): P1 Sidebar | P2 Chat | P3 Dokumen | P4 Tools | P5 Rail — port ShellLayout.tsx.
//! Ukuran panel = persentase lebar grup (bawaan 18/34/22/26), minimum dalam rem; separator bisa diseret.
use super::state::{AppState, PanelId, PANEL_IDS};
use crate::i18n::I18n;
use crate::theme::{rem, Theme, H4, P};
use crate::ui::{icon_button, ButtonVariant, ControlSize, TypeExt};
use gpui::{
    canvas, div, prelude::*, px, AnyView, App, Bounds, Context, Entity, IntoElement, MouseButton,
    MouseMoveEvent, Pixels, Window,
};

/// Panel yang bisa diubah ukurannya (rail hanya tampil/sembunyi).
/// Panel mana yang sedang benar-benar digambar. Webview native (Browser, media) menyembunyikan dirinya saat panelnya
/// tidak tampil, karena jendela anak tidak ikut hilang bila elemen GPUI-nya berhenti digambar.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct PanelShown {
    pub docs: bool,
    pub tools: bool,
}
impl gpui::Global for PanelShown {}

/// Perbarui PanelShown hanya bila berubah (pengamat dipanggil sekali per perubahan).
pub fn set_panel_shown(v: PanelShown, cx: &mut gpui::App) {
    if cx.try_global::<PanelShown>().copied() != Some(v) {
        cx.set_global(v);
    }
}

/// Jumlah per alat untuk lencana di rel: Terminal, Browser, dan Changes dipublikasikan panel sumbernya
/// (`set_rail_counts`); Jobs dihitung ShellLayout sendiri karena panelnya baru dibuat saat dibuka.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct RailCounts {
    pub terminal: usize,
    pub browser: usize,
    pub changes: usize,
}
impl gpui::Global for RailCounts {}

/// Ubah `RailCounts`; pengamat dipanggil hanya bila nilainya berubah.
pub fn set_rail_counts(cx: &mut gpui::App, f: impl FnOnce(&mut RailCounts)) {
    let old = cx.try_global::<RailCounts>().copied().unwrap_or_default();
    let mut new = old;
    f(&mut new);
    if new != old {
        cx.set_global(new);
    }
}

/// Lencana jumlah di pojok kanan atas ikon rel (maks "99+").
fn rail_badge(t: &Theme, n: usize) -> impl IntoElement {
    let c = &t.colors;
    let text = if n > 99 { "99+".to_string() } else { n.to_string() };
    div()
        .absolute()
        .top(t.space(0))
        .right(t.space(0))
        .flex()
        .items_center()
        .justify_center()
        .min_w(t.rem(rem::ICON_MD))
        .h(t.rem(rem::ICON_MD))
        .px(t.space(1))
        .rounded(t.rem(rem::RADIUS_FULL))
        .bg(c.accent)
        .text_color(c.on_accent)
        .text_size(t.rem(t.ty(crate::theme::SMALL).size))
        .font_weight(gpui::FontWeight(t.ty(crate::theme::LABEL).weight))
        .line_height(gpui::relative(1.))
        .whitespace_nowrap()
        .child(text)
}

const RESIZABLE: [PanelId; 4] =[PanelId::Sidebar, PanelId::Chat, PanelId::Docs, PanelId::Tools];
/// Lebar minimum (rem).
fn min_rem(p: PanelId) -> f32 {
    match p {
        PanelId::Sidebar => 12.,
        PanelId::Chat => 22.,
        PanelId::Docs => 16.,
        _ => 18.,
    }
}
const DEFAULT_LAYOUT: [f32; 4] = [18., 34., 22., 26.];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolId {
    Terminal,
    Jobs,
    Git,
    Changes,
    Files,
    Browser,
    Info,
    Session,
    Trajectory,
    Log,
}
/// Permintaan dari bagian lain aplikasi untuk menampilkan satu alat Panel 4 (mis. Jobs → Browser).
#[derive(Default)]
pub struct OpenToolRequest(pub Option<ToolId>);
impl gpui::Global for OpenToolRequest {}

/// Minta buka URL di Browser Panel 4 (port openInZandorBrowser); diamati ToolsHost.
pub struct BrowserOpenRequest(pub Option<String>);
impl gpui::Global for BrowserOpenRequest {}

pub fn open_in_zandor_browser(url: String, cx: &mut gpui::App) {
    cx.set_global(BrowserOpenRequest(Some(url)));
    cx.set_global(OpenToolRequest(Some(ToolId::Browser)));
}

pub const TOOL_IDS: [ToolId; 10] = [
    ToolId::Terminal, ToolId::Jobs, ToolId::Git, ToolId::Changes, ToolId::Files,
    ToolId::Browser, ToolId::Info, ToolId::Session, ToolId::Trajectory, ToolId::Log,
];
impl ToolId {
    pub fn key(self) -> &'static str {
        match self {
            ToolId::Terminal => "terminal",
            ToolId::Jobs => "jobs",
            ToolId::Git => "git",
            ToolId::Changes => "changes",
            ToolId::Files => "files",
            ToolId::Browser => "browser",
            ToolId::Info => "info",
            ToolId::Session => "session",
            ToolId::Trajectory => "trajectory",
            ToolId::Log => "log",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            ToolId::Terminal => "terminal",
            ToolId::Jobs => "server",
            ToolId::Git => "git",
            ToolId::Changes => "diff",
            ToolId::Files => "files",
            ToolId::Browser => "globe",
            ToolId::Info => "layout",
            ToolId::Session => "activity",
            ToolId::Trajectory => "trajectory",
            ToolId::Log => "list",
        }
    }
}

/// Isi panel yang dipasang dari luar (Sidebar, Chat, Docs, alat).
pub struct PanelContent {
    pub sidebar: Option<AnyView>,
    pub chat: Option<AnyView>,
    pub docs: Option<AnyView>,
    pub tool: Option<Box<dyn Fn(ToolId, &mut Window, &mut App) -> Option<AnyView>>>,
}

pub struct ShellLayout {
    app: Entity<AppState>,
    /// Persentase per panel resizable (jumlah panel terbuka dinormalisasi saat render).
    sizes: [f32; 4],
    pub active_tool: ToolId,
    group: Option<Bounds<Pixels>>,
    dragging: Option<usize>,
    pub content: PanelContent,
    saved: String,
    /// Smart resize otomatis: aktif sejak aplikasi dibuka, mati bila pengguna menyeret pemisah.
    auto_smart: bool,
    auto_w: f32,
    /// Status terlipat pada render sebelumnya, untuk mempertahankan lebar sidebar saat panel lain dibuka/ditutup.
    prev_collapsed: Option<Vec<bool>>,
    /// Jumlah background job aktif (sesi terpilih, atau semua bila belum ada sesi) untuk lencana Jobs.
    jobs_badge: usize,
}

fn layout_path() -> std::path::PathBuf {
    crate::core::services::app_data_dir().join("layout.json")
}

fn remember(cx: &App) -> bool {
    crate::core::settings::SettingsStore::global(cx).read(cx).get("rememberLayout").as_bool().unwrap_or(true)
}

impl ShellLayout {
    pub fn new(app: Entity<AppState>, content: PanelContent, cx: &mut Context<Self>) -> Self {
        cx.observe(&app, |s, _, cx| {
            s.persist(cx);
            cx.notify();
        })
        .detach();
        cx.set_global(OpenToolRequest(None));
        cx.observe_global::<OpenToolRequest>(|s, cx| {
            if let Some(t) = cx.global::<OpenToolRequest>().0 {
                s.open_tool(t, cx);
            }
        })
        .detach();
        let mut me = Self { app, sizes: DEFAULT_LAYOUT, active_tool: ToolId::Terminal, group: None, dragging: None, content, saved: String::new(), auto_smart: true, auto_w: 0., prev_collapsed: None, jobs_badge: 0 };
        cx.observe_global::<RailCounts>(|_, cx| cx.notify()).detach();
        // Jumlah job aktif: baca berkas job di latar tiap 2 dtk (panel Jobs sendiri baru dibuat saat dibuka).
        cx.spawn(async move |this, cx| loop {
            let live: Vec<Option<String>> = cx
                .background_executor()
                .spawn(async {
                    let dir = crate::core::services::app_data_dir().join("jobs");
                    crate::jobs::list(&dir)
                        .iter()
                        .filter(|j| matches!(j["status"].as_str(), Some("running" | "starting")))
                        .map(|j| j["sessionID"].as_str().map(String::from))
                        .collect()
                })
                .await;
            let alive = this
                .update(cx, |s, cx| {
                    let sel = crate::core::projects::ProjectStore::global(cx).read(cx).selection.session_id.clone();
                    let n = live.iter().filter(|sid| sel.is_none() || sid.as_deref() == sel.as_deref()).count();
                    if n != s.jobs_badge {
                        s.jobs_badge = n;
                        cx.notify();
                    }
                })
                .is_ok();
            if !alive {
                break;
            }
            cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
        })
        .detach();
        if remember(cx) {
            me.restore(cx);
        }
        me.sync_docs(cx);
        let docs = super::docs_panel::DocsState::global(cx);
        cx.observe(&docs, |s, _, cx| s.sync_docs(cx)).detach();
        me
    }

    /// Panel 3 (dokumen) otomatis tersembunyi bila tak ada dokumen terbuka, dan tampil saat ada yang dibuka.
    fn sync_docs(&mut self, cx: &mut Context<Self>) {
        let has_docs = !super::docs_panel::DocsState::global(cx).read(cx).open.is_empty();
        if has_docs == self.app.read(cx).is_collapsed(PanelId::Docs) {
            self.app.update(cx, |a, cx| a.toggle(PanelId::Docs, cx));
        }
    }

    /// Simpan ukuran panel, panel terlipat, dan alat aktif (pengaturan `rememberLayout`).
    fn snapshot(&self, cx: &App) -> String {
        let collapsed: Vec<String> = PANEL_IDS.iter().filter(|p| self.app.read(cx).is_collapsed(**p)).map(|p| format!("{p:?}")).collect();
        serde_json::json!({ "version": 1, "sizes": self.sizes, "collapsed": collapsed, "tool": self.active_tool.key() }).to_string()
    }

    fn persist(&mut self, cx: &mut Context<Self>) {
        if !remember(cx) {
            return;
        }
        let snap = self.snapshot(cx);
        if snap != self.saved {
            let _ = std::fs::write(layout_path(), &snap);
            self.saved = snap;
        }
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        let Ok(text) = std::fs::read_to_string(layout_path()) else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        if let Some(a) = v["sizes"].as_array() {
            if a.len() == 4 && a.iter().all(|x| x.as_f64().is_some_and(|f| f.is_finite() && f > 0.)) {
                for (n, x) in a.iter().enumerate() {
                    self.sizes[n] = x.as_f64().unwrap_or(25.) as f32;
                }
            }
        }
        if let Some(k) = v["tool"].as_str() {
            if let Some(t) = TOOL_IDS.iter().find(|t| t.key() == k) {
                self.active_tool = *t;
            }
        }
        cx.set_global(super::agent_control::ActiveTool(self.active_tool));
        let names: Vec<String> = v["collapsed"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        self.app.update(cx, |a, _| {
            a.collapsed.clear(); // tata letak tersimpan menggantikan tampilan awal sepenuhnya
            for p in PANEL_IDS {
                if names.contains(&format!("{p:?}")) {
                    a.collapsed.insert(p);
                }
            }
        });
        self.saved = text;
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.sizes = DEFAULT_LAYOUT;
        self.persist(cx);
        cx.notify();
    }

    /// Smart resize: sidebar "secukupnya" (16rem, maks 22% grup), sisanya dibagi rata panel utama terbuka.
    pub fn smart_resize(&mut self, cx: &mut Context<Self>) {
        self.auto_smart = true;
        let Some(g) = self.group else { return };
        let scale = cx.global::<Theme>().scale;
        let w: f32 = g.size.width.into();
        self.auto_w = w;
        let side = ((16. * 16. * scale) / w * 100.).min(22.);
        let open: Vec<usize> = (1..4).filter(|i| !self.app.read(cx).is_collapsed(RESIZABLE[*i])).collect();
        let rest = (100. - side) / open.len().max(1) as f32;
        self.sizes[0] = side;
        for i in 1..4 {
            self.sizes[i] = if open.contains(&i) { rest } else { self.sizes[i] };
        }
        self.persist(cx);
        cx.notify();
    }

    pub fn open_tool(&mut self, tool: ToolId, cx: &mut Context<Self>) {
        self.active_tool = tool;
        cx.set_global(super::agent_control::ActiveTool(tool));
        if self.app.read(cx).is_collapsed(PanelId::Tools) {
            self.app.update(cx, |a, cx| a.toggle(PanelId::Tools, cx));
        }
        self.persist(cx);
        cx.notify();
    }

    fn drag_to(&mut self, sep: usize, x: Pixels, open: &[usize], cx: &mut Context<Self>) {
        let Some(g) = self.group else { return };
        let scale = cx.global::<Theme>().scale;
        let gw: f32 = g.size.width.into();
        let total: f32 = open.iter().map(|i| self.sizes[*i]).sum();
        let (a, b) = (open[sep], open[sep + 1]);
        let before: f32 = open[..sep].iter().map(|i| self.sizes[*i]).sum::<f32>() / total * gw;
        let pair = (self.sizes[a] + self.sizes[b]) / total * gw;
        let min_a = min_rem(RESIZABLE[a]) * 16. * scale;
        let min_b = min_rem(RESIZABLE[b]) * 16. * scale;
        let xa: f32 = (x - g.left()).into();
        let new_a = (xa - before).clamp(min_a, (pair - min_b).max(min_a));
        let pa = new_a / gw * total;
        let pb = (pair - new_a) / gw * total;
        self.sizes[a] = pa;
        self.sizes[b] = pb;
        cx.notify();
    }
}

fn panel_empty(t: &Theme, title: String, body: String) -> impl IntoElement {
    div()
        .flex()
        .flex_1()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(t.space(2))
        .p(t.space(5))
        .text_center()
        .child(div().typo(t, H4).text_color(t.colors.fg).child(title))
        .child(div().max_w(t.rem(rem::SPACE[7] * 7.)).typo(t, P).text_color(t.colors.fg_muted).child(body))
}

impl Render for ShellLayout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let c = t.colors.clone();
        let sep_w = t.rem(rem::SEPARATOR);
        let collapsed: Vec<bool> = RESIZABLE.iter().map(|p| self.app.read(cx).is_collapsed(*p)).collect();
        let rail_hidden = self.app.read(cx).is_collapsed(PanelId::Rail);
        let open: Vec<usize> = (0..4).filter(|i| !collapsed[*i]).collect();
        set_panel_shown(PanelShown { docs: !collapsed[2], tools: !collapsed[3] }, cx);
        // Sidebar (P1) mempertahankan lebarnya saat panel lain dibuka/ditutup: porsi sidebar terhadap grup dijaga tetap.
        if let Some(prev) = self.prev_collapsed.as_ref().filter(|p| **p != collapsed) {
            if !prev[0] && !collapsed[0] {
                let old_total: f32 = (0..4).filter(|i| !prev[*i]).map(|i| self.sizes[i]).sum::<f32>().max(1.);
                let frac = (self.sizes[0] / old_total).clamp(0.01, 0.9);
                let others: f32 = (1..4).filter(|i| !collapsed[*i]).map(|i| self.sizes[i]).sum();
                if others > 0. {
                    self.sizes[0] = frac / (1. - frac) * others;
                }
            }
        }
        if self.prev_collapsed.as_ref().is_some_and(|p| *p != collapsed) {
            self.auto_w = 0.; // panel dibuka/ditutup: mode otomatis menghitung ulang
        }
        self.prev_collapsed = Some(collapsed.clone());
        let total: f32 = open.iter().map(|i| self.sizes[*i]).sum::<f32>().max(1.);
        let i18n = cx.global::<I18n>();
        let tool_label = i18n.t(&format!("tools.{}", self.active_tool.key()));
        let close_tools = i18n.t("panel.close.tools");
        let empty_p3 = (i18n.t("empty.p3.title"), i18n.t("empty.p3.body"));
        let rail_labels: Vec<String> = TOOL_IDS.iter().map(|x| i18n.t(&format!("tools.{}", x.key()))).collect();
        let this = cx.entity();

        let frame = |inner: gpui::AnyElement| {
            div()
                .flex()
                .flex_col()
                .size_full()
                .overflow_hidden()
                .border_1()
                .border_color(c.border)
                .rounded(t.rem(rem::RADIUS_LG))
                .bg(c.bg)
                .child(inner)
        };

        let mut group = div()
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                canvas(
                    {
                        let this = this.clone();
                        move |b, _, cx| {
                            let run = this.update(cx, |s, _| {
                                s.group = Some(b);
                                let w: f32 = b.size.width.into();
                                // Mode otomatis: hitung ulang tiap lebar grup berubah (buka, ubah ukuran, maksimalkan,
                                // pulihkan, minimize) sampai pengguna menyeret pemisah; Ctrl+Alt+0 menyalakannya lagi.
                                // Lebar sangat kecil (jendela diminimalkan) dilewati agar ukuran tak rusak.
                                let run = s.auto_smart && w > 100. && (w - s.auto_w).abs() > 1.;
                                if run {
                                    s.auto_w = w;
                                }
                                run
                            });
                            // Ditunda: notify saat prepaint tidak memicu render ulang, sehingga ukuran baru tak tampil.
                            if run {
                                let this = this.clone();
                                cx.defer(move |cx| this.update(cx, |s, cx| s.smart_resize(cx)));
                            }
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );

        for (k, idx) in open.iter().enumerate() {
            let pid = RESIZABLE[*idx];
            let inner: gpui::AnyElement = match pid {
                PanelId::Sidebar => match &self.content.sidebar {
                    Some(v) => v.clone().cached(gpui::StyleRefinement::default().flex().flex_col().size_full()).into_any_element(),
                    None => div().into_any_element(),
                },
                PanelId::Chat => match &self.content.chat {
                    Some(v) => v.clone().cached(gpui::StyleRefinement::default().flex().flex_col().size_full()).into_any_element(),
                    None => div().into_any_element(),
                },
                PanelId::Docs => match &self.content.docs {
                    Some(v) => v.clone().cached(gpui::StyleRefinement::default().flex().flex_col().size_full()).into_any_element(),
                    None => panel_empty(&t, empty_p3.0.clone(), empty_p3.1.clone()).into_any_element(),
                },
                _ => {
                    let app = self.app.clone();
                    let body = self
                        .content
                        .tool
                        .as_ref()
                        .and_then(|f| f(self.active_tool, window, cx))
                        .map(|v| v.into_any_element())
                        .unwrap_or_else(|| div().into_any_element());
                    div()
                        .flex()
                        .flex_1()
                        .flex_col()
                        .min_h_0()
                        .child(
                            // .ptb
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .gap(t.space(2))
                                .h(t.rem(rem::CONTROL_LG))
                                .border_b_1()
                                .border_color(c.border)
                                .child(
                                    div()
                                        .flex()
                                        .flex_1()
                                        .items_center()
                                        .gap(t.space(2))
                                        .min_w_0()
                                        .h_full()
                                        .pl(t.space(2))
                                        .child(crate::ui::icon(self.active_tool.icon()).color(c.fg_muted))
                                        .child(
                                            div()
                                                .flex_1()
                                                .typo(&t, crate::theme::LABEL)
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .whitespace_nowrap()
                                                .child(tool_label.clone()),
                                        ),
                                )
                                .child(
                                    div().flex_none().mr(t.space(2)).child(
                                        icon_button("ptb-close-tools", "close", close_tools.clone())
                                            .size(ControlSize::Sm)
                                            .on_click(move |_, _, cx| app.update(cx, |a, cx| a.toggle(PanelId::Tools, cx))),
                                    ),
                                ),
                        )
                        .child(div().flex().flex_1().flex_col().min_h_0().child(body))
                        .into_any_element()
                }
            };
            let frac = self.sizes[*idx] / total;
            let app_for_focus = self.app.clone();
            group = group.child(
                div()
                    .h_full()
                    .min_w(t.rem(min_rem(pid)))
                    .flex_basis(px(0.))
                    .flex_grow(frac)
                    .flex_shrink(1.)
                    .w_0()
                    // fase capture: panel tercatat aktif walau anaknya menelan klik; klik tidak dihentikan
                    .capture_any_mouse_down(move |_, _, cx| app_for_focus.update(cx, |a, cx| a.set_active_panel(pid, cx)))
                    .child(frame(inner)),
            );

            if k + 1 < open.len() {
                let sep = k;
                let grip_bg = c.fg_subtle;
                let dragging = self.dragging == Some(sep);
                group = group.child(
                    div()
                        .id(("sep", sep))
                        .group("sep")
                        .relative()
                        .flex_none()
                        .w(sep_w)
                        .h_full()
                        .cursor_col_resize()
                        .on_mouse_down(MouseButton::Left, cx.listener(move |s, _, _, cx| {
                            s.dragging = Some(sep);
                            s.auto_smart = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .absolute()
                                .top(gpui::relative(0.5))
                                .left(sep_w / 2. - sep_w / 6.)
                                .mt(-t.space(7) / 2.)
                                .w(sep_w / 3.)
                                .h(t.space(7))
                                .rounded(t.rem(rem::RADIUS_FULL))
                                .bg(grip_bg)
                                .opacity(if dragging { 1. } else { 0. })
                                .group_hover("sep", |s| s.opacity(1.)),
                        )

                );
            }
        }

        let open_for_move = open.clone();
        let mut body = div()
            .flex()
            .size_full()
            .min_h_0()
            .px(sep_w)
            .pb(sep_w)
            .on_mouse_move(cx.listener(move |s, e: &MouseMoveEvent, _, cx| {
                if let Some(sep) = s.dragging {
                    if e.dragging() {
                        s.drag_to(sep, e.position.x, &open_for_move, cx);
                    } else {
                        s.dragging = None;
                        s.persist(cx);
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|s, _, _, cx| {
                s.dragging = None;
                s.persist(cx);
                cx.notify();
            }))
            .child(group);

        if !rail_hidden {
            let tools_open = !self.app.read(cx).is_collapsed(PanelId::Tools);
            let mut rail = div()
                .flex()
                .flex_none()
                .flex_col()
                .items_center()
                .gap(t.space(1))
                .w(t.rem(rem::RAIL_WIDTH))
                .ml(sep_w)
                .pt(t.space(2))
                .border_1()
                .border_color(c.border)
                .rounded(t.rem(rem::RADIUS_LG))
                .bg(c.bg)
                .capture_any_mouse_down({
                    let app = self.app.clone();
                    move |_, _, cx| app.update(cx, |a, cx| a.set_active_panel(PanelId::Rail, cx))
                });
            let rc = cx.try_global::<RailCounts>().copied().unwrap_or_default();
            for (n, tool) in TOOL_IDS.iter().enumerate() {
                let tool = *tool;
                let pressed = tool == self.active_tool && tools_open;
                let count = match tool {
                    ToolId::Terminal => rc.terminal,
                    ToolId::Jobs => self.jobs_badge,
                    ToolId::Browser => rc.browser,
                    ToolId::Changes => rc.changes,
                    _ => 0,
                };
                let label = if count > 0 { format!("{} · {count}", rail_labels[n]) } else { rail_labels[n].clone() };
                let mut b = icon_button(("rail", n), tool.icon(), label)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        // klik ikon yang sedang aktif → tutup Panel 4
                        if tool == s.active_tool && !s.app.read(cx).is_collapsed(PanelId::Tools) {
                            s.app.update(cx, |a, cx| a.toggle(PanelId::Tools, cx));
                            s.persist(cx);
                            cx.notify();
                        } else {
                            s.open_tool(tool, cx);
                        }
                    }));
                if pressed {
                    b = b.variant(ButtonVariant::Outline).pressed(true);
                }
                rail = rail.child(div().relative().flex_none().child(b).when(count > 0, |d| d.child(rail_badge(&t, count))));
            }
            body = body.child(rail);
        }
        body
    }
}


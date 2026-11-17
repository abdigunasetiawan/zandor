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
const RESIZABLE: [PanelId; 4] = [PanelId::Sidebar, PanelId::Chat, PanelId::Docs, PanelId::Tools];
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
            ToolId::Info => "info",
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
        let mut me = Self { app, sizes: DEFAULT_LAYOUT, active_tool: ToolId::Terminal, group: None, dragging: None, content, saved: String::new() };
        if remember(cx) {
            me.restore(cx);
        }
        me
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
        let names: Vec<String> = v["collapsed"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        self.app.update(cx, |a, _| {
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
        let Some(g) = self.group else { return };
        let scale = cx.global::<Theme>().scale;
        let w: f32 = g.size.width.into();
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
                        move |b, _, cx| this.update(cx, |s, _| s.group = Some(b))
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
                    Some(v) => v.clone().into_any_element(),
                    None => div().into_any_element(),
                },
                PanelId::Chat => match &self.content.chat {
                    Some(v) => v.clone().into_any_element(),
                    None => div().into_any_element(),
                },
                PanelId::Docs => match &self.content.docs {
                    Some(v) => v.clone().into_any_element(),
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
            group = group.child(div().h_full().min_w(t.rem(min_rem(pid))).flex_basis(px(0.)).flex_grow(frac).flex_shrink(1.).w_0().child(frame(inner)));

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
                .bg(c.bg);
            for (n, tool) in TOOL_IDS.iter().enumerate() {
                let tool = *tool;
                let pressed = tool == self.active_tool && tools_open;
                let mut b = icon_button(("rail", n), tool.icon(), rail_labels[n].clone())
                    .on_click(cx.listener(move |s, _, _, cx| s.open_tool(tool, cx)));
                if pressed {
                    b = b.variant(ButtonVariant::Outline).pressed(true);
                }
                rail = rail.child(b);
            }
            body = body.child(rail);
        }
        body
    }
}


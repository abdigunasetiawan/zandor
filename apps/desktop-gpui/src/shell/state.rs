//! State aplikasi bersama: navigasi (riwayat rute) dan panel yang dilipat — port navigation.ts + services.
#![allow(dead_code)]
use gpui::{App, AppContext, Context, Entity, EventEmitter, Global};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelId {
    Sidebar,
    Chat,
    Docs,
    Tools,
    Rail,
}
pub const PANEL_IDS: [PanelId; 5] = [PanelId::Sidebar, PanelId::Chat, PanelId::Docs, PanelId::Tools, PanelId::Rail];

impl PanelId {
    pub fn label_key(self) -> &'static str {
        match self {
            PanelId::Sidebar => "panel.sidebar",
            PanelId::Chat => "panel.chat",
            PanelId::Docs => "panel.docs",
            PanelId::Tools => "panel.tools",
            PanelId::Rail => "panel.rail",
        }
    }
    /// Pintasan panel bernomor (ADR-066).
    pub fn shortcut(self) -> &'static str {
        match self {
            PanelId::Sidebar => "Ctrl+1",
            PanelId::Chat => "Ctrl+2",
            PanelId::Docs => "Ctrl+3",
            PanelId::Tools => "Ctrl+4",
            PanelId::Rail => "Ctrl+5",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            PanelId::Sidebar => "panel_left",
            PanelId::Chat => "chat",
            PanelId::Docs => "panel_doc",
            PanelId::Tools => "panel_right",
            PanelId::Rail => "panel_rail",
        }
    }
}
pub const SMART_RESIZE_SHORTCUT: &str = "Ctrl+Alt+0";

pub struct AppState {
    history: Vec<String>,
    index: usize,
    pub collapsed: HashSet<PanelId>,
}

pub enum AppEvent {
    Navigated,
    PanelsChanged,
}
impl EventEmitter<AppEvent> for AppState {}

struct AppStateHandle(Entity<AppState>);
impl Global for AppStateHandle {}

impl AppState {
    pub fn init(start: &str, cx: &mut App) -> Entity<AppState> {
        let e = cx.new(|_| AppState { history: vec![start.to_string()], index: 0, collapsed: HashSet::new() });
        cx.set_global(AppStateHandle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Entity<AppState> {
        cx.global::<AppStateHandle>().0.clone()
    }
    pub fn current(&self) -> &str {
        &self.history[self.index]
    }
    pub fn can_back(&self) -> bool {
        self.index > 0
    }
    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.history.len()
    }
    pub fn push(&mut self, route: impl Into<String>, cx: &mut Context<Self>) {
        let route = route.into();
        if route == self.current() {
            return;
        }
        self.history.truncate(self.index + 1);
        self.history.push(route);
        self.index += 1;
        cx.emit(AppEvent::Navigated);
        cx.notify();
    }
    pub fn back(&mut self, cx: &mut Context<Self>) {
        if self.can_back() {
            self.index -= 1;
            cx.emit(AppEvent::Navigated);
            cx.notify();
        }
    }
    pub fn forward(&mut self, cx: &mut Context<Self>) {
        if self.can_forward() {
            self.index += 1;
            cx.emit(AppEvent::Navigated);
            cx.notify();
        }
    }
    pub fn is_collapsed(&self, p: PanelId) -> bool {
        self.collapsed.contains(&p)
    }
    /// Chat tidak bisa dilipat bila ia satu-satunya panel utama yang terbuka (ADR-015).
    pub fn toggle(&mut self, p: PanelId, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&p) {
            let open_main = [PanelId::Chat, PanelId::Docs, PanelId::Tools].iter().filter(|x| !self.collapsed.contains(x)).count();
            let is_main = matches!(p, PanelId::Chat | PanelId::Docs | PanelId::Tools);
            if is_main && open_main <= 1 {
                return;
            }
            self.collapsed.insert(p);
        }
        cx.emit(AppEvent::PanelsChanged);
        cx.notify();
    }
    pub fn reset_layout(&mut self, cx: &mut Context<Self>) {
        self.collapsed.clear();
        cx.emit(AppEvent::PanelsChanged);
        cx.notify();
    }
}

/// "Ctrl+Alt+0" → "Ctrl+Alt+0" (Windows/Linux) atau simbol ⌘⌥ di macOS — port formatCombo.
pub fn format_combo(combo: &str) -> String {
    if cfg!(target_os = "macos") {
        combo.replace("Ctrl+", "⌘").replace("Alt+", "⌥").replace("Shift+", "⇧")
    } else {
        combo.to_string()
    }
}

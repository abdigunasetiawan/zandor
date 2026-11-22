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

/// Jumlah perintah terakhir yang diingat (palet menampilkan 3 teratas).
pub const RECENT_COMMANDS: usize = 3;

pub struct AppState {
    history: Vec<String>,
    index: usize,
    pub collapsed: HashSet<PanelId>,
    /// Panel yang terakhir disentuh pengguna (klik di dalamnya atau dibuka lewat pintasan).
    pub active_panel: Option<PanelId>,
    /// Id perintah yang terakhir dipakai pengguna, terbaru di depan (tanpa duplikat).
    recent: Vec<String>,
}

fn recent_path() -> std::path::PathBuf {
    crate::core::services::app_data_dir().join("recent-commands.json")
}

/// Pasang `id` di depan daftar terbaru: pindah bila sudah ada, potong sesuai `max`.
pub fn push_recent(list: &mut Vec<String>, id: &str, max: usize) {
    list.retain(|x| x != id);
    list.insert(0, id.to_string());
    list.truncate(max);
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
        let recent: Vec<String> = std::fs::read_to_string(recent_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let e = cx.new(|_| AppState { history: vec![start.to_string()], index: 0, collapsed: HashSet::new(), active_panel: None, recent });
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
    /// Tandai panel terakhir yang disentuh (tidak merender ulang bila tidak berubah).
    pub fn set_active_panel(&mut self, p: PanelId, cx: &mut Context<Self>) {
        if self.active_panel != Some(p) {
            self.active_panel = Some(p);
            cx.notify();
        }
    }
    pub fn recent_commands(&self) -> &[String] {
        &self.recent
    }
    /// Catat perintah yang dijalankan pengguna dan simpan ke disk (galat tulis diabaikan: hanya kenyamanan).
    pub fn record_command(&mut self, id: &str, cx: &mut Context<Self>) {
        push_recent(&mut self.recent, id, RECENT_COMMANDS);
        if let Ok(s) = serde_json::to_string(&self.recent) {
            let _ = std::fs::write(recent_path(), s);
        }
        cx.notify();
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
        } else {
            self.active_panel = Some(p); // panel yang baru dibuka dianggap yang sedang dipakai
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

#[cfg(test)]
mod tests {
    use super::push_recent;

    #[test]
    fn terakhir_dipakai_terbaru_di_depan_tanpa_duplikat_maks_3() {
        let mut l: Vec<String> = vec![];
        for id in ["a", "b", "c", "a", "d"] {
            push_recent(&mut l, id, 3);
        }
        assert_eq!(l, vec!["d", "a", "c"]);
    }
}

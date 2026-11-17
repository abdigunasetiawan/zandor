//! Akar aplikasi — port App.tsx: top bar + isi sesuai rute + toast.
use super::layout::{PanelContent, ShellLayout};
use super::state::{AppState, PanelId};
use super::titlebar::TitleBar;
use crate::i18n::{I18n, Locale};
use crate::theme::{step_scale, Theme, ThemeMode, FONT_UI};
use crate::ui::Toasts;
use gpui::{div, prelude::*, App, Context, Entity, IntoElement, Window};
use std::rc::Rc;

pub struct Root {
    app: Entity<AppState>,
    titlebar: Entity<TitleBar>,
    layout: Entity<ShellLayout>,
    toasts: Entity<Toasts>,
    palette: Option<Entity<super::command_center::CommandCenter>>,
}

impl Root {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let app = AppState::global(cx);
        let toasts = Toasts::init(cx);
        let layout = cx.new(|cx| {
            ShellLayout::new(app.clone(), PanelContent { sidebar: Some(cx.new(|cx| super::sidebar::Sidebar::new(app.clone(), cx)).into()), chat: Some(cx.new(|cx| super::chat_panel::ChatPanel::new(app.clone(), cx)).into()), docs: None, tool: None }, cx)
        });
        let me = cx.entity().downgrade();
        let titlebar = cx.new(|cx| {
            let mut tb = TitleBar::new(app.clone(), cx);
            tb.on_command = Some(Rc::new(move |id: &str, w: &mut Window, cx: &mut App| {
                let id = id.to_string();
                me.update(cx, |r, cx| r.run_command(&id, w, cx)).ok();
            }));
            tb
        });
        cx.observe(&app, |_, _, cx| cx.notify()).detach();
        cx.observe_keystrokes(move |r, ev, w, cx| {
            let combo = super::command_center::combo(&ev.keystroke);
            if let Some(c) = super::commands::COMMANDS.iter().find(|c| c.shortcut == Some(combo.as_str())) {
                let id = c.id;
                r.run_command(id, w, cx);
            }
        })
        .detach();
        let _ = window;
        Self { app, titlebar, layout, toasts, palette: None }
    }

    /// Jalankan perintah dari registry (menu, palet, pintasan).
    pub fn run_command(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let app = self.app.clone();
        let toggle = |p: PanelId, cx: &mut Context<Self>| app.update(cx, |a, cx| a.toggle(p, cx));
        match id {
            "app.palette" | "app.searchSessions" => {
                if self.palette.take().is_none() {
                    let p = cx.new(|cx| super::command_center::CommandCenter::new(window, cx));
                    cx.subscribe_in(&p, window, |r, _, e: &super::command_center::PaletteEvent, w, cx| {
                        use super::command_center::{PaletteAction, PaletteEvent};
                        r.palette = None;
                        match e {
                            PaletteEvent::Close => {}
                            PaletteEvent::Run(PaletteAction::Command(id)) => r.run_command(id, w, cx),
                            PaletteEvent::Run(PaletteAction::Session(k, s)) => {
                                let (k, s) = (k.clone(), s.clone());
                                crate::core::projects::ProjectStore::global(cx).update(cx, |st, cx| st.select(Some(k), Some(s), cx));
                                r.app.update(cx, |a, cx| a.push("/", cx));
                            }
                        }
                        cx.notify();
                    })
                    .detach();
                    self.palette = Some(p);
                }
            }
            "chat.new" => {
                let store = crate::core::projects::ProjectStore::global(cx);
                store.update(cx, |s, cx| s.create_session(crate::core::projects::CHAT_KEY, None, true, cx)).detach();
                app.update(cx, |a, cx| a.push("/", cx));
            }
            "app.openSettings" => app.update(cx, |a, cx| a.push("/settings", cx)),
            "app.providers" => app.update(cx, |a, cx| a.push("/settings/models", cx)),
            "app.memory" => app.update(cx, |a, cx| a.push("/memory", cx)),
            "app.automation" => app.update(cx, |a, cx| a.push("/automation", cx)),
            "app.appearance" => app.update(cx, |a, cx| a.push("/appearance", cx)),
            "app.goHome" => app.update(cx, |a, cx| a.push("/", cx)),
            "help.tour" => app.update(cx, |a, cx| a.push("/welcome", cx)),
            "dev.designSystem" => app.update(cx, |a, cx| a.push("/design-system", cx)),
            "nav.back" => app.update(cx, |a, cx| a.back(cx)),
            "nav.forward" => app.update(cx, |a, cx| a.forward(cx)),
            "view.toggleSidebar" => toggle(PanelId::Sidebar, cx),
            "view.toggleChat" => toggle(PanelId::Chat, cx),
            "view.toggleDocs" => toggle(PanelId::Docs, cx),
            "view.toggleTools" => toggle(PanelId::Tools, cx),
            "view.toggleRail" => toggle(PanelId::Rail, cx),
            "view.smartResize" => self.layout.update(cx, |l, cx| l.smart_resize(cx)),
            "view.resetLayout" => {
                app.update(cx, |a, cx| a.reset_layout(cx));
                self.layout.update(cx, |l, cx| l.reset(cx));
            }
            "view.zoomIn" | "view.zoomOut" | "view.zoomReset" => {
                let t = cx.global::<Theme>();
                let scale = match id {
                    "view.zoomIn" => step_scale(t.scale, 1),
                    "view.zoomOut" => step_scale(t.scale, -1),
                    _ => 1.0,
                };
                let next = Theme::new(t.mode, t.dark, scale);
                cx.set_global(next);
                window.refresh();
            }
            "view.toggleTheme" => {
                let t = cx.global::<Theme>();
                let mode = if t.dark { ThemeMode::Light } else { ThemeMode::Dark };
                let next = Theme::new(mode, t.dark, t.scale);
                cx.set_global(next);
                window.refresh();
            }
            "view.langId" | "view.langEn" => {
                cx.set_global(I18n::new(if id == "view.langEn" { Locale::En } else { Locale::Id }));
                window.refresh();
            }
            "window.close" => window.remove_window(),
            _ => {}
        }
        cx.notify();
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let route = self.app.read(cx).current().to_string();
        let content: gpui::AnyElement = if route == "/" || route.is_empty() {
            self.layout.clone().into_any_element()
        } else {
            // Halaman lain (Pengaturan, Memori, Otomasi, Tampilan, Onboarding) — diport bertahap.
            div().size_full().into_any_element()
        };
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(t.colors.bg)
            .text_color(t.colors.fg)
            .font_family(FONT_UI)
            .text_size(t.rem(crate::theme::P.size))
            .child(self.titlebar.clone())
            .child(div().flex().flex_1().min_h_0().child(content))
            .children(self.palette.clone())
            .child(self.toasts.clone())
    }
}

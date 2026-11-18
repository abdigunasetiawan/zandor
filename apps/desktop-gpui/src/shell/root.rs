//! Akar aplikasi — port App.tsx: top bar + isi sesuai rute + toast.
use super::layout::{PanelContent, ShellLayout};
use super::state::{AppState, PanelId};
use super::titlebar::TitleBar;
use crate::i18n::{I18n, Locale};
use crate::theme::{step_scale, Theme, ThemeMode};
use crate::ui::Toasts;
use gpui::{div, prelude::*, App, Context, Entity, IntoElement, Window};
use std::rc::Rc;

pub struct Root {
    app: Entity<AppState>,
    titlebar: Entity<TitleBar>,
    layout: Entity<ShellLayout>,
    toasts: Entity<Toasts>,
    settings_page: Entity<super::settings_page::SettingsPage>,
    memory_page: Entity<super::memory_page::MemoryPage>,
    automation_page: Entity<super::automation_page::AutomationPage>,
    onboarding_page: Entity<super::onboarding_page::OnboardingPage>,
    palette: Option<Entity<super::command_center::CommandCenter>>,
    gsearch: Option<Entity<super::global_search::GlobalSearch>>,
    imagegen: Option<Entity<super::image_dialog::ImageDialog>>,
}

impl Root {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let app = AppState::global(cx);
        let toasts = Toasts::init(cx);
        let layout = cx.new(|cx| {
            ShellLayout::new(app.clone(), PanelContent { sidebar: Some(cx.new(|cx| super::sidebar::Sidebar::new(app.clone(), cx)).into()), chat: Some(cx.new(|cx| super::chat_panel::ChatPanel::new(app.clone(), cx)).into()), docs: {
                super::docs_panel::DocsState::init(cx);
                Some(cx.new(|cx| super::docs_panel::DocsPanel::new(cx)).into())
            }, tool: {
                let host = cx.new(|cx| super::tools::ToolsHost::new(cx));
                Some(Box::new(move |id, _w: &mut Window, cx: &mut App| {
                    host.update(cx, |h, _| h.tool = id);
                    Some(host.clone().into())
                }))
            } }, cx)
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
            if cx.try_global::<super::keys_section::KeyCapture>().is_some_and(|k| k.0) {
                return; // sedang merekam pintasan baru di Tampilan → Shortcut
            }
            if let Some(c) = super::commands::command_for_combo(&combo, cx) {
                let id = c.id;
                r.run_command(id, w, cx);
            }
        })
        .detach();
        let attention = super::notification_driver::Attention::init(cx);
        cx.observe(&attention, |_, _, cx| cx.notify()).detach();
        cx.observe_window_activation(window, move |_, w, cx| {
            let f = w.is_window_active();
            attention.update(cx, |a, cx| a.set_focused(f, cx));
        })
        .detach();
        let settings = crate::core::settings::SettingsStore::global(cx);
        cx.observe_in(&settings, window, |_, _, w, cx| apply_settings(w, cx)).detach();
        cx.observe_window_appearance(window, |_, w, cx| apply_settings(w, cx)).detach();
        cx.observe_in(&crate::core::appearance::AppearanceStore::global(cx), window, |_, _, w, cx| apply_settings(w, cx)).detach();
        apply_settings(window, cx);
        let lay = layout.clone();
        let app2 = app.clone();
        let settings_page = cx.new(|cx| {
            let mut p = super::settings_page::SettingsPage::new(app.clone(), cx);
            p.on_reset_layout = Some(Rc::new(move |_, cx: &mut App| {
                app2.update(cx, |a, cx| a.reset_layout(cx));
                lay.update(cx, |l, cx| l.reset(cx));
            }));
            p
        });
        let memory_page = cx.new(|cx| super::memory_page::MemoryPage::new(app.clone(), cx));
        let automation_page = cx.new(|cx| super::automation_page::AutomationPage::new(app.clone(), cx));
        let onboarding_page = cx.new(|cx| super::onboarding_page::OnboardingPage::new(app.clone(), cx));
        if crate::core::settings::SettingsStore::global(cx).read(cx).bool("onboarding") {
            app.update(cx, |a, cx| a.push("/welcome", cx));
        }
        Self { app, titlebar, layout, toasts, palette: None, gsearch: None, imagegen: None, settings_page, memory_page, automation_page, onboarding_page }
    }

    /// Jalankan perintah dari registry (menu, palet, pintasan).
    pub fn run_command(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let app = self.app.clone();
        let toggle = |p: PanelId, cx: &mut Context<Self>| app.update(cx, |a, cx| a.toggle(p, cx));
        match id {
            "image.generate" => {
                if self.imagegen.is_none() {
                    let g = cx.new(|cx| super::image_dialog::ImageDialog::new(window, cx));
                    cx.subscribe(&g, |r, _, _: &super::image_dialog::ImageEvent, cx| {
                        r.imagegen = None;
                        cx.notify();
                    })
                    .detach();
                    self.imagegen = Some(g);
                }
                cx.notify();
            }
            "app.searchSessions" => {
                if self.gsearch.take().is_none() {
                    let g = cx.new(|cx| super::global_search::GlobalSearch::new(window, cx));
                    cx.subscribe(&g, |r, _, _: &super::global_search::SearchEvent, cx| {
                        r.gsearch = None;
                        cx.notify();
                    })
                    .detach();
                    self.gsearch = Some(g);
                }
                cx.notify();
            }
            "app.palette" => {
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
                            PaletteEvent::Run(PaletteAction::MessageLink(k, s, m)) => {
                                let store = crate::core::projects::ProjectStore::global(cx);
                                let missing = store.read(cx).sessions.get(k).is_some_and(|g| g.status == crate::core::projects::LoadStatus::Ready && !g.items.iter().any(|x| &x.id == s));
                                if missing {
                                    let title = cx.global::<crate::i18n::I18n>().t("msg.linkMissing");
                                    crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Warning, ..crate::ui::ToastInput::new(title) }, cx);
                                } else {
                                    let (k, s) = (k.clone(), s.clone());
                                    store.update(cx, |st, cx| st.select(Some(k), Some(s), cx));
                                    r.app.update(cx, |a, cx| a.push("/", cx));
                                    cx.set_global(super::chat_panel::MessageFocus(Some(m.clone())));
                                }
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
                set_setting("uiScale", serde_json::json!(scale), cx);
            }
            "view.toggleTheme" => {
                let dark = cx.global::<Theme>().dark;
                set_setting("theme", serde_json::json!(if dark { "light" } else { "dark" }), cx);
            }
            "view.langId" | "view.langEn" => set_setting("language", serde_json::json!(if id == "view.langEn" { "en" } else { "id" }), cx),
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
        } else if route == "/settings" || route == "/appearance" || route.starts_with("/settings/") {
            self.settings_page.clone().into_any_element()
        } else if route == "/memory" {
            self.memory_page.clone().into_any_element()
        } else if route == "/automation" {
            self.automation_page.clone().into_any_element()
        } else if route == "/welcome" {
            self.onboarding_page.clone().into_any_element()
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
            .font_family(t.font_ui.clone())
            .text_size(t.rem(t.ty(crate::theme::P).size))
            .child(self.titlebar.clone())
            .child(div().flex().flex_1().min_h_0().child(content))
            .children(self.palette.clone())
            .children(self.gsearch.clone())
            .children(self.imagegen.clone())
            .child(self.toasts.clone())
    }
}

fn set_setting(key: &str, v: serde_json::Value, cx: &mut App) {
    let st = crate::core::settings::SettingsStore::global(cx);
    if let Err(e) = st.update(cx, |s, cx| s.set(key, v, cx)) {
        eprintln!("[zandor] {e}");
    }
}

/// Terapkan tema, skala, dan bahasa dari pengaturan.
pub fn apply_settings(window: &mut Window, cx: &mut App) {
    let st = crate::core::settings::SettingsStore::global(cx).read(cx);
    let mode = match st.str("theme").as_str() {
        "dark" => ThemeMode::Dark,
        "light" => ThemeMode::Light,
        _ => ThemeMode::System,
    };
    let scale = st.num("uiScale") as f32;
    let locale = if st.str("language") == "en" { Locale::En } else { Locale::Id };
    let system_dark = matches!(window.appearance(), gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark);
    let mut theme = Theme::new(mode, system_dark, if scale > 0. { scale } else { 1.0 });
    {
        let ap = crate::core::appearance::AppearanceStore::global(cx).read(cx);
        ap.apply(&mut theme.colors, theme.dark);
        ap.apply_type(&mut theme);
        ap.apply_editor(&mut theme.colors);
    }
    cx.set_global(theme);
    cx.set_global(I18n::new(locale));
    window.refresh();
}

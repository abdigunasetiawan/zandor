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
    design_page: Entity<super::design_system_page::DesignSystemPage>,
    labs_page: Entity<super::labs_page::LabsPage>,
    onboarding_page: Entity<super::onboarding_page::OnboardingPage>,
    palette: Option<Entity<super::command_center::CommandCenter>>,
    gsearch: Option<Entity<super::global_search::GlobalSearch>>,
    imagegen: Option<Entity<super::image_dialog::ImageDialog>>,
    /// Dialog konfirmasi Dev Tools → hapus semua sesi Chat.
    confirm_chats: bool,
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
                me.update(cx, |r, cx| r.run_user_command(&id, w, cx)).ok();
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
                r.run_user_command(id, w, cx);
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
        let design_page = cx.new(|cx| super::design_system_page::DesignSystemPage::new(app.clone(), cx));
        let labs_page = cx.new(|cx| super::labs_page::LabsPage::new(app.clone(), cx));
        let onboarding_page = cx.new(|cx| super::onboarding_page::OnboardingPage::new(app.clone(), cx));
        if crate::core::settings::SettingsStore::global(cx).read(cx).bool("onboarding") {
            app.update(cx, |a, cx| a.push("/welcome", cx));
        }
        { let t = cx.global::<crate::theme::Theme>().clone(); crate::ui::mermaid::selftest(&t); }
        // Kartu izin domain browser agent: render ulang saat ada permintaan/jawaban.
        cx.set_global(super::browser_agent::PermAsk(None));
        cx.observe_global::<super::browser_agent::PermAsk>(|_, cx| cx.notify()).detach();
        // Diagram Mermaid: webview tersembunyi dipompa berkala (detak cepat hanya saat ada pekerjaan).
        cx.spawn_in(window, async move |this, cx| loop {
            let fast = crate::ui::mermaid::busy();
            cx.background_executor().timer(std::time::Duration::from_millis(if fast { 120 } else { 500 })).await;
            if this.update_in(cx, |_, w, cx| crate::ui::mermaid::pump(w, cx)).is_err() {
                return;
            }
        })
        .detach();
        // Kontrol aplikasi oleh agent (ADR-077): ambil antrean tiap POLL_MS, jalankan, jawab.
        let mut snap_ms: u64 = 0;
        cx.spawn_in(window, async move |this, cx| loop {
            // polling cepat hanya saat agent sedang mengendalikan browser (loop snapshot → aksi)
            let poll = if super::browser_agent::session_active() { 100 } else { super::agent_control::POLL_MS };
            cx.background_executor().timer(std::time::Duration::from_millis(poll)).await;
            // tangkapan halaman Browser (pengganti webview saat disembunyikan menu/dialog): tiap ~2 dtk
            snap_ms += poll;
            if snap_ms >= 2000 {
                snap_ms = 0;
                let _ = this.update_in(cx, |_, _, cx| {
                    if let Some(p) = super::browser_agent::panel(cx) {
                        p.update(cx, |b, cx| b.capture_snapshot(cx));
                    }
                });
            }
            let dir = super::agent_control::control_dir();
            let raw = cx.background_spawn(async move { crate::control::take(&dir) }).await;
            let reqs: Vec<_> = raw.iter().filter_map(super::agent_control::to_request).collect();
            for req in reqs {
                let alive = this.update_in(cx, |r, w, cx| {
                    // perintah palet ditunda: Root sedang di-update, jadi dijalankan langsung setelah execute
                    let mut cmds: Vec<String> = vec![];
                    let mut run = |id: &str, _: &mut Window, _: &mut App| cmds.push(id.to_string());
                    let out = super::agent_control::execute(&req, w, cx, &mut run);
                    for id in cmds {
                        r.run_command(&id, w, cx);
                    }
                    match out {
                        super::agent_control::Outcome::Done(res) => super::agent_control::finish(&req, res, cx),
                        super::agent_control::Outcome::Browser(job) => {
                            let req = req.clone();
                            cx.spawn_in(w, async move |_, cx| {
                                let res = super::browser_agent::run(job, cx).await;
                                // jawaban harus selalu terkirim: coba lagi bila App sedang dipinjam (WebView2 memompa pesan)
                                let res = std::cell::RefCell::new(Some(res));
                                for _ in 0..60 {
                                    if cx.update(|_, cx| super::agent_control::finish(&req, res.borrow_mut().take().unwrap_or(Ok(serde_json::Value::Null)), cx)).is_ok() {
                                        break;
                                    }
                                    cx.background_executor().timer(std::time::Duration::from_millis(50)).await;
                                }
                            })
                            .detach();
                        }
                        super::agent_control::Outcome::Image { provider, model, prompt, size, root, rel } => {
                            // tidak ditunggu: generate gambar bisa lama dan tidak boleh menahan permintaan lain
                            let task = cx.background_spawn(async move { super::image_dialog::generate(&provider, &model, &prompt, &size, &root, &rel).map(|w| (w, root)) });
                            let req = req.clone();
                            cx.spawn_in(w, async move |_, cx| {
                                let r = task.await;
                                cx.update(|_, cx| {
                                    let res = r.map(|(written, root)| {
                                        let full = std::path::Path::new(&root).join(written.replace('/', "\\")).to_string_lossy().into_owned();
                                        super::docs_panel::DocsState::global(cx).update(cx, |d, cx| d.open_file(full, cx));
                                        serde_json::Value::String(format!("Gambar disimpan: {written} (dibuka di panel Dokumen)."))
                                    });
                                    super::agent_control::finish(&req, res, cx);
                                })
                                .ok();
                            })
                            .detach();
                        }
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
        Self { app, titlebar, layout, toasts, palette: None, gsearch: None, imagegen: None, confirm_chats: false, settings_page, design_page, labs_page, onboarding_page }
    }

    /// Perintah yang dipicu pengguna (menu, palet, pintasan): dicatat ke "Terakhir digunakan" lalu dijalankan.
    /// Perintah dari agent (`agent_control`) memakai `run_command` langsung sehingga tidak tercatat.
    pub fn run_user_command(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if id != "app.palette" {
            self.app.update(cx, |a, cx| a.record_command(id, cx));
        }
        self.run_command(id, window, cx);
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
                            PaletteEvent::Run(PaletteAction::Command(id)) => r.run_user_command(id, w, cx),
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
            "dev.labs" => app.update(cx, |a, cx| a.push("/labs", cx)),
            "dev.deleteChats" => {
                if crate::core::projects::ProjectStore::global(cx).read(cx).chat_count() == 0 {
                    let title = cx.global::<I18n>().t("dev.deleteChats.none");
                    Toasts::show(crate::ui::ToastInput::new(title), cx);
                } else {
                    self.confirm_chats = true;
                    cx.notify();
                }
            }
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

impl Root {
    /// Konfirmasi hapus semua sesi Chat (aksi destruktif: selalu lewat dialog komponen, bukan dialog OS).
    fn delete_chats_dialog(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::ui::{button, dialog, ButtonVariant};
        let i = cx.global::<I18n>().clone();
        let n = crate::core::projects::ProjectStore::global(cx).read(cx).chat_count();
        dialog(i.t("dev.deleteChats.title"))
            .description(i.tp("dev.deleteChats.body", &[("n", &n.to_string())]))
            .on_dismiss(cx.listener(|r, _, _, cx| {
                r.confirm_chats = false;
                cx.notify();
            }))
            .footer(
                div()
                    .flex()
                    .justify_end()
                    .gap(t.space(2))
                    .child(button("dch-cancel").label(i.t("ui.cancel")).variant(ButtonVariant::Ghost).on_click(cx.listener(|r, _, _, cx| {
                        r.confirm_chats = false;
                        cx.notify();
                    })))
                    .child(button("dch-ok").label(i.t("dev.deleteChats.confirm")).variant(ButtonVariant::Danger).on_click(cx.listener(|r, _, _, cx| {
                        r.confirm_chats = false;
                        crate::core::projects::ProjectStore::global(cx).update(cx, |s, cx| {
                            s.delete_all_chats(cx);
                        });
                        cx.notify();
                    }))),
            )
            .into_any_element()
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let route = self.app.read(cx).current().to_string();
        if !(route == "/" || route.is_empty()) {
            super::layout::set_panel_shown(super::layout::PanelShown::default(), cx);
        }
        let content: gpui::AnyElement = if route == "/" || route.is_empty() {
            self.layout.clone().into_any_element()
        } else if route == "/settings" || route == "/appearance" || route == "/memory" || route == "/automation" || route.starts_with("/settings/") {
            self.settings_page.clone().into_any_element()
        } else if route == "/design-system" {
            self.design_page.clone().into_any_element()
        } else if route == "/labs" {
            self.labs_page.clone().into_any_element()
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
            // Klik di area GPUI (klik di webview tidak sampai ke sini) → kembalikan fokus keyboard dari webview ke GPUI.
            .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                super::browser::release_webview_focus();
                window.refresh();
            })
            .bg(if t.colors.frame.a == 0. { t.colors.frame } else { t.colors.bg }) // Kaca: latar jendela buram menembus
            .text_color(t.colors.fg)
            .font_family(t.font_ui.clone())
            .text_size(t.rem(t.ty(crate::theme::P).size))
            .child(self.titlebar.clone())
            .child(div().flex().flex_1().min_h_0().child(content))
            .children(self.palette.clone())
            .children(self.gsearch.clone())
            .children(self.imagegen.clone())
            .children(self.confirm_chats.then(|| self.delete_chats_dialog(&t, cx)))
            .children(super::browser_agent::permission_dialog(&t, cx))
            .children(link_menu(&t, cx))
            .children(crate::ui::rich::rich_menu(&t, cx))
            .child(self.toasts.clone())
    }
}

/// Menu tautan respons model (port ChatLink): buka di Browser Zandor, buka di browser eksternal, salin.
fn link_menu(t: &Theme, cx: &mut App) -> Option<gpui::AnyElement> {
    use crate::ui::markdown::LinkMenu;
    let (url, pos) = cx.try_global::<LinkMenu>()?.0.clone()?;
    let i = cx.global::<I18n>().clone();
    let (u1, u2, u3) = (url.clone(), url.clone(), url);
    let entries = vec![
        crate::ui::entry(i.t("link.openZandor"), move |_, cx| super::layout::open_in_zandor_browser(u1.clone(), cx)).icon("globe"),
        crate::ui::entry(i.t("link.openExternal"), move |_, cx| cx.open_url(&u2)).icon("external_link"),
        crate::ui::MenuEntry::Separator,
        crate::ui::entry(i.t("link.copy"), move |_, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(u3.clone()));
            let title = cx.global::<I18n>().t("link.copied");
            Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Success, timeout_ms: Some(1500), ..crate::ui::ToastInput::new(title) }, cx);
        })
        .icon("copy"),
    ];
    Some(
        crate::ui::overlay_deferred(gpui::anchored().position(pos).snap_to_window().child(crate::ui::menu_popup("link-menu", t, entries, gpui::Anchor::TopLeft, |_, cx| {
            cx.set_global(LinkMenu(None));
            cx.refresh_windows();
        })))
        .with_priority(3)
        .into_any_element(),
    )
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
    let surface = if cfg!(windows) { crate::core::settings::SettingsStore::global(cx).read(cx).str("surface") } else { String::new() };
    let glass = matches!(surface.as_str(), "glass" | "mica" | "clear");
    if glass {
        theme.colors.glassify(theme.dark, surface == "clear");
    }
    eprintln!("[zandor] permukaan={surface} alpha_bg={}", theme.colors.bg.a);
    extend_frame(window, glass);
    window.set_background_appearance(match surface.as_str() {
        "glass" => gpui::WindowBackgroundAppearance::Blurred,
        "mica" => gpui::WindowBackgroundAppearance::MicaAltBackdrop,
        "clear" => gpui::WindowBackgroundAppearance::Transparent,
        _ => gpui::WindowBackgroundAppearance::Opaque,
    });
    blur_behind(window, surface == "glass");
    cx.set_global(theme);
    cx.set_global(I18n::new(locale));
    window.refresh();
}

/// Windows: bahan latar DWM (Mica/Acrylic) hanya tampil bila bingkai diperluas ke area klien;
/// tanpa ini jendela tanpa-bingkai-bawaan tampak hitam di balik panel transparan.
fn extend_frame(window: &Window, on: bool) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE};
        use windows_sys::Win32::UI::Controls::MARGINS;
        let Ok(h) = HasWindowHandle::window_handle(window) else { return };
        if let RawWindowHandle::Win32(w) = h.as_raw() {
            // Lepas bahan latar sisa (mis. Mica) sebelum mode baru dipasang; GPUI tidak menghapusnya.
            let none: u32 = 1; // DWMSBT_NONE
            let r1 = unsafe { DwmSetWindowAttribute(w.hwnd.get() as _, DWMWA_SYSTEMBACKDROP_TYPE as u32, (&none as *const u32).cast(), 4) };
            let n = if on { -1 } else { 0 };
            let m = MARGINS { cxLeftWidth: n, cxRightWidth: n, cyTopHeight: n, cyBottomHeight: n };
            let r2 = unsafe { DwmExtendFrameIntoClientArea(w.hwnd.get() as _, &m) };
            let msg = format!("kaca: aktif={on} backdrop_none=0x{r1:x} extend=0x{r2:x} hwnd={:?}", w.hwnd);
            eprintln!("[zandor] {msg}");
            if let Ok(d) = std::env::var("TEMP") {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(format!("{d}/zandor-kaca.log")) {
                    let _ = writeln!(f, "{msg}");
                }
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (window, on);
}

/// Windows: blur-behind klasik DWM (tanpa tint). Cadangan bila acrylic dari GPUI tidak tampil.
fn blur_behind(window: &Window, on: bool) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::Graphics::Dwm::{DwmEnableBlurBehindWindow, DWM_BB_ENABLE, DWM_BLURBEHIND};
        let Ok(h) = HasWindowHandle::window_handle(window) else { return };
        if let RawWindowHandle::Win32(w) = h.as_raw() {
            let bb = DWM_BLURBEHIND { dwFlags: DWM_BB_ENABLE, fEnable: on as i32, hRgnBlur: std::ptr::null_mut(), fTransitionOnMaximized: 0 };
            let r = unsafe { DwmEnableBlurBehindWindow(w.hwnd.get() as _, &bb) };
            eprintln!("[zandor] blur_behind={on} hr=0x{r:x}");
        }
    }
    #[cfg(not(windows))]
    let _ = (window, on);
}

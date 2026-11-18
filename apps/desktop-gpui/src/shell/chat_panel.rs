//! ChatPanel — port shell/ChatPanel.tsx + AssistantMessage, ToolStep, QuestionCard, kartu izin, composer.
use super::state::AppState;
use crate::core::chat::{message_text, ChatState};
use crate::core::chat_store::ChatStore;
use crate::core::projects::{session_title, ProjectStore, CHAT_KEY};
use crate::core::services::EngineStatus;
use crate::core::types::*;
use crate::i18n::I18n;
use crate::theme::{rem, Theme};
use crate::ui::{
    badge, button, icon, icon_button, markdown, working, BadgeTone, ButtonVariant, ControlSize, IconSize, InputSubmit, TextInput,
    TypeExt,
};
use gpui::{
    div, prelude::*, AnyElement, App, ClipboardItem, Context, Entity, FontWeight, IntoElement, ScrollHandle, SharedString,
    WeakEntity, Window,
};
use std::collections::HashSet;

#[path = "composer_assist.rs"]
mod composer_assist;
#[path = "queue_dock.rs"]
mod queue_dock;
#[path = "chat_parts.rs"]
mod chat_parts;

pub struct ChatPanel {
    app: Entity<AppState>,
    store: Entity<ProjectStore>,
    chat: Entity<ChatStore>,
    engine: Entity<EngineStatus>,
    input: Entity<TextInput>,
    scroll: ScrollHandle,
    focused_msg: Option<String>,
    expanded: HashSet<String>,
    last_count: usize,
    pending_new: bool,
    models: Vec<crate::core::engine::ModelOption>,
    models_loaded: bool,
    assist: composer_assist::AssistState,
    attach_menu: bool,
    voice_menu: bool,
    ticking: bool,
    voice_devices: Vec<crate::voice::AudioInput>,
    voice_device: String,
    queue: queue_dock::QueueState,
}

impl ChatPanel {
    pub fn new(app: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.set_global(MessageFocus(None));
        cx.observe_global::<MessageFocus>(|_, cx| cx.notify()).detach();
        let store = ProjectStore::global(cx);
        let chat = ChatStore::global(cx);
        let engine = EngineStatus::global(cx);
        let ph = cx.global::<I18n>().t("chat.placeholder");
        let input = cx.new(|cx| {
            let mut ti = TextInput::new(cx, ph);
            ti.multiline = true; // Enter kirim, Shift+Enter baris baru
            ti
        });
        cx.observe(&store, |this, _, cx| {
            this.sync(cx);
            cx.notify();
        })
        .detach();
        cx.observe(&chat, |_, _, cx| cx.notify()).detach();
        cx.observe(&engine, |_, _, cx| cx.notify()).detach();
        cx.observe(&crate::core::profile::Profile::global(cx), |_, _, cx| cx.notify()).detach();
        cx.subscribe(&input, |this, _, e: &InputSubmit, cx| this.submit(e.0.to_string(), cx)).detach();
        cx.subscribe(&input, |this, _, e: &crate::ui::InputNav, cx| this.on_nav(*e, cx)).detach();
        cx.subscribe(&input, |this, _, _: &crate::ui::InputChanged, cx| {
            this.assist.active = 0;
            cx.notify();
        })
        .detach();
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        let mut me = Self {
            app,
            store,
            chat,
            engine,
            input,
            scroll: ScrollHandle::new(),
            focused_msg: None,
            expanded: HashSet::new(),
            last_count: 0,
            pending_new: false,
            models: vec![],
            models_loaded: false,
            assist: Default::default(),
            attach_menu: false,
            voice_menu: false,
            ticking: false,
            voice_devices: vec![],
            voice_device: crate::core::chat_store::read_pref(crate::voice::DEVICE_KEY).unwrap_or_default(),
            queue: Default::default(),
        };
        me.sync(cx);
        me
    }

    /// Sesi terpilih: (kunci, id, direktori).
    fn current(&self, cx: &App) -> Option<(String, String, String, String)> {
        let (key, s, p) = self.store.read(cx).selected()?;
        let dir = if !s.directory.is_empty() { s.directory.clone() } else { p.map(|p| p.path).unwrap_or_default() };
        let title = session_title(&s).map(str::to_string).unwrap_or_default();
        Some((key, s.id, dir, title))
    }

    fn load_models(&mut self, cx: &mut Context<Self>) {
        if self.models_loaded {
            return;
        }
        let Some(api) = self.store.read(cx).api() else { return };
        self.models_loaded = true;
        let task = cx.background_spawn(async move { api.list_models().unwrap_or_default() });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            this.update(cx, |s, cx| {
                if s.chat.read(cx).model.is_none() {
                    let saved = crate::core::chat_store::read_pref("zandor.model");
                    let pick = saved
                        .and_then(|k| list.iter().find(|m| format!("{}/{}", m.provider_id, m.model_id) == k))
                        .or(list.first());
                    if let Some(m) = pick {
                        let r = crate::core::engine::ModelRef { provider_id: m.provider_id.clone(), model_id: m.model_id.clone() };
                        s.chat.update(cx, |c, _| c.model = Some(r));
                    }
                }
                s.models_loaded = !list.is_empty();
                s.models = list;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        self.load_models(cx);
        if let Some((_, id, dir, _)) = self.current(cx) {
            if !dir.is_empty() {
                self.chat.update(cx, |c, cx| c.open(&dir, &id, cx));
            }
        }
    }

    fn pick_voice_device(&mut self, id: String, cx: &mut Context<Self>) {
        crate::core::chat_store::write_pref(crate::voice::DEVICE_KEY, &id);
        self.voice_device = id;
        self.voice_menu = false;
        cx.notify();
    }

    fn submit(&mut self, text: String, cx: &mut Context<Self>) {
        let mut text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(ps) = crate::core::composer::parse_slash(&text) {
            return self.run_slash(ps, cx);
        }
        if text.starts_with("//") {
            text.remove(0); // `//` = garis miring apa adanya
        }
        if let Some((_, id, _, _)) = self.current(cx) {
            if self.chat.read(cx).sessions.get(&id).is_some_and(|s| s.busy()) {
                // agent sedang bekerja: pesan masuk antrean (dok di atas composer)
                let files = self.assist.files.clone();
                match self.chat.update(cx, |c, cx| c.enqueue(&id, &text, files, cx)) {
                    Ok(_) => {
                        self.input.update(cx, |i, cx| i.set_text("", cx));
                        self.assist.files.clear();
                    }
                    Err(e) => {
                        let msg = cx.global::<I18n>().tp("chat.sendError", &[("error", &e)]);
                        composer_assist::note(cx, msg, crate::ui::ToastTone::Danger);
                    }
                }
                return;
            }
        }
        self.input.update(cx, |i, cx| i.set_text("", cx));
        let files = std::mem::take(&mut self.assist.files);
        if let Some((_, id, _, _)) = self.current(cx) {
            self.chat.update(cx, |c, cx| c.send_with(&id, text, files, cx));
            return;
        }
        // Tanpa sesi: buat sesi Chat lalu kirim.
        if self.pending_new {
            return;
        }
        self.pending_new = true;
        let task = self.store.update(cx, |s, cx| s.create_session(CHAT_KEY, None, true, cx));
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |me, cx| {
                me.pending_new = false;
                match r {
                    Ok(s) => {
                        me.sync(cx);
                        me.chat.update(cx, |c, cx| c.send_with(&s.id, text, files, cx));
                    }
                    Err(e) => {
                        crate::ui::Toasts::show(
                        crate::ui::ToastInput {
                            title: e.into(),
                            description: None,
                            tone: crate::ui::ToastTone::Danger,
                            timeout_ms: None,
                            action: None,
                        },
                        cx,
                    );
                    }
                }
            })
            .ok();
        })
        .detach();
    }
}

/// .msg__text / .msg .md: gaya teks pesan dari Tampilan → Panel chat.
fn chat_text(d: gpui::Div, t: &Theme) -> gpui::Div {
    let c = &t.chat;
    let p = t.ty(crate::theme::P);
    d.text_size(t.rem(c.size.unwrap_or(p.size)))
        .font_weight(FontWeight(c.weight.unwrap_or(p.weight)))
        .line_height(gpui::relative(c.line.unwrap_or(p.lh)))
        .opacity(c.opacity.unwrap_or(1.))
}

/// Tema untuk isi pesan: ukuran kode mengikuti --z-chat-code-size.
fn msg_theme(t: &Theme) -> Theme {
    let mut m = t.clone();
    if let Some(s) = t.chat.code_size {
        m.types[crate::theme::CODE.id].size = s;
    }
    m
}

/// Lebar kolom isi chat (rem): gaya Tampilan, atau pengaturan chatWidth.
fn chat_width(t: &Theme, cx: &App) -> f32 {
    t.chat.width.unwrap_or_else(|| match crate::core::settings::SettingsStore::global(cx).read(cx).str("chatWidth").as_str() {
        "narrow" => 38.,
        "wide" => 56.,
        _ => rem::CHAT_WIDTH,
    })
}

fn user_msg(t: &Theme, m: &ChatMessage, id: SharedString, i: &I18n) -> AnyElement {
    let text = message_text(m);
    let copy = text.clone();
    div()
        .id(id.clone())
        .group("umsg")
        .flex()
        .flex_col()
        .items_end()
        .gap(t.space(2))
        .self_end()
        .max_w(gpui::relative(0.85))
        .child(
            div()
                .px(t.space(3))
                .py(t.space(2))
                .rounded(t.rem(rem::RADIUS_LG))
                .bg(t.colors.bubble)
                .child(chat_text(div(), t).child(text)),
        )
        .child(
            div().flex().justify_end().h(t.rem(rem::CONTROL_SM)).invisible().group_hover("umsg", |s| s.visible()).child(
                icon_button(SharedString::from(format!("{id}-copy")), "copy", i.t("chat.copy"))
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))),
            ),
        )
        .into_any_element()
}

fn card(t: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(t.space(2))
        .p(t.space(3))
        .rounded(t.rem(rem::RADIUS_LG))
        .border_1()
        .border_color(t.colors.border)
        .bg(t.colors.raised)
}

impl Render for ChatPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i_owned = cx.global::<I18n>().clone();
        let i = &i_owned;
        let me = cx.entity().downgrade();
        let cur = self.current(cx);
        let engine_ready = self.engine.read(cx).state.info.is_some();
        let project_name = {
            let s = self.store.read(cx);
            s.selected().and_then(|(_, _, p)| p.map(|p| p.name))
        };
        let state: Option<ChatState> = cur.as_ref().and_then(|(_, id, _, _)| self.chat.read(cx).sessions.get(id).cloned());

        // ---- kepala ----
        let head = cur.as_ref().map(|(_, _, _, title)| {
            let title = if title.is_empty() { i.t("sb.untitled") } else { title.clone() };
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(t.space(2))
                .px(t.space(3))
                .py(t.space(2))
                .border_b_1()
                .border_color(t.colors.border)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().font_weight(FontWeight::SEMIBOLD).child(title))
                        .child(div().truncate().small(&t).text_color(t.colors.fg_muted).child(match &project_name {
                            Some(n) => i.tp("chat.inProject", &[("name", n)]),
                            None => i.t("chat.noProject"),
                        })),
                )
        });

        let busy = state.as_ref().is_some_and(|s| s.busy());
        // "Bekerja selama …" diperbarui tiap detik hanya saat sesi berjalan (useNow).
        if busy && !self.ticking {
            self.ticking = true;
            cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                let go = this.update(cx, |s, cx| {
                    let busy = s.current(cx).and_then(|c| s.chat.read(cx).sessions.get(&c.1).map(|x| x.busy())).unwrap_or(false);
                    if !busy {
                        s.ticking = false;
                    }
                    cx.notify();
                    busy
                });
                if !matches!(go, Ok(true)) {
                    break;
                }
            })
            .detach();
        }
        let voice_btn = {
            let ok = crate::voice::supported();
            icon_button("chat-voice", "mic", if ok { i.t("voice.start") } else { i.t("voice.unsupported") })
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .disabled(!ok || !engine_ready)
                .on_click(cx.listener(|s, _, w, cx| {
                    let fh = gpui::Focusable::focus_handle(s.input.read(cx), cx);
                    fh.focus(w, cx);
                    if let Err(e) = crate::voice::toggle_dictation() {
                        let msg = cx.global::<I18n>().tp("voice.error", &[("error", &e)]);
                        crate::ui::Toasts::show(crate::ui::ToastInput { title: msg.into(), description: None, tone: crate::ui::ToastTone::Danger, timeout_ms: None, action: None }, cx);
                        return;
                    }
                    let devs = crate::voice::audio_inputs();
                    if let Some(name) = crate::voice::chosen_not_default(&s.voice_device, &devs) {
                        let i = cx.global::<I18n>();
                        let title = i.tp("voice.notDefault", &[("device", &name)]);
                        let label = i.t("voice.openSound");
                        crate::ui::Toasts::show(
                            crate::ui::ToastInput {
                                title: title.into(),
                                description: None,
                                tone: crate::ui::ToastTone::Warning,
                                timeout_ms: None,
                                action: Some(crate::ui::ToastAction { label: label.into(), on_click: std::rc::Rc::new(|_, cx| cx.open_url("ms-settings:sound")) }),
                            },
                            cx,
                        );
                    }
                }))
        };
        let voice_dev = {
            let me = cx.entity().downgrade();
            let cur = self.voice_device.clone();
            let mut entries = vec![{
                let me = me.clone();
                crate::ui::entry(i.t("voice.defaultDevice"), move |_, cx| {
                    me.update(cx, |s, cx| s.pick_voice_device(String::new(), cx)).ok();
                })
                .checked(cur.is_empty())
            }];
            if self.voice_devices.is_empty() {
                entries.push(crate::ui::entry(i.t("voice.noDevice"), |_, _| {}).disabled(true));
            }
            for (n, d) in self.voice_devices.iter().enumerate() {
                let me = me.clone();
                let id = d.id.clone();
                let label = if d.label.is_empty() { i.tp("voice.deviceN", &[("n", &(n + 1).to_string())]) } else { d.label.clone() };
                entries.push(crate::ui::entry(label, move |_, cx| {
                    me.update(cx, |s, cx| s.pick_voice_device(id.clone(), cx)).ok();
                })
                .checked(cur == d.id));
            }
            let ok = crate::voice::supported();
            div()
                .relative()
                .child(
                    icon_button("chat-voice-dev", "chevron_down", i.t("voice.device"))
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .disabled(!ok)
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.voice_menu = !s.voice_menu;
                            if s.voice_menu {
                                s.voice_devices = crate::voice::audio_inputs();
                            }
                            cx.notify();
                        })),
                )
                .when(self.voice_menu, |d| {
                    d.child(crate::ui::menu_popup("m-voice", &t, entries, gpui::Anchor::BottomRight, move |_, cx| {
                        me.update(cx, |s, cx| {
                            s.voice_menu = false;
                            cx.notify();
                        })
                        .ok();
                    }))
                })
        };
        let send_btn = if busy {
            let id = cur.as_ref().map(|c| c.1.clone()).unwrap_or_default();
            icon_button("chat-stop", "stop", i.t("chat.stop"))
                .variant(ButtonVariant::Danger)
                .on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.abort(&id, cx))))
        } else {
            icon_button("chat-send", "arrow_up", i.t("chat.send"))
                .variant(ButtonVariant::Solid)
                .disabled(!engine_ready)
                .on_click(cx.listener(|s, _, _, cx| {
                    let text = s.input.read(cx).text().to_string();
                    s.submit(text, cx);
                }))
        };
        let variant_sel = {
            let c = self.chat.read(cx);
            let cur_m = c.model.clone();
            let vars = cur_m
                .and_then(|r| self.models.iter().find(|m| m.provider_id == r.provider_id && m.model_id == r.model_id))
                .map(|m| m.variants.clone())
                .unwrap_or_default();
            (!vars.is_empty()).then(|| {
                let mut items = vec![(SharedString::from(""), SharedString::from(i.t("chat.variant.default")))];
                items.extend(vars.iter().map(|v| (SharedString::from(v.clone()), SharedString::from(v.clone()))));
                let chat = self.chat.clone();
                crate::ui::select("chat-variant", items, c.variant.clone().unwrap_or_default())
                    .on_change(move |v, _, cx| chat.update(cx, |c, cx| c.set_variant((!v.is_empty()).then(|| v.to_string()), cx)))
            })
        };
        let can_queue = busy && !self.input.read(cx).text().trim().is_empty();
        let add_btn = can_queue.then(|| {
            icon_button("chat-queue", "arrow_up", i.t("queue.add")).variant(ButtonVariant::Solid).on_click(cx.listener(|s, _, _, cx| {
                let text = s.input.read(cx).text().to_string();
                s.submit(text, cx);
            }))
        });
        let model_sel = (!self.models.is_empty()).then(|| {
            let cur = self.chat.read(cx).model.clone().map(|m| format!("{}/{}", m.provider_id, m.model_id)).unwrap_or_default();
            let items = self.models.iter().map(|m| (SharedString::from(format!("{}/{}", m.provider_id, m.model_id)), SharedString::from(format!("{} · {}", m.name, m.provider_name)))).collect();
            let chat = self.chat.clone();
            crate::ui::select("chat-model", items, cur).on_change(move |v, _, cx| {
                if let Some((p, m)) = v.split_once('/') {
                    let r = crate::core::engine::ModelRef { provider_id: p.to_string(), model_id: m.to_string() };
                    chat.update(cx, |c, cx| c.select_model(r, cx));
                }
            })
        });
        let cw = chat_width(&t, cx);
        let ratio = t.chat.composer_ratio.unwrap_or(rem::CHAT_COMPOSER_RATIO);
        let sug = self.suggest(cx);
        let intercept = sug.as_ref().is_some_and(|s| !s.entries.is_empty() || s.status.is_some());
        if self.input.read(cx).intercept != intercept {
            self.input.update(cx, |inp, _| inp.intercept = intercept);
        }
        let sug_el = sug.as_ref().map(|s| self.render_suggest(s, cx));
        let chips = (!self.assist.files.is_empty()).then(|| {
            let mut row = div().flex().flex_wrap().gap(t.space(1));
            for (n, f) in self.assist.files.iter().enumerate() {
                let name = f.filename.clone().unwrap_or_else(|| f.url.clone());
                row = row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(t.space(1))
                        .pl(t.space(2))
                        .rounded(t.rem(rem::RADIUS_MD))
                        .border_1()
                        .border_color(t.colors.border_soft)
                        .bg(t.colors.surface)
                        .small(&t)
                        .child(icon("paperclip").size(IconSize::Sm))
                        .child(div().max_w(t.rem(14.)).truncate().child(name.clone()))
                        .child(
                            icon_button(SharedString::from(format!("chip-x-{n}")), "close", i.tp("chat.removeAttachment", &[("name", &name)]))
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Ghost)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    if n < s.assist.files.len() {
                                        s.assist.files.remove(n);
                                    }
                                    cx.notify();
                                })),
                        ),
                );
            }
            let total = self.assist.files.len().to_string();
            row.child(
                icon_button("chip-clear", "close", i.tp("chat.removeAllAttachments", &[("n", &total)]))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.assist.files.clear();
                        cx.notify();
                    })),
            )
        });
        let attach_btn = {
            let me2 = me.clone();
            let entries = vec![
                crate::ui::entry(i.t("chat.attach"), {
                    let me = me.clone();
                    move |_, cx| {
                        me.update(cx, |s, cx| s.attach(false, cx)).ok();
                    }
                })
                .icon("paperclip"),
                crate::ui::entry(i.t("chat.attachFolder"), {
                    let me = me.clone();
                    move |_, cx| {
                        me.update(cx, |s, cx| s.attach(true, cx)).ok();
                    }
                })
                .icon("folder"),
            ];
            div()
                .relative()
                .child(
                    icon_button("chat-attach", "plus", i.t("chat.attach"))
                        .size(ControlSize::Sm)
                        .disabled(!engine_ready)
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.attach_menu = !s.attach_menu;
                            cx.notify();
                        })),
                )
                .when(self.attach_menu, |d| {
                    d.child(crate::ui::menu_popup("m-attach", &t, entries, gpui::Anchor::BottomLeft, move |_, cx| {
                        me2.update(cx, |s, cx| {
                            s.attach_menu = false;
                            cx.notify();
                        })
                        .ok();
                    }))
                })
        };
        let mode_sel = {
            let chat = self.chat.clone();
            let items = crate::core::composer::PERMISSION_MODES
                .iter()
                .map(|m| (SharedString::from(*m), SharedString::from(i.t(&format!("chat.mode.{m}")))))
                .collect();
            crate::ui::select("chat-mode", items, self.chat.read(cx).permission_mode.clone())
                .on_change(move |v, _, cx| chat.update(cx, |c, cx| c.set_permission_mode(&v, cx)))
        };
        let qdock = cur.as_ref().and_then(|c| self.render_queue(&c.1.clone(), cx));
        let composer = div().w_full().px(t.space(3)).pb(t.space(3)).flex().flex_col().gap(t.space(2)).children(qdock).children(sug_el).children(chips).child(
            div()
                .flex()
                .flex_col()
                .gap(t.space(2))
                .p(t.space(3))
                .border_1()
                .border_color(t.colors.border_soft)
                .rounded(t.rem(rem::RADIUS_LG * 1.5))
                .bg(t.colors.composer)
                .shadow(t.shadow_1())
                .child(self.input.clone())
                .child(div().flex().items_center().gap(t.space(2)).child(attach_btn).child(mode_sel).child(div().flex_1()).children(model_sel).children(variant_sel).child(div().flex().items_center().child(voice_btn).child(voice_dev)).children(add_btn).child(send_btn)),
        );

        let Some(st) = state else {
            // ---- sambutan ----
            let hour = {
                let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                ((secs / 3600 + 7) % 24) as u32
            };
            let greet = match hour {
                4..=10 => "chat.greet.morning",
                11..=14 => "chat.greet.afternoon",
                15..=17 => "chat.greet.evening",
                _ => "chat.greet.night",
            };
            return div()
                .flex()
                .flex_col()
                .size_full()
                .children(head)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap(t.space(4))
                        .p(t.space(4))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(div().typo(&t, crate::theme::H2).child(i.tp(greet, &[("name", &format!(", {}", crate::core::profile::Profile::global(cx).read(cx).display_name()))])))
                                .child(div().text_color(t.colors.fg_muted).child(i.t(if engine_ready { "chat.greet.sub" } else { "chat.engineDown" }))),
                        )
                        .child(div().w_full().max_w(t.rem(cw * ratio)).child(composer)),
                )
                .into_any_element();
        };
        let sid = st.session_id.clone();
        let has_project = cur.as_ref().is_some_and(|c| c.0 != CHAT_KEY);

        // ---- isi ----
        let mut body = div()
            .id("chat-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(t.space(3));
        let mut inner = div().flex().flex_col().gap(t.rem(t.chat.gap.unwrap_or(0.75))).w_full().max_w(t.rem(cw)).mx_auto().when_some(t.chat.font.clone(), |d, f| d.font_family(f));
        if !st.loaded {
            inner = inner.child(div().text_color(t.colors.fg_muted).child(i.t("chat.loading")));
        } else if st.messages.is_empty() {
            inner = inner.child(div().text_color(t.colors.fg_muted).child(i.t("chat.empty")));
        }
        let mut count = 0;
        // Pesan menjadi anak langsung area gulir agar bisa dituju (ScrollHandle::scroll_to_top_of_item) dari
        // Trajectory "Ke pesan" (port requestMessageFocus).
        let gap = t.rem(t.chat.gap.unwrap_or(0.75));
        let focus = cx.try_global::<MessageFocus>().and_then(|f| f.0.clone());
        let turns = crate::core::chat::merge_assistant_turns(st.visible());
        let last_assistant = turns.iter().rposition(|x| x.message.info.role == Role::Assistant);
        let active_ix = if busy && last_assistant.is_some() && last_assistant == Some(turns.len().saturating_sub(1)) { last_assistant } else { None };
        let models = self.models.clone();
        for (ix, turn) in turns.iter().enumerate() {
            let m = &turn.message;
            count += m.parts.len() + 1;
            let streaming = active_ix == Some(ix);
            let el = match m.info.role {
                Role::User => user_msg(&t, m, SharedString::from(format!("m-{}", m.info.id)), i),
                Role::Assistant => {
                    let pid = m.info.provider_id.clone();
                    let provider_name = models.iter().find(|x| Some(&x.provider_id) == pid.as_ref()).map(|x| x.provider_name.clone()).or(pid).unwrap_or_default();
                    chat_parts::assistant_msg(&t, i, cx, &me, turn, &self.expanded, streaming, has_project, &provider_name, None)
                }
            };
            if focus.as_ref().is_some_and(|f| turn.source_ids.contains(f)) {
                self.scroll.scroll_to_top_of_item(ix);
                self.focused_msg = Some(m.info.id.clone());
                cx.set_global(MessageFocus(None));
            }
            let hl = self.focused_msg.as_deref() == Some(m.info.id.as_str());
            body = body.child(
                div()
                    .w_full()
                    .max_w(t.rem(cw))
                    .mx_auto()
                    .mb(gap)
                    .rounded(t.rem(crate::theme::rem::RADIUS_LG))
                    .when(hl, |d| d.border_1().border_color(t.colors.focus))
                    .when_some(t.chat.font.clone(), |d, f| d.font_family(f))
                    .child(el),
            );
        }
        if let SessionRunStatus::Retry { attempt, message } = &st.status {
            inner = inner.child(div().text_color(t.colors.warning).child(i.tp("chat.retry", &[("attempt", &attempt.to_string()), ("message", message)])));
        } else if busy {
            inner = inner.child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .text_color(t.colors.fg_muted)
                    .child(working("chat-working"))
                    .child(i.t("chat.working")),
            );
        }
        if let Some(e) = &st.error {
            inner = inner.child(div().text_color(t.colors.danger).child(e.clone()));
        }
        // ---- izin ----
        for p in &st.permissions {
            let mk = |r: PermissionReply, label: &str, v: ButtonVariant| {
                let (sid, rid) = (sid.clone(), p.id.clone());
                button(SharedString::from(format!("pm-{}-{}", p.id, r.as_str())))
                    .label(i.t(label))
                    .variant(v)
                    .size(ControlSize::Sm)
                    .on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.reply_permission(&sid, &rid, r, cx))))
            };
            inner = inner.child(
                card(&t)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(t.space(2))
                            .child(icon("shield").size(IconSize::Md).color(t.colors.warning))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(i.tp("chat.permission.title", &[("permission", &p.permission)]))),
                    )
                    .children((!p.patterns.is_empty()).then(|| div().code(&t).text_color(t.colors.fg_muted).child(p.patterns.join("\n"))))
                    .child(
                        div()
                            .flex()
                            .gap(t.space(2))
                            .child(mk(PermissionReply::Once, "chat.permission.once", ButtonVariant::Solid))
                            .child(mk(PermissionReply::Always, "chat.permission.always", ButtonVariant::Outline))
                            .child(mk(PermissionReply::Reject, "chat.permission.reject", ButtonVariant::Ghost)),
                    ),
            );
        }
        // ---- pertanyaan ----
        for q in &st.questions {
            let mut c = card(&t).child(
                div()
                    .flex()
                    .items_center()
                    .gap(t.space(2))
                    .child(icon("info").size(IconSize::Md).color(t.colors.accent))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(i.t("q.title"))),
            );
            let total = q.questions.len();
            for (qi, item) in q.questions.iter().enumerate() {
                c = c
                    .when(!item.header.is_empty(), |d| d.child(div().small(&t).text_color(t.colors.fg_subtle).child(item.header.clone())))
                    .child(div().child(item.question.clone()))
                    .child(div().small(&t).text_color(t.colors.fg_subtle).child(i.t(if item.multiple { "q.multiHint" } else { "q.singleHint" })));
                let mut opts = div().flex().flex_col().gap(t.space(1));
                for (oi, o) in item.options.iter().enumerate() {
                    let (sid, rid, label) = (sid.clone(), q.id.clone(), o.label.clone());
                    opts = opts.child(
                        div()
                            .id(SharedString::from(format!("qo-{}-{qi}-{oi}", q.id)))
                            .flex()
                            .flex_col()
                            .px(t.space(2))
                            .py(t.space(1))
                            .rounded(t.rem(rem::RADIUS_MD))
                            .border_1()
                            .border_color(t.colors.border_soft)
                            .cursor_pointer()
                            .hover(|s| s.bg(t.colors.hover))
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let mut ans = vec![vec![]; total];
                                ans[qi] = vec![label.clone()];
                                s.chat.update(cx, |c, cx| c.reply_question(&sid, &rid, Some(ans), cx));
                            }))
                            .child(div().font_weight(FontWeight::MEDIUM).child(o.label.clone()))
                            .when(!o.description.is_empty(), |d| d.child(div().small(&t).text_color(t.colors.fg_muted).child(o.description.clone()))),
                    );
                }
                c = c.child(opts);
            }
            let (sid2, rid2) = (sid.clone(), q.id.clone());
            c = c.child(
                div().flex().justify_end().child(
                    button(SharedString::from(format!("qs-{}", q.id)))
                        .label(i.t("q.skip"))
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.reply_question(&sid2, &rid2, None, cx)))),
                ),
            );
            inner = inner.child(c);
        }
        body = body.child(inner);
        if count != self.last_count && focus.is_none() {
            self.last_count = count;
            self.focused_msg = None;
            self.scroll.scroll_to_bottom();
        }
        let _ = &self.app;

        div()
            .flex()
            .flex_col()
            .size_full()
            .children(head)
            .child(body)
            .child(div().w_full().max_w(t.rem(cw * ratio)).mx_auto().child(composer))
            .into_any_element()
    }
}


/// Permintaan fokus ke satu pesan (id) — diisi panel Trajectory "Ke pesan", dibaca ChatPanel.
pub struct MessageFocus(pub Option<String>);
impl gpui::Global for MessageFocus {}

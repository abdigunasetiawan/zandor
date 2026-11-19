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
    badge, icon, icon_button, markdown, working, BadgeTone, ButtonVariant, ControlSize, IconSize, InputSubmit, TextInput,
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
pub(crate) mod chat_parts;
#[path = "prompt_dock.rs"]
mod prompt_dock;
#[path = "model_picker.rs"]
mod model_picker;
#[path = "composer_info.rs"]
mod composer_info;
#[path = "chat_tabs.rs"]
mod chat_tabs;
#[path = "chat_welcome.rs"]
mod chat_welcome;
use chat_welcome::greeting;

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
    msg_list: gpui::ListState,
    msg_list_sid: Option<String>,
    /// Salinan hidup sesi demo terbuka (izin/pertanyaan dijawab lokal; di-reset lewat tombol).
    demo_state: Option<ChatState>,
    pending_new: bool,
    /// Sesi yang dibuat oleh pesan pertama dari halaman bawaan (tidak lagi menampilkan sambutan) — port sentFromDraft.
    sent_from_draft: HashSet<String>,
    /// Konteks saran prompt per folder project (dibaca di latar: berkas akar + status git).
    sugg_ctx: Option<(String, chat_welcome::WelcomeCtx)>,
    sugg_loading: Option<String>,
    /// Menu pil lingkup terbuka: 1 = folder, 2 = cabang git.
    scope_menu: Option<u8>,
    /// Pilihan memori/skill tertunda di halaman sesi-baru Chat: (memori, skill) — diterapkan saat sesi dibuat (ADR-060).
    pending_memory: (Option<bool>, Option<bool>),
    /// Cache pengaturan memori (memory/settings.json) untuk sakelar composer.
    mem_settings: crate::core::mem::MemorySettings,
    models: Vec<crate::core::engine::ModelOption>,
    models_loaded: bool,
    assist: composer_assist::AssistState,
    attach_menu: bool,
    voice_menu: bool,
    ticking: bool,
    msg_menu: Option<(String, u8)>,
    pins_open: bool,
    picker: model_picker::PickerState,
    info_open: bool,
    /// Pratinjau lampiran composer: (judul, penampil; None = data tidak terbaca).
    att_preview: Option<(String, Option<Entity<super::docs_panel::DocsPanel>>)>,
    mp_search: Entity<TextInput>,
    voice_devices: Vec<crate::voice::AudioInput>,
    voice_device: String,
    queue: queue_dock::QueueState,
    dock: prompt_dock::DockState,
    tabs_ui: chat_tabs::TabsState,
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
            ti.bare = true;
            ti.smooth_caret = true;
            ti
        });
        cx.observe(&store, |this, _, cx| {
            this.sync(cx);
            cx.notify();
        })
        .detach();
        cx.observe(&chat, |_, _, cx| cx.notify()).detach();
        cx.observe_global::<crate::theme::Theme>(|_, cx| cx.notify()).detach();
        cx.observe_global::<crate::i18n::I18n>(|_, cx| cx.notify()).detach();
        cx.observe(&crate::core::appearance::AppearanceStore::global(cx), |_, _, cx| cx.notify()).detach();
        cx.observe(&engine, |_, _, cx| cx.notify()).detach();
        cx.observe(&crate::core::profile::Profile::global(cx), |_, _, cx| cx.notify()).detach();
        cx.subscribe(&input, |this, _, e: &InputSubmit, cx| this.submit(e.0.to_string(), cx)).detach();
        // pasteLongText = "attach": teks panjang → lampiran teks (port pastedAttachment)
        cx.subscribe(&input, |this, _, e: &crate::ui::InputLongPaste, cx| {
            let mut n = 1;
            while this.assist.files.iter().any(|f| f.filename.as_deref() == Some(format!("teks-ditempel-{n}.txt").as_str())) {
                n += 1;
            }
            this.assist.files.push(crate::core::attachments::FileAttachment {
                mime: "text/plain".into(),
                url: format!("data:text/plain;charset=utf-8;base64,{}", crate::core::gradient::b64_encode(e.0.as_bytes())),
                filename: Some(format!("teks-ditempel-{n}.txt")),
            });
            let msg = cx.global::<I18n>().t("chat.pastedNotice");
            crate::ui::Toasts::show(crate::ui::ToastInput::new(msg), cx);
            cx.notify();
        })
        .detach();
        // sendKey / pasteLongText diterapkan ke input composer saat pengaturan berubah
        let apply = |input: &Entity<TextInput>, cx: &mut Context<Self>| {
            let st = crate::core::settings::SettingsStore::global(cx).read(cx);
            let (ctrl, attach) = (st.str("sendKey") == "ctrl-enter", st.str("pasteLongText") == "attach");
            input.update(cx, |i, _| {
                i.ctrl_enter_sends = ctrl;
                i.long_paste_attach = attach;
            });
        };
        apply(&input, cx);
        cx.observe(&crate::core::settings::SettingsStore::global(cx), move |this, _, cx| {
            let input = this.input.clone();
            apply(&input, cx);
            cx.notify();
        })
        .detach();
        let mp_ph = cx.global::<I18n>().t("mp.search");
        let mp_search = cx.new(|cx| {
            let mut ti = TextInput::new(cx, mp_ph);
            ti.bare = true;
            ti
        });
        cx.subscribe(&mp_search, |_, _, _: &crate::ui::InputChanged, cx| cx.notify()).detach();
        cx.observe(&crate::core::pings::PingStore::global(cx), |_, _, cx| cx.notify()).detach();
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
            msg_list: {
                let l = gpui::ListState::new(0, gpui::ListAlignment::Top, gpui::px(600.));
                l.set_follow_mode(gpui::FollowMode::Tail);
                l
            },
            msg_list_sid: None,
            demo_state: None,
            pending_new: false,
            sent_from_draft: HashSet::new(),
            sugg_ctx: None,
            sugg_loading: None,
            scope_menu: None,
            pending_memory: (None, None),
            mem_settings: crate::core::mem::parse_settings(&super::memory_page::read(crate::core::mem::FILE_SETTINGS)),
            models: vec![],
            models_loaded: false,
            assist: Default::default(),
            attach_menu: false,
            voice_menu: false,
            ticking: false,
            msg_menu: None,
            pins_open: false,
            picker: model_picker::PickerState { tab: "models".into(), sort: "status".into(), group: "provider".into(), ..Default::default() },
            mp_search,
            info_open: false,
            att_preview: None,
            voice_devices: vec![],
            voice_device: crate::core::chat_store::read_pref(crate::voice::DEVICE_KEY).unwrap_or_default(),
            queue: Default::default(),
            dock: Default::default(),
            tabs_ui: Default::default(),
        };
        me.sync(cx);
        me
    }

    /// Sesi terpilih: (kunci, id, direktori).
    fn patch_mem(&mut self, f: impl FnOnce(&mut crate::core::mem::MemorySettings), cx: &mut Context<Self>) {
        use crate::core::mem;
        let mut cur = mem::parse_settings(&super::memory_page::read(mem::FILE_SETTINGS));
        f(&mut cur);
        match super::memory_page::write(mem::FILE_SETTINGS, &mem::serialize_settings(&cur)) {
            Ok(()) => self.mem_settings = cur,
            Err(e) => {
                crate::ui::Toasts::show(crate::ui::ToastInput { tone: crate::ui::ToastTone::Danger, ..crate::ui::ToastInput::new(e) }, cx);
            }
        }
        cx.notify();
    }

    /// Sakelar Memori + Skill creator di composer (port ComposerMemory.tsx). Sesi project berbagi pengaturan project;
    /// sesi Chat diatur per sesi; halaman sesi-baru menahan pilihan sampai sesi dibuat.
    fn memory_toggles(&self, t: &Theme, i: &I18n, cx: &mut Context<Self>) -> gpui::Div {
        use crate::core::mem;
        let cur = self.current(cx);
        let sid = cur.as_ref().map(|c| c.1.clone());
        let key = cur.as_ref().map(|c| c.0.clone()).filter(|k| k != CHAT_KEY);
        let pkey = key.and_then(|k| self.store.read(cx).projects.iter().find(|p| p.id == k).map(|p| mem::project_key(&p.path)));
        let in_project = pkey.is_some();
        let s = &self.mem_settings;
        let value = |skill: bool| -> bool {
            if sid.is_none() && !in_project {
                if let Some(v) = if skill { self.pending_memory.1 } else { self.pending_memory.0 } {
                    return v;
                }
            }
            if skill { mem::skill_creator_enabled(s, pkey.as_deref(), sid.as_deref()) } else { mem::memory_enabled(s, pkey.as_deref(), sid.as_deref()) }
        };
        let scope = i.t(if in_project { "memtoggle.scopeProject" } else { "memtoggle.scopeSession" });
        let mut row = div().flex().items_center().gap(t.space(1));
        for skill in [false, true] {
            let on = value(skill);
            let label = i.t(if skill { "memtoggle.skill" } else { "memtoggle.memory" });
            let tip = format!("{label} — {}. {scope}", i.t(if on { "memtoggle.on" } else { "memtoggle.off" }));
            let (sid, pkey) = (sid.clone(), pkey.clone());
            row = row.child(
                icon_button(if skill { "mt-skill" } else { "mt-mem" }, if skill { "bookmark" } else { "brain" }, label)
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .success_on(on)
                    .tooltip_text(tip)
                    .on_click(cx.listener(move |me, _, _, cx| {
                        let on = !on;
                        match (&sid, &pkey) {
                            (None, None) => {
                                if skill {
                                    me.pending_memory.1 = Some(on);
                                } else {
                                    me.pending_memory.0 = Some(on);
                                }
                                cx.notify();
                            }
                            _ => {
                                let (sid, pkey) = (sid.clone().unwrap_or_default(), pkey.clone());
                                me.patch_mem(move |st| {
                                    let (sessions, projects) = if skill { (&mut st.skill_creator_sessions, &mut st.skill_creator_projects) } else { (&mut st.sessions, &mut st.projects) };
                                    match pkey {
                                        Some(k) => {
                                            projects.insert(k, on);
                                            sessions.remove(&sid);
                                        }
                                        None => {
                                            sessions.insert(sid, on);
                                        }
                                    }
                                }, cx);
                            }
                        }
                    })),
            );
        }
        row
    }

    fn current(&self, cx: &App) -> Option<(String, String, String, String)> {
        let sel = &self.store.read(cx).selection;
        if sel.key.as_deref() == Some(crate::core::demo::DEMO_KEY) {
            let id = sel.session_id.clone()?;
            let d = crate::core::demo::demo_by_id(&id)?;
            return Some((crate::core::demo::DEMO_KEY.into(), id, String::new(), cx.global::<I18n>().t(&d.title_key)));
        }
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
        // pengaturan memori bisa berubah dari halaman Memori: muat ulang saat sesi/terpilih berubah
        self.mem_settings = crate::core::mem::parse_settings(&super::memory_page::read(crate::core::mem::FILE_SETTINGS));
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
        // draf dibuat di project terpilih (atau Chat), seperti DraftChat versi React
        let key = self.store.read(cx).selection.key.clone().unwrap_or_else(|| CHAT_KEY.to_string());
        let task = self.store.update(cx, |s, cx| s.create_session(&key, None, true, cx));
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |me, cx| {
                me.pending_new = false;
                match r {
                    Ok(s) => {
                        me.sent_from_draft.insert(s.id.clone());
                        // terapkan pilihan memori/skill tertunda ke sesi Chat baru
                        let (m, k) = std::mem::take(&mut me.pending_memory);
                        if m.is_some() || k.is_some() {
                            me.patch_mem(|st| {
                                if let Some(on) = m {
                                    st.sessions.insert(s.id.clone(), on);
                                }
                                if let Some(on) = k {
                                    st.skill_creator_sessions.insert(s.id.clone(), on);
                                }
                            }, cx);
                        }
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

/// Pesan pengguna: chip lampiran, gelembung teks, dan baris bawah (waktu + aksi) yang muncul saat hover.
/// Data satu render untuk daftar pesan virtual.
struct ChatItems {
    turns: Vec<crate::core::chat::Turn>,
    sid: String,
    key: String,
    has_pinned: bool,
    has_project: bool,
    busy: bool,
    last_assistant: Option<usize>,
    active_ix: Option<usize>,
    loaded: bool,
    retry: Option<(f64, String)>,
    error: Option<String>,
}

impl ChatPanel {
    /// Satu item daftar pesan: [bar pin] + giliran + trailer (memuat/kosong/retry/bekerja/galat).
    fn chat_item(&mut self, ix: usize, c: &ChatItems, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.global::<Theme>().clone();
        let i_owned = cx.global::<I18n>().clone();
        let i = &i_owned;
        let cw = chat_width(&t, cx);
        let gap = t.rem(t.chat.gap.unwrap_or(0.75));
        let wrap = |el: AnyElement| {
            div()
                .w_full()
                .px(t.space(3))
                .when(ix == 0, |d| d.pt(t.space(3)))
                .child(div().w_full().max_w(t.rem(cw)).mx_auto().when_some(t.chat.font.clone(), |d, f| d.font_family(f)).child(el))
                .into_any_element()
        };
        let off = usize::from(c.has_pinned);
        if c.has_pinned && ix == 0 {
            return wrap(self.pinned_bar(&t, i, &c.sid, &c.turns, cx).unwrap_or_else(|| div().into_any_element()));
        }
        let k = ix - off;
        if k >= c.turns.len() {
            let mut inner = div().flex().flex_col().gap(gap).pb(t.space(3));
            if !c.loaded {
                inner = inner.child(div().text_color(t.colors.fg_muted).child(i.t("chat.loading")));
            } else if c.turns.is_empty() {
                inner = inner.child(div().text_color(t.colors.fg_muted).child(i.t("chat.empty")));
            }
            if let Some((attempt, message)) = &c.retry {
                inner = inner.child(div().text_color(t.colors.warning).child(i.tp("chat.retry", &[("attempt", &attempt.to_string()), ("message", message)])));
            } else if c.busy {
                inner = inner.child(div().flex().items_center().gap(t.space(2)).text_color(t.colors.fg_muted).child(working("chat-working")).child(i.t("chat.working")));
            }
            if let Some(e) = &c.error {
                inner = inner.child(div().text_color(t.colors.danger).child(e.clone()));
            }
            return wrap(inner.into_any_element());
        }
        let turn = &c.turns[k];
        let m = &turn.message;
        let streaming = c.active_ix == Some(k);
        let me = cx.entity().downgrade();
        let el = match m.info.role {
            Role::User => {
                let tb = if c.key == crate::core::demo::DEMO_KEY { div().into_any_element() } else { self.message_toolbar(&t, i, turn, &c.sid, &c.key, false, cx) };
                user_msg(&t, m, SharedString::from(format!("m-{}", m.info.id)), tb).unwrap_or_else(|| div().into_any_element())
            }
            Role::Assistant => {
                let pid = m.info.provider_id.clone();
                let provider_name = self.models.iter().find(|x| Some(&x.provider_id) == pid.as_ref()).map(|x| x.provider_name.clone()).or(pid).unwrap_or_default();
                let is_last = c.last_assistant == Some(k) && !c.busy;
                let tb = (m.info.completed.is_some() && !streaming && c.key != crate::core::demo::DEMO_KEY).then(|| self.message_toolbar(&t, i, turn, &c.sid, &c.key, is_last, cx));
                chat_parts::assistant_msg(&t, i, cx, &me, turn, &self.expanded, streaming, c.has_project, &provider_name, tb)
            }
        };
        let hl = self.focused_msg.as_deref() == Some(m.info.id.as_str());
        wrap(div().w_full().mb(gap).rounded(t.rem(crate::theme::rem::RADIUS_LG)).when(hl, |d| d.border_1().border_color(t.colors.focus)).child(el).into_any_element())
    }
}

fn user_msg(t: &Theme, m: &ChatMessage, id: SharedString, toolbar: AnyElement) -> Option<AnyElement> {
    let text = message_text(m);
    let files: Vec<String> = m
        .parts
        .iter()
        .filter_map(|p| match &p.kind {
            PartKind::File { filename, url, .. } => Some(filename.clone().unwrap_or_else(|| url.rsplit('/').next().unwrap_or("").to_string())),
            _ => None,
        })
        .collect();
    let noted: Vec<(String, Option<String>)> = m
        .parts
        .iter()
        .flat_map(|p| match &p.kind {
            PartKind::Text { text, synthetic: true } => crate::core::attachments::attachments_from_note(text),
            _ => vec![],
        })
        .collect();
    if text.is_empty() && files.is_empty() && noted.is_empty() {
        return None;
    }
    use chrono::TimeZone;
    let created = chrono::Local.timestamp_millis_opt(m.info.created as i64).single();
    let short = created.map(|d| d.format("%-d %b %H.%M").to_string()).unwrap_or_default();
    let full = created.map(|d| d.format("%A, %-d %B %Y %H.%M.%S %Z").to_string()).unwrap_or_default();
    let chip = |name: String| {
        div()
            .flex()
            .items_center()
            .gap(t.space(1))
            .max_w_full()
            .px(t.space(2))
            .border_1()
            .border_color(t.colors.border)
            .rounded_full()
            .text_size(t.rem(t.ty(crate::theme::SMALL).size))
            .child(icon("paperclip").size(IconSize::Sm))
            .child(div().truncate().child(name))
    };
    let mut bubble = div().max_w_full().px(t.space(3)).py(t.space(2)).rounded(t.rem(rem::RADIUS_LG)).bg(t.colors.bubble);
    if !files.is_empty() || !noted.is_empty() {
        let mut chips = div().flex().flex_wrap().gap(t.space(1)).mb(t.space(1));
        for f in files {
            chips = chips.child(chip(f));
        }
        for (n, _) in noted {
            chips = chips.child(chip(n));
        }
        bubble = bubble.child(chips);
    }
    if !text.is_empty() {
        bubble = bubble.child(chat_text(div(), t).child(text));
    }
    Some(
        div()
            .id(id.clone())
            .group("umsg")
            .flex()
            .flex_col()
            .items_end()
            .gap(t.space(2))
            .self_end()
            .max_w(gpui::relative(0.85))
            .child(bubble)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(t.space(2))
                    .min_h(t.rem(rem::CONTROL_SM))
                    .invisible()
                    .group_hover("umsg", |s| s.visible())
                    .child(
                        div()
                            .id(SharedString::from(format!("{id}-time")))
                            .small(t)
                            .text_color(t.colors.fg_subtle)
                            .whitespace_nowrap()
                            .tooltip(crate::ui::tooltip::text_tooltip(full.into()))
                            .child(short),
                    )
                    .child(toolbar),
            )
            .into_any_element(),
    )
}

impl Render for ChatPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.global::<Theme>().clone();
        let i_owned = cx.global::<I18n>().clone();
        let i = &i_owned;
        let me = cx.entity().downgrade();
        let tabs_bar = self.tabs_bar(&t, i, cx);
        let cur = self.current(cx);
        let engine_ready = self.engine.read(cx).state.info.is_some();
        let is_demo = cur.as_ref().is_some_and(|c| c.0 == crate::core::demo::DEMO_KEY);
        if is_demo {
            let id = cur.as_ref().map(|c| c.1.clone()).unwrap_or_default();
            if self.demo_state.as_ref().map(|s| s.session_id.as_str()) != crate::core::demo::demo_by_id(&id).map(|d| d.state.session_id.as_str()) {
                self.demo_state = crate::core::demo::demo_by_id(&id).map(|d| d.state.clone());
            }
        }
        let state: Option<ChatState> = if is_demo { self.demo_state.clone() } else { cur.as_ref().and_then(|(_, id, _, _)| self.chat.read(cx).sessions.get(id).cloned()) };

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
                .round(true)
                .on_click(cx.listener(move |s, _, _, cx| s.chat.update(cx, |c, cx| c.abort(&id, cx))))
        } else {
            icon_button("chat-send", "arrow_up", i.t("chat.send"))
                .variant(ButtonVariant::Solid)
                .round(true)
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
                crate::ui::select("chat-variant", items, c.variant.clone().unwrap_or_default()).flat_left(true)
                    .on_change(move |v, _, cx| chat.update(cx, |c, cx| c.set_variant((!v.is_empty()).then(|| v.to_string()), cx)))
            })
        };
        let can_queue = busy && !self.input.read(cx).text().trim().is_empty();
        let add_btn = can_queue.then(|| {
            icon_button("chat-queue", "arrow_up", i.t("queue.add")).variant(ButtonVariant::Solid).round(true).on_click(cx.listener(|s, _, _, cx| {
                let text = s.input.read(cx).text().to_string();
                s.submit(text, cx);
            }))
        });
        let model_sel = self.model_picker(&t, i, cx);
        let info_sid = cur.as_ref().map(|c| c.1.clone()).unwrap_or_default();
        let info_el = self.composer_info(&t, i, &info_sid, cx);
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
                        .child({
                            let f = f.clone();
                            let label = i.tp("chat.previewFile", &[("name", &name)]);
                            icon_button(SharedString::from(format!("chip-pv-{n}")), "paperclip", label).size(ControlSize::Sm).variant(ButtonVariant::Ghost).on_click(cx.listener(move |s, _, _, cx| s.open_preview(&f, cx)))
                        })
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
                        .round(true)
                        .soft(true)
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
            crate::ui::select("chat-mode", items, self.chat.read(cx).permission_mode.clone()).pill(true).lead_icon("shield").up(true).descriptions({
                let d: std::collections::HashMap<String, String> = crate::core::composer::PERMISSION_MODES.iter().map(|m| (m.to_string(), i.t(&format!("chat.modeDesc.{m}")))).collect();
                move |v| d.get(v).map(|s| SharedString::from(s.clone()))
            })
                .on_change(move |v, _, cx| chat.update(cx, |c, cx| c.set_permission_mode(&v, cx)))
        };
        let mem_toggles = self.memory_toggles(&t, i, cx);
        let welcome_layout = match (&state, &cur) {
            (None, _) => true,
            (Some(st), Some(c)) => st.loaded && st.messages.is_empty() && st.error.is_none() && !self.sent_from_draft.contains(&c.1),
            _ => false,
        };
        let qdock = cur.as_ref().and_then(|c| self.render_queue(&c.1.clone(), cx));
        let qdock = match (&state, &cur) {
            (Some(st), Some(c)) => self.render_dock(st, &c.1.clone(), qdock, cx),
            _ => qdock,
        };
        // Demo: tanpa composer; dock izin/pertanyaan/rencana berdiri sendiri di bawah transkrip.
        let (qdock, demo_dock) = if is_demo { (None, qdock) } else { (qdock, None) };
        let demo_banner = is_demo.then(|| {
            let def = cur.as_ref().and_then(|c| crate::core::demo::demo_by_id(&c.1));
            let pending = def.is_some_and(|d| d.state.permissions.len() + d.state.questions.len() > 0);
            div()
                .flex()
                .items_center()
                .gap(t.space(2))
                .px(t.space(3))
                .py(t.space(2))
                .border_b_1()
                .border_color(t.colors.border)
                .child(crate::ui::badge(i.t("demo.badge")).tone(crate::ui::BadgeTone::Outline))
                .child(div().flex_1().min_w_0().small(&t).text_color(t.colors.fg_muted).child(i.t("demo.banner")))
                .when(pending, |d| {
                    d.child(crate::ui::button("demo-reset").size(ControlSize::Sm).label(i.t("demo.reset")).on_click(cx.listener(|s, _, _, cx| {
                        s.demo_state = None;
                        s.dock = Default::default();
                        cx.notify();
                    })))
                })
        });
        let composer = div().w_full().when(!welcome_layout, |d| d.px(t.space(3)).pb(t.space(3))).flex().flex_col().gap(t.space(2)).children(qdock).children(sug_el).children(chips).child(
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
                .child(div().flex().items_center().gap(t.space(2)).child(attach_btn).child(mode_sel).child(mem_toggles).child(div().flex_1()).children(info_el).child(div().flex().flex_none().items_center().border_1().border_color(t.colors.border_soft).rounded_full().children(model_sel).children(variant_sel.map(|v| div().border_l_1().border_color(t.colors.border_soft).child(v)))).child(div().flex().flex_none().items_center().border_1().border_color(t.colors.border_soft).rounded_full().child(voice_btn).child(div().h(t.rem(rem::CONTROL_SM)).border_l_1().border_color(t.colors.border_soft).child(voice_dev))).children(add_btn).child(send_btn)),
        );

        let welcome = match (&state, &cur) {
            (None, _) => true,
            (Some(st), Some(c)) => st.loaded && st.messages.is_empty() && st.error.is_none() && !self.sent_from_draft.contains(&c.1),
            _ => false,
        };
        if welcome {
            // ---- halaman sesi baru (port DraftChat/Greeting/SessionScope + saran) ----
            let scope = self.session_scope(&t, i, cx);
            let chips = self.suggestion_chips(&t, i, cx);
            return div()
                .flex()
                .flex_col()
                .size_full()
                .child(tabs_bar)
                .child(
                    div()
                        .id("chat-welcome")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .items_center()
                        .justify_center()
                        .gap(t.space(4))
                        .p(t.space(4))
                        .child(greeting(&t, i, engine_ready, cx))
                        .child(scope)
                        .child(div().w_full().max_w(t.rem(cw * ratio)).child(composer))
                        .children(chips),
                )
                .into_any_element();
        }
        let Some(st) = state else { return div().into_any_element() };
        let sid = st.session_id.clone();
        let has_project = cur.as_ref().is_some_and(|c| c.0 != CHAT_KEY);

        // ---- isi ----
        // Daftar virtual (gpui::list): hanya pesan yang terlihat yang dirender/di-parse. Mode Tail mengikuti
        // streaming selama pengguna berada di bawah.
        let turns = crate::core::chat::merge_assistant_turns(st.visible());
        let last_assistant = turns.iter().rposition(|x| x.message.info.role == Role::Assistant);
        let active_ix = if busy && last_assistant.is_some() && last_assistant == Some(turns.len().saturating_sub(1)) { last_assistant } else { None };
        let count: usize = turns.iter().map(|x| x.message.parts.len() + 1).sum();
        let has_pinned = self.pinned_bar(&t, i, &sid, &turns, cx).is_some();
        let off = usize::from(has_pinned);
        let n = off + turns.len() + 1;
        let focus = cx.try_global::<MessageFocus>().and_then(|f| f.0.clone());
        let ml = self.msg_list.clone();
        if self.msg_list_sid.as_deref() != Some(sid.as_str()) {
            self.msg_list_sid = Some(sid.clone());
            ml.reset(n);
            ml.scroll_to_end();
            self.last_count = count;
        } else if ml.item_count() != n {
            let old = ml.item_count();
            if n > old && old > 0 {
                // pertahankan tinggi yang sudah diukur; ukur ulang ekor lama (trailer) + tambah item baru
                ml.splice(old - 1..old, n - old + 1);
            } else {
                ml.reset(n);
            }
        }
        ml.remeasure();
        if let Some(f) = &focus {
            if let Some(ix) = turns.iter().position(|x| x.source_ids.contains(f)) {
                ml.scroll_to(gpui::ListOffset { item_ix: ix + off, offset_in_item: gpui::px(0.) });
                self.focused_msg = Some(turns[ix].message.info.id.clone());
                cx.set_global(MessageFocus(None));
            }
        } else if count != self.last_count {
            self.last_count = count;
            self.focused_msg = None;
            ml.scroll_to_end();
        }
        let ictx = std::rc::Rc::new(ChatItems {
            turns,
            sid: sid.clone(),
            key: cur.as_ref().map(|c| c.0.clone()).unwrap_or_default(),
            has_pinned,
            has_project,
            busy,
            last_assistant,
            active_ix,
            loaded: st.loaded,
            retry: if let SessionRunStatus::Retry { attempt, message } = &st.status { Some((*attempt, message.clone())) } else { None },
            error: st.error.clone(),
        });
        let me_list = cx.entity();
        let body = div().flex().flex_col().flex_1().min_h_0().child(
            gpui::list(ml, move |ix, _w, cx| {
                let ictx = ictx.clone();
                me_list.update(cx, |this, cx| this.chat_item(ix, &ictx, cx))
            })
            .flex_1()
            .min_h_0(),
        );
        let _ = &self.app;

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(tabs_bar)
            .children(demo_banner)
            .child(body)
            .when(is_demo, |d| d.children(demo_dock.map(|dk| div().w_full().max_w(t.rem(cw * ratio)).mx_auto().px(t.space(3)).pb(t.space(3)).child(dk))))
            .when(!is_demo, |d| d.child(div().w_full().max_w(t.rem(cw * ratio)).mx_auto().child(composer)))
            .children(self.att_preview.clone().map(|(title, view)| {
                crate::ui::dialog(title)
                    .width_rem(rem::SPACE[7] * 14.)
                    .on_dismiss(cx.listener(|s, _, _, cx| {
                        s.att_preview = None;
                        cx.notify();
                    }))
                    .child(match view {
                        Some(v) => div().id("att-pv").max_h(t.rem(36.)).overflow_scroll().child(v).into_any_element(),
                        None => div().small(&t).text_color(t.colors.danger).child(i.t("chat.attachmentPreviewError")).into_any_element(),
                    })
                    .footer(div().flex().justify_end().child(crate::ui::button("att-pv-close").size(ControlSize::Sm).label(i.t("ui.close")).on_click(cx.listener(|s, _, _, cx| {
                        s.att_preview = None;
                        cx.notify();
                    }))))
            }))
            .into_any_element()
    }
}


impl ChatPanel {
    /// Port AttachmentPreview: data URL didekode; path lokal dibaca langsung (GPUI punya akses berkas).
    fn open_preview(&mut self, f: &crate::core::attachments::FileAttachment, cx: &mut Context<Self>) {
        let name = f.filename.clone().unwrap_or_else(|| cx.global::<I18n>().t("chat.attachmentPreview"));
        let bytes = if let Some(rest) = f.url.strip_prefix("data:") {
            rest.split_once(',').and_then(|(_, b)| crate::core::gradient::b64_decode(b))
        } else {
            let p = f.url.strip_prefix("file:///").or_else(|| f.url.strip_prefix("file://")).unwrap_or(&f.url);
            let p = p.replace("%20", " ");
            crate::files::read_limited(std::path::Path::new(&p), 40 * 1024 * 1024).ok()
        };
        let view = bytes.map(|b| {
            let n = name.clone();
            cx.new(|cx| super::docs_panel::DocsPanel::preview(n, b, cx))
        });
        self.att_preview = Some((name, view));
        cx.notify();
    }
}

/// Permintaan fokus ke satu pesan (id) — diisi panel Trajectory "Ke pesan", dibaca ChatPanel.
pub struct MessageFocus(pub Option<String>);
impl gpui::Global for MessageFocus {}

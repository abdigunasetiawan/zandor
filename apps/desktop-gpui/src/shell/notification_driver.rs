//! Penggerak pemberitahuan sesi — port NotificationDriver.tsx: memantau state chat, mendeteksi peralihan lewat
//! AttentionTracker, lalu memberi tahu lewat toast, notifikasi sistem, dan suara sesuai Pengaturan. Juga menyimpan
//! penanda "selesai belum dilihat" untuk titik status sesi (SessionDot).
use crate::core::attention::{AttentionEvent, AttentionTracker, Indicator, SETTLE_MS};
use crate::core::chat_store::ChatStore;
use crate::core::projects::{session_title, ProjectStore};
use crate::core::settings::SettingsStore;
use crate::i18n::I18n;
use gpui::{App, AppContext, Context, Entity, Global};

pub struct Attention {
    pub tracker: AttentionTracker,
    pub focused: bool,
    settle: bool,
}

struct AttentionHandle(Entity<Attention>);
impl Global for AttentionHandle {}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.)
}

impl Attention {
    pub fn init(cx: &mut App) -> Entity<Attention> {
        let e = cx.new(|cx: &mut Context<Attention>| {
            cx.observe(&ChatStore::global(cx), |s, _, cx| s.run(cx)).detach();
            cx.observe(&ProjectStore::global(cx), |s, _, cx| s.run(cx)).detach();
            Attention { tracker: AttentionTracker::default(), focused: true, settle: false }
        });
        cx.set_global(AttentionHandle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Option<Entity<Attention>> {
        cx.try_global::<AttentionHandle>().map(|h| h.0.clone())
    }

    /// Status tampilan satu sesi.
    pub fn indicator_of(id: &str, cx: &App) -> Indicator {
        let chat = ChatStore::global(cx);
        let st = chat.read(cx).sessions.get(id);
        match Self::global(cx) {
            Some(a) => a.read(cx).tracker.indicator(id, st, false),
            None => AttentionTracker::default().indicator(id, st, false),
        }
    }

    pub fn set_focused(&mut self, f: bool, cx: &mut Context<Self>) {
        self.focused = f;
        if f {
            self.run(cx);
        }
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let chat = ChatStore::global(cx);
        let projects = ProjectStore::global(cx);
        let viewing = projects.read(cx).selection.session_id.clone();
        let events = {
            let c = chat.read(cx);
            let list: Vec<(&str, &crate::core::chat::ChatState, bool)> = c.sessions.iter().map(|(k, v)| (k.as_str(), v, false)).collect();
            self.tracker.observe(&list, viewing.as_deref(), self.focused, now_ms())
        };
        for ev in events {
            self.handle(&ev, viewing.as_deref(), cx);
        }
        if self.tracker.has_pending() && !self.settle {
            self.settle = true;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(SETTLE_MS as u64 + 50)).await;
                this.update(cx, |s, cx| {
                    s.settle = false;
                    s.run(cx);
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    fn handle(&mut self, ev: &AttentionEvent, viewing: Option<&str>, cx: &mut Context<Self>) {
        let s = SettingsStore::global(cx).read(cx).all();
        let b = |k: &str| s.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
        let enabled = match ev {
            AttentionEvent::Done { .. } => b("notifyDone"),
            AttentionEvent::Action { .. } => b("notifyAction"),
            AttentionEvent::Failed { .. } => b("notifyFailed"),
        };
        if !enabled {
            return;
        }
        let sid = ev.session_id().to_string();
        let watching = viewing == Some(sid.as_str()) && self.focused;
        if b("notifyOnlyAway") && watching {
            return;
        }
        let i = cx.global::<I18n>().clone();
        let found = {
            let p = ProjectStore::global(cx);
            let st = p.read(cx);
            st.sessions.iter().find_map(|(k, g)| g.items.iter().find(|x| x.id == sid).map(|x| (k.clone(), session_title(x).map(str::to_string))))
        };
        let name = found.as_ref().and_then(|f| f.1.clone()).filter(|t| !t.starts_with("New session - ")).unwrap_or_else(|| i.t("sb.untitled"));
        let (title, body, tone, ms, sound_ev) = match ev {
            AttentionEvent::Action { permission, detail, .. } => (i.t(if *permission { "notify.permission" } else { "notify.question" }), format!("{name} — {detail}"), crate::ui::ToastTone::Warning, 10000, "action"),
            AttentionEvent::Done { .. } => (i.t("notify.done"), name.clone(), crate::ui::ToastTone::Success, 6000, "done"),
            AttentionEvent::Failed { limit, message, .. } => (i.t(if *limit { "notify.limit" } else { "notify.failed" }), format!("{name} — {message}"), crate::ui::ToastTone::Danger, 10000, "failed"),
        };
        if b("notifyToast") {
            let action = found.map(|(key, _)| {
                let sid = sid.clone();
                crate::ui::ToastAction {
                    label: i.t("notify.open").into(),
                    on_click: std::rc::Rc::new(move |_, cx: &mut App| {
                        let (k, id) = (key.clone(), sid.clone());
                        ProjectStore::global(cx).update(cx, |p, cx| p.select(Some(k), Some(id), cx));
                        super::state::AppState::global(cx).update(cx, |a, cx| a.push("/", cx));
                    }),
                }
            });
            crate::ui::Toasts::show(crate::ui::ToastInput { title: title.clone().into(), description: Some(body.clone().into()), tone, timeout_ms: Some(ms), action }, cx);
        }
        if b("notifySystem") && !self.focused {
            let (t2, b2) = (title.clone(), body.clone());
            std::thread::spawn(move || {
                let _ = crate::sound::system_notify(&t2, &b2);
            });
        }
        if b("notifySound") {
            let key = match sound_ev {
                "done" => "soundDone",
                "failed" => "soundFailed",
                _ => "soundAction",
            };
            let choice = s.get(key).and_then(|v| v.as_str()).unwrap_or("none").to_string();
            let vol = s.get("notifyVolume").and_then(|v| v.as_f64()).unwrap_or(0.6);
            crate::sound::play(&choice, vol, sound_ev);
        }
    }
}

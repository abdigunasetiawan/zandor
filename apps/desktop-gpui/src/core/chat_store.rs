//! ChatStore — port packages/core/src/chat/store.ts: state chat per sesi, langganan SSE per direktori,
//! muat riwayat, kirim prompt, batalkan, jawab izin & pertanyaan.
use super::chat::ChatState;
use super::engine::{EngineApi, ModelRef};
use super::types::{EngineEvent, PermissionReply};
use futures::StreamExt;
use gpui::{App, AppContext, Context, Entity, Global};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

enum Msg {
    Event(EngineEvent),
    Reconnected(String),
}

pub struct ChatStore {
    api: Option<EngineApi>,
    pub sessions: HashMap<String, ChatState>,
    dirs: HashMap<String, String>,
    subs: HashMap<String, Arc<AtomicBool>>,
    tx: futures::channel::mpsc::UnboundedSender<Msg>,
    pub model: Option<ModelRef>,
    /// Log aktivitas: (waktu ms, ringkasan event) terbaru, maks 500.
    pub activity: std::collections::VecDeque<(f64, String)>,
}

struct ChatHandle(Entity<ChatStore>);
impl Global for ChatHandle {}

impl ChatStore {
    pub fn init(cx: &mut App) -> Entity<ChatStore> {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Msg>();
        let e = cx.new(|_| ChatStore { api: None, sessions: HashMap::new(), dirs: HashMap::new(), subs: HashMap::new(), tx, model: None, activity: Default::default() });
        cx.set_global(ChatHandle(e.clone()));
        let weak = e.downgrade();
        cx.spawn(async move |cx| {
            while let Some(m) = rx.next().await {
                let Some(e) = weak.upgrade() else { break };
                cx.update(|cx| {
                    e.update(cx, |s, cx| match m {
                        Msg::Event(ev) => {
                            if !matches!(ev, EngineEvent::PartDelta { .. }) {
                                let label = format!("{:?}", ev).split(['(', ' ', '{']).next().unwrap_or("").to_string();
                                let sid = ev.session_id().unwrap_or("-").to_string();
                                s.activity.push_back((super::projects::now_ms(), format!("{label} · {sid}")));
                                while s.activity.len() > 500 {
                                    s.activity.pop_front();
                                }
                            }
                            let Some(sid) = ev.session_id().map(str::to_string) else {
                                // galat global: tampilkan di semua sesi yang terbuka
                                for st in s.sessions.values_mut() {
                                    st.apply(&ev);
                                }
                                cx.notify();
                                return;
                            };
                            if let Some(st) = s.sessions.get_mut(&sid) {
                                if st.apply(&ev) {
                                    cx.notify();
                                }
                            }
                        }
                        Msg::Reconnected(dir) => {
                            let ids: Vec<String> =
                                s.dirs.iter().filter(|(_, d)| **d == dir).map(|(id, _)| id.clone()).collect();
                            for id in ids {
                                s.load(&id, cx);
                            }
                        }
                    })
                });
            }
        })
        .detach();
        e
    }

    pub fn global(cx: &App) -> Entity<ChatStore> {
        cx.global::<ChatHandle>().0.clone()
    }

    pub fn set_api(&mut self, api: Option<EngineApi>, cx: &mut Context<Self>) {
        for (_, stop) in self.subs.drain() {
            stop.store(true, Ordering::Relaxed);
        }
        self.api = api;
        if self.api.is_some() {
            let opened: Vec<(String, String)> = self.dirs.iter().map(|(a, b)| (a.clone(), b.clone())).collect();
            for (id, dir) in opened {
                self.open(&dir, &id, cx);
            }
        }
        cx.notify();
    }

    fn ensure_sub(&mut self, dir: &str) {
        let Some(api) = &self.api else { return };
        if self.subs.contains_key(dir) {
            return;
        }
        let (tx1, tx2, d) = (self.tx.clone(), self.tx.clone(), dir.to_string());
        let stop = api.subscribe(
            dir,
            move |ev| {
                let _ = tx1.unbounded_send(Msg::Event(ev));
            },
            move |ok| {
                if ok {
                    let _ = tx2.unbounded_send(Msg::Reconnected(d.clone()));
                }
            },
        );
        self.subs.insert(dir.to_string(), stop);
    }

    /// Buka sesi: pastikan langganan direktori & riwayat termuat.
    pub fn open(&mut self, dir: &str, id: &str, cx: &mut Context<Self>) {
        self.dirs.insert(id.to_string(), dir.to_string());
        self.sessions.entry(id.to_string()).or_insert_with(|| ChatState::new(id));
        self.ensure_sub(dir);
        if !self.sessions[id].loaded {
            self.load(id, cx);
        }
    }

    fn load(&mut self, id: &str, cx: &mut Context<Self>) {
        let (Some(api), Some(dir)) = (self.api.clone(), self.dirs.get(id).cloned()) else { return };
        let id = id.to_string();
        let task = cx.background_spawn({
            let id = id.clone();
            async move {
                let msgs = api.list_messages(&dir, &id)?;
                let perms = api.list_permissions(&dir).unwrap_or_default();
                let qs = api.list_questions(&dir).unwrap_or_default();
                let todos = api.list_todos(&dir, &id).ok();
                let revert = api.get_revert(&dir, &id).unwrap_or(None);
                Ok::<_, super::engine::EngineError>((msgs, perms, qs, todos, revert))
            }
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                let st = s.sessions.entry(id.clone()).or_insert_with(|| ChatState::new(&id));
                match r {
                    Ok((msgs, perms, qs, todos, revert)) => {
                        let perms = perms.into_iter().filter(|p| p.session_id == id).collect();
                        let qs = qs.into_iter().filter(|q| q.session_id == id).collect();
                        st.load(msgs, perms, revert, Some(qs), todos);
                    }
                    Err(e) => {
                        st.loaded = true;
                        st.error = Some(e.0);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn bg(&self, id: &str, cx: &mut Context<Self>, f: impl FnOnce(&EngineApi, &str) -> Result<(), super::engine::EngineError> + Send + 'static) {
        let (Some(api), Some(dir)) = (self.api.clone(), self.dirs.get(id).cloned()) else { return };
        let id = id.to_string();
        let task = cx.background_spawn(async move { f(&api, &dir) });
        cx.spawn(async move |this, cx| {
            if let Err(e) = task.await {
                this.update(cx, |s, cx| {
                    if let Some(st) = s.sessions.get_mut(&id) {
                        st.error = Some(e.0);
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn send(&mut self, id: &str, text: String, cx: &mut Context<Self>) {
        if let Some(st) = self.sessions.get_mut(id) {
            st.error = None;
            st.status = super::types::SessionRunStatus::Busy;
        }
        let sid = id.to_string();
        let model = self.model.clone();
        self.bg(id, cx, move |api, dir| api.send_prompt(dir, &sid, &text, model.as_ref(), None, &[]));
        cx.notify();
    }

    pub fn abort(&mut self, id: &str, cx: &mut Context<Self>) {
        let sid = id.to_string();
        self.bg(id, cx, move |api, dir| api.abort(dir, &sid));
    }

    pub fn reply_permission(&mut self, id: &str, req: &str, reply: PermissionReply, cx: &mut Context<Self>) {
        if let Some(st) = self.sessions.get_mut(id) {
            st.permissions.retain(|p| p.id != req);
        }
        let req = req.to_string();
        self.bg(id, cx, move |api, dir| api.reply_permission(dir, &req, reply));
        cx.notify();
    }

    pub fn reply_question(&mut self, id: &str, req: &str, answers: Option<Vec<Vec<String>>>, cx: &mut Context<Self>) {
        if let Some(st) = self.sessions.get_mut(id) {
            st.questions.retain(|q| q.id != req);
        }
        let req = req.to_string();
        self.bg(id, cx, move |api, dir| match answers {
            Some(a) => api.reply_question(dir, &req, a),
            None => api.reject_question(dir, &req),
        });
        cx.notify();
    }
}

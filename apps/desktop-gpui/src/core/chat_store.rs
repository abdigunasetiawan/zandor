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
    /// Mode izin: ask | auto-edit | auto-safe | auto-all (disimpan di prefs.json).
    pub permission_mode: String,
    /// Varian model (mis. tingkat penalaran); None = bawaan.
    pub variant: Option<String>,
    /// Antrean pesan per sesi (port queues/queuePaused di store.ts).
    pub queues: HashMap<String, Vec<QueuedMessage>>,
    pub queue_paused: std::collections::HashSet<String>,
    force_next: std::collections::HashSet<String>,
    q_seq: u64,
    /// Pesan yang disematkan per sesi (port pinnedMessages, kunci prefs zandor.pinnedMessages).
    pub pinned_messages: HashMap<String, Vec<String>>,
}

pub const MAX_QUEUE: usize = 50;

#[derive(Clone, Debug)]
pub struct QueuedMessage {
    pub id: String,
    pub text: String,
    pub files: Vec<super::attachments::FileAttachment>,
}

fn prefs_path() -> std::path::PathBuf {
    super::services::app_data_dir().join("prefs.json")
}
pub fn read_pref(key: &str) -> Option<String> {
    let t = std::fs::read_to_string(prefs_path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    v.get(key)?.as_str().map(String::from)
}
pub fn write_pref(key: &str, val: &str) {
    let p = prefs_path();
    let mut v: serde_json::Value =
        std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::json!({}));
    v[key] = serde_json::json!(val);
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&p, v.to_string()) {
        eprintln!("[zandor] gagal menyimpan preferensi: {e}");
    }
}

const PINNED_MSG_PREF: &str = "zandor.pinnedMessages";

/// Port parsePinnedMessages: objek sesi → daftar id pesan (string saja).
pub fn parse_pinned(raw: Option<&str>) -> HashMap<String, Vec<String>> {
    let Some(v) = raw.and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok()) else { return HashMap::new() };
    v.as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, ids)| {
                    let ids: Vec<String> = ids.as_array()?.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
                    (!ids.is_empty()).then(|| (k.clone(), ids))
                })
                .collect()
        })
        .unwrap_or_default()
}

struct ChatHandle(Entity<ChatStore>);
impl Global for ChatHandle {}

impl ChatStore {
    pub fn init(cx: &mut App) -> Entity<ChatStore> {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Msg>();
        let e = cx.new(|_| ChatStore { api: None, sessions: HashMap::new(), dirs: HashMap::new(), subs: HashMap::new(), tx, model: None, activity: Default::default(), permission_mode: read_pref("zandor.permissionMode").filter(|m| super::composer::PERMISSION_MODES.contains(&m.as_str())).unwrap_or_else(|| super::composer::DEFAULT_PERMISSION_MODE.into()), variant: read_pref("zandor.variant").filter(|v| !v.is_empty()), queues: HashMap::new(), queue_paused: Default::default(), force_next: Default::default(), q_seq: 0, pinned_messages: parse_pinned(read_pref(PINNED_MSG_PREF).as_deref()) });
        cx.set_global(ChatHandle(e.clone()));
        let weak = e.downgrade();
        cx.spawn(async move |cx| {
            while let Some(m) = rx.next().await {
                let Some(e) = weak.upgrade() else { break };
                cx.update(|cx| {
                    e.update(cx, |s, cx| match m {
                        Msg::Event(ev) => {
                            {
                                use super::types::{PartKind, SessionRunStatus, ToolStatus};
                                let entry: Option<(&'static str, &'static str, String)> = match &ev {
                                    EngineEvent::PartUpdated(p) => match &p.kind {
                                        PartKind::Tool { tool, state, .. } if state.status == ToolStatus::Error => Some(("tool", "error", format!("{tool}: {}", state.error.clone().unwrap_or_default()))),
                                        PartKind::Tool { tool, state, .. } if state.status == ToolStatus::Completed => Some(("tool", "info", format!("{tool} {}", state.title.clone().unwrap_or_default()))),
                                        _ => None,
                                    },
                                    EngineEvent::PermissionAsked(r) => Some(("tool", "warn", format!("izin diminta: {} {}", r.permission, r.patterns.join(", ")))),
                                    EngineEvent::PermissionReplied { reply, .. } => Some(("tool", "info", format!("izin dijawab: {reply:?}"))),
                                    EngineEvent::SessionError { error, .. } => Some(("chat", "error", format!("{}: {}", error.name, error.message))),
                                    EngineEvent::SessionStatus { status: SessionRunStatus::Retry { attempt, message }, .. } => Some(("chat", "warn", format!("coba ulang #{attempt}: {message}"))),
                                    EngineEvent::QuestionAsked(_) => Some(("chat", "info", "agent bertanya".to_string())),
                                    _ => None,
                                };
                                if let Some((src, lvl, msg)) = entry {
                                    super::activity::log(cx, src, lvl, msg);
                                }
                            }
                            if !matches!(ev, EngineEvent::PartDelta { .. }) {
                                let label = format!("{:?}", ev).split(['(', ' ', '{']).next().unwrap_or("").to_string();
                                let sid = ev.session_id().unwrap_or("-").to_string();
                                s.activity.push_back((super::projects::now_ms(), format!("{label} · {sid}")));
                                while s.activity.len() > 500 {
                                    s.activity.pop_front();
                                }
                            }
                            if let EngineEvent::PermissionAsked(req) = &ev {
                                if s.maybe_auto_reply(req, cx) {
                                    return;
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
                            s.pump(&sid, false, cx);
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

    /// Jawab izin otomatis sesuai mode; true = ditangani (tidak masuk state).
    fn maybe_auto_reply(&mut self, req: &super::types::PermissionRequest, cx: &mut Context<Self>) -> bool {
        if !super::composer::auto_allowed(&self.permission_mode, req) {
            return false;
        }
        let (Some(api), Some(dir)) = (self.api.clone(), self.dirs.get(&req.session_id).cloned()) else { return false };
        let (rid, sid, r2) = (req.id.clone(), req.session_id.clone(), req.clone());
        let task = cx.background_spawn(async move { api.reply_permission(&dir, &rid, PermissionReply::Once) });
        cx.spawn(async move |this, cx| {
            if let Err(e) = task.await {
                eprintln!("[zandor] gagal menjawab izin otomatis: {}", e.0);
                this.update(cx, |s, cx| {
                    if let Some(st) = s.sessions.get_mut(&sid) {
                        st.apply(&EngineEvent::PermissionAsked(r2));
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        true
    }

    pub fn set_variant(&mut self, v: Option<String>, cx: &mut Context<Self>) {
        write_pref("zandor.variant", v.as_deref().unwrap_or(""));
        self.variant = v;
        cx.notify();
    }

    /// Pilih model dan simpan sebagai preferensi.
    pub fn select_model(&mut self, m: ModelRef, cx: &mut Context<Self>) {
        write_pref("zandor.model", &format!("{}/{}", m.provider_id, m.model_id));
        self.model = Some(m);
        cx.notify();
    }

    pub fn set_permission_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        if mode == self.permission_mode {
            return;
        }
        self.permission_mode = mode.to_string();
        write_pref("zandor.permissionMode", mode);
        // permintaan yang menunggu ikut dijawab sesuai mode baru
        let pending: Vec<_> = self.sessions.values().flat_map(|s| s.permissions.clone()).collect();
        for p in pending {
            if self.maybe_auto_reply(&p, cx) {
                if let Some(st) = self.sessions.get_mut(&p.session_id) {
                    st.permissions.retain(|x| x.id != p.id);
                }
            }
        }
        cx.notify();
    }

    /// Jalankan panggilan engine di latar untuk sesi ini; hasil dikembalikan ke pemanggil.
    pub fn call<T: Send + 'static>(
        &self,
        id: &str,
        cx: &App,
        f: impl FnOnce(&EngineApi, &str) -> Result<T, super::engine::EngineError> + Send + 'static,
    ) -> gpui::Task<Result<T, String>> {
        let (Some(api), Some(dir)) = (self.api.clone(), self.dirs.get(id).cloned()) else {
            return gpui::Task::ready(Err("engine belum siap".into()));
        };
        cx.background_spawn(async move { f(&api, &dir).map_err(|e| e.0) })
    }

    pub fn list_commands(&self, dir: &str, cx: &App) -> gpui::Task<Vec<super::engine::EngineCommand>> {
        let Some(api) = self.api.clone() else { return gpui::Task::ready(vec![]) };
        let d = dir.to_string();
        cx.background_spawn(async move { api.list_commands(&d).unwrap_or_default() })
    }

    pub fn find_files(&self, dir: &str, query: &str, cx: &App) -> gpui::Task<Result<Vec<String>, String>> {
        let Some(api) = self.api.clone() else { return gpui::Task::ready(Err("engine belum siap".into())) };
        let (d, q) = (dir.to_string(), query.to_string());
        cx.background_spawn(async move { api.find_files(&d, &q, 30).map_err(|e| e.0) })
    }

    pub fn run_command(
        &mut self,
        id: &str,
        name: String,
        args: String,
        files: Vec<super::attachments::FileAttachment>,
        cx: &mut Context<Self>,
    ) {
        if let Some(st) = self.sessions.get_mut(id) {
            st.error = None;
            st.status = super::types::SessionRunStatus::Busy;
        }
        let (sid, model, variant) = (id.to_string(), self.model.clone(), self.variant.clone());
        self.bg(id, cx, move |api, dir| api.run_command(dir, &sid, &name, &args, model.as_ref(), variant.as_deref(), &files));
        cx.notify();
    }

    pub fn compact(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(model) = self.model.clone() else {
            if let Some(st) = self.sessions.get_mut(id) {
                st.error = Some("belum ada model; tambahkan provider terlebih dahulu".into());
            }
            cx.notify();
            return;
        };
        let sid = id.to_string();
        self.bg(id, cx, move |api, dir| api.summarize(dir, &sid, &model));
    }

    pub fn toggle_message_pin(&mut self, sid: &str, mid: &str, cx: &mut Context<Self>) {
        let cur = self.pinned_messages.entry(sid.to_string()).or_default();
        if let Some(p) = cur.iter().position(|x| x == mid) {
            cur.remove(p);
        } else {
            cur.push(mid.to_string());
        }
        self.pinned_messages.retain(|_, v| !v.is_empty());
        let json: serde_json::Map<String, serde_json::Value> = self.pinned_messages.iter().map(|(k, v)| (k.clone(), serde_json::json!(v))).collect();
        write_pref(PINNED_MSG_PREF, &serde_json::Value::Object(json).to_string());
        cx.notify();
    }

    /// Ulangi jawaban: kembalikan sesi ke pesan pengguna terakhir lalu kirim ulang isinya (port regenerate).
    pub fn regenerate(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(st) = self.sessions.get(id) else { return Err("engine belum siap".into()) };
        if st.busy() {
            return Err("sesi sedang berjalan".into());
        }
        let user = st.visible().filter(|m| m.info.role == super::types::Role::User).last().cloned().ok_or("tidak ada pesan pengguna untuk diulang")?;
        let text = super::chat::message_text(&user);
        let files: Vec<super::attachments::FileAttachment> = user
            .parts
            .iter()
            .filter_map(|p| match &p.kind {
                super::types::PartKind::File { mime, filename, url } if !url.is_empty() => Some(super::attachments::FileAttachment { mime: mime.clone(), url: url.clone(), filename: filename.clone() }),
                _ => None,
            })
            .collect();
        if let Some(st) = self.sessions.get_mut(id) {
            st.revert = Some(user.info.id.clone());
            st.error = None;
            st.status = super::types::SessionRunStatus::Busy;
        }
        let (sid, mid, model, variant) = (id.to_string(), user.info.id.clone(), self.model.clone(), self.variant.clone());
        self.bg(id, cx, move |api, dir| {
            api.revert(dir, &sid, &mid)?;
            api.send_prompt(dir, &sid, &text, model.as_ref(), variant.as_deref(), &files)
        });
        cx.notify();
        Ok(())
    }

    /// Cabang baru dari pesan ini (port fork).
    pub fn fork(&self, id: &str, message_id: &str, cx: &mut Context<Self>) -> gpui::Task<Result<super::engine::SessionInfo, String>> {
        let (Some(api), Some(dir)) = (self.api.clone(), self.dirs.get(id).cloned()) else { return gpui::Task::ready(Err("engine belum siap".into())) };
        let (sid, mid) = (id.to_string(), message_id.to_string());
        cx.background_spawn(async move { api.fork(&dir, &sid, Some(&mid)).map_err(|e| e.0) })
    }

    pub fn revert_to(&mut self, id: &str, message_id: &str, cx: &mut Context<Self>) {
        if let Some(st) = self.sessions.get_mut(id) {
            st.revert = Some(message_id.to_string());
        }
        let (sid, mid) = (id.to_string(), message_id.to_string());
        self.bg(id, cx, move |api, dir| api.revert(dir, &sid, &mid));
        cx.notify();
    }

    pub fn unrevert(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(st) = self.sessions.get_mut(id) {
            st.revert = None;
        }
        let sid = id.to_string();
        self.bg(id, cx, move |api, dir| api.unrevert(dir, &sid));
        cx.notify();
    }

    pub fn send(&mut self, id: &str, text: String, cx: &mut Context<Self>) {
        self.send_with(id, text, vec![], cx)
    }

    pub fn send_with(&mut self, id: &str, text: String, files: Vec<super::attachments::FileAttachment>, cx: &mut Context<Self>) {
        if let Some(st) = self.sessions.get_mut(id) {
            st.error = None;
            st.status = super::types::SessionRunStatus::Busy;
        }
        let sid = id.to_string();
        let model = self.model.clone();
        let variant = self.variant.clone();
        self.bg(id, cx, move |api, dir| api.send_prompt(dir, &sid, &text, model.as_ref(), variant.as_deref(), &files));
        cx.notify();
    }

    // ---------- antrean pesan ----------

    pub fn queue_of(&self, id: &str) -> &[QueuedMessage] {
        self.queues.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    fn set_queue(&mut self, id: &str, items: Vec<QueuedMessage>, cx: &mut Context<Self>) {
        if items.is_empty() {
            self.queues.remove(id);
            self.queue_paused.remove(id);
        } else {
            self.queues.insert(id.to_string(), items);
        }
        cx.notify();
    }

    /// Tambah pesan ke antrean sesi.
    pub fn enqueue(&mut self, id: &str, text: &str, files: Vec<super::attachments::FileAttachment>, cx: &mut Context<Self>) -> Result<String, String> {
        let t = text.trim();
        if !self.sessions.contains_key(id) {
            return Err("sesi tidak ditemukan".into());
        }
        if t.is_empty() && files.is_empty() {
            return Err("pesan kosong".into());
        }
        let mut cur = self.queue_of(id).to_vec();
        if cur.len() >= MAX_QUEUE {
            return Err(format!("antrean penuh (maks {MAX_QUEUE})"));
        }
        self.q_seq += 1;
        let qid = format!("q{}", self.q_seq);
        cur.push(QueuedMessage { id: qid.clone(), text: t.to_string(), files });
        self.set_queue(id, cur, cx);
        Ok(qid)
    }

    pub fn edit_queued(&mut self, id: &str, qid: &str, text: &str, cx: &mut Context<Self>) {
        let t = text.trim();
        let mut cur = self.queue_of(id).to_vec();
        let Some(item) = cur.iter_mut().find(|q| q.id == qid) else { return };
        if t.is_empty() && item.files.is_empty() {
            return self.remove_queued(id, qid, cx);
        }
        item.text = t.to_string();
        self.set_queue(id, cur, cx);
    }

    pub fn remove_queued(&mut self, id: &str, qid: &str, cx: &mut Context<Self>) {
        let cur: Vec<_> = self.queue_of(id).iter().filter(|q| q.id != qid).cloned().collect();
        self.set_queue(id, cur, cx);
    }

    /// Pindahkan item ke indeks `to` (0 = dikirim paling dulu).
    pub fn move_queued(&mut self, id: &str, qid: &str, to: isize, cx: &mut Context<Self>) {
        let mut cur = self.queue_of(id).to_vec();
        let Some(from) = cur.iter().position(|q| q.id == qid) else { return };
        let to = to.clamp(0, cur.len() as isize - 1) as usize;
        if to == from {
            return;
        }
        let item = cur.remove(from);
        cur.insert(to, item);
        self.set_queue(id, cur, cx);
    }

    /// Kirim langsung: pindah ke depan; bila agent bekerja, hentikan dulu lalu kirim saat idle.
    pub fn send_queued_now(&mut self, id: &str, qid: &str, cx: &mut Context<Self>) {
        self.move_queued(id, qid, 0, cx);
        if !self.queue_of(id).iter().any(|q| q.id == qid) {
            return;
        }
        self.queue_paused.remove(id);
        if self.sessions.get(id).is_some_and(|s| s.busy()) {
            self.force_next.insert(id.to_string());
            let sid = id.to_string();
            self.bg(id, cx, move |api, dir| api.abort(dir, &sid));
        }
        self.pump(id, true, cx);
    }

    pub fn resume_queue(&mut self, id: &str, cx: &mut Context<Self>) {
        self.queue_paused.remove(id);
        self.pump(id, true, cx);
    }

    /// Kirim pesan terdepan bila sesi idle dan antrean tidak dijeda.
    fn pump(&mut self, id: &str, force: bool, cx: &mut Context<Self>) {
        let Some(s) = self.sessions.get(id) else { return };
        if self.queue_of(id).is_empty() || self.api.is_none() {
            return;
        }
        let forced = force || self.force_next.contains(id);
        if s.busy() || !s.loaded || self.queue_paused.contains(id) {
            return;
        }
        if (s.error.is_some() && !forced) || !s.permissions.is_empty() || !s.questions.is_empty() {
            return;
        }
        let mut q = self.queue_of(id).to_vec();
        let head = q.remove(0);
        self.force_next.remove(id);
        self.set_queue(id, q, cx);
        self.send_with(id, head.text, head.files, cx);
    }

    pub fn abort(&mut self, id: &str, cx: &mut Context<Self>) {
        // Stop oleh pengguna menjeda antrean.
        if !self.queue_of(id).is_empty() && !self.force_next.contains(id) {
            self.queue_paused.insert(id.to_string());
        }
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

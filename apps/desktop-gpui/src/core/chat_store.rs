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
    /// Model yang disematkan (zandor.pinnedModels) dan terakhir dipakai (zandor.recentModels, maks 3), kunci provider/model.
    pub pinned_models: Vec<String>,
    pub recent_models: Vec<String>,
    /// Penamaan ulang judul (lihat `autotitle`): jumlah giliran pengguna yang sudah dinilai per sesi, supaya event
    /// `session.updated` yang beruntun tidak memicu panggilan model berulang untuk keadaan yang sama.
    retitled_at: HashMap<String, usize>,
    /// Kesimpulan + saran aksi per sesi (lihat `next_steps`); `ns_for` = id pesan pengguna yang sudah/sedang diringkas.
    pub next_steps: HashMap<String, Vec<super::next_steps::NextSteps>>,
    ns_for: HashMap<String, String>,
    /// Konteks: batas jendela asli per model (kunci provider/model) dan sesi yang idle berikutnya tidak boleh memicu
    /// auto compact (idle setelah ringkasan: token terakhir = permintaan ringkasan, bukan percakapan sebenarnya).
    ctx: ContextState,
}

#[derive(Default)]
struct ContextState {
    limits: HashMap<String, u64>,
    skip_idle: std::collections::HashSet<String>,
}

pub const MAX_QUEUE: usize = 50;
pub const MAX_RECENT: usize = 3;

/// Port parsePinned: array string unik; selain itu kosong.
pub fn parse_keys(raw: Option<&str>) -> Vec<String> {
    let v: serde_json::Value = raw.and_then(|r| serde_json::from_str(r).ok()).unwrap_or_default();
    let mut out: Vec<String> = vec![];
    for x in v.as_array().into_iter().flatten().filter_map(|x| x.as_str()) {
        if !out.iter().any(|o| o == x) {
            out.push(x.to_string());
        }
    }
    out
}

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

fn model_key(m: &ModelRef) -> String {
    format!("{}/{}", m.provider_id, m.model_id)
}

fn notify(cx: &mut App, title: String, tone: crate::ui::ToastTone) {
    crate::ui::Toasts::show(crate::ui::ToastInput { title: title.into(), description: None, tone, timeout_ms: None, action: None }, cx);
}

struct ChatHandle(Entity<ChatStore>);
impl Global for ChatHandle {}

impl ChatStore {
    pub fn init(cx: &mut App) -> Entity<ChatStore> {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Msg>();
        let e = cx.new(|_| ChatStore { api: None, sessions: HashMap::new(), dirs: HashMap::new(), subs: HashMap::new(), tx, model: None, activity: Default::default(), permission_mode: read_pref("zandor.permissionMode").filter(|m| super::composer::PERMISSION_MODES.contains(&m.as_str())).unwrap_or_else(|| super::composer::DEFAULT_PERMISSION_MODE.into()), variant: read_pref("zandor.variant").filter(|v| !v.is_empty()), queues: HashMap::new(), queue_paused: Default::default(), force_next: Default::default(), q_seq: 0, pinned_messages: parse_pinned(read_pref(PINNED_MSG_PREF).as_deref()), pinned_models: parse_keys(read_pref("zandor.pinnedModels").as_deref()), recent_models: parse_keys(read_pref("zandor.recentModels").as_deref()).into_iter().take(MAX_RECENT).collect(), retitled_at: HashMap::new(), next_steps: HashMap::new(), ns_for: HashMap::new(), ctx: ContextState::default() });
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
                            if let EngineEvent::SessionUpdated { session_id, title, updated, .. } = &ev {
                                super::projects::ProjectStore::global(cx).update(cx, |p, cx| p.apply_session_update(session_id, title.as_deref(), *updated, cx));
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
                            if matches!(
                                ev,
                                EngineEvent::SessionStatus { status: super::types::SessionRunStatus::Idle, .. } | EngineEvent::SessionUpdated { title: Some(_), .. }
                            ) {
                                s.maybe_retitle(&sid, cx);
                            }
                            if matches!(ev, EngineEvent::SessionStatus { status: super::types::SessionRunStatus::Idle, .. }) {
                                s.maybe_next_steps(&sid, cx);
                                s.maybe_auto_compact(&sid, cx);
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

    /// Nilai ulang judul sesi dari awal percakapan (lihat `autotitle`): sapaan pembuka tidak boleh jadi judul tetap.
    /// Dipanggil saat sesi idle dan saat engine mengirim judul; hanya berjalan sekali per giliran pengguna.
    fn maybe_retitle(&mut self, sid: &str, cx: &mut Context<Self>) {
        use super::autotitle::{self, Verdict};
        let (Some(st), Some(dir)) = (self.sessions.get(sid), self.dirs.get(sid).cloned()) else { return };
        if st.busy() || !st.loaded {
            return;
        }
        let turns = autotitle::user_turns(&st.messages);
        if turns == 0 || turns > autotitle::MAX_TURNS || self.retitled_at.get(sid).is_some_and(|n| *n >= turns) {
            return;
        }
        let transcript = autotitle::transcript(&st.messages);
        let pstore = super::projects::ProjectStore::global(cx);
        let Some((current, api)) = pstore.update(cx, |p, _| p.begin_retitle(sid)) else { return };
        self.retitled_at.insert(sid.to_string(), turns);
        // model kecil dari peran model; bila tidak diatur, model sesi (atau bawaan engine bila juga kosong)
        let small = crate::engine::read_small_model(&super::services::app_data_dir().join("model-roles.json"))
            .and_then(|m| m.split_once('/').map(|(p, m)| ModelRef { provider_id: p.to_string(), model_id: m.to_string() }));
        let model = small.or_else(|| self.model.clone());
        let prompt = autotitle::prompt(&current, &transcript);
        let sid = sid.to_string();
        let sid_bg = sid.clone();
        let work = cx.background_spawn(async move {
            let reply = api.complete_once(&dir, &prompt, model.as_ref()).map_err(|e| e.0)?;
            match autotitle::parse_reply(&reply) {
                Verdict::Title(t) => {
                    api.rename_session(&dir, &sid_bg, &t).map_err(|e| e.0)?;
                    Ok(Verdict::Title(t))
                }
                v => Ok(v),
            }
        });
        cx.spawn(async move |_, cx| {
            let res: Result<Verdict, String> = work.await;
            cx.update(|cx| {
                if let Err(e) = &res {
                    super::activity::log(cx, "chat", "warn", format!("penamaan ulang judul gagal: {e}"));
                }
                pstore.update(cx, |p, cx| match res {
                    Ok(Verdict::Title(t)) => p.finish_retitle(&sid, Some(t), true, cx),
                    Ok(Verdict::Keep) => p.finish_retitle(&sid, None, true, cx),
                    Ok(Verdict::NoTopic) | Err(_) => p.finish_retitle(&sid, None, false, cx),
                });
            });
        })
        .detach();
    }

    /// Buat kesimpulan + saran aksi untuk jawaban yang baru selesai (lihat `next_steps`). Model kecil (atau model sesi)
    /// yang menulisnya, bukan model yang menjawab, supaya hasilnya seragam di model apa pun.
    fn maybe_next_steps(&mut self, sid: &str, cx: &mut Context<Self>) {
        use super::next_steps as ns;
        if !super::settings::SettingsStore::global(cx).read(cx).bool("nextSteps") {
            return;
        }
        let (Some(st), Some(dir), Some(api)) = (self.sessions.get(sid), self.dirs.get(sid).cloned(), self.api.clone()) else { return };
        if st.busy() || !st.loaded || !st.permissions.is_empty() || !st.questions.is_empty() || self.queues.get(sid).is_some_and(|q| !q.is_empty()) {
            return;
        }
        let Some((user_id, request, answer, worked)) = ns::last_exchange(&st.messages) else { return };
        if !ns::worth_asking(&answer, worked) || self.ns_for.get(sid) == Some(&user_id) {
            return;
        }
        self.ns_for.insert(sid.to_string(), user_id.clone());
        let small = crate::engine::read_small_model(&super::services::app_data_dir().join("model-roles.json"))
            .and_then(|m| m.split_once('/').map(|(p, m)| ModelRef { provider_id: p.to_string(), model_id: m.to_string() }));
        let model = small.or_else(|| self.model.clone());
        let prompt = ns::prompt(&request, &answer);
        let uid = user_id.clone();
        let req = request.clone();
        let work = cx.background_spawn(async move {
            match api.complete_once(&dir, &prompt, model.as_ref()) {
                Ok(r) if ns::is_skip(&r) => None, // model menilai tak perlu kartu: hormati, tanpa cadangan
                Ok(r) => ns::parse_reply(&r, &uid).map(|n| ns::ground(n, &req, &answer)).filter(|n| !n.recap.is_empty() || !n.actions.is_empty()).or_else(|| ns::fallback(&answer, &uid)),
                Err(_) => ns::fallback(&answer, &uid),
            }
        });
        let sid = sid.to_string();
        cx.spawn(async move |this, cx| {
            let r = work.await;
            this.update(cx, |s, cx| {
                // kartu menetap di bawah jawabannya (juga bila pengguna sudah lanjut: tampil sebagai catatan, tanpa tombol)
                if let Some(n) = r {
                    let list = s.next_steps.entry(sid.clone()).or_default();
                    list.retain(|x| x.for_msg != n.for_msg);
                    list.push(n);
                    if list.len() > 50 {
                        list.remove(0);
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Pengguna mengklik satu aksi di kartu: tandai sebagai terpilih (kartu tetap tampil sebagai catatan) lalu kirim pesannya.
    pub fn choose_next_step(&mut self, sid: &str, for_msg: &str, idx: usize, cx: &mut Context<Self>) {
        let msg = self.next_steps.get_mut(sid).and_then(|v| v.iter_mut().find(|n| n.for_msg == for_msg)).and_then(|n| {
            let m = n.actions.get(idx).map(|a| a.message.clone());
            if m.is_some() {
                n.chosen = Some(idx);
            }
            m
        });
        if let Some(m) = msg {
            self.send(sid, m, cx);
        }
    }

    pub fn dismiss_next_step(&mut self, sid: &str, for_msg: &str, cx: &mut Context<Self>) {
        if let Some(v) = self.next_steps.get_mut(sid) {
            v.retain(|n| n.for_msg != for_msg);
        }
        cx.notify();
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
                let live = api.session_status(&dir, &id).ok();
                Ok::<_, super::engine::EngineError>((msgs, perms, qs, todos, revert, live))
            }
        });
        cx.spawn(async move |this, cx| {
            let r = task.await;
            this.update(cx, |s, cx| {
                let st = s.sessions.entry(id.clone()).or_insert_with(|| ChatState::new(&id));
                match r {
                    Ok((msgs, perms, qs, todos, revert, live)) => {
                        let perms = perms.into_iter().filter(|p| p.session_id == id).collect();
                        let qs = qs.into_iter().filter(|q| q.session_id == id).collect();
                        st.load(msgs, perms, revert, Some(qs), todos, live);
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

    fn touch_recent(&mut self, m: &ModelRef) {
        let key = format!("{}/{}", m.provider_id, m.model_id);
        if self.recent_models.first() == Some(&key) {
            return;
        }
        let mut r = vec![key.clone()];
        r.extend(self.recent_models.iter().filter(|k| **k != key).cloned());
        r.truncate(MAX_RECENT);
        write_pref("zandor.recentModels", &serde_json::to_string(&r).unwrap_or_default());
        self.recent_models = r;
    }

    /// Sematkan/lepas model (kunci tetap tersimpan walau model hilang dari daftar).
    pub fn toggle_model_pin(&mut self, m: &ModelRef, cx: &mut Context<Self>) {
        let key = format!("{}/{}", m.provider_id, m.model_id);
        if let Some(i) = self.pinned_models.iter().position(|k| *k == key) {
            self.pinned_models.remove(i);
        } else {
            self.pinned_models.push(key);
        }
        write_pref("zandor.pinnedModels", &serde_json::to_string(&self.pinned_models).unwrap_or_default());
        cx.notify();
    }

    pub fn set_variant(&mut self, v: Option<String>, cx: &mut Context<Self>) {
        write_pref("zandor.variant", v.as_deref().unwrap_or(""));
        self.variant = v;
        cx.notify();
    }

    /// Pilih model dan simpan sebagai preferensi.
    pub fn select_model(&mut self, m: ModelRef, cx: &mut Context<Self>) {
        self.touch_recent(&m);
        write_pref("zandor.model", &format!("{}/{}", m.provider_id, m.model_id));
        self.model = Some(m);
        cx.notify();
    }

    /// Ganti model di tengah sesi `sid` dengan pengecekan konteks: bila riwayat sesi tidak muat (atau mepet ambang)
    /// di jendela model baru, riwayat diringkas lebih dulu dengan model LAMA (yang masih muat), lalu model diganti.
    /// Auto compact mati = hanya peringatan. Mengembalikan hasil pengecekan agar pemanggil bisa memberi tahu pengguna.
    pub fn switch_model(&mut self, sid: Option<&str>, m: ModelRef, cx: &mut Context<Self>) -> super::context_policy::SwitchFit {
        use super::context_policy::{switch_fit, SwitchFit};
        let (max, auto, thr) = Self::context_settings(cx);
        let used = sid.and_then(|s| self.context_used(s));
        let new_limit = super::context_policy::effective_limit(self.ctx.limits.get(&model_key(&m)).copied(), max);
        let same = self.model.as_ref().is_some_and(|c| c.provider_id == m.provider_id && c.model_id == m.model_id);
        let fit = if same { SwitchFit::Unknown } else { switch_fit(used, new_limit, thr) };
        if let (SwitchFit::NeedsCompact, Some(sid)) = (fit, sid) {
            let busy = self.sessions.get(sid).is_some_and(|s| s.busy());
            if auto && !busy {
                // compact() menangkap self.model (lama) sebelum diganti di bawah.
                self.compact(sid, cx);
                let i = cx.global::<crate::i18n::I18n>().clone();
                let (u, l) = (used.unwrap_or(0.), new_limit.unwrap_or(0));
                notify(cx, i.tp("context.switchCompact", &[("used", &super::usage::format_tokens(u, false)), ("limit", &super::usage::format_tokens(l as f64, false))]), crate::ui::ToastTone::Warning);
            } else {
                let i = cx.global::<crate::i18n::I18n>().clone();
                let (u, l) = (used.unwrap_or(0.), new_limit.unwrap_or(0));
                notify(cx, i.tp("context.switchTight", &[("used", &super::usage::format_tokens(u, false)), ("limit", &super::usage::format_tokens(l as f64, false))]), crate::ui::ToastTone::Warning);
            }
        }
        self.select_model(m, cx);
        fit
    }

    /// Daftar batas jendela asli per model, dari daftar model engine.
    pub fn set_context_limits(&mut self, limits: HashMap<String, u64>) {
        self.ctx.limits = limits;
    }

    /// (batas maksimum buatan pengguna, auto compact aktif, ambang %) dari Pengaturan.
    fn context_settings(cx: &App) -> (u64, bool, f64) {
        let st = super::settings::SettingsStore::global(cx);
        let st = st.read(cx);
        (st.num("contextMax").max(0.) as u64, st.bool("autoCompact"), st.num("autoCompactAt").clamp(1., 100.))
    }

    /// Token konteks terakhir yang dilaporkan sesi (permintaan model terakhir).
    fn context_used(&self, sid: &str) -> Option<f64> {
        super::usage::session_usage(&self.sessions.get(sid)?.messages, None, None).context_tokens
    }

    /// Dipanggil saat sesi idle: ringkas otomatis bila konteks (terhadap batas model aktif + batas buatan pengguna)
    /// melewati ambang. Idle sesudah ringkasan dilewati satu kali agar tidak berulang.
    fn maybe_auto_compact(&mut self, sid: &str, cx: &mut Context<Self>) {
        if self.ctx.skip_idle.remove(sid) {
            return;
        }
        let (max, auto, thr) = Self::context_settings(cx);
        let (Some(model), true) = (self.model.clone(), auto) else { return };
        if self.sessions.get(sid).is_none_or(|s| s.busy() || !s.loaded) {
            return;
        }
        let limit = super::context_policy::effective_limit(self.ctx.limits.get(&model_key(&model)).copied(), max);
        let used = self.context_used(sid);
        if !super::context_policy::should_auto_compact(used, limit, thr) {
            return;
        }
        let pct = used.zip(limit).map(|(u, l)| u / l as f64 * 100.).unwrap_or(thr);
        self.compact(sid, cx);
        let i = cx.global::<crate::i18n::I18n>().clone();
        notify(cx, i.tp("context.autoCompacted", &[("pct", &format!("{pct:.0}"))]), crate::ui::ToastTone::Info);
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
        // idle setelah ringkasan jangan memicu auto compact lagi (lihat ContextState)
        self.ctx.skip_idle.insert(sid.clone());
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

    /// Lanjutkan giliran yang terputus (lihat `resume`): kirim pesan pendek `visible` + catatan sintetis berisi
    /// permintaan awal dan ringkasan progres, supaya model melanjutkan alih-alih mengulang dari nol.
    pub fn resume(&mut self, id: &str, visible: String, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(st) = self.sessions.get(id) else { return Err("engine belum siap".into()) };
        if st.busy() {
            return Err("sesi sedang berjalan".into());
        }
        let msgs: Vec<_> = st.visible().cloned().collect();
        let (request, progress) = super::resume::gather(&msgs).ok_or("tidak ada permintaan pengguna untuk dilanjutkan")?;
        let note = super::resume::build_note(&request, &progress, &st.todos);
        if let Some(st) = self.sessions.get_mut(id) {
            st.error = None;
            st.status = super::types::SessionRunStatus::Busy;
        }
        let (sid, model, variant) = (id.to_string(), self.model.clone(), self.variant.clone());
        self.bg(id, cx, move |api, dir| api.send_prompt_with(dir, &sid, &visible, model.as_ref(), variant.as_deref(), &[], &[note]));
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
        if let Some(m) = model.clone() {
            self.touch_recent(&m);
        }
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

#[cfg(test)]
mod keys_tests {
    #[test]
    fn keys() {
        assert_eq!(super::parse_keys(Some(r#"["a",1,"a",null,"b"]"#)), vec!["a", "b"]);
        assert!(super::parse_keys(Some("{rusak")).is_empty());
        assert!(super::parse_keys(None).is_empty());
    }
}

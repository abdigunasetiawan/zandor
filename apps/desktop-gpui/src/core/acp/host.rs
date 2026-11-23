//! Host ACP: menjalankan agent (satu proses per agent, dipakai banyak sesi), memetakan sesi Zandor ↔ sesi ACP,
//! menyimpan transkrip lokal, dan mengubah `session/update` menjadi `EngineEvent` untuk ChatStore.
//!
//! Sesi Zandor tetap dibuat lewat engine OpenCode (daftar sesi, judul, hapus). Saat model yang dipilih milik agent
//! ACP, giliran dikirim ke agent; pesan giliran itu disimpan di `<data>/acp/sessions/<id>.json` dan digabung dengan
//! pesan engine saat sesi dimuat. Ganti model di tengah sesi: riwayat yang belum dilihat model baru dioper sebagai
//! konteks (lihat `convert::history_text`).
use super::agents::{self, AgentDef, AgentsConfig, Missing};
use super::convert::{self, ascending};
use super::rpc::{Conn, Incoming, RpcError};
use crate::core::attachments::FileAttachment;
use crate::core::engine::ModelOption;
use crate::core::types::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

pub type EventFn = Arc<dyn Fn(EngineEvent) + Send + Sync>;

const PROTOCOL_VERSION: u64 = 1;
const HISTORY_CHARS: usize = 24_000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CachedModel {
    pub id: String,
    pub name: String,
    pub variants: Vec<String>,
    pub context: Option<u64>,
    pub image: bool,
}

/// Status satu agent untuk Pengaturan/Onboarding.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub login: &'static str,
    pub needs_adapter: bool,
    pub enabled: bool,
    pub launch: Result<String, Missing>,
    pub cli: Option<PathBuf>,
    pub models: usize,
    pub running: bool,
    pub error: Option<String>,
}

struct Sub {
    dir: String,
    stop: Arc<AtomicBool>,
    f: EventFn,
}

struct AgentRt {
    agent: &'static str,
    conn: Arc<Conn>,
    caps: Value,
    epoch: u64,
}

struct Turn {
    mid: String,
    cur: Option<(String, &'static str)>,
    tools: HashMap<String, String>,
    cancelled: bool,
}

#[derive(Default)]
struct Sess {
    dir: String,
    agent: Option<String>,
    acp_sid: Option<String>,
    live_gen: Option<u64>,
    messages: Vec<Value>,
    todos: Option<Vec<Value>>,
    synced_at: f64,
    oc_synced_at: f64,
    turn: Option<Turn>,
    config: Value,
    modes: Value,
    models: Value,
    exists: bool,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SessFile {
    version: u32,
    dir: String,
    agent: Option<String>,
    acp_session_id: Option<String>,
    messages: Vec<Value>,
    todos: Option<Vec<Value>>,
    synced_at: f64,
    oc_synced_at: f64,
}

struct Perm {
    agent: &'static str,
    epoch: u64,
    rpc_id: Value,
    zsid: String,
    options: Vec<Value>,
    req: PermissionRequest,
}

#[derive(Default)]
struct State {
    config: AgentsConfig,
    /// Proses agent per (agent, folder kerja): sebagian agent memakai folder proses, bukan `cwd` sesi.
    agents: HashMap<String, AgentRt>,
    sessions: HashMap<String, Sess>,
    by_acp: HashMap<String, String>,
    perms: HashMap<String, Perm>,
    models: BTreeMap<String, Vec<CachedModel>>,
    permission_mode: String,
    oneshot: HashMap<String, String>,
    loading: HashSet<String>,
    errors: HashMap<&'static str, String>,
}

struct Inner {
    root: PathBuf,
    st: Mutex<State>,
    subs: Mutex<Vec<Sub>>,
    spawn_lock: Mutex<()>,
    epoch: AtomicU64,
    perm_seq: AtomicU64,
}

#[derive(Clone)]
pub struct AcpHost(Arc<Inner>);

static HOST: OnceLock<AcpHost> = OnceLock::new();

/// Host global (dibuat saat pertama dipakai, data di `<app data>/acp`).
pub fn host() -> &'static AcpHost {
    HOST.get_or_init(|| AcpHost::new(crate::core::services::app_data_dir().join("acp")))
}

fn now() -> f64 {
    crate::core::projects::now_ms()
}

fn key(dir: &str) -> String {
    crate::core::projects::path_key(dir)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Pilihan konfigurasi sesi (ACP `configOptions`), grup diratakan.
fn config_values(opt: &Value) -> Vec<(String, String)> {
    let mut out = vec![];
    for o in opt.get("options").and_then(Value::as_array).into_iter().flatten() {
        if let Some(sub) = o.get("options").and_then(Value::as_array) {
            for x in sub {
                if let Some(v) = x.get("value").and_then(Value::as_str) {
                    out.push((v.to_string(), x.get("name").and_then(Value::as_str).unwrap_or(v).to_string()));
                }
            }
        } else if let Some(v) = o.get("value").and_then(Value::as_str) {
            out.push((v.to_string(), o.get("name").and_then(Value::as_str).unwrap_or(v).to_string()));
        }
    }
    out
}

fn config_by_category<'a>(config: &'a Value, cat: &str) -> Option<&'a Value> {
    config.as_array()?.iter().find(|o| o.get("category").and_then(Value::as_str) == Some(cat))
}

/// Model yang ditawarkan agent di balasan `session/new`.
pub fn extract_models(result: &Value) -> Vec<CachedModel> {
    let config = result.get("configOptions").cloned().unwrap_or(Value::Null);
    let variants: Vec<String> = config_by_category(&config, "thought_level").map(config_values).unwrap_or_default().into_iter().map(|(v, _)| v).filter(|v| v != "default").collect();
    if let Some(m) = config_by_category(&config, "model") {
        let list = config_values(m);
        if !list.is_empty() {
            return list.into_iter().map(|(id, name)| CachedModel { id, name, variants: variants.clone(), ..Default::default() }).collect();
        }
    }
    result
        .pointer("/models/availableModels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m.get("modelId").and_then(Value::as_str)?.to_string();
            Some(CachedModel { name: m.get("name").and_then(Value::as_str).unwrap_or(&id).to_string(), id, variants: variants.clone(), ..Default::default() })
        })
        .collect()
}

/// Mode agent yang paling sesuai dengan mode izin Zandor (agent menegakkan sendiri; Zandor tetap menjawab
/// permintaan izin yang datang sesuai mode). Urutan = preferensi.
pub fn mode_for(permission_mode: &str, available: &[String]) -> Option<String> {
    let prefs: &[&str] = match permission_mode {
        "auto-edit" => &["acceptEdits", "accept-edits", "auto"],
        "auto-safe" => &["auto", "acceptEdits", "accept-edits"],
        "auto-all" => &["bypassPermissions", "full-access", "accept-edits", "acceptEdits", "auto"],
        _ => &["default", "read-only", "manual"],
    };
    prefs.iter().find(|p| available.iter().any(|a| a == *p)).map(|p| p.to_string())
}

fn auth_hint(def: &AgentDef, e: &RpcError) -> String {
    if e.is_auth() {
        format!("{} belum login. Jalankan `{}` di terminal, lalu coba lagi. ({})", def.name, def.login, e.describe())
    } else {
        format!("{}: {}", def.name, e.describe())
    }
}

fn missing_text(def: &AgentDef, m: &Missing) -> String {
    match m {
        Missing::Cli => format!("CLI `{}` tidak ditemukan. Pasang {} lalu login dengan `{}`.", def.cli, def.name, def.login),
        Missing::Adapter { .. } => format!("Adaptor ACP {} belum dipasang. Buka Pengaturan > Model > Agent CLI lalu pilih Pasang adaptor.", def.name),
        Missing::Node => "Node.js tidak ditemukan di PATH; adaptor ACP butuh Node.js.".into(),
        Missing::Command(c) => format!("Perintah agent `{c}` tidak ditemukan."),
    }
}

impl AcpHost {
    pub fn new(root: PathBuf) -> Self {
        let config = AgentsConfig::load(&root.join("agents.json"));
        let models = std::fs::read_to_string(root.join("models.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let st = State { config, models, permission_mode: "ask".into(), ..Default::default() };
        AcpHost(Arc::new(Inner { root, st: Mutex::new(st), subs: Mutex::new(vec![]), spawn_lock: Mutex::new(()), epoch: AtomicU64::new(1), perm_seq: AtomicU64::new(1) }))
    }

    fn tools_dir(&self) -> PathBuf {
        self.0.root.join("tools")
    }
    fn scratch_dir(&self) -> PathBuf {
        let d = self.0.root.join("scratch");
        let _ = std::fs::create_dir_all(&d);
        d
    }
    fn sess_path(&self, zsid: &str) -> PathBuf {
        let safe: String = zsid.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
        self.0.root.join("sessions").join(format!("{safe}.json"))
    }

    // ---------------- langganan event ----------------

    pub fn subscribe(&self, dir: &str, stop: Arc<AtomicBool>, f: EventFn) {
        let mut subs = lock(&self.0.subs);
        subs.retain(|s| !s.stop.load(Ordering::Relaxed));
        subs.push(Sub { dir: key(dir), stop, f });
    }

    fn emit(&self, dir: &str, ev: EngineEvent) {
        let k = key(dir);
        let subs: Vec<EventFn> = lock(&self.0.subs).iter().filter(|s| !s.stop.load(Ordering::Relaxed) && s.dir == k).map(|s| s.f.clone()).collect();
        for f in subs {
            f(ev.clone());
        }
    }

    // ---------------- konfigurasi & status ----------------

    pub fn config(&self, id: &str) -> agents::AgentConfig {
        lock(&self.0.st).config.get(id)
    }

    pub fn set_enabled(&self, id: &str, on: bool) -> Result<(), String> {
        let def = agents::by_id(id).ok_or("agent tidak dikenal")?;
        let conn = {
            let mut st = lock(&self.0.st);
            st.config.agents.entry(id.to_string()).or_default().enabled = on;
            st.config.save(&self.0.root.join("agents.json"))?;
            if on {
                vec![]
            } else {
                let keys: Vec<String> = st.agents.iter().filter(|(_, a)| a.agent == def.id).map(|(k, _)| k.clone()).collect();
                keys.iter().filter_map(|k| st.agents.remove(k)).collect()
            }
        };
        for rt in conn {
            rt.conn.kill();
        }
        Ok(())
    }

    pub fn status(&self, id: &str) -> Option<AgentStatus> {
        let def = agents::by_id(id)?;
        let st = lock(&self.0.st);
        let cfg = st.config.get(id);
        Some(AgentStatus {
            id: def.id,
            name: def.name,
            login: def.login,
            needs_adapter: def.adapter.is_some(),
            enabled: cfg.enabled,
            launch: agents::resolve(def, &cfg, &self.tools_dir()).map(|l| l.display()),
            cli: agents::find_cli(def),
            models: st.models.get(id).map(Vec::len).unwrap_or(0),
            running: st.agents.values().any(|a| a.agent == def.id && a.conn.alive()),
            error: st.errors.get(def.id).cloned(),
        })
    }

    pub fn set_permission_mode(&self, mode: &str) {
        lock(&self.0.st).permission_mode = mode.to_string();
    }

    /// Model semua agent yang aktif (dari cache; diperbarui setiap sesi baru dan lewat `refresh_models`).
    pub fn models(&self) -> Vec<ModelOption> {
        let st = lock(&self.0.st);
        let mut out = vec![];
        for def in &agents::AGENTS {
            if !st.config.get(def.id).enabled {
                continue;
            }
            for m in st.models.get(def.id).into_iter().flatten() {
                out.push(ModelOption {
                    provider_id: def.provider_id.into(),
                    provider_name: def.name.into(),
                    model_id: m.id.clone(),
                    name: m.name.clone(),
                    variants: m.variants.clone(),
                    context_limit: m.context,
                    output_limit: None,
                    reasoning: Some(!m.variants.is_empty()),
                    tool_call: Some(true),
                    attachment: Some(m.image),
                    inputs: if m.image { vec!["text".into(), "image".into()] } else { vec!["text".into()] },
                });
            }
        }
        out
    }

    /// Agent aktif yang daftar modelnya belum pernah diambil (untuk pengambilan awal di latar).
    pub fn agents_needing_models(&self) -> Vec<&'static str> {
        let st = lock(&self.0.st);
        agents::AGENTS.iter().filter(|d| st.config.get(d.id).enabled && st.models.get(d.id).is_none_or(Vec::is_empty)).map(|d| d.id).collect()
    }

    fn record_models(&self, def: &AgentDef, result: &Value, caps: &Value) {
        let mut list = extract_models(result);
        if list.is_empty() {
            return;
        }
        let image = caps.pointer("/promptCapabilities/image").and_then(Value::as_bool).unwrap_or(false);
        for m in &mut list {
            m.image = image;
        }
        let mut st = lock(&self.0.st);
        if let Some(old) = st.models.get(def.id) {
            for m in &mut list {
                m.context = old.iter().find(|o| o.id == m.id).and_then(|o| o.context);
            }
        }
        st.models.insert(def.id.to_string(), list);
        let _ = std::fs::create_dir_all(&self.0.root);
        let _ = std::fs::write(self.0.root.join("models.json"), serde_json::to_string(&st.models).unwrap_or_default());
    }

    /// Uji agent: jalankan, buka sesi sementara, ambil daftar model. Hasil = jumlah model.
    pub fn refresh_models(&self, id: &str) -> Result<usize, String> {
        let def = agents::by_id(id).ok_or("agent tidak dikenal")?;
        let r = (|| -> Result<usize, String> {
            let (conn, caps, _) = self.ensure_conn(def, &self.scratch_dir())?;
            let res = conn.request("session/new", json!({ "cwd": self.scratch_dir(), "mcpServers": [] }), Some(Duration::from_secs(120))).map_err(|e| auth_hint(def, &e))?;
            self.record_models(def, &res, &caps);
            if let Some(sid) = res.get("sessionId").and_then(Value::as_str) {
                self.close_session(&conn, &caps, sid);
            }
            Ok(extract_models(&res).len())
        })();
        let mut st = lock(&self.0.st);
        match &r {
            Ok(_) => st.errors.remove(def.id),
            Err(e) => st.errors.insert(def.id, e.clone()),
        };
        r
    }

    pub fn install(&self, id: &str) -> Result<(), String> {
        let def = agents::by_id(id).ok_or("agent tidak dikenal")?;
        agents::install_adapter(def, &self.tools_dir())
    }

    // ---------------- proses agent ----------------

    fn ensure_conn(&self, def: &'static AgentDef, cwd: &Path) -> Result<(Arc<Conn>, Value, u64), String> {
        let _g = lock(&self.0.spawn_lock);
        let rt_key = format!("{}|{}", def.id, key(&cwd.display().to_string()));
        let cfg = {
            let st = lock(&self.0.st);
            if let Some(rt) = st.agents.get(&rt_key).filter(|a| a.conn.alive()) {
                return Ok((rt.conn.clone(), rt.caps.clone(), rt.epoch));
            }
            st.config.get(def.id)
        };
        let launch = agents::resolve(def, &cfg, &self.tools_dir()).map_err(|m| missing_text(def, &m))?;
        let epoch = self.0.epoch.fetch_add(1, Ordering::Relaxed);
        let weak: Weak<Inner> = Arc::downgrade(&self.0);
        let conn = Conn::spawn(&launch, cwd, move |m| {
            if let Some(inner) = weak.upgrade() {
                AcpHost(inner).incoming(def, epoch, m);
            }
        })?;
        let init = conn
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    // Agent memakai alat bawaannya sendiri untuk berkas dan terminal.
                    "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false },
                    "clientInfo": { "name": "zandor", "title": "Zandor", "version": env!("CARGO_PKG_VERSION") }
                }),
                Some(Duration::from_secs(120)),
            )
            .map_err(|e| {
                conn.kill();
                auth_hint(def, &e)
            })?;
        let caps = init.get("agentCapabilities").cloned().unwrap_or(Value::Null);
        lock(&self.0.st).agents.insert(rt_key, AgentRt { agent: def.id, conn: conn.clone(), caps: caps.clone(), epoch });
        Ok((conn, caps, epoch))
    }

    fn close_session(&self, conn: &Conn, caps: &Value, sid: &str) {
        if caps.pointer("/sessionCapabilities/close").is_some() {
            let _ = conn.request("session/close", json!({ "sessionId": sid }), Some(Duration::from_secs(10)));
        }
    }

    fn incoming(&self, def: &'static AgentDef, epoch: u64, m: Incoming) {
        match m {
            Incoming::Notification { method, params } if method == "session/update" => {
                let sid = params.get("sessionId").and_then(Value::as_str).unwrap_or("").to_string();
                if let Some(upd) = params.get("update") {
                    self.on_update(&sid, upd);
                }
            }
            Incoming::Notification { .. } => {}
            Incoming::Request { id, method, params } if method == "session/request_permission" => self.on_permission(def, epoch, id, &params),
            Incoming::Request { id, .. } => {
                // fs/terminal tidak diiklankan; permintaan lain ditolak dengan jelas.
                if let Some(c) = self.conn_of(def.id, epoch) {
                    c.respond(id, Err(RpcError::new(-32601, "metode tidak didukung oleh Zandor")));
                }
            }
            Incoming::Closed => self.on_closed(def, epoch),
        }
    }

    fn conn_of(&self, agent: &str, epoch: u64) -> Option<Arc<Conn>> {
        lock(&self.0.st).agents.values().find(|a| a.agent == agent && a.epoch == epoch).map(|a| a.conn.clone())
    }

    fn on_closed(&self, def: &AgentDef, epoch: u64) {
        let mut st = lock(&self.0.st);
        st.agents.retain(|_, a| !(a.agent == def.id && a.epoch == epoch));
        for s in st.sessions.values_mut() {
            if s.live_gen == Some(epoch) {
                s.live_gen = None;
            }
        }
        st.perms.retain(|_, p| p.epoch != epoch);
    }

    // ---------------- sesi & transkrip ----------------

    fn load_sess<'a>(&self, st: &'a mut State, zsid: &str, dir: Option<&str>) -> &'a mut Sess {
        if !st.sessions.contains_key(zsid) {
            let file: Option<SessFile> = std::fs::read_to_string(self.sess_path(zsid)).ok().and_then(|t| serde_json::from_str(&t).ok());
            let s = match file {
                Some(f) => Sess { dir: f.dir, agent: f.agent, acp_sid: f.acp_session_id, messages: f.messages, todos: f.todos, synced_at: f.synced_at, oc_synced_at: f.oc_synced_at, exists: true, ..Default::default() },
                None => Sess { dir: dir.unwrap_or_default().to_string(), ..Default::default() },
            };
            if let Some(a) = &s.acp_sid {
                st.by_acp.insert(a.clone(), zsid.to_string());
            }
            st.sessions.insert(zsid.to_string(), s);
        }
        let s = st.sessions.get_mut(zsid).expect("sesi baru dimasukkan");
        if s.dir.is_empty() {
            s.dir = dir.unwrap_or_default().to_string();
        }
        s
    }

    fn save(&self, zsid: &str, s: &mut Sess) {
        let f = SessFile { version: 1, dir: s.dir.clone(), agent: s.agent.clone(), acp_session_id: s.acp_sid.clone(), messages: s.messages.clone(), todos: s.todos.clone(), synced_at: s.synced_at, oc_synced_at: s.oc_synced_at };
        let p = self.sess_path(zsid);
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_string(&f).unwrap_or_default()).and_then(|_| std::fs::rename(&tmp, &p)).is_ok() {
            s.exists = true;
        }
    }

    /// Ada transkrip ACP untuk sesi ini (tanpa membuat entri baru).
    fn has(&self, zsid: &str) -> bool {
        lock(&self.0.st).sessions.get(zsid).is_some_and(|s| s.exists || !s.messages.is_empty()) || self.sess_path(zsid).is_file()
    }

    pub fn messages(&self, zsid: &str) -> Vec<ChatMessage> {
        if !self.has(zsid) {
            return vec![];
        }
        let mut st = lock(&self.0.st);
        self.load_sess(&mut st, zsid, None).messages.iter().filter_map(to_message).collect()
    }

    pub fn owns_message(&self, zsid: &str, mid: &str) -> bool {
        if !self.has(zsid) {
            return false;
        }
        let mut st = lock(&self.0.st);
        self.load_sess(&mut st, zsid, None).messages.iter().any(|m| m.pointer("/info/id").and_then(Value::as_str) == Some(mid))
    }

    pub fn message_created(&self, zsid: &str, mid: &str) -> Option<f64> {
        self.messages(zsid).into_iter().find(|m| m.info.id == mid).map(|m| m.info.created)
    }

    pub fn todos(&self, zsid: &str) -> Option<Vec<Todo>> {
        if !self.has(zsid) {
            return None;
        }
        let mut st = lock(&self.0.st);
        let s = self.load_sess(&mut st, zsid, None);
        s.todos.as_ref().map(|t| to_todos(Some(&Value::Array(t.clone()))))
    }

    pub fn busy(&self, zsid: &str) -> bool {
        lock(&self.0.st).sessions.get(zsid).is_some_and(|s| s.turn.is_some())
    }

    /// Waktu aktivitas ACP terakhir (untuk urutan sidebar).
    pub fn last_activity(&self, zsid: &str) -> Option<f64> {
        if !self.has(zsid) {
            return None;
        }
        self.messages(zsid).iter().map(|m| m.info.completed.unwrap_or(m.info.created)).reduce(f64::max)
    }

    pub fn permissions(&self, dir: &str) -> Vec<PermissionRequest> {
        let k = key(dir);
        let st = lock(&self.0.st);
        st.perms.values().filter(|p| st.sessions.get(&p.zsid).is_some_and(|s| key(&s.dir) == k)).map(|p| p.req.clone()).collect()
    }

    pub fn forget(&self, zsid: &str) {
        let mut st = lock(&self.0.st);
        if let Some(s) = st.sessions.remove(zsid) {
            if let Some(a) = s.acp_sid {
                st.by_acp.remove(&a);
            }
        }
        let _ = std::fs::remove_file(self.sess_path(zsid));
    }

    /// Buang pesan ACP sejak `cutoff` (waktu dibuat ≥ cutoff, atau id ≥ `from_id`). Konteks agent ikut tidak sah:
    /// giliran berikutnya membuka sesi ACP baru dengan riwayat yang tersisa.
    pub fn truncate(&self, zsid: &str, from_id: Option<&str>, cutoff: Option<f64>) -> usize {
        if !self.has(zsid) {
            return 0;
        }
        let mut st = lock(&self.0.st);
        let s = self.load_sess(&mut st, zsid, None);
        if s.turn.is_some() {
            return 0;
        }
        let cut = |m: &Value| {
            let id = m.pointer("/info/id").and_then(Value::as_str).unwrap_or("");
            let created = m.pointer("/info/time/created").and_then(Value::as_f64).unwrap_or(0.);
            from_id.is_some_and(|f| id >= f) || cutoff.is_some_and(|c| created >= c)
        };
        let removed: Vec<String> = s.messages.iter().filter(|m| cut(m)).filter_map(|m| m.pointer("/info/id").and_then(Value::as_str).map(String::from)).collect();
        if removed.is_empty() {
            return 0;
        }
        s.messages.retain(|m| !cut(m));
        let old = s.acp_sid.take();
        s.live_gen = None;
        s.synced_at = 0.;
        let dir = s.dir.clone();
        self.save(zsid, s);
        if let Some(a) = old {
            st.by_acp.remove(&a);
        }
        drop(st);
        for mid in &removed {
            self.emit(&dir, EngineEvent::MessageRemoved { session_id: zsid.to_string(), message_id: mid.clone() });
        }
        removed.len()
    }

    /// Salin pesan ACP sebelum `cutoff` ke sesi hasil cabang (id baru, tanpa sesi ACP: konteks dioper ulang).
    pub fn copy_for_fork(&self, src: &str, dst: &str, dir: &str, cutoff: Option<f64>) {
        if !self.has(src) {
            return;
        }
        let msgs: Vec<Value> = {
            let mut st = lock(&self.0.st);
            self.load_sess(&mut st, src, None).messages.iter().filter(|m| cutoff.is_none_or(|c| m.pointer("/info/time/created").and_then(Value::as_f64).unwrap_or(0.) < c)).cloned().collect()
        };
        if msgs.is_empty() {
            return;
        }
        let mut out = vec![];
        let mut ids: HashMap<String, String> = HashMap::new();
        for mut m in msgs {
            let old = m.pointer("/info/id").and_then(Value::as_str).unwrap_or("").to_string();
            let new = ascending("msg");
            ids.insert(old, new.clone());
            m["info"]["id"] = json!(new);
            m["info"]["sessionID"] = json!(dst);
            if let Some(p) = m["info"].get("parentID").and_then(Value::as_str).and_then(|p| ids.get(p)).cloned() {
                m["info"]["parentID"] = json!(p);
            }
            for p in m["parts"].as_array_mut().into_iter().flatten() {
                p["id"] = json!(ascending("prt"));
                p["messageID"] = json!(new);
                p["sessionID"] = json!(dst);
            }
            out.push(m);
        }
        let mut st = lock(&self.0.st);
        let s = self.load_sess(&mut st, dst, Some(dir));
        s.messages = out;
        self.save(dst, s);
    }

    /// Catatan sintetis untuk model OpenCode: pesan ACP yang belum pernah dilihatnya (ganti dari agent ke OpenCode).
    pub fn opencode_handoff(&self, zsid: &str) -> Option<String> {
        if !self.has(zsid) {
            return None;
        }
        let mut st = lock(&self.0.st);
        let s = self.load_sess(&mut st, zsid, None);
        let since = s.oc_synced_at;
        let msgs: Vec<ChatMessage> = s.messages.iter().filter_map(to_message).filter(|m| m.info.created > since).collect();
        if msgs.is_empty() {
            return None;
        }
        s.oc_synced_at = now();
        self.save(zsid, s);
        convert::history_text(&msgs.iter().collect::<Vec<_>>(), HISTORY_CHARS)
    }

    // ---------------- giliran ----------------

    /// Mulai satu giliran. Kembali segera; jawaban mengalir lewat event. `history` = semua pesan sesi (engine + ACP)
    /// sebelum pesan ini, dipakai bila agent belum melihatnya.
    #[allow(clippy::too_many_arguments)]
    pub fn prompt(&self, dir: &str, zsid: &str, text: &str, provider_id: &str, model_id: &str, variant: Option<&str>, files: &[FileAttachment], synthetic: &[String], history: Vec<ChatMessage>) -> Result<(), String> {
        let def = agents::by_provider(provider_id).ok_or("provider ACP tidak dikenal")?;
        let mut st = lock(&self.0.st);
        if !st.config.get(def.id).enabled {
            return Err(format!("{} belum diaktifkan. Aktifkan di Pengaturan > Model > Agent CLI.", def.name));
        }
        let s = self.load_sess(&mut st, zsid, Some(dir));
        if s.turn.is_some() {
            return Err("agent masih bekerja di sesi ini".into());
        }
        let t = now();
        let uid = ascending("msg");
        let mut parts = vec![convert::text_part(&ascending("prt"), &uid, zsid, "text", text, false)];
        for f in files {
            let mut p = json!({ "id": ascending("prt"), "messageID": uid, "sessionID": zsid, "type": "file", "mime": f.mime, "url": f.url });
            if let Some(n) = &f.filename {
                p["filename"] = json!(n);
            }
            parts.push(p);
        }
        for n in synthetic {
            parts.push(convert::text_part(&ascending("prt"), &uid, zsid, "text", n, true));
        }
        let user = json!({ "info": convert::message_info(&uid, zsid, Role::User, t, None, Some(provider_id), Some(model_id)), "parts": parts });
        let aid = ascending("msg");
        let asst = json!({ "info": convert::message_info(&aid, zsid, Role::Assistant, t + 1., Some(&uid), Some(provider_id), Some(model_id)), "parts": [] });
        s.messages.push(user.clone());
        s.messages.push(asst.clone());
        s.turn = Some(Turn { mid: aid.clone(), cur: None, tools: HashMap::new(), cancelled: false });
        let dir_s = s.dir.clone();
        self.save(zsid, s);
        drop(st);
        self.emit_message(&dir_s, &user);
        self.emit_message(&dir_s, &asst);
        self.emit(&dir_s, EngineEvent::SessionStatus { session_id: zsid.into(), status: SessionRunStatus::Busy });

        let me = self.clone();
        let (zsid, dir, model_id, variant, files, synthetic, text) = (zsid.to_string(), dir.to_string(), model_id.to_string(), variant.map(String::from), files.to_vec(), synthetic.to_vec(), text.to_string());
        std::thread::Builder::new()
            .name("zandor-acp-turn".into())
            .spawn(move || {
                let r = me.run_turn(def, &dir, &zsid, &text, &model_id, variant.as_deref(), &files, &synthetic, &history);
                me.finish(&zsid, r);
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn emit_message(&self, dir: &str, m: &Value) {
        if let Some(info) = to_message_info(m.get("info")) {
            self.emit(dir, EngineEvent::MessageUpdated(info));
        }
        for p in m.get("parts").and_then(Value::as_array).into_iter().flatten() {
            if let Some(p) = to_part(Some(p)) {
                self.emit(dir, EngineEvent::PartUpdated(p));
            }
        }
    }

    /// Pastikan sesi ACP terbuka untuk sesi Zandor ini. Hasil: (id sesi ACP, baru dibuat).
    fn ensure_session(&self, def: &'static AgentDef, conn: &Arc<Conn>, caps: &Value, epoch: u64, zsid: &str, dir: &str) -> Result<(String, bool), String> {
        let (prev, live) = {
            let mut st = lock(&self.0.st);
            let s = self.load_sess(&mut st, zsid, Some(dir));
            let same = s.agent.as_deref() == Some(def.id);
            (if same { s.acp_sid.clone() } else { None }, same && s.live_gen == Some(epoch))
        };
        if let (Some(sid), true) = (&prev, live) {
            return Ok((sid.clone(), false));
        }
        let params = |sid: &str| json!({ "sessionId": sid, "cwd": dir, "mcpServers": [] });
        if let Some(sid) = prev {
            // Lanjutkan sesi ACP lama (aplikasi/agent sempat dimulai ulang). resume tidak memutar ulang riwayat;
            // load memutar ulang lewat session/update, yang diabaikan karena transkrip sudah tersimpan.
            let resumed = if caps.pointer("/sessionCapabilities/resume").is_some() {
                conn.request("session/resume", params(&sid), Some(Duration::from_secs(120))).ok()
            } else if caps.get("loadSession").and_then(Value::as_bool) == Some(true) {
                lock(&self.0.st).loading.insert(sid.clone());
                let r = conn.request("session/load", params(&sid), Some(Duration::from_secs(180))).ok();
                lock(&self.0.st).loading.remove(&sid);
                r
            } else {
                None
            };
            if let Some(res) = resumed {
                let mut st = lock(&self.0.st);
                st.by_acp.insert(sid.clone(), zsid.to_string());
                let s = self.load_sess(&mut st, zsid, Some(dir));
                s.live_gen = Some(epoch);
                Self::take_session_info(s, &res);
                return Ok((sid, false));
            }
        }
        let res = conn.request("session/new", json!({ "cwd": dir, "mcpServers": [] }), Some(Duration::from_secs(120))).map_err(|e| auth_hint(def, &e))?;
        let sid = res.get("sessionId").and_then(Value::as_str).ok_or("agent tidak memberi sessionId")?.to_string();
        self.record_models(def, &res, caps);
        let mut st = lock(&self.0.st);
        let old = self.load_sess(&mut st, zsid, Some(dir)).acp_sid.replace(sid.clone());
        if let Some(old) = old.filter(|o| *o != sid) {
            st.by_acp.remove(&old);
        }
        st.by_acp.insert(sid.clone(), zsid.to_string());
        let s = st.sessions.get_mut(zsid).expect("sesi ada");
        s.agent = Some(def.id.to_string());
        s.live_gen = Some(epoch);
        s.synced_at = 0.;
        Self::take_session_info(s, &res);
        Ok((sid, true))
    }

    fn take_session_info(s: &mut Sess, res: &Value) {
        if let Some(c) = res.get("configOptions").filter(|c| c.is_array()) {
            s.config = c.clone();
        }
        if let Some(m) = res.get("modes").filter(|m| !m.is_null()) {
            s.modes = m.clone();
        }
        if let Some(m) = res.get("models").filter(|m| !m.is_null()) {
            s.models = m.clone();
        }
    }

    /// Terapkan model, tingkat penalaran, dan mode izin ke sesi ACP.
    fn apply_settings(&self, def: &AgentDef, conn: &Conn, zsid: &str, sid: &str, model_id: &str, variant: Option<&str>) -> Result<(), String> {
        let (config, models, modes, perm) = {
            let st = lock(&self.0.st);
            let s = st.sessions.get(zsid).ok_or("sesi hilang")?;
            (s.config.clone(), s.models.clone(), s.modes.clone(), st.permission_mode.clone())
        };
        let set_opt = |opt: &Value, value: &str| -> Result<(), String> {
            let id = opt.get("id").and_then(Value::as_str).unwrap_or_default();
            let res = conn.request("session/set_config_option", json!({ "sessionId": sid, "configId": id, "value": value }), Some(Duration::from_secs(30))).map_err(|e| e.describe())?;
            if let Some(c) = res.get("configOptions").filter(|c| c.is_array()) {
                if let Some(s) = lock(&self.0.st).sessions.get_mut(zsid) {
                    s.config = c.clone();
                }
            }
            Ok(())
        };
        let current = |opt: &Value| opt.get("currentValue").and_then(Value::as_str).unwrap_or("").to_string();
        // model
        if let Some(opt) = config_by_category(&config, "model") {
            if current(opt) != model_id {
                if !config_values(opt).iter().any(|(v, _)| v == model_id) {
                    return Err(format!("model `{model_id}` tidak tersedia di {}", def.name));
                }
                set_opt(opt, model_id).map_err(|e| format!("gagal memilih model di {}: {e}", def.name))?;
            }
        } else if let Some(list) = models.get("availableModels").and_then(Value::as_array) {
            let cur = models.get("currentModelId").and_then(Value::as_str).unwrap_or("");
            if cur != model_id {
                if !list.iter().any(|m| m.get("modelId").and_then(Value::as_str) == Some(model_id)) {
                    return Err(format!("model `{model_id}` tidak tersedia di {}", def.name));
                }
                conn.request("session/set_model", json!({ "sessionId": sid, "modelId": model_id }), Some(Duration::from_secs(30))).map_err(|e| format!("gagal memilih model di {}: {}", def.name, e.describe()))?;
                if let Some(s) = lock(&self.0.st).sessions.get_mut(zsid) {
                    s.models["currentModelId"] = json!(model_id);
                }
            }
        }
        // tingkat penalaran (varian) — hanya bila agent menawarkannya; gagal tidak menghentikan giliran
        let config = lock(&self.0.st).sessions.get(zsid).map(|s| s.config.clone()).unwrap_or(config);
        if let (Some(v), Some(opt)) = (variant, config_by_category(&config, "thought_level")) {
            if current(opt) != v && config_values(opt).iter().any(|(x, _)| x == v) {
                let _ = set_opt(opt, v);
            }
        }
        // mode izin
        let config = lock(&self.0.st).sessions.get(zsid).map(|s| s.config.clone()).unwrap_or(config);
        if let Some(opt) = config_by_category(&config, "mode") {
            let avail: Vec<String> = config_values(opt).into_iter().map(|(v, _)| v).collect();
            if let Some(m) = mode_for(&perm, &avail).filter(|m| *m != current(opt)) {
                let _ = set_opt(opt, &m);
            }
        } else if let Some(list) = modes.get("availableModes").and_then(Value::as_array) {
            let avail: Vec<String> = list.iter().filter_map(|m| m.get("id").and_then(Value::as_str).map(String::from)).collect();
            let cur = modes.get("currentModeId").and_then(Value::as_str).unwrap_or("");
            if let Some(m) = mode_for(&perm, &avail).filter(|m| m != cur) {
                if conn.request("session/set_mode", json!({ "sessionId": sid, "modeId": m }), Some(Duration::from_secs(30))).is_ok() {
                    if let Some(s) = lock(&self.0.st).sessions.get_mut(zsid) {
                        s.modes["currentModeId"] = json!(m);
                    }
                }
            }
        }
        Ok(())
    }

    fn prompt_blocks(text: &str, synthetic: &[String], files: &[FileAttachment], context: Option<String>, image: bool) -> Vec<Value> {
        let mut blocks = vec![];
        if let Some(c) = context {
            blocks.push(json!({ "type": "text", "text": c }));
        }
        blocks.push(json!({ "type": "text", "text": text }));
        for n in synthetic {
            blocks.push(json!({ "type": "text", "text": n }));
        }
        let mut notes = vec![];
        for f in files {
            let name = f.filename.clone().unwrap_or_else(|| "lampiran".into());
            if let (true, Some(data)) = (image && f.mime.starts_with("image/"), f.url.strip_prefix("data:").and_then(|r| r.split_once(";base64,")).map(|(_, d)| d.to_string())) {
                blocks.push(json!({ "type": "image", "mimeType": f.mime, "data": data }));
            } else if let Some(p) = crate::core::attachments::path_from_file_url(&f.url) {
                blocks.push(json!({ "type": "resource_link", "uri": f.url, "name": name }));
                notes.push(format!("- {name}: {p}"));
            } else {
                notes.push(format!("- {name}: (tidak tersimpan di disk)"));
            }
        }
        if !notes.is_empty() {
            blocks.push(json!({ "type": "text", "text": format!("Lampiran pengguna:\n{}", notes.join("\n")) }));
        }
        blocks
    }

    #[allow(clippy::too_many_arguments)]
    fn run_turn(&self, def: &'static AgentDef, dir: &str, zsid: &str, text: &str, model_id: &str, variant: Option<&str>, files: &[FileAttachment], synthetic: &[String], history: &[ChatMessage]) -> Result<Value, String> {
        let (conn, caps, epoch) = self.ensure_conn(def, Path::new(dir))?;
        let (sid, fresh) = self.ensure_session(def, &conn, &caps, epoch, zsid, dir)?;
        self.apply_settings(def, &conn, zsid, &sid, model_id, variant)?;
        let since = if fresh { 0. } else { lock(&self.0.st).sessions.get(zsid).map(|s| s.synced_at).unwrap_or(0.) };
        // Pesan yang belum dilihat agent ini: semua (sesi baru) atau yang datang sejak giliran terakhirnya.
        let unseen: Vec<&ChatMessage> = history.iter().filter(|m| m.info.created > since).collect();
        let context = convert::history_text(&unseen, HISTORY_CHARS);
        let image = caps.pointer("/promptCapabilities/image").and_then(Value::as_bool).unwrap_or(false);
        let blocks = Self::prompt_blocks(text, synthetic, files, context, image);
        if lock(&self.0.st).sessions.get(zsid).and_then(|s| s.turn.as_ref()).is_some_and(|t| t.cancelled) {
            return Err("dibatalkan".into());
        }
        conn.request("session/prompt", json!({ "sessionId": sid, "prompt": blocks }), None).map_err(|e| {
            if e.code == -32099 {
                if let Some(s) = lock(&self.0.st).sessions.get_mut(zsid) {
                    s.live_gen = None;
                }
            }
            auth_hint(def, &e)
        })
    }

    fn finish(&self, zsid: &str, r: Result<Value, String>) {
        let t = now();
        let mut st = lock(&self.0.st);
        let Some(s) = st.sessions.get_mut(zsid) else { return };
        let Some(turn) = s.turn.take() else { return };
        let dir = s.dir.clone();
        let mut err_event = None;
        if let Some(m) = s.messages.iter_mut().find(|m| m.pointer("/info/id").and_then(Value::as_str) == Some(turn.mid.as_str())) {
            m["info"]["time"]["completed"] = json!(t);
            match &r {
                Ok(res) => {
                    let stop = res.get("stopReason").and_then(Value::as_str).unwrap_or("end_turn");
                    m["info"]["finish"] = json!(match stop {
                        "end_turn" => "stop",
                        "max_tokens" => "length",
                        other => other,
                    });
                    if let Some(tk) = res.get("usage").and_then(convert::tokens_from_usage) {
                        m["info"]["tokens"] = tk;
                    }
                    if stop == "cancelled" || turn.cancelled {
                        m["info"]["error"] = json!({ "name": "MessageAbortedError", "data": { "message": "Dibatalkan" } });
                    }
                }
                Err(_) if turn.cancelled => {
                    m["info"]["error"] = json!({ "name": "MessageAbortedError", "data": { "message": "Dibatalkan" } });
                }
                Err(e) => {
                    m["info"]["error"] = json!({ "name": "AcpError", "data": { "message": e } });
                    err_event = Some(MessageError { name: "AcpError".into(), message: e.clone() });
                }
            }
            // Bagian tool yang masih berjalan saat giliran berakhir ditandai berhenti.
            for p in m["parts"].as_array_mut().into_iter().flatten() {
                if matches!(p.pointer("/state/status").and_then(Value::as_str), Some("pending" | "running")) {
                    p["state"]["status"] = json!("error");
                    p["state"]["error"] = json!("tidak selesai");
                    p["state"]["time"]["end"] = json!(t);
                }
            }
        }
        if r.is_ok() {
            s.synced_at = t;
        }
        let asst = s.messages.iter().find(|m| m.pointer("/info/id").and_then(Value::as_str) == Some(turn.mid.as_str())).cloned();
        self.save(zsid, s);
        // izin yang tertinggal untuk sesi ini sudah tidak berlaku
        let stale: Vec<String> = st.perms.iter().filter(|(_, p)| p.zsid == zsid).map(|(k, _)| k.clone()).collect();
        for k in &stale {
            st.perms.remove(k);
        }
        drop(st);
        if let Some(a) = asst {
            self.emit_message(&dir, &a);
        }
        for k in stale {
            self.emit(&dir, EngineEvent::PermissionReplied { session_id: zsid.into(), request_id: k, reply: PermissionReply::Reject });
        }
        if let Some(e) = err_event {
            self.emit(&dir, EngineEvent::SessionError { session_id: Some(zsid.into()), error: e });
        }
        self.emit(&dir, EngineEvent::SessionStatus { session_id: zsid.into(), status: SessionRunStatus::Idle });
        self.emit(&dir, EngineEvent::SessionUpdated { session_id: zsid.into(), revert: None, title: None, updated: Some(t) });
    }

    /// Hentikan giliran ACP yang sedang berjalan. false = tidak ada giliran ACP (pemanggil meneruskan ke engine).
    pub fn cancel(&self, zsid: &str) -> bool {
        let (conn, sid, perms) = {
            let mut st = lock(&self.0.st);
            let Some(s) = st.sessions.get_mut(zsid) else { return false };
            let Some(turn) = s.turn.as_mut() else { return false };
            turn.cancelled = true;
            let (agent, sid, epoch) = (s.agent.clone(), s.acp_sid.clone(), s.live_gen);
            let conn = st.agents.values().find(|a| Some(a.agent) == agent.as_deref() && Some(a.epoch) == epoch).map(|a| a.conn.clone());
            let ids: Vec<String> = st.perms.iter().filter(|(_, p)| p.zsid == zsid).map(|(k, _)| k.clone()).collect();
            let perms: Vec<Perm> = ids.iter().filter_map(|k| st.perms.remove(k)).collect();
            (conn, sid, perms)
        };
        if let Some(c) = &conn {
            for p in perms {
                c.respond(p.rpc_id, Ok(json!({ "outcome": { "outcome": "cancelled" } })));
            }
            if let Some(sid) = sid {
                c.notify("session/cancel", json!({ "sessionId": sid }));
            }
        }
        true
    }

    pub fn owns_permission(id: &str) -> bool {
        id.starts_with("acpperm_")
    }

    pub fn reply_permission(&self, id: &str, reply: PermissionReply) -> Result<(), String> {
        let (p, dir) = {
            let mut st = lock(&self.0.st);
            let p = st.perms.remove(id).ok_or("permintaan izin sudah tidak berlaku")?;
            let dir = st.sessions.get(&p.zsid).map(|s| s.dir.clone()).unwrap_or_default();
            (p, dir)
        };
        let conn = self.conn_of(p.agent, p.epoch).ok_or("agent sudah berhenti")?;
        let outcome = match convert::pick_option(&p.options, reply) {
            Some(opt) => json!({ "outcome": { "outcome": "selected", "optionId": opt } }),
            None => json!({ "outcome": { "outcome": "cancelled" } }),
        };
        conn.respond(p.rpc_id, Ok(outcome));
        self.emit(&dir, EngineEvent::PermissionReplied { session_id: p.zsid, request_id: id.to_string(), reply });
        Ok(())
    }

    fn on_permission(&self, def: &'static AgentDef, epoch: u64, rpc_id: Value, params: &Value) {
        let acp_sid = params.get("sessionId").and_then(Value::as_str).unwrap_or("");
        let id = format!("acpperm_{}", self.0.perm_seq.fetch_add(1, Ordering::Relaxed));
        let mut st = lock(&self.0.st);
        let Some(zsid) = st.by_acp.get(acp_sid).cloned() else {
            drop(st);
            if let Some(c) = self.conn_of(def.id, epoch) {
                c.respond(rpc_id, Ok(json!({ "outcome": { "outcome": "cancelled" } })));
            }
            return;
        };
        // Tampilkan tool yang meminta izin sebagai langkah (sama dengan tool_call).
        let tc = params.get("toolCall").cloned().unwrap_or(Value::Null);
        let mut tool_ref = None;
        let mut updated_part = None;
        let dir = {
            let s = st.sessions.get_mut(&zsid).expect("sesi terpetakan");
            if let Some(turn) = s.turn.as_mut() {
                let call = tc.get("toolCallId").and_then(Value::as_str).unwrap_or("").to_string();
                tool_ref = Some(ToolRef { message_id: turn.mid.clone(), call_id: call.clone() });
                if !call.is_empty() {
                    updated_part = Self::upsert_tool(s, &zsid, &call, &tc);
                }
            }
            s.dir.clone()
        };
        let req = convert::permission_request(&id, &zsid, params, tool_ref);
        st.perms.insert(id, Perm { agent: def.id, epoch, rpc_id, zsid, options: params.get("options").and_then(Value::as_array).cloned().unwrap_or_default(), req: req.clone() });
        drop(st);
        if let Some(p) = updated_part.and_then(|p| to_part(Some(&p))) {
            self.emit(&dir, EngineEvent::PartUpdated(p));
        }
        self.emit(&dir, EngineEvent::PermissionAsked(req));
    }

    /// Tambah/perbarui part tool dalam pesan asisten giliran aktif; hasil = part terbaru.
    fn upsert_tool(s: &mut Sess, zsid: &str, call: &str, upd: &Value) -> Option<Value> {
        let t = now();
        let turn = s.turn.as_mut()?;
        let mid = turn.mid.clone();
        let pid = match turn.tools.get(call) {
            Some(p) => p.clone(),
            None => {
                let p = ascending("prt");
                turn.tools.insert(call.to_string(), p.clone());
                turn.cur = None;
                let msg = s.messages.iter_mut().find(|m| m.pointer("/info/id").and_then(Value::as_str) == Some(mid.as_str()))?;
                msg["parts"].as_array_mut()?.push(json!({ "id": p, "messageID": mid, "sessionID": zsid, "type": "tool", "callID": call, "tool": "tool" }));
                p
            }
        };
        let msg = s.messages.iter_mut().find(|m| m.pointer("/info/id").and_then(Value::as_str) == Some(mid.as_str()))?;
        let part = msg["parts"].as_array_mut()?.iter_mut().find(|p| p.get("id").and_then(Value::as_str) == Some(pid.as_str()))?;
        convert::merge_tool(part, upd, t);
        Some(part.clone())
    }

    fn on_update(&self, acp_sid: &str, upd: &Value) {
        let kind = upd.get("sessionUpdate").and_then(Value::as_str).unwrap_or("");
        let mut st = lock(&self.0.st);
        if let Some(buf) = st.oneshot.get_mut(acp_sid) {
            if kind == "agent_message_chunk" {
                if let Some(t) = upd.pointer("/content/text").and_then(Value::as_str) {
                    buf.push_str(t);
                }
            }
            return;
        }
        if st.loading.contains(acp_sid) {
            return;
        }
        let Some(zsid) = st.by_acp.get(acp_sid).cloned() else { return };
        let Some(s) = st.sessions.get_mut(&zsid) else { return };
        let dir = s.dir.clone();
        let mut events = vec![];
        match kind {
            "plan" => {
                let entries = upd.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
                let todos = to_todos(Some(&Value::Array(entries.clone())));
                s.todos = Some(entries);
                events.push(EngineEvent::TodoUpdated { session_id: zsid.clone(), todos });
            }
            "config_option_update" => {
                if let Some(c) = upd.get("configOptions").filter(|c| c.is_array()) {
                    s.config = c.clone();
                }
            }
            "current_mode_update" => {
                if let Some(m) = upd.get("currentModeId") {
                    s.modes["currentModeId"] = m.clone();
                }
            }
            "usage_update" => {
                // ukuran jendela konteks model aktif (Claude/Codex melaporkannya)
                let model = s.messages.last().and_then(|m| m.pointer("/info/modelID")).and_then(Value::as_str).map(String::from);
                if let (Some(size), Some(agent), Some(model)) = (upd.get("size").and_then(Value::as_u64), s.agent.clone(), model) {
                    if let Some(list) = st.models.get_mut(&agent) {
                        if let Some(m) = list.iter_mut().find(|m| m.id == model) {
                            m.context = Some(size);
                        }
                    }
                }
            }
            "agent_message_chunk" | "agent_thought_chunk" => {
                let Some(text) = upd.pointer("/content/text").and_then(Value::as_str).filter(|t| !t.is_empty()) else { return };
                let pk: &'static str = if kind == "agent_message_chunk" { "text" } else { "reasoning" };
                let Some(turn) = s.turn.as_mut() else { return };
                let mid = turn.mid.clone();
                let cur = turn.cur.clone();
                let Some(msg) = s.messages.iter_mut().find(|m| m.pointer("/info/id").and_then(Value::as_str) == Some(mid.as_str())) else { return };
                let Some(parts) = msg["parts"].as_array_mut() else { return };
                match cur.filter(|(_, k)| *k == pk) {
                    Some((pid, _)) => {
                        if let Some(p) = parts.iter_mut().find(|p| p.get("id").and_then(Value::as_str) == Some(pid.as_str())) {
                            let prev = p.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                            p["text"] = json!(format!("{prev}{text}"));
                            events.push(EngineEvent::PartDelta { session_id: zsid.clone(), message_id: mid.clone(), part_id: pid, field: "text".into(), delta: text.to_string() });
                        }
                    }
                    None => {
                        let pid = ascending("prt");
                        let p = convert::text_part(&pid, &mid, &zsid, pk, text, false);
                        events.extend(to_part(Some(&p)).map(EngineEvent::PartUpdated));
                        parts.push(p);
                        if let Some(t) = s.turn.as_mut() {
                            t.cur = Some((pid, pk));
                        }
                    }
                }
            }
            "tool_call" | "tool_call_update" => {
                let Some(call) = upd.get("toolCallId").and_then(Value::as_str) else { return };
                if s.turn.is_none() {
                    return;
                }
                if let Some(p) = Self::upsert_tool(s, &zsid, call, upd).and_then(|p| to_part(Some(&p))) {
                    events.push(EngineEvent::PartUpdated(p));
                }
            }
            _ => {}
        }
        drop(st);
        for e in events {
            self.emit(&dir, e);
        }
    }

    /// Satu jawaban teks sinkron dari agent (judul otomatis, ringkasan langkah berikutnya) di folder sementara.
    pub fn complete_once(&self, provider_id: &str, model_id: &str, prompt: &str) -> Result<String, String> {
        let def = agents::by_provider(provider_id).ok_or("provider ACP tidak dikenal")?;
        let (conn, caps, _) = self.ensure_conn(def, &self.scratch_dir())?;
        let res = conn.request("session/new", json!({ "cwd": self.scratch_dir(), "mcpServers": [] }), Some(Duration::from_secs(120))).map_err(|e| auth_hint(def, &e))?;
        let sid = res.get("sessionId").and_then(Value::as_str).ok_or("agent tidak memberi sessionId")?.to_string();
        let config = res.get("configOptions").cloned().unwrap_or(Value::Null);
        if let Some(opt) = config_by_category(&config, "model") {
            if config_values(opt).iter().any(|(v, _)| v == model_id) && opt.get("currentValue").and_then(Value::as_str) != Some(model_id) {
                let _ = conn.request("session/set_config_option", json!({ "sessionId": sid, "configId": opt.get("id"), "value": model_id }), Some(Duration::from_secs(30)));
            }
        }
        lock(&self.0.st).oneshot.insert(sid.clone(), String::new());
        let r = conn.request("session/prompt", json!({ "sessionId": sid, "prompt": [{ "type": "text", "text": prompt }] }), Some(Duration::from_secs(180)));
        if r.is_err() {
            conn.notify("session/cancel", json!({ "sessionId": sid }));
        }
        let text = lock(&self.0.st).oneshot.remove(&sid).unwrap_or_default();
        self.close_session(&conn, &caps, &sid);
        r.map_err(|e| auth_hint(def, &e))?;
        Ok(text)
    }
}

/// Uji tanpa proses agent: konversi model, mode, dan penyimpanan transkrip.
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("zandor-acp-{name}-{}-{}", std::process::id(), now() as u64));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn model_dari_config_options_dan_cadangan_models() {
        let res = json!({"configOptions":[
            {"id":"model","category":"model","type":"select","currentValue":"default","options":[{"value":"default","name":"Default"},{"value":"opus","name":"Opus 5.5"}]},
            {"id":"effort","category":"thought_level","type":"select","currentValue":"default","options":[{"value":"default","name":"Default"},{"value":"high","name":"High"}]}]});
        let m = extract_models(&res);
        assert_eq!(m.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["default", "opus"]);
        assert_eq!(m[1].name, "Opus 5.5");
        assert_eq!(m[1].variants, vec!["high"]);
        let grouped = json!({"configOptions":[{"id":"m","category":"model","options":[{"group":"g","name":"G","options":[{"value":"a","name":"A"}]}]}]});
        assert_eq!(extract_models(&grouped)[0].id, "a");
        let legacy = json!({"models":{"availableModels":[{"modelId":"gemini-x","name":"Gemini X"}],"currentModelId":"gemini-x"}});
        assert_eq!(extract_models(&legacy)[0].name, "Gemini X");
        assert!(extract_models(&json!({})).is_empty());
    }

    #[test]
    fn mode_izin_dipetakan_per_agent() {
        let claude: Vec<String> = ["default", "acceptEdits", "plan", "auto", "bypassPermissions"].map(String::from).to_vec();
        let codex: Vec<String> = ["read-only", "auto", "full-access"].map(String::from).to_vec();
        let agy: Vec<String> = ["default", "accept-edits", "plan"].map(String::from).to_vec();
        assert_eq!(mode_for("ask", &claude).as_deref(), Some("default"));
        assert_eq!(mode_for("ask", &codex).as_deref(), Some("read-only"));
        assert_eq!(mode_for("auto-edit", &claude).as_deref(), Some("acceptEdits"));
        assert_eq!(mode_for("auto-edit", &agy).as_deref(), Some("accept-edits"));
        assert_eq!(mode_for("auto-all", &codex).as_deref(), Some("full-access"));
        assert_eq!(mode_for("auto-all", &agy).as_deref(), Some("accept-edits"));
        assert_eq!(mode_for("ask", &[]), None);
    }

    #[test]
    fn giliran_disimpan_digabung_dan_dipotong() {
        let h = AcpHost::new(tmp("sess"));
        let events = Arc::new(Mutex::new(vec![]));
        let ev2 = events.clone();
        h.subscribe("C:\\proj", Arc::new(AtomicBool::new(false)), Arc::new(move |e| ev2.lock().unwrap().push(e.name())));
        // Simulasikan giliran tanpa proses: siapkan sesi + turn seperti prompt() lalu alirkan update.
        {
            let mut st = lock(&h.0.st);
            let s = h.load_sess(&mut st, "ses_1", Some("C:\\proj"));
            let mid = ascending("msg");
            s.messages.push(json!({"info": convert::message_info(&mid, "ses_1", Role::Assistant, 10., None, Some("acp-claude"), Some("opus")), "parts": []}));
            s.turn = Some(Turn { mid, cur: None, tools: HashMap::new(), cancelled: false });
            s.agent = Some("claude".into());
            s.acp_sid = Some("acp-1".into());
            st.by_acp.insert("acp-1".into(), "ses_1".into());
        }
        h.on_update("acp-1", &json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"pikir"}}));
        h.on_update("acp-1", &json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"ha"}}));
        h.on_update("acp-1", &json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"lo"}}));
        h.on_update("acp-1", &json!({"sessionUpdate":"tool_call","toolCallId":"t1","kind":"execute","title":"ls","status":"in_progress","rawInput":{"command":"ls"}}));
        h.on_update("acp-1", &json!({"sessionUpdate":"tool_call_update","toolCallId":"t1","status":"completed"}));
        h.on_update("acp-1", &json!({"sessionUpdate":"plan","entries":[{"content":"A","priority":"high","status":"in_progress"}]}));
        h.on_update("tidak-dikenal", &json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"x"}}));
        h.finish("ses_1", Ok(json!({"stopReason":"end_turn","usage":{"inputTokens":5,"outputTokens":2}})));

        let names = events.lock().unwrap().clone();
        assert!(names.contains(&"PartDelta") && names.contains(&"TodoUpdated") && names.contains(&"SessionStatus"), "{names:?}");
        // muat ulang dari disk lewat host baru
        let h2 = AcpHost::new(h.0.root.clone());
        let msgs = h2.messages("ses_1");
        assert_eq!(msgs.len(), 1);
        let m = &msgs[0];
        assert_eq!(m.info.finish.as_deref(), Some("stop"));
        assert_eq!(m.info.tokens.as_ref().unwrap().input, 5.);
        let kinds: Vec<&str> = m.parts.iter().map(|p| match &p.kind {
            PartKind::Reasoning { .. } => "reasoning",
            PartKind::Text { text, .. } if text == "halo" => "text",
            PartKind::Tool { tool, .. } if tool == "bash" => "bash",
            _ => "?",
        }).collect();
        assert_eq!(kinds, ["reasoning", "text", "bash"]);
        assert_eq!(h2.todos("ses_1").unwrap().len(), 1);
        assert!(!h2.busy("ses_1"));
        assert!(h2.owns_message("ses_1", &m.info.id));
        // handoff ke OpenCode hanya sekali
        assert!(h2.opencode_handoff("ses_1").unwrap().contains("halo"));
        assert!(h2.opencode_handoff("ses_1").is_none());
        // potong: pesan hilang, sesi ACP dilupakan
        assert_eq!(h2.truncate("ses_1", Some(&m.info.id), None), 1);
        assert!(h2.messages("ses_1").is_empty());
        assert!(lock(&h2.0.st).sessions["ses_1"].acp_sid.is_none());
        h2.forget("ses_1");
        assert!(!h2.sess_path("ses_1").exists());
        let _ = std::fs::remove_dir_all(&h.0.root);
    }

    #[test]
    fn sesi_tanpa_transkrip_tidak_membuat_berkas() {
        let h = AcpHost::new(tmp("none"));
        assert!(h.messages("ses_x").is_empty());
        assert!(h.opencode_handoff("ses_x").is_none());
        assert!(!h.owns_message("ses_x", "msg_1"));
        assert!(!h.sess_path("ses_x").exists());
        assert!(!h.cancel("ses_x"));
    }

    #[test]
    fn prompt_ditolak_bila_agent_mati() {
        let h = AcpHost::new(tmp("off"));
        let e = h.prompt("C:\\p", "ses_1", "hai", "acp-codex", "gpt", None, &[], &[], vec![]).unwrap_err();
        assert!(e.contains("belum diaktifkan"), "{e}");
    }

    /// Uji langsung dengan agent sungguhan (memakai kuota langganan; tidak jalan di CI):
    /// `ZANDOR_ACP_LIVE=claude ZANDOR_ACP_LIVE_CMD=node ZANDOR_ACP_LIVE_ARGS="C:/…/dist/index.js" cargo test --offline live_acp -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_acp() {
        let agent = std::env::var("ZANDOR_ACP_LIVE").expect("set ZANDOR_ACP_LIVE");
        let def = agents::by_id(&agent).expect("agent dikenal");
        let root = tmp("live");
        let h = AcpHost::new(root.clone());
        {
            let mut st = lock(&h.0.st);
            let args = std::env::var("ZANDOR_ACP_LIVE_ARGS").map(|a| a.split('|').map(String::from).collect()).unwrap_or_default();
            st.config.agents.insert(agent.clone(), agents::AgentConfig { enabled: true, command: std::env::var("ZANDOR_ACP_LIVE_CMD").ok(), args });
            st.permission_mode = "auto-all".into();
        }
        let n = h.refresh_models(&agent).expect("model");
        println!("{n} model");
        let model = std::env::var("ZANDOR_ACP_LIVE_MODEL").unwrap_or_else(|_| h.models()[0].model_id.clone());
        let work = root.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let dir = work.display().to_string();
        let (tx, rx) = std::sync::mpsc::channel::<EngineEvent>();
        let tx = Mutex::new(tx);
        h.subscribe(&dir, Arc::new(AtomicBool::new(false)), Arc::new(move |e| {
            let _ = tx.lock().unwrap().send(e);
        }));
        let h2 = h.clone();
        let run = |text: &str| {
            h.prompt(&dir, "ses_live", text, def.provider_id, &model, None, &[], &[], vec![]).unwrap();
            loop {
                match rx.recv_timeout(Duration::from_secs(300)).expect("event") {
                    // Izin dijawab seperti ChatStore pada mode auto-all.
                    EngineEvent::PermissionAsked(r) => {
                        println!("izin: {} {:?}", r.permission, r.patterns);
                        h2.reply_permission(&r.id, PermissionReply::Once).unwrap();
                    }
                    EngineEvent::PartUpdated(p) => {
                        if let PartKind::Tool { tool, state, .. } = &p.kind {
                            println!("tool {tool} {:?} {:?}", state.status, state.title);
                        }
                    }
                    EngineEvent::SessionError { error, .. } => println!("galat: {}", error.message),
                    EngineEvent::SessionStatus { status: SessionRunStatus::Idle, .. } => break,
                    _ => {}
                }
            }
        };
        run("Buat berkas halo.txt berisi kata halo di folder kerja saat ini, lalu balas singkat: selesai.");
        let msgs = h.messages("ses_live");
        let last = msgs.last().unwrap();
        println!("finish={:?} error={:?} tokens={:?}", last.info.finish, last.info.error, last.info.tokens);
        assert!(last.info.error.is_none(), "{:?}", last.info.error);
        assert!(work.join("halo.txt").is_file(), "berkas dibuat agent");
        // giliran kedua memakai sesi ACP yang sama (konteks tersambung)
        run("Apa nama berkas yang baru saja kamu buat? Jawab nama berkasnya saja.");
        let msgs = h.messages("ses_live");
        let answer = crate::core::chat::message_text(msgs.last().unwrap());
        println!("jawaban: {answer}");
        assert!(answer.to_lowercase().contains("halo"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn blok_prompt_dengan_konteks_dan_lampiran() {
        let files = vec![
            FileAttachment { mime: "image/png".into(), url: "data:image/png;base64,QUJD".into(), filename: Some("a.png".into()) },
            FileAttachment { mime: "text/plain".into(), url: "file:///C:/w/b.txt".into(), filename: Some("b.txt".into()) },
        ];
        let b = AcpHost::prompt_blocks("tanya", &["catatan".into()], &files, Some("riwayat".into()), true);
        assert_eq!(b[0]["text"], "riwayat");
        assert_eq!(b[1]["text"], "tanya");
        assert_eq!(b[2]["text"], "catatan");
        assert_eq!(b[3]["type"], "image");
        assert_eq!(b[3]["data"], "QUJD");
        assert_eq!(b[4]["type"], "resource_link");
        assert!(b[5]["text"].as_str().unwrap().contains("b.txt"));
        // tanpa dukungan gambar: gambar data-URL jadi catatan
        let b = AcpHost::prompt_blocks("x", &[], &files[..1], None, false);
        assert!(b.last().unwrap()["text"].as_str().unwrap().contains("a.png"));
    }
}

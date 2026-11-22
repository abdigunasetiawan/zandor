//! Project & sesi — port packages/core/src/engine/projects.ts (ProjectStore).
//! PROJECT = folder (registry JSON milik Zandor). SESI milik engine, terikat direktori. Sesi tanpa project ("Chat")
//! memakai folder `chats/<chat-…>/` per sesi. Hapus project = lepas dari daftar; sesi di engine tidak dihapus.
//! Engine remote: `remotes` + rute per project (EngineApi::set_routes) — chat dan terminal project remote ke engine-nya.
#![allow(dead_code)]
use super::engine::{is_default_session_title, EngineApi, SessionInfo};
use gpui::{App, AppContext, Context, Entity, EventEmitter, Global, Task};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

pub const CHAT_KEY: &str = "chat";
const MAX_TABS: usize = 12;
const REGISTRY_VERSION: u64 = 1;
pub const PROJECT_COLORS: [&str; 9] = ["slate", "red", "orange", "amber", "green", "teal", "blue", "violet", "pink"];

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub id: String,
    pub path: String,
    pub name: String,
    pub color: Option<String>,
    pub added_at: f64,
    pub engine: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadStatus {
    Idle,
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Debug)]
pub struct SessionsState {
    pub status: LoadStatus,
    pub items: Vec<SessionInfo>,
    pub error: Option<String>,
}
impl Default for SessionsState {
    fn default() -> Self {
        Self { status: LoadStatus::Idle, items: vec![], error: None }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub key: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionTab {
    pub key: String,
    pub session_id: String,
}

fn is_windows_path(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// Rapikan path: pemisah konsisten, tanpa pemisah di ujung (kecuali akar drive).
pub fn normalize_path(path: &str) -> String {
    let p = path.trim();
    if is_windows_path(p) {
        let s = p.replace('/', "\\");
        let s = s.trim_end_matches('\\').to_string();
        if s.len() == 2 { format!("{s}\\") } else { s }
    } else if p.len() > 1 {
        p.trim_end_matches('/').to_string()
    } else {
        p.to_string()
    }
}

/// Kunci identitas: Windows tidak peka huruf besar/kecil.
pub fn path_key(path: &str) -> String {
    let n = normalize_path(path);
    if is_windows_path(&n) { n.to_lowercase() } else { n }
}

pub fn base_name(path: &str) -> String {
    let n = normalize_path(path);
    let last = n.split(['\\', '/']).filter(|s| !s.is_empty()).last().unwrap_or(&n).to_string();
    if last.len() == 2 && last.ends_with(':') { format!("{last}\\") } else { last }
}

/// Judul tampil: judul bawaan engine dianggap belum bernama → None.
pub fn session_title(s: &SessionInfo) -> Option<&str> {
    if is_default_session_title(&s.title) { None } else { Some(&s.title) }
}

/// Nama folder sesi Chat: `chat-YYYYMMDD-HHMMSS-xxxx`.
pub fn chat_folder_name(now_ms: f64, suffix: &str) -> String {
    let secs = (now_ms / 1000.) as i64;
    let (y, mo, d, h, mi, s) = civil_local(secs);
    let suf: String = suffix.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).take(6).collect();
    format!("chat-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}-{}", if suf.is_empty() { "x".into() } else { suf })
}

/// Detik epoch → tanggal lokal (zona waktu OS via offset UTC proses).
fn civil_local(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let secs = secs + local_offset_secs();
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // algoritma Howard Hinnant
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d, (rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32)
}

#[cfg(windows)]
fn local_offset_secs() -> i64 {
    #[repr(C)]
    struct Tzi {
        bias: i32,
        rest: [u8; 168],
    }
    unsafe extern "system" {
        fn GetTimeZoneInformation(tz: *mut Tzi) -> u32;
    }
    let mut tz = Tzi { bias: 0, rest: [0; 168] };
    // SAFETY: struktur berukuran TIME_ZONE_INFORMATION (172 byte); hanya `Bias` yang dibaca.
    let r = unsafe { GetTimeZoneInformation(&mut tz) };
    let _ = r;
    -(tz.bias as i64) * 60
}
#[cfg(not(windows))]
fn local_offset_secs() -> i64 {
    0
}

pub fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.)
}

pub fn new_id() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).ok();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Engine opencode remote (port remote.ts). Kata sandi disimpan di registry (tidak terenkripsi), sama dengan TS.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteEngine {
    pub id: String,
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
}

impl RemoteEngine {
    pub fn info(&self) -> super::engine::ConnectionInfo {
        super::engine::ConnectionInfo { url: self.url.clone(), username: self.username.clone(), password: self.password.clone() }
    }
}

/// Status cek health engine remote.
#[derive(Clone, Debug, PartialEq)]
pub enum RemoteStatus {
    Checking,
    Ready(String),
    Error(String),
}

/// Port remoteUrlProblem: None = boleh; "invalid" / "insecure" (HTTP hanya untuk loopback).
pub fn remote_url_problem(url: &str) -> Option<&'static str> {
    let u = url.trim();
    let Some((scheme, rest)) = u.split_once("://") else { return Some("invalid") };
    let scheme = scheme.to_ascii_lowercase();
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    if auth.is_empty() || auth.contains('@') || rest.contains('?') || rest.contains('#') || u.chars().any(char::is_whitespace) {
        return Some("invalid");
    }
    let host = if auth.starts_with('[') { auth.split(']').next().map(|h| format!("{h}]")).unwrap_or_default() } else { auth.split(':').next().unwrap_or("").to_string() };
    let host = host.to_ascii_lowercase();
    match scheme.as_str() {
        "https" => None,
        "http" => (!["localhost", "127.0.0.1", "[::1]"].contains(&host.as_str())).then_some("insecure"),
        _ => Some("invalid"),
    }
    .or(if host.is_empty() { Some("invalid") } else { None })
}

pub fn normalize_remote_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn parse_remotes(v: &Value) -> Vec<RemoteEngine> {
    let mut out: Vec<RemoteEngine> = vec![];
    for r in v.as_array().into_iter().flatten() {
        let (Some(id), Some(url)) = (r["id"].as_str(), r["url"].as_str()) else { continue };
        if out.iter().any(|x| x.id == id) {
            continue;
        }
        out.push(RemoteEngine {
            id: id.into(),
            name: r["name"].as_str().filter(|s| !s.trim().is_empty()).unwrap_or(url).into(),
            url: normalize_remote_url(url),
            username: r["username"].as_str().unwrap_or("opencode").into(),
            password: r["password"].as_str().unwrap_or("").into(),
        });
    }
    out
}

/// Penyimpanan registry + folder Chat (pengganti host Tauri).
#[derive(Clone)]
pub struct ProjectHost {
    pub registry: PathBuf,
    pub chats: PathBuf,
}

pub struct ProjectStore {
    pub loaded: bool,
    pub projects: Vec<Project>,
    pub chat_dir: Option<String>,
    pub engine_ready: bool,
    pub sessions: HashMap<String, SessionsState>,
    pub selection: Selection,
    pub tabs: Vec<SessionTab>,
    pub remotes: Vec<RemoteEngine>,
    pub remote_status: HashMap<String, RemoteStatus>,
    api: Option<EngineApi>,
    host: ProjectHost,
    request_seq: HashMap<String, u64>,
    pub last_error: Option<String>,
    /// Penamaan ulang otomatis (lihat `autotitle`): judul terakhir yang berasal dari engine/Zandor per sesi.
    auto_titles: HashMap<String, String>,
    /// Sesi yang pernah diganti namanya pengguna — tidak pernah diubah otomatis.
    user_titled: std::collections::HashSet<String>,
    /// Sesi yang judulnya sudah dinilai cukup (model menjawab TETAP atau judul baru sudah dipasang).
    title_settled: std::collections::HashSet<String>,
    /// Penilaian judul yang sedang berjalan (satu per sesi).
    retitling: std::collections::HashSet<String>,
}

pub enum ProjectEvent {
    Changed,
    Error(String),
}
impl EventEmitter<ProjectEvent> for ProjectStore {}

struct ProjectStoreHandle(Entity<ProjectStore>);
impl Global for ProjectStoreHandle {}

fn by_recent(a: &SessionInfo, b: &SessionInfo) -> std::cmp::Ordering {
    b.updated.partial_cmp(&a.updated).unwrap_or(std::cmp::Ordering::Equal).then(b.created.partial_cmp(&a.created).unwrap_or(std::cmp::Ordering::Equal))
}

/// Hasil parse registry: (projects, selection, tabs, remotes, galat).
type Parsed = (Vec<Project>, Selection, Vec<SessionTab>, Value, Option<String>);

pub fn parse_registry(text: Option<&str>) -> Parsed {
    let empty = (vec![], Selection::default(), vec![], Value::Array(vec![]), None);
    let Some(text) = text else { return empty };
    let Ok(raw) = serde_json::from_str::<Value>(text) else {
        return (vec![], Selection::default(), vec![], Value::Array(vec![]), Some("registry project rusak (bukan JSON valid)".into()));
    };
    if raw.get("version").and_then(Value::as_u64) != Some(REGISTRY_VERSION) || !raw.get("projects").is_some_and(Value::is_array) {
        return (vec![], Selection::default(), vec![], Value::Array(vec![]), Some("registry project berformat tidak dikenal".into()));
    }
    let mut projects = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for p in raw["projects"].as_array().into_iter().flatten() {
        let (Some(id), Some(path), Some(name)) = (p["id"].as_str(), p["path"].as_str(), p["name"].as_str()) else { continue };
        let k = path_key(path);
        if seen.contains(&k) || seen.contains(&format!("id:{id}")) {
            continue;
        }
        seen.insert(k);
        seen.insert(format!("id:{id}"));
        projects.push(Project {
            id: id.into(),
            path: normalize_path(path),
            name: name.into(),
            color: p["color"].as_str().filter(|c| PROJECT_COLORS.contains(c)).map(String::from),
            added_at: p["addedAt"].as_f64().unwrap_or(0.),
            engine: p["engine"].as_str().filter(|s| !s.is_empty()).map(String::from),
        });
    }
    let sel = Selection {
        key: raw["selection"]["key"].as_str().map(String::from),
        session_id: raw["selection"]["sessionId"].as_str().map(String::from),
    };
    let mut tabs: Vec<SessionTab> = Vec::new();
    for t in raw["tabs"].as_array().into_iter().flatten() {
        if let (Some(k), Some(s)) = (t["key"].as_str(), t["sessionId"].as_str()) {
            if !tabs.iter().any(|x| x.session_id == s) {
                tabs.push(SessionTab { key: k.into(), session_id: s.into() });
            }
        }
    }
    let skip = tabs.len().saturating_sub(MAX_TABS);
    let remotes = raw.get("remotes").cloned().filter(Value::is_array).unwrap_or(Value::Array(vec![]));
    (projects, sel, tabs.into_iter().skip(skip).collect(), remotes, None)
}

impl ProjectStore {
    pub fn init(host: ProjectHost, cx: &mut App) -> Entity<ProjectStore> {
        let text = crate::registry::load(&host.registry).ok().flatten();
        let (projects, selection, tabs, remotes, err) = parse_registry(text.as_deref());
        let _ = std::fs::create_dir_all(&host.chats);
        let chat_dir = Some(normalize_path(&host.chats.to_string_lossy()));
        let e = cx.new(|_| ProjectStore {
            loaded: true,
            projects,
            chat_dir,
            engine_ready: false,
            sessions: HashMap::new(),
            selection,
            tabs,
            remotes: parse_remotes(&remotes),
            remote_status: HashMap::new(),
            api: None,
            host,
            request_seq: HashMap::new(),
            auto_titles: HashMap::new(),
            user_titled: Default::default(),
            title_settled: Default::default(),
            retitling: Default::default(),
            last_error: err,
        });
        cx.set_global(ProjectStoreHandle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Entity<ProjectStore> {
        cx.global::<ProjectStoreHandle>().0.clone()
    }
    pub fn api(&self) -> Option<EngineApi> {
        self.api.clone()
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(ProjectEvent::Changed);
        cx.notify();
    }
    fn fail(&mut self, msg: String, cx: &mut Context<Self>) {
        cx.emit(ProjectEvent::Error(msg));
    }

    fn persist(&self) {
        let projects: Vec<Value> = self
            .projects
            .iter()
            .map(|p| {
                let mut o = json!({ "id": p.id, "path": p.path, "name": p.name, "color": p.color, "addedAt": p.added_at });
                if let Some(e) = &p.engine {
                    o["engine"] = json!(e);
                }
                o
            })
            .collect();
        let mut data = json!({
            "version": REGISTRY_VERSION,
            "projects": projects,
            "selection": { "key": self.selection.key, "sessionId": self.selection.session_id },
            "tabs": self.tabs.iter().map(|t| json!({ "key": t.key, "sessionId": t.session_id })).collect::<Vec<_>>(),
        });
        if !self.remotes.is_empty() {
            data["remotes"] = Value::Array(
                self.remotes.iter().map(|r| json!({ "id": r.id, "name": r.name, "url": r.url, "username": r.username, "password": r.password })).collect(),
            );
        }
        if let Err(e) = crate::registry::save(&self.host.registry, &data.to_string()) {
            eprintln!("[zandor] {e}");
        }
    }

    /// Pasang/lepas engine lokal; saat terpasang semua daftar sesi dimuat ulang.
    pub fn set_api(&mut self, api: Option<EngineApi>, cx: &mut Context<Self>) {
        self.engine_ready = api.is_some();
        self.api = api;
        self.sync_routes();
        self.check_all_remotes(cx);
        self.changed(cx);
        if self.api.is_some() {
            self.refresh_all(cx);
        }
    }

    fn directory_of(&self, key: &str) -> Option<String> {
        if key == CHAT_KEY {
            return self.chat_dir.clone();
        }
        self.projects.iter().find(|p| p.id == key).map(|p| p.path.clone())
    }

    fn dir_for_session(&self, key: &str, session_id: &str) -> Option<String> {
        if key == CHAT_KEY {
            let s = self.sessions.get(key).and_then(|g| g.items.iter().find(|x| x.id == session_id));
            return s.map(|s| s.directory.clone()).filter(|d| !d.is_empty()).or_else(|| self.chat_dir.clone());
        }
        self.directory_of(key)
    }

    /// Terapkan metadata sesi dari event `session.updated` (judul otomatis engine, waktu update) ke daftar sidebar.
    /// Sesi yang tidak ada di daftar diabaikan (daftar dimuat dari engine, bukan dibuat dari event).
    pub fn apply_session_update(&mut self, session_id: &str, title: Option<&str>, updated: Option<f64>, cx: &mut Context<Self>) {
        let mut touched = false;
        if let Some(t) = title.filter(|t| !is_default_session_title(t) && !self.user_titled.contains(session_id)) {
            self.auto_titles.insert(session_id.to_string(), t.to_string());
        }
        for g in self.sessions.values_mut() {
            let Some(it) = g.items.iter_mut().find(|x| x.id == session_id) else { continue };
            if let Some(t) = title.filter(|t| *t != it.title) {
                it.title = t.to_string();
                touched = true;
            }
            if let Some(u) = updated.filter(|u| *u > it.updated) {
                it.updated = u;
                touched = true;
            }
            if touched {
                g.items.sort_by(by_recent);
            }
            break;
        }
        if touched {
            self.changed(cx);
        }
    }

    /// Mulai satu penilaian judul untuk sesi bila layak: judul saat ini berasal dari engine/Zandor (bukan bawaan dan
    /// belum diganti pengguna), belum dinilai cukup, dan belum ada penilaian berjalan. Mengembalikan judul saat ini
    /// dan engine API; `None` bila tidak layak. Pasangannya: [`Self::finish_retitle`].
    pub fn begin_retitle(&mut self, session_id: &str) -> Option<(String, EngineApi)> {
        let api = self.api.clone()?;
        let cur = self.sessions.values().flat_map(|g| g.items.iter()).find(|x| x.id == session_id)?.title.clone();
        let eligible = !is_default_session_title(&cur)
            && self.auto_titles.get(session_id) == Some(&cur)
            && !self.user_titled.contains(session_id)
            && !self.title_settled.contains(session_id);
        (eligible && self.retitling.insert(session_id.to_string())).then_some((cur, api))
    }

    /// Akhiri penilaian judul. `new_title` = judul baru yang sudah dipasang di engine (ikut ditandai cukup);
    /// `settled` = berhenti menilai ulang (model menjawab TETAP, atau judul baru terpasang).
    pub fn finish_retitle(&mut self, session_id: &str, new_title: Option<String>, settled: bool, cx: &mut Context<Self>) {
        self.retitling.remove(session_id);
        if settled {
            self.title_settled.insert(session_id.to_string());
        }
        if let Some(t) = new_title {
            self.auto_titles.insert(session_id.to_string(), t.clone());
            self.apply_session_update(session_id, Some(&t), None, cx);
        }
    }

    pub fn refresh_all(&mut self, cx: &mut Context<Self>) {
        let keys: Vec<String> = std::iter::once(CHAT_KEY.to_string()).chain(self.projects.iter().map(|p| p.id.clone())).collect();
        for k in keys {
            self.refresh(&k, cx);
        }
    }

    /// Muat sesi satu kelompok; respons basi dibuang.
    pub fn refresh(&mut self, key: &str, cx: &mut Context<Self>) {
        let (Some(dir), Some(api)) = (self.directory_of(key), self.api.clone()) else { return };
        let seq = self.request_seq.get(key).copied().unwrap_or(0) + 1;
        self.request_seq.insert(key.into(), seq);
        let entry = self.sessions.entry(key.into()).or_default();
        entry.status = LoadStatus::Loading;
        entry.error = None;
        self.changed(cx);
        let key = key.to_string();
        let chats = self.host.chats.clone();
        let is_chat = key == CHAT_KEY;
        let task = cx.background_spawn(async move {
            if !is_chat {
                return api.list_sessions(&dir).map_err(|e| e.0);
            }
            // Chat: folder Chat itu sendiri + semua folder sesi di bawahnya; folder gagal diabaikan selama ada yang berhasil.
            let mut dirs = vec![dir.clone()];
            for p in crate::files::chat_folder_list(&chats) {
                let s = p.to_string_lossy().into_owned();
                if !dirs.contains(&s) {
                    dirs.push(s);
                }
            }
            let mut out: Vec<SessionInfo> = Vec::new();
            let mut first_err = None;
            let mut ok = 0;
            for d in dirs {
                match api.list_sessions(&d) {
                    Ok(list) => {
                        ok += 1;
                        for s in list {
                            if !out.iter().any(|x| x.id == s.id) {
                                out.push(s);
                            }
                        }
                    }
                    Err(e) => {
                        first_err.get_or_insert(e.0);
                    }
                }
            }
            if ok == 0 {
                if let Some(e) = first_err {
                    return Err(e);
                }
            }
            Ok(out)
        });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            this.update(cx, |s, cx| {
                if s.request_seq.get(&key) != Some(&seq) {
                    return;
                }
                let entry = s.sessions.entry(key.clone()).or_default();
                match res {
                    Ok(mut items) => {
                        items.sort_by(by_recent);
                        entry.status = LoadStatus::Ready;
                        entry.items = items;
                        entry.error = None;
                        s.drop_stale_selection();
                    }
                    Err(e) => {
                        entry.status = LoadStatus::Error;
                        entry.error = Some(e);
                    }
                }
                s.changed(cx);
            })
            .ok();
        })
        .detach();
    }

    fn drop_stale_selection(&mut self) {
        let before = self.tabs.len();
        let sessions = &self.sessions;
        self.tabs.retain(|t| {
            let g = sessions.get(&t.key);
            !(g.is_some_and(|g| g.status == LoadStatus::Ready && !g.items.iter().any(|i| i.id == t.session_id)))
        });
        let mut dirty = before != self.tabs.len();
        if let (Some(key), Some(sid)) = (self.selection.key.clone(), self.selection.session_id.clone()) {
            if let Some(g) = self.sessions.get(&key) {
                if g.status == LoadStatus::Ready && !g.items.iter().any(|i| i.id == sid) {
                    self.selection = Selection { key: Some(key), session_id: None };
                    dirty = true;
                }
            }
        }
        if dirty {
            self.persist();
        }
    }

    /// Tambah folder sebagai project; folder yang sudah ada dikembalikan & dipilih.
    /// Perbarui rute klien gabungan: project dengan engine remote → engine itu.
    fn sync_routes(&self) {
        let Some(api) = &self.api else { return };
        let list = self
            .projects
            .iter()
            .filter_map(|p| {
                let r = self.remotes.iter().find(|r| Some(&r.id) == p.engine.as_ref())?;
                Some((path_key(&p.path), r.info()))
            })
            .collect();
        api.set_routes(list);
    }

    pub fn remote_by_id(&self, id: &str) -> Option<&RemoteEngine> {
        self.remotes.iter().find(|r| r.id == id)
    }

    pub fn projects_using(&self, id: &str) -> usize {
        self.projects.iter().filter(|p| p.engine.as_deref() == Some(id)).count()
    }

    fn validate_remote(name: &str, url: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("nama engine kosong".into());
        }
        match remote_url_problem(url) {
            Some("insecure") => Err("insecure".into()),
            Some(_) => Err("invalid".into()),
            None => Ok(()),
        }
    }

    /// Tambah (id None) atau ubah engine remote. Galat "invalid"/"insecure" = masalah URL.
    pub fn save_remote(&mut self, id: Option<String>, name: &str, url: &str, username: &str, password: &str, cx: &mut Context<Self>) -> Result<String, String> {
        Self::validate_remote(name, url)?;
        let r = RemoteEngine {
            id: id.clone().unwrap_or_else(new_id),
            name: name.trim().chars().take(60).collect(),
            url: normalize_remote_url(url),
            username: if username.trim().is_empty() { "opencode".into() } else { username.trim().into() },
            password: password.into(),
        };
        let rid = r.id.clone();
        match self.remotes.iter().position(|x| Some(&x.id) == id.as_ref()) {
            Some(i) => self.remotes[i] = r,
            None => self.remotes.push(r),
        }
        self.persist();
        self.sync_routes();
        self.check_remote(&rid, cx);
        self.changed(cx);
        for p in self.projects.iter().filter(|p| p.engine.as_deref() == Some(&rid)).map(|p| p.id.clone()).collect::<Vec<_>>() {
            self.refresh(&p, cx);
        }
        Ok(rid)
    }

    /// Hapus engine remote; ditolak bila masih dipakai project.
    pub fn remove_remote(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let n = self.projects_using(id);
        if n > 0 {
            return Err(format!("engine masih dipakai {n} project"));
        }
        self.remotes.retain(|r| r.id != id);
        self.remote_status.remove(id);
        self.persist();
        self.sync_routes();
        self.changed(cx);
        Ok(())
    }

    /// Cek health nyata engine remote (status tampil di daftar).
    pub fn check_remote(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(r) = self.remote_by_id(id).cloned() else { return };
        self.remote_status.insert(id.into(), RemoteStatus::Checking);
        cx.notify();
        let task = cx.background_spawn(async move { EngineApi::single(r.info()).health().map_err(|e| e.0) });
        let id = id.to_string();
        cx.spawn(async move |this, cx| {
            let res = task.await;
            this.update(cx, |s, cx| {
                s.remote_status.insert(id, match res {
                    Ok(v) => RemoteStatus::Ready(v),
                    Err(e) => RemoteStatus::Error(e),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn check_all_remotes(&mut self, cx: &mut Context<Self>) {
        for id in self.remotes.iter().map(|r| r.id.clone()).collect::<Vec<_>>() {
            if !self.remote_status.contains_key(&id) {
                self.check_remote(&id, cx);
            }
        }
    }

    /// Tambah project di engine remote: path adalah folder di mesin remote (tidak dicek di disk lokal).
    pub fn add_remote_project(&mut self, path: &str, engine: &str, cx: &mut Context<Self>) -> Result<Project, String> {
        if self.remote_by_id(engine).is_none() {
            return Err("engine remote tidak dikenal".into());
        }
        let norm = normalize_path(path);
        if norm.is_empty() {
            return Err("path project kosong".into());
        }
        if self.projects.iter().any(|p| path_key(&p.path) == path_key(&norm)) {
            return Err("project dengan path ini sudah ada".into());
        }
        let project = Project { id: new_id(), name: base_name(&norm), path: norm, color: None, added_at: now_ms(), engine: Some(engine.into()) };
        self.projects.push(project.clone());
        self.selection = Selection { key: Some(project.id.clone()), session_id: None };
        self.persist();
        self.sync_routes();
        self.changed(cx);
        self.refresh(&project.id.clone(), cx);
        Ok(project)
    }

    pub fn add_project(&mut self, path: &str, cx: &mut Context<Self>) -> Result<Project, String> {
        let norm = normalize_path(path);
        if norm.is_empty() {
            return Err("path project kosong".into());
        }
        if self.chat_dir.as_deref().is_some_and(|c| path_key(c) == path_key(&norm)) {
            return Err("folder ini dipakai untuk Chat dan tidak bisa menjadi project".into());
        }
        if let Some(existing) = self.projects.iter().find(|p| path_key(&p.path) == path_key(&norm)).cloned() {
            let sid = if self.selection.key.as_deref() == Some(&existing.id) { self.selection.session_id.clone() } else { None };
            self.select(Some(existing.id.clone()), sid, cx);
            return Ok(existing);
        }
        let project = Project { id: new_id(), name: base_name(&norm), path: norm, color: None, added_at: now_ms(), engine: None };
        self.projects.push(project.clone());
        self.selection = Selection { key: Some(project.id.clone()), session_id: None };
        self.persist();
        self.changed(cx);
        self.refresh(&project.id.clone(), cx);
        Ok(project)
    }

    /// Buat folder baru di `Documents/Zandor Projects/<name>` (nama unik) lalu daftarkan sebagai project.
    /// Folder dibuat Zandor sendiri sehingga tidak perlu dialog "percayai folder".
    pub fn create_project_folder(&mut self, name: &str, cx: &mut Context<Self>) -> Result<Project, String> {
        let fallback = self.host.chats.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.host.chats.clone());
        let root = super::project_intent::projects_root(&fallback);
        let dir = super::project_intent::create_unique_dir(&root, name)?;
        self.add_project(&dir.to_string_lossy(), cx)
    }

    pub fn remove_project(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.projects.iter().any(|p| p.id == id) {
            return;
        }
        self.projects.retain(|p| p.id != id);
        self.sync_routes();
        self.sessions.remove(id);
        self.tabs.retain(|t| t.key != id);
        if self.selection.key.as_deref() == Some(id) {
            self.selection = Selection::default();
        }
        self.persist();
        self.changed(cx);
    }

    pub fn rename_project(&mut self, id: &str, name: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let n = name.trim();
        if n.is_empty() {
            return Err("nama project tidak boleh kosong".into());
        }
        let p = self.projects.iter_mut().find(|p| p.id == id).ok_or("project tidak ditemukan")?;
        p.name = n.into();
        self.persist();
        self.changed(cx);
        Ok(())
    }

    pub fn set_project_color(&mut self, id: &str, color: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(c) = color {
            if !PROJECT_COLORS.contains(&c) {
                return Err(format!("warna tidak dikenal: {c}"));
            }
        }
        if let Some(p) = self.projects.iter_mut().find(|p| p.id == id) {
            p.color = color.map(String::from);
        }
        self.persist();
        self.changed(cx);
        Ok(())
    }

    pub fn select(&mut self, key: Option<String>, session_id: Option<String>, cx: &mut Context<Self>) {
        if let (Some(k), Some(s)) = (&key, &session_id) {
            if !self.tabs.iter().any(|t| &t.session_id == s) {
                self.tabs.push(SessionTab { key: k.clone(), session_id: s.clone() });
                if self.tabs.len() > MAX_TABS {
                    let n = self.tabs.len() - MAX_TABS;
                    self.tabs.drain(0..n);
                }
            }
        }
        let prev = self.selection.clone();
        self.selection = Selection { key, session_id };
        self.persist();
        self.changed(cx);
        if let (Some(k), Some(s)) = (prev.key, prev.session_id) {
            if self.selection.session_id.as_deref() != Some(s.as_str()) {
                self.discard_if_empty(&k, &s, cx);
            }
        }
    }

    /// Sesi kosong = judul bawaan engine dan belum pernah ada aktivitas (updated ≈ created).
    fn is_empty_session(&self, key: &str, session_id: &str) -> bool {
        self.sessions
            .get(key)
            .and_then(|g| g.items.iter().find(|s| s.id == session_id))
            .is_some_and(|s| is_default_session_title(&s.title) && !self.user_titled.contains(&s.id) && s.updated - s.created < 1500.)
    }

    /// Hapus sesi yang ditinggalkan tanpa satu pun chat, agar daftar tidak penuh sampah.
    fn discard_if_empty(&mut self, key: &str, session_id: &str, cx: &mut Context<Self>) {
        if self.is_empty_session(key, session_id) {
            self.delete_session(key, session_id, cx);
        }
    }

    /// Tutup tab; bila aktif, pindah ke tetangganya.
    pub fn close_tab(&mut self, session_id: &str, cx: &mut Context<Self>) {
        let Some(i) = self.tabs.iter().position(|t| t.session_id == session_id) else { return };
        let closed_key = self.tabs[i].key.clone();
        self.tabs.remove(i);
        if self.selection.session_id.as_deref() == Some(session_id) {
            let next = self.tabs.get(i.min(self.tabs.len().saturating_sub(1))).cloned();
            self.selection = match next {
                Some(n) if !self.tabs.is_empty() => Selection { key: Some(n.key), session_id: Some(n.session_id) },
                _ => Selection { key: self.selection.key.clone(), session_id: None },
            };
        }
        self.persist();
        self.changed(cx);
        self.discard_if_empty(&closed_key, session_id, cx);
    }

    /// Tutup banyak tab: "others" | "right" | "left" | "all".
    pub fn close_tabs_by(&mut self, mode: &str, anchor: Option<&str>, cx: &mut Context<Self>) {
        let i = anchor.and_then(|a| self.tabs.iter().position(|t| t.session_id == a));
        if mode != "all" && i.is_none() {
            return;
        }
        let i = i.unwrap_or(0);
        let keep: Vec<SessionTab> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(idx, _)| match mode {
                "all" => false,
                "others" => *idx == i,
                "right" => *idx <= i,
                _ => *idx >= i,
            })
            .map(|(_, t)| t.clone())
            .collect();
        if keep.len() == self.tabs.len() {
            return;
        }
        if let Some(sid) = self.selection.session_id.clone() {
            if !keep.iter().any(|t| t.session_id == sid) {
                let anchor_tab = if mode == "all" { None } else { self.tabs.get(i).cloned() };
                self.selection = match anchor_tab {
                    Some(a) => Selection { key: Some(a.key), session_id: Some(a.session_id) },
                    None => Selection { key: self.selection.key.clone(), session_id: None },
                };
            }
        }
        self.tabs = keep;
        self.persist();
        self.changed(cx);
    }

    /// Sesi hasil operasi di luar createSession (mis. fork): masukkan & pilih.
    pub fn adopt_session(&mut self, key: &str, session: SessionInfo, cx: &mut Context<Self>) {
        let g = self.sessions.entry(key.into()).or_default();
        g.status = LoadStatus::Ready;
        g.items.retain(|s| s.id != session.id);
        g.items.insert(0, session.clone());
        self.select(Some(key.into()), Some(session.id), cx);
    }

    /// Buat sesi baru (async). `select=false` tidak memindahkan fokus.
    pub fn create_session(&mut self, key: &str, title: Option<String>, select: bool, cx: &mut Context<Self>) -> Task<Result<SessionInfo, String>> {
        let Some(api) = self.api.clone() else { return Task::ready(Err("engine belum siap".into())) };
        let Some(mut dir) = self.directory_of(key) else {
            return Task::ready(Err(if key == CHAT_KEY { "direktori Chat belum tersedia" } else { "project tidak ditemukan" }.into()));
        };
        if key == CHAT_KEY {
            match crate::files::chat_folder_create(&self.host.chats, &chat_folder_name(now_ms(), &new_id())) {
                Ok(p) => dir = p.to_string_lossy().into_owned(),
                Err(e) => return Task::ready(Err(e)),
            }
        }
        let key = key.to_string();
        let work = cx.background_spawn(async move { api.create_session(&dir, title.as_deref()).map_err(|e| e.0) });
        cx.spawn(async move |this, cx| {
            let session = work.await?;
            this.update(cx, |s, cx| {
                let g = s.sessions.entry(key.clone()).or_default();
                g.status = LoadStatus::Ready;
                g.items.retain(|x| x.id != session.id);
                g.items.insert(0, session.clone());
                if select {
                    s.select(Some(key.clone()), Some(session.id.clone()), cx);
                } else {
                    s.changed(cx);
                }
            })
            .map_err(|e| e.to_string())?;
            Ok(session)
        })
    }

    pub fn rename_session(&mut self, key: &str, session_id: &str, title: &str, cx: &mut Context<Self>) {
        let t = title.trim().to_string();
        if t.is_empty() {
            self.fail("judul sesi tidak boleh kosong".into(), cx);
            return;
        }
        let (Some(dir), Some(api)) = (self.dir_for_session(key, session_id), self.api.clone()) else { return };
        // nama dari pengguna: penamaan otomatis berhenti untuk sesi ini (juga event `session.updated` susulannya)
        self.user_titled.insert(session_id.to_string());
        let (key, sid) = (key.to_string(), session_id.to_string());
        let work = cx.background_spawn(async move { api.rename_session(&dir, &sid, &t).map_err(|e| e.0) });
        cx.spawn(async move |this, cx| {
            let res = work.await;
            this.update(cx, |s, cx| match res {
                Ok(updated) => {
                    if let Some(g) = s.sessions.get_mut(&key) {
                        for it in g.items.iter_mut() {
                            if it.id == updated.id {
                                *it = updated.clone();
                            }
                        }
                        g.items.sort_by(by_recent);
                    }
                    s.changed(cx);
                }
                Err(e) => s.fail(e, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Jumlah sesi Chat (tanpa project) yang akan terhapus oleh `delete_all_chats`.
    pub fn chat_count(&self) -> usize {
        self.sessions.get(CHAT_KEY).map_or(0, |g| g.items.len())
    }

    /// Hapus semua sesi Chat (bukan sesi project). Tiap sesi lewat `delete_session` (engine + folder kerja kosong).
    pub fn delete_all_chats(&mut self, cx: &mut Context<Self>) -> usize {
        let ids: Vec<String> = self.sessions.get(CHAT_KEY).map(|g| g.items.iter().map(|s| s.id.clone()).collect()).unwrap_or_default();
        for id in &ids {
            self.delete_session(CHAT_KEY, id, cx);
        }
        ids.len()
    }

    pub fn delete_session(&mut self, key: &str, session_id: &str, cx: &mut Context<Self>) {
        let (Some(dir), Some(api)) = (self.dir_for_session(key, session_id), self.api.clone()) else { return };
        let (key, sid) = (key.to_string(), session_id.to_string());
        let chats = self.host.chats.clone();
        let chat_dir = self.chat_dir.clone();
        let work = cx.background_spawn(async move {
            api.delete_session(&dir, &sid).map_err(|e| e.0)?;
            if key == CHAT_KEY && chat_dir.as_deref() != Some(dir.as_str()) {
                let _ = crate::files::chat_folder_remove_if_empty(&chats, std::path::Path::new(&dir));
            }
            Ok::<_, String>((key, sid))
        });
        cx.spawn(async move |this, cx| {
            let res = work.await;
            this.update(cx, |s, cx| match res {
                Ok((key, sid)) => {
                    if let Some(g) = s.sessions.get_mut(&key) {
                        g.items.retain(|x| x.id != sid);
                    }
                    s.close_tab(&sid, cx);
                    if s.selection.key.as_deref() == Some(&key) && s.selection.session_id.as_deref() == Some(&sid) {
                        s.select(Some(key), None, cx);
                    }
                    s.changed(cx);
                }
                Err(e) => s.fail(e, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Sesi terpilih beserta project-nya.
    pub fn selected(&self) -> Option<(String, SessionInfo, Option<Project>)> {
        let key = self.selection.key.clone()?;
        let sid = self.selection.session_id.as_ref()?;
        let session = self.sessions.get(&key)?.items.iter().find(|s| &s.id == sid)?.clone();
        let project = self.projects.iter().find(|p| p.id == key).cloned();
        Some((key, session, project))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_urls() {
        assert_eq!(remote_url_problem("https://engine.contoh.id"), None);
        assert_eq!(remote_url_problem("http://127.0.0.1:4096"), None);
        assert_eq!(remote_url_problem("http://localhost:4096/"), None);
        assert_eq!(remote_url_problem("http://contoh.id"), Some("insecure"));
        assert_eq!(remote_url_problem("https://a:b@contoh.id"), Some("invalid"));
        assert_eq!(remote_url_problem("ftp://contoh.id"), Some("invalid"));
        assert_eq!(remote_url_problem("bukan url"), Some("invalid"));
        assert_eq!(remote_url_problem("https://x.id/?a=1"), Some("invalid"));
    }
    #[test]
    fn path_dan_nama() {
        assert_eq!(normalize_path("C:/a/b/"), r"C:\a\b");
        assert_eq!(normalize_path("C:\\"), "C:\\");
        assert_eq!(path_key(r"C:\A\B"), r"c:\a\b");
        assert_eq!(base_name(r"C:\proj\zandor\"), "zandor");
        assert_eq!(base_name("/home/u/x/"), "x");
    }
    #[test]
    fn registry_rusak_dan_duplikat() {
        assert!(parse_registry(Some("{")).4.is_some());
        let (p, sel, tabs, _, err) = parse_registry(Some(
            r#"{"version":1,"projects":[{"id":"a","path":"C:/x","name":"x","color":"red"},{"id":"b","path":"c:\\X\\","name":"dup"}],"selection":{"key":"a","sessionId":null},"tabs":[{"key":"a","sessionId":"s1"},{"key":"a","sessionId":"s1"}]}"#,
        ));
        assert!(err.is_none());
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].color.as_deref(), Some("red"));
        assert_eq!(sel.key.as_deref(), Some("a"));
        assert_eq!(tabs.len(), 1);
    }
    #[test]
    fn nama_folder_chat() {
        let n = chat_folder_name(0., "AB-cd!12345");
        assert!(n.starts_with("chat-19") || n.starts_with("chat-197"));
        assert!(n.ends_with("-abcd12"));
        assert!(crate::files::valid_chat_folder_name(&n));
    }
}

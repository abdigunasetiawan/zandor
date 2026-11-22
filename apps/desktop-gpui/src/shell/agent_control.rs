//! Kontrol aplikasi oleh agent (ADR-077) — port agent-control.ts + AgentControlDriver.tsx. Plugin Zandor di engine
//! menulis permintaan ke `<app_data>/control`; aplikasi mengambilnya tiap 700 ms (crate::control::take), menjalankannya,
//! lalu menjawab (crate::control::reply). Permintaan datang dari model (tidak dipercaya): setiap aksi divalidasi di sini;
//! hanya aksi tampilan/aplikasi yang tersedia (akses berkas/shell tetap lewat tool engine yang meminta izin).
use super::layout::{ToolId, TOOL_IDS};
use super::state::{AppState, PanelId, PANEL_IDS};
use crate::i18n::I18n;
use crate::ui::{ToastInput, ToastTone, Toasts};
use gpui::{App, AppContext, Window};
use serde_json::{json, Value};

pub const POLL_MS: u64 = 700;
/// Aksi baca-saja: tanpa toast.
const QUIET: [&str; 5] = ["status", "list_commands", "list_terminals", "get_settings", "suggest_project"];
/// Pengaturan yang tidak boleh diubah agent: izin kontrol agent sendiri.
pub const PROTECTED_SETTINGS: [&str; 2] = ["agentControl", "browserAgent"];
/// Perintah palet yang tidak boleh dijalankan agent (menutup aplikasi, alat pengembang).
pub const BLOCKED_COMMANDS: [&str; 3] = ["window.close", "dev.openDevtools", "dev.copyChatHtml"];
pub const IMAGE_SIZES: [&str; 3] = ["1024x1024", "1536x1024", "1024x1536"];

#[derive(Clone, Debug, PartialEq)]
pub struct ControlRequest {
    pub id: String,
    pub action: String,
    pub args: serde_json::Map<String, Value>,
    pub session_id: String,
    pub directory: String,
}

/// Validasi permintaan mentah dari berkas (port toRequest).
pub fn to_request(raw: &Value) -> Option<ControlRequest> {
    let o = raw.as_object()?;
    let s = |k: &str| o.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    Some(ControlRequest {
        id: o.get("id")?.as_str()?.to_string(),
        action: o.get("action")?.as_str()?.to_string(),
        args: o.get("args").and_then(|v| v.as_object()).cloned().unwrap_or_default(),
        session_id: s("sessionID"),
        directory: s("directory"),
    })
}

fn norm(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_string()
}

/// Letak folder `dir` terhadap root yang dikenal (port locateDir): (root, prefix relatif berakhiran "/" atau "").
pub fn locate_dir(roots: &[String], dir: &str) -> Option<(String, String)> {
    let d = norm(dir);
    let dl = d.to_lowercase();
    let mut best: Option<(String, String)> = None;
    for r in roots {
        let rl = norm(r).to_lowercase();
        if dl == rl {
            return Some((r.clone(), String::new()));
        }
        if dl.starts_with(&format!("{rl}/")) && best.as_ref().is_none_or(|b| norm(&b.0).len() < rl.len()) {
            let rest = &d[rl.len() + 1..];
            if !rest.split('/').any(|s| s == "..") {
                best = Some((r.clone(), format!("{rest}/")));
            }
        }
    }
    best
}

/// Nama berkas aman dari prompt (port imageFileName).
pub fn image_file_name(prompt: &str, stamp: &str) -> String {
    let mut slug = String::new();
    for c in prompt.to_lowercase().chars() {
        let c = match c {
            'à'..='å' => 'a',
            'è'..='ë' => 'e',
            'ì'..='ï' => 'i',
            'ò'..='ö' => 'o',
            'ù'..='ü' => 'u',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_matches('-').chars().take(40).collect();
    let slug = slug.trim_end_matches('-');
    format!("images/{}-{stamp}.png", if slug.is_empty() { "gambar" } else { slug })
}

/// Path relatif yang aman di dalam folder project (port safeRelPath).
pub fn safe_rel_path(path: &str) -> Option<String> {
    let p = path.trim().replace('\\', "/");
    let p = p.strip_prefix("./").unwrap_or(&p).to_string();
    let b = p.as_bytes();
    if p.is_empty() || p.starts_with('/') || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') {
        return None;
    }
    if p.split('/').any(|s| s == ".." || s.is_empty()) {
        return None;
    }
    Some(p)
}

/// Rute halaman yang dikenal aplikasi.
pub fn is_route(path: &str) -> bool {
    matches!(path, "/" | "/settings" | "/appearance" | "/memory" | "/automation" | "/welcome" | "/design-system" | "/labs") || path.strip_prefix("/settings/").is_some_and(|s| !s.is_empty() && !s.contains('/'))
}

pub(super) fn known_roots(cx: &App) -> Vec<String> {
    let s = crate::core::projects::ProjectStore::global(cx).read(cx);
    s.projects.iter().map(|p| p.path.clone()).chain(s.chat_dir.clone()).collect()
}

/// Buka `path` (relatif ke `dir`, atau absolut) di Panel 3 bila di dalam project/folder Chat (port openInDocs).
fn open_file(path: &str, dir: &str, cx: &mut App) -> bool {
    let roots = known_roots(cx);
    let b = path.as_bytes();
    let abs = path.starts_with('/') || path.starts_with('\\') || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'));
    let full = if abs {
        match locate_dir(&roots, path) {
            Some((_, prefix)) if !prefix.trim_end_matches('/').is_empty() => path.replace('/', "\\"),
            _ => return false,
        }
    } else {
        if path.split(['/', '\\']).any(|s| s == "..") {
            return false;
        }
        match locate_dir(&roots, dir) {
            Some(_) => std::path::Path::new(dir).join(path.replace('/', "\\")).to_string_lossy().into_owned(),
            None => return false,
        }
    };
    let docs = super::docs_panel::DocsState::global(cx);
    docs.update(cx, |d, cx| d.open_file(full, cx));
    reveal(PanelId::Docs, cx);
    true
}

/// Buka dokumen (path absolut) di Panel 3 dan munculkan panelnya; dokumen pertama menjadi tab aktif.
pub(super) fn open_documents(paths: Vec<String>, cx: &mut App) {
    let docs = super::docs_panel::DocsState::global(cx);
    for p in paths.into_iter().rev() {
        docs.update(cx, |d, cx| d.open_file(p, cx));
    }
    reveal(PanelId::Docs, cx);
}

/// Munculkan Panel 4 (alat) bila sedang dilipat.
pub(super) fn reveal_tools(cx: &mut App) {
    reveal(PanelId::Tools, cx);
}

fn reveal(p: PanelId, cx: &mut App) {
    let app = AppState::global(cx);
    if app.read(cx).is_collapsed(p) {
        app.update(cx, |a, cx| a.toggle(p, cx));
    }
}

fn str_arg(a: &serde_json::Map<String, Value>, k: &str) -> String {
    a.get(k).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Saran project dari agent (aksi `suggest_project`): diisi di sini, dibaca `ChatPanel` yang menampilkan kartu.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentSuggest {
    pub session_id: String,
    pub reason: String,
    pub summary: String,
    pub name: String,
}
#[derive(Default)]
pub struct SuggestProjectRequest(pub Option<AgentSuggest>);
impl gpui::Global for SuggestProjectRequest {}

/// Hasil langsung, atau generate gambar yang harus dijalankan di thread latar.
pub enum Outcome {
    Done(Result<Value, String>),
    Image { provider: String, model: String, prompt: String, size: String, root: String, rel: String },
    /// Aksi browser-use (F1): berjalan async di browser_agent.rs; jawaban dikirim setelah selesai.
    Browser(super::browser_agent::Job),
}

fn ok(s: impl Into<String>) -> Outcome {
    Outcome::Done(Ok(Value::String(s.into())))
}
fn fail(s: impl Into<String>) -> Outcome {
    Outcome::Done(Err(s.into()))
}

/// Jalankan satu permintaan (port executeControl). `run` menjalankan perintah palet.
pub fn execute(req: &ControlRequest, window: &mut Window, cx: &mut App, run: &mut dyn FnMut(&str, &mut Window, &mut App)) -> Outcome {
    let settings = crate::core::settings::SettingsStore::global(cx);
    if !settings.read(cx).bool("agentControl") {
        return fail("Kontrol aplikasi oleh agent dimatikan pengguna (Pengaturan → Umum → Agent boleh mengoperasikan Zandor).");
    }
    let a = &req.args;
    let i = cx.global::<I18n>().clone();
    match req.action.as_str() {
        "status" => {
            let app = AppState::global(cx).read(cx);
            let panels: serde_json::Map<String, Value> = PANEL_IDS.iter().map(|p| (panel_key(*p).to_string(), json!(if app.is_collapsed(*p) { "closed" } else { "open" }))).collect();
            let page = app.current().to_string();
            let store = crate::core::projects::ProjectStore::global(cx).read(cx);
            let sel = store.selected();
            let docs = super::docs_panel::DocsState::global(cx).read(cx).open.clone();
            let tool = cx.try_global::<ActiveTool>().map(|t| t.0.key()).unwrap_or("terminal");
            Outcome::Done(Ok(json!({
                "panels": panels,
                "activeTool": tool,
                "page": page,
                "project": sel.as_ref().map(|(_, _, p)| p.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "Chat".into())),
                "directory": sel.as_ref().map(|(_, s, _)| s.directory.clone()),
                "openDocuments": docs,
            })))
        }
        "list_commands" => Outcome::Done(Ok(Value::Array(
            super::commands::COMMANDS.iter().filter(|c| !BLOCKED_COMMANDS.contains(&c.id)).map(|c| json!({ "id": c.id, "title": i.t(c.title_key), "enabled": true })).collect(),
        ))),
        "run_command" => {
            let id = str_arg(a, "id");
            if id.is_empty() {
                return fail("id perintah wajib (lihat list_commands).");
            }
            if BLOCKED_COMMANDS.contains(&id.as_str()) {
                return fail(format!("Perintah \"{id}\" tidak boleh dijalankan agent."));
            }
            if super::commands::find(&id).is_none() {
                return fail(format!("Perintah \"{id}\" tidak ada atau sedang nonaktif."));
            }
            run(&id, window, cx);
            ok(format!("Perintah \"{id}\" dijalankan."))
        }
        "panel" => {
            let name = str_arg(a, "panel");
            let state = Some(str_arg(a, "state")).filter(|s| !s.is_empty()).unwrap_or_else(|| "toggle".into());
            let Some(p) = PANEL_IDS.iter().copied().find(|p| panel_key(*p) == name) else {
                return fail(format!("panel harus salah satu: {}", PANEL_IDS.iter().map(|p| panel_key(*p)).collect::<Vec<_>>().join(", ")));
            };
            if !["open", "close", "toggle"].contains(&state.as_str()) {
                return fail("state harus open, close, atau toggle");
            }
            let app = AppState::global(cx);
            let closed = app.read(cx).is_collapsed(p);
            if state == "toggle" || (state == "open" && closed) || (state == "close" && !closed) {
                app.update(cx, |s, cx| s.toggle(p, cx));
            }
            ok(format!("Panel {name}: {state}."))
        }
        "open_tool" => {
            let name = str_arg(a, "tool");
            let Some(t) = TOOL_IDS.iter().copied().find(|t| t.key() == name) else {
                return fail(format!("tool harus salah satu: {}", TOOL_IDS.iter().map(|t| t.key()).collect::<Vec<_>>().join(", ")));
            };
            cx.set_global(super::layout::OpenToolRequest(Some(t)));
            ok(format!("Alat {name} dibuka di Panel 4."))
        }
        "open_url" => {
            let url = str_arg(a, "url");
            match check_url(&url) {
                Ok(u) => {
                    super::layout::open_in_zandor_browser(u.clone(), cx);
                    ok(format!("Membuka {u} di browser Panel 4."))
                }
                Err(e) => fail(e),
            }
        }
        "open_file" => {
            let path = str_arg(a, "path");
            if path.is_empty() {
                return fail("path wajib.");
            }
            if open_file(&path, &req.directory, cx) {
                ok(format!("{path} dibuka di Panel 3."))
            } else {
                fail(format!("{path} berada di luar folder project/Chat yang dikenal Zandor."))
            }
        }
        "navigate" => {
            let page = str_arg(a, "page");
            let page = page.trim_start_matches('/');
            let path = if page.is_empty() || page == "home" { "/".to_string() } else { format!("/{page}") };
            if !is_route(&path) {
                return fail(format!("Halaman \"{page}\" tidak dikenal."));
            }
            AppState::global(cx).update(cx, |s, cx| s.push(path.clone(), cx));
            ok(format!("Membuka {path}."))
        }
        "list_terminals" => Outcome::Done(Ok(Value::Array(super::terminal_panel::list_terminals(cx)))),
        "close_terminal" => {
            let id = str_arg(a, "id");
            if id.is_empty() {
                return fail("id terminal wajib (lihat list_terminals).");
            }
            if super::terminal_panel::close_terminal(&id, cx) {
                ok(format!("Terminal {id} ditutup."))
            } else {
                fail(format!("Terminal \"{id}\" tidak ditemukan."))
            }
        }
        "get_settings" => Outcome::Done(Ok(Value::Object(settings.read(cx).all()))),
        "set_setting" => {
            let key = str_arg(a, "key");
            if !crate::core::settings::keys().contains(&key.as_str()) {
                return fail(format!("Kunci pengaturan \"{key}\" tidak dikenal (lihat get_settings)."));
            }
            if PROTECTED_SETTINGS.contains(&key.as_str()) {
                return fail(format!("Pengaturan \"{key}\" hanya bisa diubah pengguna."));
            }
            let v = a.get("value").cloned().unwrap_or(Value::Null);
            match settings.update(cx, |s, cx| s.set(&key, v.clone(), cx)) {
                Ok(()) => ok(format!("{key} = {v}")),
                Err(e) => fail(e),
            }
        }
        "suggest_project" => {
            if !settings.read(cx).bool("suggestProject") {
                return fail("Pengguna mematikan saran project; kerjakan langsung di sesi ini.");
            }
            let in_chat = crate::core::projects::ProjectStore::global(cx).read(cx).chat_dir.clone().is_some_and(|c| locate_dir(&[c], &req.directory).is_some_and(|(_, p)| !p.is_empty()));
            if !in_chat {
                return fail("Sesi ini sudah berada di project; tidak perlu menyarankan project baru.");
            }
            let (reason, summary) = (str_arg(a, "reason"), str_arg(a, "summary"));
            if reason.is_empty() || summary.is_empty() {
                return fail("reason dan summary wajib.");
            }
            let given = crate::core::project_intent::slugify(&str_arg(a, "name"));
            let name = if given.is_empty() { crate::core::project_intent::suggest_name(&summary, crate::core::project_intent::Kind::Build) } else { given };
            cx.set_global(SuggestProjectRequest(Some(AgentSuggest { session_id: req.session_id.clone(), reason, summary, name })));
            ok("Kartu saran project sudah ditampilkan ke pengguna. Hentikan giliranmu sekarang dan tunggu pilihannya; jangan menginstal atau menulis berkas dulu.")
        }
        "generate_image" => {
            let prompt = str_arg(a, "prompt");
            if prompt.is_empty() {
                return fail("prompt wajib.");
            }
            let size = Some(str_arg(a, "size")).filter(|s| !s.is_empty()).unwrap_or_else(|| "1024x1024".into());
            if !IMAGE_SIZES.contains(&size.as_str()) {
                return fail(format!("size harus salah satu: {}", IMAGE_SIZES.join(", ")));
            }
            if locate_dir(&known_roots(cx), &req.directory).is_none() {
                return fail("Folder sesi ini tidak dikenal Zandor; gambar tidak disimpan.");
            }
            let rel = match a.get("path") {
                None | Some(Value::Null) => Some(image_file_name(&prompt, &chrono_stamp())),
                Some(v) => safe_rel_path(v.as_str().unwrap_or("")),
            };
            let Some(rel) = rel else { return fail("path harus relatif di dalam folder project (tanpa ..).") };
            let pref = crate::core::model_prefs::ModelPrefsStore::global(cx).and_then(|m| m.read(cx).prefs.default_image.clone());
            let Some(r) = pref else {
                return fail("Model gambar bawaan belum dipilih (Pengaturan → Provider dan model → Model generate gambar bawaan).");
            };
            Toasts::show(ToastInput { timeout_ms: Some(4000), ..ToastInput::new(i.t("agent.imageBusy")) }, cx);
            Outcome::Image { provider: r.provider_id, model: r.model_id, prompt, size, root: req.directory.clone(), rel }
        }
        other if other.starts_with("browser_") => {
            if !settings.read(cx).bool("browserAgent") {
                return fail("Kendali browser oleh agent dimatikan pengguna (Pengaturan → Umum → Agent boleh mengendalikan browser Panel 4).");
            }
            Outcome::Browser(super::browser_agent::Job { action: other["browser_".len()..].to_string(), args: a.clone(), directory: req.directory.clone() })
        }
        other => fail(format!("Aksi \"{other}\" tidak dikenal.")),
    }
}

/// Alat Panel 4 yang sedang aktif (diisi ShellLayout) untuk `status`.
pub struct ActiveTool(pub ToolId);
impl gpui::Global for ActiveTool {}

fn panel_key(p: PanelId) -> &'static str {
    match p {
        PanelId::Sidebar => "sidebar",
        PanelId::Chat => "chat",
        PanelId::Docs => "docs",
        PanelId::Tools => "tools",
        PanelId::Rail => "rail",
    }
}

/// Validasi URL open_url: hanya http/https tanpa kredensial.
pub fn check_url(url: &str) -> Result<String, String> {
    let (scheme, rest) = url.split_once("://").ok_or("url tidak valid.")?;
    let scheme = scheme.to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(if scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) && !scheme.is_empty() { "Hanya URL http/https.".into() } else { "url tidak valid.".into() });
    }
    let host_part = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host_part.is_empty() || host_part.contains(char::is_whitespace) {
        return Err("url tidak valid.".into());
    }
    if host_part.contains('@') {
        return Err("URL dengan kredensial ditolak.".into());
    }
    let tail = &rest[host_part.len()..];
    Ok(format!("{scheme}://{}{}", host_part.to_lowercase(), if tail.is_empty() { "/" } else { tail }))
}

/// Cap waktu lokal YYYYMMDD-HHMMSS (tanpa crate tambahan).
fn chrono_stamp() -> String {
    #[cfg(windows)]
    {
        let mut t: windows_sys::Win32::Foundation::SYSTEMTIME = unsafe { std::mem::zeroed() };
        unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t) };
        return format!("{:04}{:02}{:02}-{:02}{:02}{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond);
    }
    #[allow(unreachable_code)]
    {
        let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        format!("{s}")
    }
}

/// Toast + jawab (port handle di AgentControlDriver).
pub fn finish(req: &ControlRequest, res: Result<Value, String>, cx: &mut App) {
    let i = cx.global::<I18n>().clone();
    match &res {
        Err(e) => {
            Toasts::show(ToastInput { tone: ToastTone::Danger, timeout_ms: Some(6000), ..ToastInput::new(i.tp("agent.failed", &[("error", e)])) }, cx);
        }
        // aksi browser tidak memunculkan toast (hasil panjang; Panel 4 sudah menunjukkan aktivitasnya)
        Ok(Value::String(s)) if !QUIET.contains(&req.action.as_str()) && !req.action.starts_with("browser_") => {
            Toasts::show(ToastInput { timeout_ms: Some(3500), ..ToastInput::new(i.tp("agent.toast", &[("what", s)])) }, cx);
        }
        _ => {}
    }
    let body = match res {
        Ok(Value::Null) => json!({ "ok": true }),
        Ok(v) => json!({ "ok": true, "result": v }),
        Err(e) => json!({ "ok": false, "error": e }),
    };
    let (dir, id) = (control_dir(), req.id.clone());
    cx.background_spawn(async move {
        if let Err(e) = crate::control::reply(&dir, &id, &body) {
            eprintln!("[zandor] control reply: {e}");
        }
    })
    .detach();
}

pub fn control_dir() -> std::path::PathBuf {
    // ZANDOR_CONTROL_DIR hanya untuk uji (aplikasi Tauri yang berjalan bersamaan memakai folder data yang sama)
    std::env::var_os("ZANDOR_CONTROL_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| crate::core::services::app_data_dir().join("control"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validasi_dan_jalur() {
        assert!(to_request(&json!({"id": "a"})).is_none());
        let r = to_request(&json!({"id": "a", "action": "status", "args": [1], "directory": "C:/p"})).unwrap();
        assert!(r.args.is_empty());
        assert_eq!(r.directory, "C:/p");
        let roots = vec!["C:\\Proj".to_string(), "C:\\Data\\chats".to_string()];
        assert_eq!(locate_dir(&roots, "c:/proj"), Some(("C:\\Proj".into(), "".into())));
        assert_eq!(locate_dir(&roots, "C:\\Data\\chats\\chat-1"), Some(("C:\\Data\\chats".into(), "chat-1/".into())));
        assert_eq!(locate_dir(&roots, "C:\\Other"), None);
        assert_eq!(safe_rel_path("./a/b.png").as_deref(), Some("a/b.png"));
        assert!(safe_rel_path("../x").is_none() && safe_rel_path("C:/x").is_none() && safe_rel_path("/x").is_none());
        assert_eq!(image_file_name("Kucing Lucu!! di atap", "20260101-010203"), "images/kucing-lucu-di-atap-20260101-010203.png");
        assert_eq!(image_file_name("!!!", "s"), "images/gambar-s.png");
        assert!(is_route("/settings/models") && is_route("/") && !is_route("/nope"));
        assert_eq!(check_url("https://Example.com").unwrap(), "https://example.com/");
        assert!(check_url("ftp://x").is_err() && check_url("https://u:p@x").is_err() && check_url("nope").is_err());
    }
}

//! Spike S7 — terminal: xterm.js + dua backend PTY yang dibandingkan.
//!   A) `portable-pty` di proses Rust Zandor, byte dialirkan lewat `tauri::ipc::Channel` (raw).
//!   B) API `/pty` milik `opencode serve` (WebSocket + tiket), dipanggil langsung dari UI.
//! Self-test: env `ZANDOR_SELFTEST=<file.json>`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Manager, RunEvent, State};
use zandor_engine_supervisor::{Engine, EngineConfig};

struct LocalTerm {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    pid: Option<u32>,
}

#[derive(Default)]
struct AppState {
    terms: Mutex<HashMap<u32, LocalTerm>>,
    next: AtomicU32,
    engine: Mutex<Option<Engine>>,
    #[cfg(windows)]
    job: Mutex<Option<win::Job>>,
}

#[derive(Serialize)]
struct Opened {
    id: u32,
    pid: Option<u32>,
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

/// Backend A: buka PTY lokal. Output dikirim sebagai byte mentah (tanpa JSON) lewat channel.
#[tauri::command]
fn local_open(
    state: State<AppState>,
    shell: String,
    args: Vec<String>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    on_data: Channel<InvokeResponseBody>,
) -> Result<Opened, String> {
    let pair = native_pty_system().openpty(size(cols, rows)).map_err(|e| e.to_string())?;
    let mut cmd = CommandBuilder::new(shell);
    cmd.args(args);
    if let Some(c) = cwd {
        cmd.cwd(c);
    }
    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);
    let pid = child.process_id();
    #[cfg(windows)]
    if let (Some(p), Some(job)) = (pid, state.job.lock().unwrap().as_ref()) {
        // Anti proses yatim: shell masuk Job Object KILL_ON_JOB_CLOSE milik app.
        let _ = job.assign_pid(p);
    }
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let id = state.next.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::Builder::new()
        .name(format!("pty-read-{id}"))
        .spawn(move || {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if on_data.send(InvokeResponseBody::Raw(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    state.terms.lock().unwrap().insert(id, LocalTerm { master: pair.master, writer, child, pid });
    Ok(Opened { id, pid })
}

#[tauri::command]
fn local_write(state: State<AppState>, id: u32, data: String) -> Result<(), String> {
    let mut t = state.terms.lock().unwrap();
    let term = t.get_mut(&id).ok_or("terminal tidak ada")?;
    term.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    term.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
fn local_resize(state: State<AppState>, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let t = state.terms.lock().unwrap();
    t.get(&id).ok_or("terminal tidak ada")?.master.resize(size(cols, rows)).map_err(|e| e.to_string())
}

#[tauri::command]
fn local_close(state: State<AppState>, id: u32) -> Result<Option<u32>, String> {
    if let Some(mut term) = state.terms.lock().unwrap().remove(&id) {
        let _ = term.child.kill();
        let _ = term.child.wait();
        return Ok(term.pid);
    }
    Ok(None)
}

#[derive(Serialize)]
struct EngineView {
    url: String,
    password: String,
}

#[tauri::command]
fn engine_info(state: State<AppState>) -> Option<EngineView> {
    state
        .engine
        .lock()
        .unwrap()
        .as_ref()
        .map(|e| EngineView { url: e.info().url.clone(), password: e.info().password.clone() })
}

#[tauri::command]
fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        win::alive(pid)
    }
    #[cfg(unix)]
    unsafe {
        libc_kill0(pid)
    }
}

#[cfg(unix)]
unsafe fn libc_kill0(pid: u32) -> bool {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    kill(pid as i32, 0) == 0
}

#[tauri::command]
fn selftest_path() -> Option<String> {
    std::env::var("ZANDOR_SELFTEST").ok()
}

#[tauri::command]
fn selftest_log(msg: String) {
    if let Ok(p) = std::env::var("ZANDOR_SELFTEST") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(format!("{p}.log")) {
            let _ = writeln!(f, "{msg}");
        }
    }
}

#[tauri::command]
fn selftest_done(app: AppHandle, report: serde_json::Value) {
    if let Ok(p) = std::env::var("ZANDOR_SELFTEST") {
        let _ = std::fs::write(p, serde_json::to_string_pretty(&report).unwrap());
    }
    app.exit(0);
}

fn sidecar() -> PathBuf {
    std::env::current_exe().unwrap().parent().unwrap().join(if cfg!(windows) { "opencode.exe" } else { "opencode" })
}

fn main() {
    // Windows: status "abaikan Ctrl+C" diwariskan dari proses induk ke semua turunan.
    // Bila Zandor diluncurkan oleh proses yang mengabaikan Ctrl+C, shell di terminal tidak bisa
    // diinterupsi. Pulihkan sebelum membuat PTY / engine. (Bisa dimatikan untuk eksperimen.)
    #[cfg(windows)]
    if std::env::var_os("ZANDOR_S7_NO_CTRLC_RESET").is_none() {
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0);
        }
    }
    let app = tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            #[cfg(windows)]
            {
                *app.state::<AppState>().job.lock().unwrap() = win::Job::new().ok();
            }
            let h = app.handle().clone();
            std::thread::Builder::new().name("engine-supervisor".into()).spawn(move || {
                let mut cfg = EngineConfig::new(sidecar());
                cfg.cwd = std::env::var("ZANDOR_S7_CWD").ok().map(PathBuf::from);
                match Engine::start(cfg, |_| {}) {
                    Ok(e) => *h.state::<AppState>().engine.lock().unwrap() = Some(e),
                    Err(e) => eprintln!("engine gagal: {e}"),
                }
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            local_open,
            local_write,
            local_resize,
            local_close,
            engine_info,
            process_alive,
            selftest_path,
            selftest_log,
            selftest_done
        ])
        .build(tauri::generate_context!())
        .expect("gagal membangun app");
    app.run(|h, ev| {
        if let RunEvent::Exit = ev {
            let st = h.state::<AppState>();
            let terms: Vec<LocalTerm> = st.terms.lock().unwrap().drain().map(|(_, t)| t).collect();
            for mut t in terms {
                let _ = t.child.kill();
            }
            let engine = st.engine.lock().unwrap().take();
            if let Some(mut e) = engine {
                e.stop();
            }
        }
    });
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, STILL_ACTIVE};
    use windows_sys::Win32::System::JobObjects::*;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    pub struct Job(HANDLE);
    unsafe impl Send for Job {}

    impl Job {
        pub fn new() -> std::io::Result<Self> {
            unsafe {
                let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if h.is_null() {
                    return Err(std::io::Error::last_os_error());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                Ok(Job(h))
            }
        }
        pub fn assign_pid(&self, pid: u32) -> std::io::Result<()> {
            unsafe {
                let p = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                if p.is_null() {
                    return Err(std::io::Error::last_os_error());
                }
                let ok = AssignProcessToJobObject(self.0, p);
                CloseHandle(p);
                if ok == 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    pub fn alive(pid: u32) -> bool {
        unsafe {
            let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if p.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(p, &mut code);
            CloseHandle(p);
            ok != 0 && code == STILL_ACTIVE as u32
        }
    }
}

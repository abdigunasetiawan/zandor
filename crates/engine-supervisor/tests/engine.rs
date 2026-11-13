//! Test integrasi S1 dengan binary `opencode` asli (versi di-pin).
//! Jalankan dulu: `node spikes/tools/fetch-opencode.mjs`.
//! Override lokasi binary dengan env `ZANDOR_OPENCODE_BIN`.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use zandor_engine_supervisor::{http, http_json, Engine, EngineConfig, EngineError};

const PINNED: &str = "1.19.5";

/// Binary engine untuk test. `ZANDOR_OPENCODE_BIN` menimpa; bila tidak ada → default sidecar produksi.
/// Bila binary tidak ada: test dilewati (dengan pesan), kecuali `ZANDOR_REQUIRE_ENGINE=1` (CI) → gagal.
fn opencode_bin() -> Option<PathBuf> {
    let p = std::env::var("ZANDOR_OPENCODE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src-tauri/binaries/opencode-x86_64-pc-windows-msvc.exe")
        });
    if p.exists() {
        return Some(p);
    }
    assert!(
        std::env::var("ZANDOR_REQUIRE_ENGINE").as_deref() != Ok("1"),
        "binary engine wajib ada (ZANDOR_REQUIRE_ENGINE=1): {p:?} — jalankan `bun run engine:fetch`"
    );
    eprintln!("SKIP: binary engine tidak ada ({p:?}); jalankan `bun run engine:fetch` untuk menjalankan test ini");
    None
}

/// Lewati test bila binary tidak tersedia.
macro_rules! need_engine {
    () => {
        match opencode_bin() {
            Some(p) => p,
            None => return,
        }
    };
}

/// Direktori data unik per engine: test berjalan paralel dan SQLite hanya boleh dimiliki satu proses
/// ("database is locked"); ini juga menjaga data opencode milik user tidak tersentuh.
fn cfg(bin: PathBuf) -> EngineConfig {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("zandor-sup-{}-{id}", std::process::id()));
    let mut c = EngineConfig::new(bin);
    for (k, d) in [
        ("DATA", "d"),
        ("CONFIG", "c"),
        ("CACHE", "k"),
        ("STATE", "s"),
    ] {
        c.env.push((
            format!("XDG_{k}_HOME"),
            root.join(d).to_string_lossy().into_owned(),
        ));
    }
    c.expected_version = Some(PINNED.into());
    c
}

/// Daftar (pid, ppid, nama) semua proses.
fn process_table() -> Vec<(u32, u32, String)> {
    if cfg!(windows) {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-CimInstance Win32_Process | % { \"$($_.ProcessId) $($_.ParentProcessId) $($_.Name)\" }",
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let mut it = l.trim().splitn(3, ' ');
                Some((
                    it.next()?.parse().ok()?,
                    it.next()?.parse().ok()?,
                    it.next()?.to_string(),
                ))
            })
            .collect()
    } else {
        let out = Command::new("ps")
            .args(["-A", "-o", "pid=,ppid=,comm="])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((
                    it.next()?.parse().ok()?,
                    it.next()?.parse().ok()?,
                    it.collect::<Vec<_>>().join(" "),
                ))
            })
            .collect()
    }
}

fn descendants(root: u32) -> Vec<(u32, String)> {
    let t = process_table();
    let mut out = Vec::new();
    let mut frontier = vec![root];
    while let Some(p) = frontier.pop() {
        for (pid, ppid, name) in &t {
            if *ppid == p && *pid != p {
                out.push((*pid, name.clone()));
                frontier.push(*pid);
            }
        }
    }
    out
}

fn alive(pids: &[u32]) -> Vec<u32> {
    let t = process_table();
    pids.iter()
        .copied()
        .filter(|p| t.iter().any(|(pid, _, _)| pid == p))
        .collect()
}

fn wait_dead(pids: &[u32], timeout: Duration) -> Vec<u32> {
    let deadline = Instant::now() + timeout;
    loop {
        let a = alive(pids);
        if a.is_empty() || Instant::now() > deadline {
            return a;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn spawn_grandchild(e: &Engine) -> Vec<(u32, String)> {
    let info = e.info();
    let body = if cfg!(windows) {
        r#"{"command":"ping","args":["-n","600","127.0.0.1"]}"#
    } else {
        r#"{"command":"sleep","args":["600"]}"#
    };
    let (code, resp) = http_json(
        &info.host_port,
        "POST",
        "/pty",
        Some(&info.basic_auth_header()),
        Some(body),
        Duration::from_secs(10),
    )
    .unwrap();
    assert_eq!(code, 200, "{resp}");
    std::thread::sleep(Duration::from_millis(800));
    let d = descendants(info.pid);
    assert!(
        d.iter()
            .any(|(_, n)| n
                .to_lowercase()
                .contains(if cfg!(windows) { "ping" } else { "sleep" })),
        "proses cucu tidak ditemukan: {d:?}"
    );
    d
}

#[test]
fn start_health_auth_and_pinned_version() {
    let bin = need_engine!();
    let e = Engine::start(cfg(bin.clone()), |_| {}).expect("start");
    let i = e.info().clone();
    println!("engine siap dalam {:?} di {}", i.startup, i.url);
    assert!(
        i.url.starts_with("http://127.0.0.1:"),
        "harus loopback: {}",
        i.url
    );
    assert_eq!(i.version, PINNED);
    assert_eq!(e.health().unwrap(), PINNED);

    // Tanpa token / token salah → 401.
    let (c, _) = http(
        &i.host_port,
        "GET",
        "/global/health",
        None,
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(c, 401);
    let (c, _) = http(
        &i.host_port,
        "GET",
        "/global/health",
        Some("Basic b3BlbmNvZGU6c2FsYWg="), // opencode:salah
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(c, 401);
    // Endpoint sesi juga terlindungi.
    let (c, _) = http(
        &i.host_port,
        "GET",
        "/session",
        None,
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(c, 401);
}

#[test]
fn version_pin_is_enforced() {
    let bin = need_engine!();
    let mut c = cfg(bin.clone());
    c.expected_version = Some("0.0.0-tidak-ada".into());
    match Engine::start(c, |_| {}) {
        Err(EngineError::VersionMismatch { got, .. }) => assert_eq!(got, PINNED),
        Err(e) => panic!("error tak terduga: {e}"),
        Ok(_) => panic!("seharusnya gagal"),
    }
}

#[test]
fn two_engines_get_distinct_ports() {
    let bin = need_engine!();
    let a = Engine::start(cfg(bin.clone()), |_| {}).unwrap();
    let b = Engine::start(cfg(bin.clone()), |_| {}).unwrap();
    assert_ne!(a.info().host_port, b.info().host_port);
    assert_ne!(a.info().password, b.info().password);
}

#[test]
fn stop_kills_engine_and_grandchildren() {
    let bin = need_engine!();
    let mut e = Engine::start(cfg(bin.clone()), |_| {}).unwrap();
    let pid = e.info().pid;
    let tree = spawn_grandchild(&e);
    let mut pids: Vec<u32> = tree.iter().map(|(p, _)| *p).collect();
    pids.push(pid);
    println!("pohon proses sebelum stop: engine {pid} + {tree:?}");
    let t = Instant::now();
    e.stop();
    println!("stop selesai dalam {:?}", t.elapsed());
    let left = wait_dead(&pids, Duration::from_secs(5));
    assert!(left.is_empty(), "proses yatim tersisa: {left:?}");
    assert!(!e.is_running());
}

#[test]
fn unexpected_exit_is_reported() {
    let bin = need_engine!();
    let (tx, rx) = mpsc::channel();
    let e = Engine::start(cfg(bin.clone()), move |x| {
        let _ = tx.send(x);
    })
    .unwrap();
    e.kill_main_process_for_test();
    let info = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("callback exit tidak dipanggil");
    println!("exit terdeteksi, code={:?}", info.code);
    assert!(!e.is_running());
}

#[test]
fn stop_does_not_report_unexpected_exit() {
    let bin = need_engine!();
    let (tx, rx) = mpsc::channel::<()>();
    let mut e = Engine::start(cfg(bin.clone()), move |_| {
        let _ = tx.send(());
    })
    .unwrap();
    e.stop();
    assert!(rx.recv_timeout(Duration::from_secs(1)).is_err());
}

/// Simulasi crash aplikasi: parent dibunuh paksa (TerminateProcess / SIGKILL),
/// tidak ada kode cleanup yang sempat jalan.
#[test]
fn parent_crash_leaves_no_orphans() {
    let bin = need_engine!();
    let mut holder = Command::new(env!("CARGO_BIN_EXE_engine-hold"))
        .arg(&bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(holder.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let engine_pid: u32 = line
        .trim()
        .strip_prefix("READY ")
        .expect(&line)
        .parse()
        .unwrap();
    std::thread::sleep(Duration::from_millis(800));
    let tree = descendants(engine_pid);
    println!("engine {engine_pid}, keturunan {tree:?}");
    assert!(!tree.is_empty(), "harus ada proses cucu");
    let mut pids: Vec<u32> = tree.iter().map(|(p, _)| *p).collect();
    pids.push(engine_pid);

    holder.kill().unwrap(); // crash paksa
    let _ = holder.wait();

    let left = wait_dead(&pids, Duration::from_secs(5));
    if cfg!(target_os = "macos") && !left.is_empty() {
        panic!("macOS: proses yatim tersisa (risiko terbuka, lihat ADR): {left:?}");
    }
    assert!(left.is_empty(), "proses yatim tersisa: {left:?}");
}

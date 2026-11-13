//! Helper test S1: jalankan engine + satu proses cucu (via `POST /pty`), cetak PID,
//! lalu diam selamanya. Test integrasi membunuh proses ini secara paksa untuk
//! mensimulasikan crash aplikasi dan memeriksa tidak ada proses yatim.
use std::time::Duration;
use zandor_engine_supervisor::{http_json, Engine, EngineConfig};

fn main() {
    let bin = std::env::args().nth(1).expect("argumen: <path opencode>");
    let engine = Engine::start(EngineConfig::new(bin), |_| {}).expect("start engine");
    let info = engine.info().clone();
    let body = if cfg!(windows) {
        r#"{"command":"ping","args":["-n","600","127.0.0.1"],"title":"zandor-s1-grandchild"}"#
    } else {
        r#"{"command":"sleep","args":["600"],"title":"zandor-s1-grandchild"}"#
    };
    let (code, resp) = http_json(
        &info.host_port,
        "POST",
        "/pty",
        Some(&info.basic_auth_header()),
        Some(body),
        Duration::from_secs(10),
    )
    .expect("POST /pty");
    assert_eq!(code, 200, "POST /pty gagal: {resp}");
    println!("READY {}", info.pid);
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

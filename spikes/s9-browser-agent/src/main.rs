//! Spike S9 (F0 browser-use): membuktikan di Windows —
//!  S-B1: dua profil WebView2 (data directory berbeda) berdampingan sebagai child webview, cookie terisolasi;
//!  S-B2: CDP lewat `WebViewExtWindows::controller()` — klik/ketik tepercaya, scroll, screenshot, latensi;
//!  S-B3: skrip snapshot + ref lewat initialization script, anti-timpa halaman, redaksi password, ukuran keluaran.
//! Hasil dicetak sebagai baris `PASS/FAIL` + `INFO`, dan ditulis ke `%TEMP%/zandor-s9/hasil.txt`.
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
use windows::core::PCWSTR;
use wry::{Rect, WebContext, WebView, WebViewBuilder, WebViewExtWindows};

type Views = HashMap<&'static str, WebView>;
enum Ev {
    Run(Box<dyn FnOnce(&Views) + Send>),
    Quit,
}
type Proxy = EventLoopProxy<Ev>;

const SNAPSHOT_JS: &str = include_str!("snapshot.js");
const FIXTURE: &str = include_str!("fixture.html");

fn serve_fixture() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            let body = FIXTURE.as_bytes();
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = s.write_all(body);
        }
    });
    port
}

fn eval(p: &Proxy, id: &'static str, js: &str) -> String {
    let (tx, rx) = mpsc::channel::<String>();
    let js = js.to_string();
    let _ = p.send_event(Ev::Run(Box::new(move |v| {
        let tx2 = tx.clone();
        if let Err(e) = v[id].evaluate_script_with_callback(&js, move |s| {
            let _ = tx2.send(s);
        }) {
            let _ = tx.send(format!("ERR {e}"));
        }
    })));
    rx.recv_timeout(Duration::from_secs(15)).unwrap_or_else(|_| "TIMEOUT".into())
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Panggil satu metode CDP lewat controller WebView2 dan tunggu hasilnya (JSON string).
fn cdp(p: &Proxy, id: &'static str, method: &str, params: &str) -> String {
    let (tx, rx) = mpsc::channel::<String>();
    let (m, pr) = (method.to_string(), params.to_string());
    let _ = p.send_event(Ev::Run(Box::new(move |v| {
        let tx_err = tx.clone();
        let res: Result<(), String> = (|| unsafe {
            let core = v[id].controller().CoreWebView2().map_err(|e| format!("CoreWebView2: {e}"))?;
            let (mw, pw) = (wide(&m), wide(&pr));
            let h = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |e, r| {
                let _ = tx.send(match e {
                    Ok(()) => r,
                    Err(e) => format!("ERR {e}"),
                });
                Ok(())
            }));
            core.CallDevToolsProtocolMethod(PCWSTR(mw.as_ptr()), PCWSTR(pw.as_ptr()), &h).map_err(|e| format!("Call: {e}"))
        })();
        if let Err(e) = res {
            let _ = tx_err.send(format!("ERR {e}"));
        }
    })));
    rx.recv_timeout(Duration::from_secs(15)).unwrap_or_else(|_| "TIMEOUT".into())
}

struct Report(Vec<String>);
impl Report {
    fn line(&mut self, s: String) {
        println!("{s}");
        self.0.push(s);
    }
    fn check(&mut self, name: &str, ok: bool, detail: &str) {
        self.line(format!("{} {name} — {detail}", if ok { "PASS" } else { "FAIL" }));
    }
    fn info(&mut self, s: &str) {
        self.line(format!("INFO {s}"));
    }
}

fn json(s: &str) -> serde_json::Value {
    // evaluate_script_with_callback mengembalikan JSON; string JS menjadi string JSON ber-escape → buka lapisan kedua.
    let v: serde_json::Value = serde_json::from_str(s).unwrap_or(serde_json::Value::Null);
    match v {
        serde_json::Value::String(inner) => serde_json::from_str(&inner).unwrap_or(serde_json::Value::String(inner)),
        other => other,
    }
}

fn run_tests(p: Proxy, port: u16, load_rx: mpsc::Receiver<&'static str>, out: PathBuf) {
    let mut r = Report(vec![]);
    // tunggu A dan B selesai memuat
    let (mut a, mut b) = (false, false);
    let t0 = Instant::now();
    while !(a && b) && t0.elapsed() < Duration::from_secs(40) {
        match load_rx.recv_timeout(Duration::from_secs(1)) {
            Ok("A") => a = true,
            Ok("B") => b = true,
            _ => {}
        }
    }
    r.check("S-B1 dua webview memuat fixture", a && b, &format!("A={a} B={b} dalam {:?}", t0.elapsed()));
    if !(a && b) {
        finish(r, &out, &p);
        return;
    }
    std::thread::sleep(Duration::from_millis(500));

    // ---- S-B3 snapshot ----
    let t = Instant::now();
    let snap = json(&eval(&p, "A", "__zsnap()"));
    let dt = t.elapsed();
    let tree = snap["tree"].as_str().unwrap_or("").to_string();
    r.info(&format!("snapshot {} byte, {} ref, {:?}\n{tree}", tree.len(), snap["refs"], dt));
    r.check("S-B3 snapshot memuat tombol/kolom/tautan", tree.contains("button \"Kirim\"") && tree.contains("link \"Tautan lain\"") && tree.contains("textbox \"Nama\""), "role+nama terbaca");
    r.check("S-B3 password disamarkan", tree.contains("<disamarkan>") && !tree.contains("rahasia123"), "nilai field password tidak keluar");
    r.check("S-B3 teks tersembunyi tidak ikut", !snap["text"].as_str().unwrap_or("").contains("ABAIKAN INSTRUKSI"), "display:none dikeluarkan dari text (innerText)");
    let hacked = eval(&p, "A", "String(typeof __zsnap) + ':' + String(__zsnap().length>10)");
    r.check("S-B3 halaman tidak bisa menimpa API agent", hacked.contains("function") && hacked.contains("true"), &hacked);

    // cari ref tombol & kolom nama
    let find_ref = |label: &str| -> String {
        tree.lines().find(|l| l.contains(label)).and_then(|l| l.split(']').next()).map(|s| s.trim_start_matches('[').to_string()).unwrap_or_default()
    };
    let (btn, name_ref, far_ref) = (find_ref("button \"Kirim\""), find_ref("textbox \"Nama\""), find_ref("Tombol jauh"));
    r.info(&format!("ref tombol={btn} nama={name_ref} jauh={far_ref}"));

    // ---- S-B2 klik tepercaya via CDP ----
    let rc = json(&eval(&p, "A", &format!("__zrect('{btn}')")));
    let (x, y) = (rc["x"].as_f64().unwrap_or(0.0), rc["y"].as_f64().unwrap_or(0.0));
    r.check("hit-test tombol", rc["hit"] == true, &rc.to_string());
    let t = Instant::now();
    let mv = cdp(&p, "A", "Input.dispatchMouseEvent", &format!(r#"{{"type":"mouseMoved","x":{x},"y":{y}}}"#));
    let pr = cdp(&p, "A", "Input.dispatchMouseEvent", &format!(r#"{{"type":"mousePressed","x":{x},"y":{y},"button":"left","clickCount":1}}"#));
    let re = cdp(&p, "A", "Input.dispatchMouseEvent", &format!(r#"{{"type":"mouseReleased","x":{x},"y":{y},"button":"left","clickCount":1}}"#));
    let click_dt = t.elapsed();
    r.info(&format!("CDP klik: move={mv} press={pr} release={re} ({click_dt:?})"));
    let st = json(&eval(&p, "A", "JSON.stringify(st)"));
    r.check("S-B2 klik CDP sampai ke halaman", st["clicks"] == 1, &st.to_string());
    r.check("S-B2 klik CDP bernilai isTrusted", st["trustedClicks"] == 1, &format!("trustedClicks={}", st["trustedClicks"]));

    // ketik: fokus + insertText + Enter
    let _ = eval(&p, "A", &format!("__zfocus('{name_ref}')"));
    let ins = cdp(&p, "A", "Input.insertText", r#"{"text":"halo dunia"}"#);
    let kd = cdp(&p, "A", "Input.dispatchKeyEvent", r#"{"type":"keyDown","key":"Enter","code":"Enter","windowsVirtualKeyCode":13,"text":"\r"}"#);
    let ku = cdp(&p, "A", "Input.dispatchKeyEvent", r#"{"type":"keyUp","key":"Enter","code":"Enter","windowsVirtualKeyCode":13}"#);
    r.info(&format!("CDP ketik: insert={ins} kd={kd} ku={ku}"));
    let st = json(&eval(&p, "A", "JSON.stringify(st)"));
    r.check("S-B2 insertText mengisi kolom", st["value"] == "halo dunia", &st.to_string());
    r.check("S-B2 tombol Enter tepercaya", st["trustedKeys"].as_i64().unwrap_or(0) >= 1, &format!("trustedKeys={}", st["trustedKeys"]));

    // scroll + ref jauh
    let rc = json(&eval(&p, "A", &format!("__zrect('{far_ref}')")));
    r.check("scrollIntoView + hit-test elemen jauh", rc["hit"] == true, &rc.to_string());
    let sc = cdp(&p, "A", "Input.dispatchMouseEvent", r#"{"type":"mouseWheel","x":200,"y":200,"deltaX":0,"deltaY":-300}"#);
    r.info(&format!("mouseWheel: {sc}"));

    // screenshot
    let t = Instant::now();
    let shot = cdp(&p, "A", "Page.captureScreenshot", r#"{"format":"png"}"#);
    let shot_dt = t.elapsed();
    let data = serde_json::from_str::<serde_json::Value>(&shot).ok().and_then(|v| v["data"].as_str().map(String::from)).unwrap_or_default();
    r.check("S-B2 Page.captureScreenshot", data.len() > 1000, &format!("{} karakter base64, {:?}", data.len(), shot_dt));
    if !data.is_empty() {
        let _ = std::fs::write(out.with_file_name("shot-A.b64"), &data);
    }

    // ---- S-B1 isolasi profil ----
    let _ = eval(&p, "A", "document.cookie = 'profil=A; path=/'; localStorage.setItem('p','A'); 1");
    let ca = eval(&p, "A", "document.cookie + '|' + localStorage.getItem('p')");
    let cb = eval(&p, "B", "document.cookie + '|' + localStorage.getItem('p')");
    r.info(&format!("A: {ca}   B: {cb}"));
    r.check("S-B1 cookie/localStorage A tidak terlihat di B", ca.contains("profil=A") && !cb.contains("profil=A"), &format!("A={ca} B={cb}"));
    let sb = json(&eval(&p, "B", "__zsnap()"));
    r.check("S-B1 CDP+snapshot juga jalan di profil B", sb["refs"].as_i64().unwrap_or(0) >= 3, &format!("refs B={}", sb["refs"]));

    // ---- webview tersembunyi (airspace / panel lain) ----
    let rc = json(&eval(&p, "A", &format!("scrollTo(0,0); __zrect('{btn}')")));
    let rc = if rc["x"].is_null() { json(&eval(&p, "A", &format!("__zrect('{btn}')"))) } else { rc };
    let (hx, hy) = (rc["x"].as_f64().unwrap_or(0.0), rc["y"].as_f64().unwrap_or(0.0));
    let before = json(&eval(&p, "A", "JSON.stringify(st)"))["clicks"].as_i64().unwrap_or(-1);
    let _ = p.send_event(Ev::Run(Box::new(|v| {
        let _ = v["A"].set_visible(false);
    })));
    std::thread::sleep(Duration::from_millis(400));
    let vis = eval(&p, "A", "document.visibilityState");
    r.info(&format!("document.visibilityState saat disembunyikan = {vis}"));
    let _ = cdp(&p, "A", "Input.dispatchMouseEvent", &format!(r#"{{"type":"mousePressed","x":{hx},"y":{hy},"button":"left","clickCount":1}}"#));
    let _ = cdp(&p, "A", "Input.dispatchMouseEvent", &format!(r#"{{"type":"mouseReleased","x":{hx},"y":{hy},"button":"left","clickCount":1}}"#));
    let after = json(&eval(&p, "A", "JSON.stringify(st)"))["clicks"].as_i64().unwrap_or(-1);
    r.info(&format!("webview disembunyikan: klik CDP clicks {before} -> {after}"));
    r.check("klik CDP tetap jalan saat webview disembunyikan", after == before + 1, &format!("{before} -> {after}"));
    let t = Instant::now();
    let hs = cdp(&p, "A", "Page.captureScreenshot", r#"{"format":"png"}"#);
    r.info(&format!("webview disembunyikan: screenshot -> {} ({:?})", hs.chars().take(60).collect::<String>(), t.elapsed()));
    finish(r, &out, &p);
}

fn finish(r: Report, out: &PathBuf, p: &Proxy) {
    let fails = r.0.iter().filter(|l| l.starts_with("FAIL")).count();
    let passes = r.0.iter().filter(|l| l.starts_with("PASS")).count();
    let tail = format!("RINGKAS {passes} PASS, {fails} FAIL");
    println!("{tail}");
    let mut all = r.0.join("\n");
    all.push('\n');
    all.push_str(&tail);
    let _ = std::fs::write(out, all);
    let _ = p.send_event(Ev::Quit);
}

fn main() {
    let port = serve_fixture();
    let base = std::env::temp_dir().join("zandor-s9");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let out = base.join("hasil.txt");

    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window: &'static _ = Box::leak(Box::new(WindowBuilder::new().with_title("S9 browser agent").with_inner_size(LogicalSize::new(1200.0, 700.0)).build(&event_loop).unwrap()));
    let ctx_a: &'static mut WebContext = Box::leak(Box::new(WebContext::new(Some(base.join("profil-A")))));
    let ctx_b: &'static mut WebContext = Box::leak(Box::new(WebContext::new(Some(base.join("profil-B")))));
    let url = format!("http://127.0.0.1:{port}/");
    let (ltx, lrx) = mpsc::channel::<&'static str>();
    let mk = |ctx: &'static mut WebContext, tag: &'static str, x: f64, tx: Sender<&'static str>| {
        WebViewBuilder::new_with_web_context(ctx)
            .with_initialization_script(SNAPSHOT_JS)
            .with_url(&url)
            .with_bounds(Rect { position: LogicalPosition::new(x, 0.0).into(), size: LogicalSize::new(590.0, 660.0).into() })
            .with_on_page_load_handler(move |ev, _| {
                if matches!(ev, wry::PageLoadEvent::Finished) {
                    let _ = tx.send(tag);
                }
            })
            .build_as_child(window)
            .expect("webview")
    };
    let mut views: Views = HashMap::new();
    views.insert("A", mk(ctx_a, "A", 0.0, ltx.clone()));
    views.insert("B", mk(ctx_b, "B", 600.0, ltx));

    let p2 = proxy.clone();
    std::thread::spawn(move || run_tests(p2, port, lrx, out));

    event_loop.run(move |ev, _, flow| {
        *flow = ControlFlow::Wait;
        match ev {
            Event::UserEvent(Ev::Run(f)) => f(&views),
            Event::UserEvent(Ev::Quit) => *flow = ControlFlow::Exit,
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

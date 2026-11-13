//! Test S4 dengan keychain OS asli + `opencode serve` 1.19.5 asli + mock provider lokal.
//! Laporan temuan ditulis ke `target/s4-report.json`.
//! Jalankan: `cargo test -p zandor-spike-s4 -- --test-threads=1 --nocapture`

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zandor_engine_supervisor::{http_json, Engine, EngineConfig};
use zandor_spike_s4::*;

const TEST_SERVICE: &str = "id.web.zandor.spike-s4-test";

fn opencode_bin() -> PathBuf {
    let exe = if cfg!(windows) { "opencode.exe" } else { "opencode" };
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/debug").join(exe);
    assert!(p.exists(), "jalankan `cargo build -p zandor-spike-s1` dulu (menyalin sidecar): {p:?}");
    p
}

fn temp_dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("zandor-s4-{tag}-{}", random_secret("")));
    std::fs::create_dir_all(&p).unwrap();
    // Path panjang (bukan 8.3) agar sama dengan yang dilaporkan engine.
    std::fs::canonicalize(&p).map(strip_verbatim).unwrap_or(p)
}

fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy().to_string();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s))
}

/// Direktori XDG terisolasi → config/data opencode milik user tidak tersentuh.
struct Sandbox {
    ws: PathBuf,
    xdg: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        Self { ws: temp_dir("ws"), xdg: temp_dir("xdg") }
    }
    fn env(&self) -> Vec<(String, String)> {
        ["CONFIG", "DATA", "CACHE", "STATE"]
            .iter()
            .map(|k| (format!("XDG_{k}_HOME"), self.xdg.join(k.to_lowercase()).to_string_lossy().to_string()))
            .collect()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.ws);
        let _ = std::fs::remove_dir_all(&self.xdg);
    }
}

/// Cari secret di semua file (rekursif). Mengembalikan path relatif yang memuatnya.
fn files_containing(root: &Path, needle: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(bytes) = std::fs::read(&p) {
                let hay = String::from_utf8_lossy(&bytes);
                // juga cek UTF-16LE (file Windows tertentu)
                let utf16: Vec<u8> = needle.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
                if hay.contains(needle) || bytes.windows(utf16.len()).any(|w| w == utf16.as_slice()) {
                    hits.push(p.strip_prefix(root).unwrap_or(&p).to_string_lossy().to_string());
                }
            }
        }
    }
    hits
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// (path, header Authorization, body)
type MockCall = (String, Option<String>, String);
#[derive(Clone, Default)]
struct MockLog(Arc<Mutex<Vec<MockCall>>>);

/// Mock provider OpenAI-compatible: mencatat (path, header Authorization), menjawab "halo dari mock".
fn start_mock() -> (String, MockLog) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", l.local_addr().unwrap());
    let log = MockLog::default();
    let log2 = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let log = log2.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut first = String::new();
                if r.read_line(&mut first).is_err() {
                    return;
                }
                let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
                let (mut len, mut auth) = (0usize, None);
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
                        break;
                    }
                    let (k, v) = h.split_once(':').unwrap_or(("", ""));
                    match k.trim().to_ascii_lowercase().as_str() {
                        "content-length" => len = v.trim().parse().unwrap_or(0),
                        "authorization" => auth = Some(v.trim().to_string()),
                        _ => {}
                    }
                }
                let mut body = vec![0; len];
                let _ = r.read_exact(&mut body);
                let body = String::from_utf8_lossy(&body).to_string();
                log.0.lock().unwrap().push((path.clone(), auth, body.clone()));
                let mut s = s;
                if !path.ends_with("/chat/completions") {
                    let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    return;
                }
                let chunk = |delta: &str, finish: &str| {
                    format!(
                        "data: {{\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"m1\",\"choices\":[{{\"index\":0,\"delta\":{delta},\"finish_reason\":{finish}}}]}}\n\n"
                    )
                };
                let wants_tool = body.contains("ENVTEST") && body.contains("\"tools\"") && !body.contains("\"role\":\"tool\"");
                if wants_tool {
                    // Minta agent menjalankan tool bash yang mencetak seluruh env prosesnya.
                    let args = serde_json::json!({ "command": "node -p \"JSON.stringify(process.env)\"", "description": "cetak env" }).to_string();
                    let tc = serde_json::json!({ "role": "assistant", "tool_calls": [{ "index": 0, "id": "call_env", "type": "function", "function": { "name": "bash", "arguments": args } }] });
                    let mut out = String::from("HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: close

");
                    out += &chunk(&tc.to_string(), "null");
                    out += &chunk("{}", "\"tool_calls\"");
                    out += "data: [DONE]

";
                    let _ = s.write_all(out.as_bytes());
                } else if body.contains("\"stream\":true") {
                    let mut out = String::from("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n");
                    out += &chunk(r#"{"role":"assistant","content":"halo dari mock"}"#, "null");
                    out += &chunk("{}", "\"stop\"");
                    out += "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"m1\",\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":3,\"total_tokens\":8}}\n\n";
                    out += "data: [DONE]\n\n";
                    let _ = s.write_all(out.as_bytes());
                } else {
                    let j = r#"{"id":"c1","object":"chat.completion","created":0,"model":"m1","choices":[{"index":0,"message":{"role":"assistant","content":"halo dari mock"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}"#;
                    let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{j}", j.len()).as_bytes());
                }
            });
        }
    });
    (base, log)
}

fn report(key: &str, value: serde_json::Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/s4-report.json");
    let mut m: BTreeMap<String, serde_json::Value> =
        std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    m.insert(key.into(), value);
    std::fs::write(&path, serde_json::to_string_pretty(&m).unwrap()).unwrap();
}

/// Membersihkan entri keychain test walau test gagal.
struct KeychainGuard(SecretStore, String);
impl Drop for KeychainGuard {
    fn drop(&mut self) {
        let _ = self.0.delete(&self.1);
    }
}

#[test]
fn keychain_roundtrip() {
    let store = SecretStore::new(TEST_SERVICE);
    let acct = format!("provider:roundtrip-{}", random_secret(""));
    let _g = KeychainGuard(SecretStore::new(TEST_SERVICE), acct.clone());
    assert_eq!(store.get(&acct).unwrap(), None);
    store.set(&acct, "sk-satu").unwrap();
    assert_eq!(store.get(&acct).unwrap().as_deref(), Some("sk-satu"));
    store.set(&acct, "sk-dua ✓ ünïcödé").unwrap(); // timpa + unicode
    assert_eq!(store.get(&acct).unwrap().as_deref(), Some("sk-dua ✓ ünïcödé"));
    // Token OAuth/JWT bisa melebihi batas Windows (2560 byte) → dipecah.
    let long: String = (0..5000).map(|i| if i % 97 == 0 { '😀' } else { 'x' }).collect();
    store.set(&acct, &long).unwrap();
    assert_eq!(store.get(&acct).unwrap().as_deref(), Some(long.as_str()));
    let raw = |a: &str| keyring::Entry::new(TEST_SERVICE, a).unwrap().get_password().ok();
    assert!(raw(&acct).unwrap().starts_with("zandor-chunked:v1:"));
    assert!(raw(&format!("{acct}#3")).is_some());
    store.set(&acct, "pendek").unwrap(); // kembali pendek → potongan lama dihapus
    assert_eq!(store.get(&acct).unwrap().as_deref(), Some("pendek"));
    assert!(raw(&format!("{acct}#1")).is_none(), "potongan lama tersisa");
    store.set(&acct, &long).unwrap();
    assert!(store.delete(&acct).unwrap());
    assert!(!store.delete(&acct).unwrap());
    assert_eq!(store.get(&acct).unwrap(), None);
}

#[test]
/// Kontrol: tanpa plugin guard, bahaya pewarisan env memang terjadi.
fn e2e_without_env_guard() {
    let r = run_e2e(false);
    report("e2e_without_guard", r.clone());
    for k in ["pty_inherits_secret", "pty_inherits_server_password", "bash_tool_output_has_secret", "secret_sent_to_model"] {
        assert_eq!(r[k], true, "kontrol: {k} diharapkan true tanpa guard: {r}");
    }
    assert!(!r["files_with_secret_opencode_dirs"].as_array().unwrap().is_empty(), "kontrol: secret diharapkan tersimpan di DB lewat output tool");
}

#[test]
fn e2e_with_env_guard_plugin() {
    let r = run_e2e(true);
    report("e2e_with_guard", r.clone());
    for k in ["pty_inherits_secret", "pty_inherits_server_password", "pty_inherits_config_content", "bash_tool_output_has_secret", "bash_tool_output_has_server_password", "secret_sent_to_model"] {
        assert_eq!(r[k], false, "{k} harus false dengan guard: {r}");
    }
    assert!(r["files_with_secret_ws"].as_array().unwrap().is_empty() && r["files_with_secret_opencode_dirs"].as_array().unwrap().is_empty(), "secret tertulis ke file: {r}");
}

fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

fn run_e2e(guard: bool) -> serde_json::Value {
    let store = SecretStore::new(TEST_SERVICE);
    let pid = "zmock";
    let secret = random_secret("sk-zandor-test-");
    store.set(&provider_account(pid), &secret).unwrap();
    let _g = KeychainGuard(SecretStore::new(TEST_SERVICE), provider_account(pid));

    let (base_url, mock) = start_mock();
    let sb = Sandbox::new();
    let providers = [CustomProvider { id: pid, name: "Zandor Mock", base_url: &base_url, models: &["m1"] }];
    let mut cfg = EngineConfig::new(opencode_bin());
    cfg.cwd = Some(sb.ws.clone());
    cfg.env = engine_env(&store, &providers).unwrap();
    cfg.env.extend(sb.env());
    // Model kecil (judul sesi) juga diarahkan ke mock agar tidak ada panggilan keluar.
    cfg.env.retain(|(k, _)| k != "OPENCODE_CONFIG_CONTENT");
    let mut content = serde_json::json!({ "provider": { pid: provider_config(&providers[0]) }, "model": "zmock/m1", "small_model": "zmock/m1", "permission": { "bash": "allow" } });
    if guard {
        let plugin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugin/zandor-env-guard.js");
        content["plugin"] = serde_json::json!([file_url(&plugin)]);
    }
    content["$schema"] = "https://opencode.ai/config.json".into();
    cfg.env.push(("OPENCODE_CONFIG_CONTENT".into(), content.to_string()));
    assert!(!content.to_string().contains(&secret), "config tidak boleh memuat secret");

    let mut engine = Engine::start(cfg, |_| {}).expect("start engine");
    let info = engine.info().clone();
    let auth = info.basic_auth_header();
    let q = format!("directory={}", enc(&sb.ws.to_string_lossy()));
    let call = |m: &str, path: &str, body: Option<&str>| {
        let sep = if path.contains('?') { '&' } else { '?' };
        http_json(&info.host_port, m, &format!("{path}{sep}{q}"), Some(&auth), body, Duration::from_secs(90)).unwrap()
    };

    // Isolasi XDG bekerja? (DB opencode harus dibuat di sandbox)
    let isolated = sb.xdg.join("data").join("opencode").exists();

    // 1) Provider terdaftar
    let (c, providers_body) = call("GET", "/config/providers", None);
    assert_eq!(c, 200);
    assert!(providers_body.contains("\"zmock\""), "provider zmock tidak terdaftar: {}", &providers_body[..providers_body.len().min(400)]);

    // 2) Chat nyata lewat engine → mock menerima Bearer <secret dari keychain>
    let (c, s) = call("POST", "/session", Some("{}"));
    assert_eq!(c, 200, "{s}");
    let sid = s.split("\"id\":\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    let (c, reply) = call(
        "POST",
        &format!("/session/{sid}/message"),
        Some(r#"{"model":{"providerID":"zmock","modelID":"m1"},"parts":[{"type":"text","text":"halo"}]}"#),
    );
    assert_eq!(c, 200, "{reply}");
    assert!(reply.contains("halo dari mock"), "jawaban mock tidak sampai: {}", &reply[..reply.len().min(600)]);
    let expected = format!("Bearer {secret}");
    {
        let calls = mock.0.lock().unwrap();
        assert!(!calls.is_empty(), "mock tidak dipanggil");
        assert!(calls.iter().all(|(_, a, _)| a.as_deref() == Some(expected.as_str())), "header auth salah");
    }

    // 2b) Tool bash milik agent (dipicu mock): apakah env rahasia terlihat di output tool?
    let (c, reply2) = call(
        "POST",
        &format!("/session/{sid}/message"),
        Some(r#"{"model":{"providerID":"zmock","modelID":"m1"},"parts":[{"type":"text","text":"ENVTEST jalankan perintah"}]}"#),
    );
    assert_eq!(c, 200, "{reply2}");
    let (_, msgs) = call("GET", &format!("/session/{sid}/message"), None);
    let bash_ran = msgs.contains("\"tool\":\"bash\"") && msgs.contains("call_env");
    let bash_output_has_secret = msgs.contains(&secret);
    let bash_output_has_pw = msgs.contains(&info.password);
    let calls = mock.0.lock().unwrap().clone();
    let secret_sent_to_model = calls.iter().any(|(_, _, b)| b.contains(&secret));

    // 3) Apakah secret terlihat lewat API engine?
    let api_leaks: Vec<&str> = ["/config", "/config/providers", "/provider"]
        .into_iter()
        .filter(|p| call("GET", p, None).1.contains(&secret))
        .collect();

    // 4) Apakah proses anak engine (PTY/shell tool) mewarisi env berisi secret?
    let dump = sb.ws.join("env-dump.txt");
    let body = if cfg!(windows) {
        r#"{"command":"cmd.exe","args":["/c","set > env-dump.txt"]}"#
    } else {
        r#"{"command":"sh","args":["-c","env > env-dump.txt"]}"#
    };
    let (c, _) = call("POST", "/pty", Some(body));
    assert_eq!(c, 200);
    let mut inherited = None;
    for _ in 0..40 {
        if let Ok(t) = std::fs::read_to_string(&dump) {
            if t.len() > 10 {
                inherited = Some((t.contains(&secret), t.contains(&info.password), t.contains("\"zmock\"")));
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let _ = std::fs::remove_file(&dump); // berisi secret bila diwariskan

    engine.stop();
    std::thread::sleep(Duration::from_millis(300));

    // 5) Tidak ada file berisi secret di workspace & seluruh direktori data/config/cache opencode
    let ws_hits = files_containing(&sb.ws, &secret);
    let xdg_hits = files_containing(&sb.xdg, &secret);
    let (inh_secret, inh_pw, inh_cfg) = inherited.unwrap_or((false, false, false));
    let r = serde_json::json!({
        "xdg_isolation_works": isolated,
        "guard_plugin": guard,
        "mock_calls": calls.len(),
        "bash_tool_ran": bash_ran,
        "bash_tool_output_has_secret": bash_output_has_secret,
        "bash_tool_output_has_server_password": bash_output_has_pw,
        "secret_sent_to_model": secret_sent_to_model,
        "auth_header_matches_keychain": true,
        "secret_visible_via_api": api_leaks,
        "pty_inherits_secret": inh_secret,
        "pty_inherits_server_password": inh_pw,
        "pty_inherits_config_content": inh_cfg,
        "env_dump_captured": inherited.is_some(),
        "files_with_secret_ws": ws_hits,
        "files_with_secret_opencode_dirs": xdg_hits,
    });
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
    assert!(isolated, "XDG_DATA_HOME tidak dihormati; test file-scan tidak valid");
    assert!(bash_ran, "tool bash tidak dijalankan agent");
    assert!(inherited.is_some(), "dump env PTY tidak tertangkap");
    r
}

/// `OPENCODE_AUTH_CONTENT` membuat opencode membaca kredensial dari env. Apakah operasi
/// tulis auth (login / PUT /auth) menuliskan kredensial dari env itu ke auth.json?
#[test]
fn auth_content_env_and_auth_set_leak() {
    let secret = random_secret("sk-zandor-authcontent-");
    let sb = Sandbox::new();
    let mut cfg = EngineConfig::new(opencode_bin());
    cfg.cwd = Some(sb.ws.clone());
    cfg.env = sb.env();
    cfg.env.push((
        "OPENCODE_AUTH_CONTENT".into(),
        serde_json::json!({ "anthropic": { "type": "api", "key": secret } }).to_string(),
    ));
    let mut engine = Engine::start(cfg, |_| {}).expect("start");
    let info = engine.info().clone();
    let auth = info.basic_auth_header();

    let (_, provs) = http_json(&info.host_port, "GET", "/provider", Some(&auth), None, Duration::from_secs(20)).unwrap();
    let anthropic_connected = provs.contains("\"connected\"") && provs.contains("anthropic");
    let before = files_containing(&sb.xdg, &secret);

    // Simulasi user login provider lain lewat jalur resmi opencode.
    let (c, b) = http_json(
        &info.host_port,
        "PUT",
        "/auth/openai",
        Some(&auth),
        Some(r#"{"type":"api","key":"sk-dummy-bukan-rahasia"}"#),
        Duration::from_secs(10),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let after = files_containing(&sb.xdg, &secret);
    engine.stop();

    let r = serde_json::json!({
        "put_auth_status": c,
        "put_auth_body": b.chars().take(120).collect::<String>(),
        "provider_list_mentions_anthropic": anthropic_connected,
        "files_with_env_secret_before_auth_set": before,
        "files_with_env_secret_after_auth_set": after,
    });
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
    report("auth_content_env", r);
    assert!(before.is_empty(), "secret dari env tertulis ke file bahkan sebelum auth.set: {before:?}");
}

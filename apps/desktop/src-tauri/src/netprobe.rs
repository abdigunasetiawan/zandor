// Probe jaringan untuk provider (Fase 6): ambil daftar model dari endpoint OpenAI-compatible, ping satu model, dan
// deteksi server lokal (Ollama, LM Studio). Dijalankan di Rust agar kunci API dari keychain tidak pernah menyentuh
// JavaScript. Hanya http/https; waktu tunggu selalu terbatas supaya status provider tidak berputar tanpa akhir.
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const MAX_ERROR_CHARS: usize = 300;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub models: Vec<String>,
    /// "ok" | "auth" | "notfound" | "ratelimit" | "server" | "network" | "invalid"
    pub kind: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalFound {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub models: Vec<String>,
}

fn agent(timeout: Duration) -> ureq::Agent {
    // Hanya fitur `native-tls` yang aktif: penyedia TLS dipilih eksplisit. Tanpa ini ureq memakai Rustls dan panik pada
    // endpoint https (provider cloud); endpoint http lokal tidak terpengaruh.
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

fn clip(s: &str) -> String {
    let t: String = s.chars().take(MAX_ERROR_CHARS).collect();
    t.trim().to_string()
}

fn classify(status: u16) -> &'static str {
    match status {
        200..=299 => "ok",
        401 | 403 => "auth",
        404 => "notfound",
        429 => "ratelimit",
        500..=599 => "server",
        _ => "invalid",
    }
}

fn explain(kind: &str, status: u16, body: &str) -> String {
    let base = match kind {
        "auth" => "kunci API ditolak",
        "notfound" => "endpoint atau model tidak ditemukan",
        "ratelimit" => "terkena batas laju (rate limit) atau kuota",
        "server" => "server provider bermasalah",
        _ => "respons tidak terduga",
    };
    let detail = clip(body);
    if detail.is_empty() {
        format!("{base} (HTTP {status})")
    } else {
        format!("{base} (HTTP {status}): {detail}")
    }
}

fn valid_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err("URL harus diawali http:// atau https://".into())
    }
}

fn failure(kind: &str, started: Instant, error: String) -> ProbeResult {
    ProbeResult {
        ok: false,
        status: None,
        latency_ms: started.elapsed().as_millis() as u64,
        models: vec![],
        kind: kind.into(),
        error: Some(error),
    }
}

/// Ambil `GET {base}/models`. Mengerti bentuk OpenAI (`{data:[{id}]}`) dan array polos.
pub fn probe(base_url: &str, api_key: Option<&str>, timeout: Duration) -> ProbeResult {
    let started = Instant::now();
    if let Err(e) = valid_url(base_url) {
        return failure("invalid", started, e);
    }
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = agent(timeout).get(&url);
    if let Some(k) = api_key.filter(|k| !k.is_empty()) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = match req.call() {
        Ok(r) => r,
        Err(e) => return failure("network", started, format!("tidak bisa terhubung: {e}")),
    };
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    let kind = classify(status);
    if kind != "ok" {
        return ProbeResult {
            ok: false,
            status: Some(status),
            latency_ms: started.elapsed().as_millis() as u64,
            models: vec![],
            kind: kind.into(),
            error: Some(explain(kind, status, &body)),
        };
    }
    let models = parse_models(&body);
    ProbeResult {
        ok: true,
        status: Some(status),
        latency_ms: started.elapsed().as_millis() as u64,
        error: if models.is_empty() {
            Some("endpoint merespons tetapi tidak mengembalikan daftar model".into())
        } else {
            None
        },
        models,
        kind: "ok".into(),
    }
}

pub fn parse_models(body: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return vec![];
    };
    let arr = v
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| v.as_array());
    let mut out: Vec<String> = arr
        .map(|a| {
            a.iter()
                .filter_map(|m| m.get("id").and_then(Value::as_str).or_else(|| m.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

/// Kirim satu permintaan chat 1 token ke `model` untuk mengukur latensi dan kesehatan model.
pub fn ping_model(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    timeout: Duration,
) -> ProbeResult {
    let started = Instant::now();
    if let Err(e) = valid_url(base_url) {
        return failure("invalid", started, e);
    }
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": [{ "role": "user", "content": "ping" }],
        "max_tokens": 1,
        "stream": false,
    })
    .to_string();
    let mut req = agent(timeout)
        .post(&url)
        .header("content-type", "application/json");
    if let Some(k) = api_key.filter(|k| !k.is_empty()) {
        req = req.header("Authorization", &format!("Bearer {k}"));
    }
    let mut resp = match req.send(body) {
        Ok(r) => r,
        Err(e) => return failure("network", started, format!("tidak bisa terhubung: {e}")),
    };
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let kind = classify(status);
    ProbeResult {
        ok: kind == "ok",
        status: Some(status),
        latency_ms: started.elapsed().as_millis() as u64,
        models: vec![],
        kind: kind.into(),
        error: if kind == "ok" {
            None
        } else {
            Some(explain(kind, status, &text))
        },
    }
}

const LOCAL_CANDIDATES: [(&str, &str, &str); 2] = [
    ("ollama", "Ollama", "http://localhost:11434/v1"),
    ("lmstudio", "LM Studio", "http://localhost:1234/v1"),
];

/// Cari server lokal yang berjalan. Kandidat dapat diganti (untuk test).
pub fn detect_local_with(candidates: &[(&str, &str, &str)], timeout: Duration) -> Vec<LocalFound> {
    candidates
        .iter()
        .filter_map(|(id, name, url)| {
            let r = probe(url, None, timeout);
            (r.ok && !r.models.is_empty()).then(|| LocalFound {
                id: (*id).into(),
                name: (*name).into(),
                base_url: (*url).into(),
                models: r.models,
            })
        })
        .collect()
}

pub fn detect_local() -> Vec<LocalFound> {
    detect_local_with(&LOCAL_CANDIDATES, Duration::from_millis(800))
}

// ---------- uji model provider katalog lewat metadata engine ----------
//
// Provider katalog (mis. opencode, anthropic, openrouter) tidak punya base URL di registry kita, tetapi engine opencode
// menyebutkannya per model (`GET /config/providers` → `models[id].api.{url,npm}`) dan sering juga kuncinya. Rust mengambil
// metadata itu sendiri (kredensial engine + kunci TIDAK pernah dikirim ke JavaScript) lalu mengirim satu permintaan 1 token.

#[derive(Clone, Debug, PartialEq)]
pub struct EngineTarget {
    pub url: String,
    /// Adapter AI SDK model ini (mis. `@ai-sdk/openai-compatible`, `@ai-sdk/anthropic`).
    pub npm: String,
    pub key: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Adapter {
    OpenAiCompatible,
    Anthropic,
}

/// Adapter yang bisa diuji. Selain ini dilaporkan jujur sebagai belum didukung.
pub fn adapter_of(npm: &str) -> Option<Adapter> {
    match npm {
        "@ai-sdk/anthropic" => Some(Adapter::Anthropic),
        "@ai-sdk/openai-compatible"
        | "@ai-sdk/openai"
        | "@ai-sdk/xai"
        | "@ai-sdk/groq"
        | "@ai-sdk/deepseek"
        | "@ai-sdk/mistral"
        | "@ai-sdk/togetherai"
        | "@ai-sdk/fireworks"
        | "@ai-sdk/cerebras"
        | "@ai-sdk/deepinfra"
        | "@ai-sdk/perplexity"
        | "@openrouter/ai-sdk-provider" => Some(Adapter::OpenAiCompatible),
        _ => None,
    }
}

/// Turunkan target dari body `GET /config/providers` (fungsi murni agar mudah diuji).
pub fn parse_engine_target(
    body: &str,
    provider: &str,
    model: &str,
) -> Result<EngineTarget, String> {
    let v: Value =
        serde_json::from_str(body).map_err(|e| format!("respons engine bukan JSON: {e}"))?;
    let providers = v
        .get("providers")
        .and_then(Value::as_array)
        .ok_or("respons engine tidak berisi daftar provider")?;
    let p = providers
        .iter()
        .find(|p| p.get("id").and_then(Value::as_str) == Some(provider))
        .ok_or_else(|| format!("provider {provider} tidak ada di engine"))?;
    let m = p
        .get("models")
        .and_then(|m| m.get(model))
        .ok_or_else(|| format!("model {model} tidak ada di provider {provider}"))?;
    let url = m
        .get("api")
        .and_then(|a| a.get("url"))
        .and_then(Value::as_str)
        .filter(|u| !u.is_empty())
        .ok_or("engine tidak menyebutkan endpoint API model ini")?
        .to_string();
    let npm = m
        .get("api")
        .and_then(|a| a.get("npm"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let key = p
        .get("key")
        .and_then(Value::as_str)
        .filter(|k| !k.is_empty())
        .map(str::to_string);
    Ok(EngineTarget { url, npm, key })
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 baku (dipakai untuk header Basic auth engine). Tanpa dependensi tambahan.
pub fn base64(input: &[u8]) -> String {
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Ambil target ping dari engine yang berjalan (Basic auth). Galat dikembalikan sebagai `ProbeResult` gagal.
pub fn engine_target(
    engine_url: &str,
    user: &str,
    pass: &str,
    provider: &str,
    model: &str,
    timeout: Duration,
) -> Result<EngineTarget, ProbeResult> {
    let started = Instant::now();
    let url = format!("{}/config/providers", engine_url.trim_end_matches('/'));
    let auth = format!("Basic {}", base64(format!("{user}:{pass}").as_bytes()));
    let mut resp = agent(timeout)
        .get(&url)
        .header("Authorization", &auth)
        .call()
        .map_err(|e| failure("network", started, format!("engine tidak terjangkau: {e}")))?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(failure(
            "server",
            started,
            format!("engine menjawab HTTP {status}"),
        ));
    }
    parse_engine_target(&body, provider, model).map_err(|e| failure("invalid", started, e))
}

/// Kirim satu permintaan 1 token sesuai adapter. Adapter yang belum didukung dilaporkan apa adanya.
pub fn ping_target(target: &EngineTarget, model: &str, timeout: Duration) -> ProbeResult {
    match adapter_of(&target.npm) {
        Some(Adapter::OpenAiCompatible) => ping_model(&target.url, target.key.as_deref(), model, timeout),
        Some(Adapter::Anthropic) => ping_anthropic(&target.url, target.key.as_deref(), model, timeout),
        None => failure(
            "invalid",
            Instant::now(),
            format!(
                "uji koneksi belum mendukung adapter {} (hanya format OpenAI-compatible dan Anthropic)",
                if target.npm.is_empty() { "(tidak diketahui)" } else { &target.npm }
            ),
        ),
    }
}

/// Permintaan 1 token ke API pesan Anthropic (`POST {url}/messages`).
pub fn ping_anthropic(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    timeout: Duration,
) -> ProbeResult {
    let started = Instant::now();
    if let Err(e) = valid_url(base_url) {
        return failure("invalid", started, e);
    }
    let url = format!("{}/messages", base_url.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "max_tokens": 1,
        "messages": [{ "role": "user", "content": "ping" }],
    })
    .to_string();
    let mut req = agent(timeout)
        .post(&url)
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01");
    if let Some(k) = api_key.filter(|k| !k.is_empty()) {
        req = req.header("x-api-key", k);
    }
    let mut resp = match req.send(body) {
        Ok(r) => r,
        Err(e) => return failure("network", started, format!("tidak bisa terhubung: {e}")),
    };
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let kind = classify(status);
    ProbeResult {
        ok: kind == "ok",
        status: Some(status),
        latency_ms: started.elapsed().as_millis() as u64,
        models: vec![],
        kind: kind.into(),
        error: if kind == "ok" {
            None
        } else {
            Some(explain(kind, status, &text))
        },
    }
}

/// Hasil gagal dengan pesan (untuk pemanggil di luar modul ini).
pub fn failed(kind: &str, message: String) -> ProbeResult {
    failure(kind, Instant::now(), message)
}

#[cfg(test)]
mod tests {
    /// Jaringan nyata: `cargo test --lib netprobe -- --ignored`. Endpoint https tidak boleh panik.
    #[test]
    #[ignore = "butuh internet"]
    fn https_endpoint_is_probed_without_panicking() {
        let r = probe("https://open-vsx.org/api", None, Duration::from_secs(20));
        assert!(r.status.is_some(), "tidak ada status HTTP: {:?}", r.error);
    }

    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Server HTTP sangat sederhana: membalas `responses` secara berurutan (satu per koneksi) dan merekam permintaan.
    fn serve(responses: Vec<(u16, &'static str)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut s, _)) = listener.accept() else {
                    return;
                };
                // Baca header sampai selesai, lalu sisa body sesuai Content-Length (klien menulis keduanya terpisah).
                let mut data: Vec<u8> = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = s.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&data).into_owned();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let want = text[..end]
                            .to_lowercase()
                            .lines()
                            .find_map(|l| {
                                l.strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if data.len() >= end + 4 + want {
                            break;
                        }
                    }
                }
                seen2
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&data).into_owned());
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(reply.as_bytes());
            }
        });
        (format!("http://{addr}/v1"), seen)
    }

    const T: Duration = Duration::from_secs(5);

    #[test]
    fn probe_lists_models_and_sends_the_bearer_key() {
        let (url, seen) = serve(vec![(
            200,
            r#"{"data":[{"id":"b"},{"id":"a"},{"id":"a"}]}"#,
        )]);
        let r = probe(&url, Some("sk-uji"), T);
        assert!(r.ok, "{r:?}");
        assert_eq!(r.models, vec!["a", "b"]);
        assert_eq!(r.kind, "ok");
        let req = seen.lock().unwrap()[0].to_lowercase();
        assert!(req.starts_with("get /v1/models"));
        assert!(req.contains("authorization: bearer sk-uji"));
    }

    #[test]
    fn probe_classifies_auth_ratelimit_and_server_errors_honestly() {
        for (status, kind) in [
            (401, "auth"),
            (429, "ratelimit"),
            (500, "server"),
            (404, "notfound"),
        ] {
            let (url, _) = serve(vec![(status, r#"{"error":"detail dari server"}"#)]);
            let r = probe(&url, None, T);
            assert!(!r.ok);
            assert_eq!(r.kind, kind);
            assert_eq!(r.status, Some(status));
            assert!(r.error.unwrap().contains("detail dari server"));
        }
    }

    #[test]
    fn probe_reports_unreachable_and_bad_scheme_without_hanging() {
        let r = probe("http://127.0.0.1:9/v1", None, Duration::from_millis(800));
        assert!(!r.ok);
        assert_eq!(r.kind, "network");
        assert_eq!(probe("ftp://x", None, T).kind, "invalid");
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"opencode:rahasia"), "b3BlbmNvZGU6cmFoYXNpYQ==");
    }

    #[test]
    fn adapters_are_recognised_and_unknown_is_reported() {
        assert_eq!(
            adapter_of("@ai-sdk/openai-compatible"),
            Some(Adapter::OpenAiCompatible)
        );
        assert_eq!(adapter_of("@ai-sdk/anthropic"), Some(Adapter::Anthropic));
        assert_eq!(adapter_of("@ai-sdk/google"), None);
        let t = EngineTarget {
            url: "http://x/v1".into(),
            npm: "@ai-sdk/google".into(),
            key: None,
        };
        let r = ping_target(&t, "m", Duration::from_secs(1));
        assert!(!r.ok);
        assert_eq!(r.kind, "invalid");
        assert!(r.error.unwrap().contains("@ai-sdk/google"));
    }

    #[test]
    fn engine_target_is_derived_from_config_providers() {
        let body = r#"{"providers":[
            {"id":"opencode","key":"public","models":{"big":{"api":{"id":"big","url":"https://opencode.ai/zen/v1","npm":"@ai-sdk/openai-compatible"}}}},
            {"id":"anthropic","models":{"c":{"api":{"id":"c","url":"https://api.anthropic.com/v1","npm":"@ai-sdk/anthropic"}}}}
        ],"default":{}}"#;
        let t = parse_engine_target(body, "opencode", "big").unwrap();
        assert_eq!(t.url, "https://opencode.ai/zen/v1");
        assert_eq!(t.npm, "@ai-sdk/openai-compatible");
        assert_eq!(t.key.as_deref(), Some("public"));
        let a = parse_engine_target(body, "anthropic", "c").unwrap();
        assert_eq!(a.key, None);
        assert!(parse_engine_target(body, "zzz", "c")
            .unwrap_err()
            .contains("zzz"));
        assert!(parse_engine_target(body, "opencode", "hilang")
            .unwrap_err()
            .contains("hilang"));
        assert!(parse_engine_target("bukan json", "a", "b").is_err());
    }

    #[test]
    fn anthropic_ping_sends_key_and_version_headers() {
        let (base, seen) = serve(vec![(200, "{}")]);
        let r = ping_anthropic(&base, Some("kunci-uji"), "claude-x", Duration::from_secs(5));
        assert!(r.ok, "{:?}", r.error);
        let req = seen.lock().unwrap()[0].to_lowercase();
        assert!(req.contains("post /v1/messages"), "{req}");
        assert!(req.contains("x-api-key: kunci-uji"), "{req}");
        assert!(req.contains("anthropic-version: 2023-06-01"), "{req}");
        assert!(req.contains("\"max_tokens\":1"), "{req}");
    }

    #[test]
    fn engine_target_fetch_uses_basic_auth() {
        let body: &'static str = r#"{"providers":[{"id":"p","models":{"m":{"api":{"url":"http://e/v1","npm":"@ai-sdk/openai-compatible"}}}}]}"#;
        let (base, seen) = serve(vec![(200, body)]);
        let t = engine_target(
            &base,
            "opencode",
            "rahasia",
            "p",
            "m",
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(t.url, "http://e/v1");
        let req = seen.lock().unwrap()[0].to_lowercase();
        assert!(req.contains("get /v1/config/providers"), "{req}");
        assert!(
            req.contains("authorization: basic b3blbmnvzgu6cmfoyxnpyq=="),
            "{req}"
        );
    }

    #[test]
    fn ping_posts_a_one_token_chat_request() {
        let (url, seen) = serve(vec![(200, r#"{"choices":[]}"#)]);
        let r = ping_model(&url, Some("k"), "model-x", T);
        assert!(r.ok, "{r:?}");
        let req = seen.lock().unwrap()[0].clone();
        assert!(req.starts_with("POST /v1/chat/completions"));
        assert!(req.contains(r#""model":"model-x""#));
        assert!(req.contains(r#""max_tokens":1"#));
    }

    #[test]
    fn ping_surfaces_rate_limit() {
        let (url, _) = serve(vec![(429, "kuota habis")]);
        let r = ping_model(&url, None, "m", T);
        assert_eq!(r.kind, "ratelimit");
        assert!(r.error.unwrap().contains("kuota habis"));
    }

    #[test]
    fn detect_local_finds_only_servers_that_answer_with_models() {
        let (up, _) = serve(vec![(200, r#"{"data":[{"id":"llama3"}]}"#)]);
        let up_static: &'static str = Box::leak(up.into_boxed_str());
        let found = detect_local_with(
            &[
                ("ollama", "Ollama", up_static),
                ("lmstudio", "LM Studio", "http://127.0.0.1:9/v1"),
            ],
            Duration::from_millis(800),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "ollama");
        assert_eq!(found[0].models, vec!["llama3"]);
    }

    #[test]
    fn hanging_server_ends_with_a_timeout_error_not_a_hang() {
        // Server yang menerima koneksi tetapi tidak pernah membalas (mirip proxy yang macet).
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let held: Vec<_> = listener.incoming().take(2).filter_map(Result::ok).collect();
            std::thread::sleep(Duration::from_secs(10));
            drop(held);
        });
        let url = format!("http://{addr}/v1");
        let t0 = Instant::now();
        let r = ping_model(&url, None, "m", Duration::from_millis(700));
        assert!(!r.ok);
        assert_eq!(r.kind, "network");
        assert!(
            r.error.unwrap().contains("timeout"),
            "pesan harus menyebut timeout"
        );
        assert!(
            t0.elapsed() < Duration::from_secs(4),
            "harus berakhir mendekati batas waktu"
        );
        let p = probe(&url, None, Duration::from_millis(700));
        assert_eq!(p.kind, "network");
    }

    #[test]
    fn parse_models_accepts_plain_arrays_and_garbage() {
        assert_eq!(parse_models(r#"["x","y"]"#), vec!["x", "y"]);
        assert!(parse_models("bukan json").is_empty());
        assert!(parse_models(r#"{"data":"salah"}"#).is_empty());
    }
}

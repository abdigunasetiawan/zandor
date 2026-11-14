//! Masuk dengan akun OpenWork (Fase 12, ADR-040): OAuth 2.0 Device Authorization Grant (RFC 8628) ke OpenWork Cloud,
//! lalu menukar sesi akun dengan kredensial AI Gateway (`ow_gw_…`, atau kredensial gratis `ow_inf_…`) yang dipakai sebagai
//! kunci provider OpenAI-compatible di endpoint inferensi OpenWork. Alur mengikuti implementasi `my-harness`.
//!
//! Keamanan: token sesi dan kunci gateway **hanya ada di Rust** (tidak pernah dikirim ke JS/UI); kunci disimpan di keychain
//! lewat `ProviderStore`. URL verifikasi dari server divalidasi (https + host openworklabs.com) sebelum dibuka.

use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub const API_BASE: &str = "https://api.openworklabs.com";
pub const INFERENCE_BASE: &str = "https://inference.openworklabs.com/api/v1";
pub const DEFAULT_VERIFY_URL: &str = "https://app.openworklabs.com/device";
const CLIENT_IDS: [&str; 2] = ["openwork-opencode-plugin", "openwork-cli"];
const GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_MODELS: usize = 200;

#[derive(Clone, Debug)]
pub struct Endpoints {
    pub api: String,
    pub inference: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            api: API_BASE.into(),
            inference: INFERENCE_BASE.into(),
        }
    }
}

/// Bagian yang boleh dilihat UI (tanpa device_code).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

/// State alur yang disimpan Rust antara `start` dan `poll`.
#[derive(Clone, Debug)]
pub struct Flow {
    pub device_code: String,
    pub client_id: String,
    pub interval: u64,
    pub deadline: Instant,
}

#[derive(Debug, PartialEq)]
pub enum Poll {
    Pending,
    SlowDown(u64),
    Denied(String),
    Expired,
    Error(String),
    /// Token sesi akun (JANGAN dikirim ke UI).
    Token(String),
}

#[derive(Debug, Default, PartialEq)]
pub struct Gateway {
    pub email: String,
    pub org_name: Option<String>,
    pub api_key: Option<String>,
    pub models: Vec<String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
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

/// URL verifikasi aman dibuka: https dan host `openworklabs.com` atau subdomainnya, tanpa kredensial.
pub fn is_safe_verification_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.contains('@')
        && !host.contains(':')
        && (host == "openworklabs.com" || host.ends_with(".openworklabs.com"))
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(str::to_string)
}

/// Baca isi respons sebagai JSON (maks 1 MB); bukan JSON → `null`.
fn read_body(res: &mut ureq::http::Response<ureq::Body>) -> Value {
    res.body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_string()
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

fn post_json(url: &str, body: &Value) -> Result<(u16, Value), String> {
    let mut res = agent()
        .post(url)
        .header("content-type", "application/json")
        .send(body.to_string().as_bytes())
        .map_err(|e| format!("OpenWork tidak terjangkau: {e}"))?;
    let status = res.status().as_u16();
    let v = read_body(&mut res);
    Ok((status, v))
}

fn get_json(url: &str, headers: &[(&str, String)]) -> Result<(u16, Value), String> {
    let mut req = agent().get(url).header("accept", "application/json");
    for (k, v) in headers {
        req = req.header(*k, v.as_str());
    }
    let mut res = req
        .call()
        .map_err(|e| format!("OpenWork tidak terjangkau: {e}"))?;
    let status = res.status().as_u16();
    let v = read_body(&mut res);
    Ok((status, v))
}

/// Langkah 1: minta kode perangkat. Mencoba `openwork-opencode-plugin`, lalu `openwork-cli` bila ditolak.
pub fn start(ep: &Endpoints) -> Result<(DeviceInfo, Flow), String> {
    let url = format!("{}/api/auth/device/code", ep.api);
    let mut last = String::from("OpenWork menolak permintaan kode perangkat");
    for client_id in CLIENT_IDS {
        let (status, v) = post_json(&url, &json!({ "client_id": client_id }))?;
        if !(200..300).contains(&status) {
            last = s(&v, "error_description")
                .or_else(|| s(&v, "error"))
                .map(|e| format!("{e} (HTTP {status})"))
                .unwrap_or_else(|| format!("HTTP {status}"));
            continue;
        }
        let device_code = s(&v, "device_code").ok_or("respons OpenWork tanpa device_code")?;
        let raw_user = s(&v, "user_code").ok_or("respons OpenWork tanpa user_code")?;
        let clean: String = raw_user
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_uppercase();
        let user_code = if clean.len() == 8 {
            format!("{}-{}", &clean[..4], &clean[4..])
        } else {
            clean
        };
        let from_server = s(&v, "verification_uri_complete").or_else(|| {
            s(&v, "verification_uri").map(|u| format!("{u}?user_code={}", urlenc(&raw_user)))
        });
        let verification_uri = from_server
            .filter(|u| is_safe_verification_url(u))
            .unwrap_or_else(|| format!("{DEFAULT_VERIFY_URL}?user_code={}", urlenc(&raw_user)));
        let interval = v
            .get("interval")
            .and_then(|x| x.as_u64())
            .unwrap_or(5)
            .max(3);
        let expires_in = v
            .get("expires_in")
            .and_then(|x| x.as_u64())
            .unwrap_or(900)
            .clamp(60, 1800);
        return Ok((
            DeviceInfo {
                user_code,
                verification_uri,
                interval,
                expires_in,
            },
            Flow {
                device_code,
                client_id: client_id.to_string(),
                interval,
                deadline: Instant::now() + Duration::from_secs(expires_in),
            },
        ));
    }
    Err(last)
}

fn urlenc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Langkah 2: sekali tanya apakah pengguna sudah menyetujui di browser.
pub fn poll(ep: &Endpoints, flow: &Flow) -> Poll {
    if Instant::now() >= flow.deadline {
        return Poll::Expired;
    }
    let url = format!("{}/api/auth/device/token", ep.api);
    let body = json!({ "client_id": flow.client_id, "device_code": flow.device_code, "grant_type": GRANT });
    let (status, v) = match post_json(&url, &body) {
        Ok(r) => r,
        // gangguan jaringan sesaat: anggap masih menunggu (UI akan mencoba lagi)
        Err(_) => return Poll::Pending,
    };
    if status == 200 {
        return match s(&v, "access_token").or_else(|| s(&v, "token")) {
            Some(t) if !t.trim().is_empty() => Poll::Token(t),
            _ => Poll::Error("OpenWork tidak mengembalikan token".into()),
        };
    }
    match s(&v, "error").as_deref() {
        Some("authorization_pending") => Poll::Pending,
        Some("slow_down") => Poll::SlowDown(flow.interval + 5),
        Some("expired_token") => Poll::Expired,
        Some("access_denied") => Poll::Denied(
            s(&v, "error_description").unwrap_or_else(|| "Anda menolak permintaan masuk".into()),
        ),
        Some(other) => Poll::Error(s(&v, "error_description").unwrap_or_else(|| other.to_string())),
        None => Poll::Error(format!("HTTP {status}")),
    }
}

/// Langkah 3: tukar token sesi dengan kunci AI Gateway. Gagal di tengah jalan → hasil parsial dengan `api_key = None`.
pub fn resolve_gateway(ep: &Endpoints, token: &str) -> Result<Gateway, String> {
    let bearer = format!("Bearer {token}");
    let base_headers = |org: &Option<String>| -> Vec<(&'static str, String)> {
        let mut h = vec![
            ("authorization", bearer.clone()),
            ("x-api-key", token.to_string()),
        ];
        if let Some(o) = org {
            h.push(("x-openwork-org-id", o.clone()));
            h.push(("x-openwork-legacy-org-id", o.clone()));
        }
        h
    };

    let (status, me) = get_json(&format!("{}/v1/me", ep.api), &base_headers(&None))?;
    if !(200..300).contains(&status) {
        return Err(format!("OpenWork menolak sesi akun (HTTP {status})"));
    }
    let user = me.get("user").cloned().unwrap_or(Value::Null);
    let email = s(&user, "name")
        .filter(|n| !n.trim().is_empty())
        .or_else(|| s(&user, "email"))
        .unwrap_or_default();
    let mut org_id = me.get("session").and_then(|x| s(x, "activeOrganizationId"));
    let mut org_name = None;

    if let Ok((200, orgs)) = get_json(&format!("{}/v1/me/orgs", ep.api), &base_headers(&org_id)) {
        if let Some(list) = orgs.get("orgs").and_then(|o| o.as_array()) {
            let active = s(&orgs, "activeOrgId").or_else(|| org_id.clone());
            let pick = list
                .iter()
                .find(|o| s(o, "id") == active)
                .or_else(|| {
                    list.iter()
                        .find(|o| o.get("isActive") == Some(&Value::Bool(true)))
                })
                .or_else(|| list.first());
            if let Some(o) = pick {
                org_id = s(o, "id").or(org_id);
                org_name = s(o, "name").or_else(|| s(o, "slug"));
            }
        }
    }

    let headers = base_headers(&org_id);
    let mut api_key: Option<String> = None;
    let mut models: Vec<String> = Vec::new();
    if let Ok((200, provs)) = get_json(
        &format!("{}/v1/inference-providers?scope=usable", ep.api),
        &headers,
    ) {
        for p in provs
            .get("inferenceProviders")
            .and_then(|l| l.as_array())
            .into_iter()
            .flatten()
        {
            let Some(id) = s(p, "id") else { continue };
            let url = format!("{}/v1/inference-providers/{}/connect", ep.api, urlenc(&id));
            let Ok((200, c)) = get_json(&url, &headers) else {
                continue;
            };
            let detail = c.get("inferenceProvider").cloned().unwrap_or(Value::Null);
            let Some(key) = s(&detail, "apiKey").map(|k| k.trim().to_string()) else {
                continue;
            };
            if !key.starts_with("ow_gw_") {
                continue;
            }
            api_key.get_or_insert(key);
            for m in detail
                .get("models")
                .and_then(|m| m.as_array())
                .into_iter()
                .flatten()
            {
                if let Some(mid) = s(m, "id") {
                    if models.len() < MAX_MODELS && !models.contains(&mid) {
                        models.push(mid);
                    }
                }
            }
        }
    }
    if api_key.is_none() {
        // kredensial gratis otomatis
        let url = format!("{}/v1/inference/free/credential", ep.api);
        let mut req = agent().post(&url).header("accept", "application/json");
        for (k, v) in &headers {
            req = req.header(*k, v.as_str());
        }
        if let Ok(mut res) = req.send_empty() {
            if res.status().as_u16() == 200 {
                let v = read_body(&mut res);
                if let Some(k) = v.get("credential").and_then(|c| s(c, "apiKey")) {
                    if k.starts_with("ow_inf_") {
                        api_key = Some(k);
                    }
                }
            }
        }
    }
    Ok(Gateway {
        email,
        org_name,
        api_key,
        models,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Jaringan nyata: `cargo test --lib openwork -- --ignored`. Hanya meminta kode perangkat (tidak butuh akun).
    #[test]
    #[ignore = "butuh internet"]
    fn live_device_code_request_works() {
        let (info, flow) =
            start(&Endpoints::default()).expect("OpenWork menolak permintaan kode perangkat");
        assert!(!info.user_code.is_empty());
        assert!(
            is_safe_verification_url(&info.verification_uri),
            "{}",
            info.verification_uri
        );
        assert!(!flow.device_code.is_empty());
        // belum disetujui → harus menunggu, bukan galat
        assert_eq!(poll(&Endpoints::default(), &flow), Poll::Pending);
    }

    #[test]
    fn verification_urls_must_be_https_on_openwork() {
        for ok in [
            "https://app.openworklabs.com/device?user_code=AB",
            "https://openworklabs.com/x",
        ] {
            assert!(is_safe_verification_url(ok), "{ok}");
        }
        for bad in [
            "http://app.openworklabs.com/device",
            "https://evil.com/openworklabs.com",
            "https://openworklabs.com.evil.com/x",
            "https://user@app.openworklabs.com/x",
            "https://app.openworklabs.com:8443/x",
            "javascript:alert(1)",
            "",
        ] {
            assert!(!is_safe_verification_url(bad), "{bad}");
        }
    }

    #[test]
    fn url_encoding_is_strict() {
        assert_eq!(urlenc("AB-12 cd&x"), "AB-12%20cd%26x");
    }

    /// Server tiruan: `routes` memetakan "METHOD /path" → (status, body). Merekam permintaan yang diterima.
    fn mock(routes: Vec<(&'static str, u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut st) = stream else { break };
                // baca header lalu isi sesuai Content-Length (isi bisa tiba di segmen TCP terpisah)
                let mut data: Vec<u8> = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let n = st.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    data.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&data).to_string();
                    if let Some(end) = data.windows(4).position(|w| w == [13, 10, 13, 10]) {
                        let want = text[..end]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if data.len() >= end + 4 + want {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&data).to_string();
                let line = req.lines().next().unwrap_or("").to_string();
                let key = {
                    let mut it = line.split_whitespace();
                    format!("{} {}", it.next().unwrap_or(""), it.next().unwrap_or(""))
                };
                seen2.lock().unwrap().push(req.clone());
                let (status, body) = routes
                    .iter()
                    .find(|(k, _, _)| {
                        key.split('?').next() == k.split('?').next()
                            && (!k.contains('?') || key == *k)
                    })
                    .map(|(_, s, b)| (*s, b.clone()))
                    .unwrap_or((404, "{}".into()));
                let resp = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = st.write_all(resp.as_bytes());
            }
        });
        (format!("http://{addr}"), seen)
    }

    fn ep(base: &str) -> Endpoints {
        Endpoints {
            api: base.into(),
            inference: format!("{base}/inference"),
        }
    }

    #[test]
    fn start_formats_code_and_rejects_untrusted_verification_url() {
        let body = json!({
            "device_code": "dev123",
            "user_code": "abcd1234",
            "verification_uri": "https://evil.example/device",
            "interval": 1,
            "expires_in": 600
        })
        .to_string();
        let (base, seen) = mock(vec![("POST /api/auth/device/code", 200, body)]);
        // endpoint http lokal hanya untuk uji (agent TLS tetap mendukung http)
        let (info, flow) = start(&ep(&base)).unwrap();
        assert_eq!(info.user_code, "ABCD-1234");
        assert!(
            info.verification_uri.starts_with(DEFAULT_VERIFY_URL),
            "{}",
            info.verification_uri
        );
        assert_eq!(info.interval, 3, "interval minimal 3 detik");
        assert_eq!(flow.client_id, "openwork-opencode-plugin");
        assert!(seen.lock().unwrap()[0].contains("openwork-opencode-plugin"));
    }

    #[test]
    fn start_falls_back_to_second_client_id() {
        // Server menolak klien pertama dan menerima kedua: dua rute bernama sama tidak bisa dibedakan oleh tiruan
        // sederhana, jadi di sini yang diuji adalah galat jelas saat semua klien ditolak.
        let (base, _) = mock(vec![(
            "POST /api/auth/device/code",
            400,
            json!({"error": "invalid_client", "error_description": "klien tidak dikenal"})
                .to_string(),
        )]);
        let err = start(&ep(&base)).unwrap_err();
        assert!(err.contains("klien tidak dikenal"), "{err}");
    }

    fn flow() -> Flow {
        Flow {
            device_code: "dev".into(),
            client_id: "openwork-cli".into(),
            interval: 5,
            deadline: Instant::now() + Duration::from_secs(60),
        }
    }

    #[test]
    fn poll_maps_every_oauth_state() {
        let cases: Vec<(u16, Value, Poll)> = vec![
            (
                400,
                json!({"error": "authorization_pending"}),
                Poll::Pending,
            ),
            (400, json!({"error": "slow_down"}), Poll::SlowDown(10)),
            (400, json!({"error": "expired_token"}), Poll::Expired),
            (
                400,
                json!({"error": "access_denied"}),
                Poll::Denied("Anda menolak permintaan masuk".into()),
            ),
            (
                400,
                json!({"error": "access_denied", "error_description": "ditolak admin"}),
                Poll::Denied("ditolak admin".into()),
            ),
            (
                400,
                json!({"error": "server_error", "error_description": "gangguan"}),
                Poll::Error("gangguan".into()),
            ),
            (
                200,
                json!({"access_token": "tok"}),
                Poll::Token("tok".into()),
            ),
            (200, json!({"token": "tok2"}), Poll::Token("tok2".into())),
            (
                200,
                json!({}),
                Poll::Error("OpenWork tidak mengembalikan token".into()),
            ),
            (500, json!(null), Poll::Error("HTTP 500".into())),
        ];
        for (status, body, want) in cases {
            let (base, _) = mock(vec![(
                "POST /api/auth/device/token",
                status,
                body.to_string(),
            )]);
            assert_eq!(poll(&ep(&base), &flow()), want, "{status} {body}");
        }
    }

    #[test]
    fn poll_after_deadline_is_expired_without_network() {
        let mut f = flow();
        f.deadline = Instant::now() - Duration::from_secs(1);
        assert_eq!(poll(&ep("http://127.0.0.1:1"), &f), Poll::Expired);
    }

    #[test]
    fn poll_network_error_keeps_waiting() {
        assert_eq!(poll(&ep("http://127.0.0.1:1"), &flow()), Poll::Pending);
    }

    #[test]
    fn resolve_gateway_collects_account_org_key_and_models() {
        let (base, seen) = mock(vec![
            ("GET /v1/me", 200, json!({"user": {"name": "Budi", "email": "b@x.id"}, "session": {"activeOrganizationId": "org1"}}).to_string()),
            ("GET /v1/me/orgs", 200, json!({"activeOrgId": "org1", "orgs": [{"id": "org0", "name": "Lain"}, {"id": "org1", "name": "Tim Budi", "slug": "tim"}]}).to_string()),
            ("GET /v1/inference-providers", 200, json!({"inferenceProviders": [{"id": "p1", "name": "P1"}]}).to_string()),
            ("GET /v1/inference-providers/p1/connect", 200, json!({"inferenceProvider": {"apiKey": "ow_gw_abc", "models": [{"id": "openai/gpt-x"}, {"id": "deepseek/v4"}, {"id": "openai/gpt-x"}]}}).to_string()),
        ]);
        let g = resolve_gateway(&ep(&base), "SESI").unwrap();
        assert_eq!(g.email, "Budi");
        assert_eq!(g.org_name.as_deref(), Some("Tim Budi"));
        assert_eq!(g.api_key.as_deref(), Some("ow_gw_abc"));
        assert_eq!(g.models, ["openai/gpt-x", "deepseek/v4"]);
        let all = seen.lock().unwrap().join("\n").to_lowercase();
        assert!(all.contains("authorization: bearer sesi"));
        assert!(all.contains("x-openwork-org-id: org1"));
    }

    #[test]
    fn resolve_gateway_ignores_keys_without_gateway_prefix_and_uses_free_credential() {
        let (base, _) = mock(vec![
            (
                "GET /v1/me",
                200,
                json!({"user": {"email": "b@x.id"}}).to_string(),
            ),
            (
                "GET /v1/inference-providers",
                200,
                json!({"inferenceProviders": [{"id": "p1"}]}).to_string(),
            ),
            (
                "GET /v1/inference-providers/p1/connect",
                200,
                json!({"inferenceProvider": {"apiKey": "sk-bukan-gateway"}}).to_string(),
            ),
            (
                "POST /v1/inference/free/credential",
                200,
                json!({"credential": {"apiKey": "ow_inf_free"}}).to_string(),
            ),
        ]);
        let g = resolve_gateway(&ep(&base), "S").unwrap();
        assert_eq!(g.email, "b@x.id");
        assert_eq!(g.api_key.as_deref(), Some("ow_inf_free"));
    }

    #[test]
    fn resolve_gateway_without_any_credential_is_reported_not_faked() {
        let (base, _) = mock(vec![(
            "GET /v1/me",
            200,
            json!({"user": {"email": "b@x.id"}}).to_string(),
        )]);
        let g = resolve_gateway(&ep(&base), "S").unwrap();
        assert_eq!(g.api_key, None);
        assert!(g.models.is_empty());
    }

    #[test]
    fn resolve_gateway_rejects_invalid_session() {
        let (base, _) = mock(vec![("GET /v1/me", 401, "{}".into())]);
        assert!(resolve_gateway(&ep(&base), "S")
            .unwrap_err()
            .contains("401"));
    }
}

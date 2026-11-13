//! Klien Open VSX untuk tema editor (Fase 10, ADR-038). Hanya host `open-vsx.org` lewat HTTPS; respons dibatasi ukuran.
//! Rust yang melakukan jaringan (bukan WebView) agar CSP UI tetap ketat dan batas ukuran bisa ditegakkan.

use serde::Serialize;
use std::time::Duration;

const HOST_PREFIX: &str = "https://open-vsx.org/";
const MAX_SEARCH_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_VSIX_BYTES: u64 = 15 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct OvsxItem {
    pub namespace: String,
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub download: String,
    pub downloads: u64,
}

fn agent(timeout: Duration) -> ureq::Agent {
    // Fitur `native-tls` saja yang diaktifkan (lisensi/ukuran): penyedia TLS harus dipilih eksplisit, jika tidak ureq
    // memakai Rustls dan panik pada https.
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .build()
        .into()
}

/// Hanya URL HTTPS di `open-vsx.org` (tanpa kredensial, tanpa alihan ke host lain karena path-nya dikunci).
pub fn is_allowed_url(url: &str) -> bool {
    url.starts_with(HOST_PREFIX) && !url.contains('@') && !url.contains("..") && url.len() < 512
}

pub fn encode_query(q: &str) -> String {
    let mut out = String::new();
    for b in q.trim().bytes().take(80) {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(b as char),
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn parse_search(json: &str) -> Result<Vec<OvsxItem>, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("respons Open VSX bukan JSON: {e}"))?;
    let arr = v
        .get("extensions")
        .and_then(|e| e.as_array())
        .ok_or("respons Open VSX tidak memuat daftar ekstensi")?;
    let s = |x: &serde_json::Value, k: &str| {
        x.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
    };
    let mut out = Vec::new();
    for e in arr {
        let download = e
            .get("files")
            .and_then(|f| f.get("download"))
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        if !is_allowed_url(&download) {
            continue;
        }
        let name = s(e, "name");
        out.push(OvsxItem {
            namespace: s(e, "namespace"),
            display_name: if s(e, "displayName").is_empty() {
                name.clone()
            } else {
                s(e, "displayName")
            },
            name,
            version: s(e, "version"),
            description: s(e, "description").chars().take(300).collect(),
            download,
            downloads: e.get("downloadCount").and_then(|d| d.as_u64()).unwrap_or(0),
        });
    }
    Ok(out)
}

pub fn search_themes(query: &str) -> Result<Vec<OvsxItem>, String> {
    let q = encode_query(query);
    if q.is_empty() {
        return Err("kata kunci kosong".into());
    }
    let url =
        format!("{HOST_PREFIX}api/-/search?query={q}&category=Themes&size=12&sortBy=downloadCount");
    let mut res = agent(Duration::from_secs(20))
        .get(&url)
        .call()
        .map_err(|e| format!("Open VSX tidak terjangkau: {e}"))?;
    if res.status().as_u16() != 200 {
        return Err(format!("Open VSX membalas HTTP {}", res.status().as_u16()));
    }
    let text = res
        .body_mut()
        .with_config()
        .limit(MAX_SEARCH_BYTES)
        .read_to_string()
        .map_err(|e| format!("gagal membaca respons: {e}"))?;
    parse_search(&text)
}

pub fn download_vsix(url: &str) -> Result<Vec<u8>, String> {
    if !is_allowed_url(url) {
        return Err("URL tidak diizinkan (hanya https://open-vsx.org/)".into());
    }
    let mut res = agent(Duration::from_secs(60))
        .get(url)
        .call()
        .map_err(|e| format!("gagal mengunduh: {e}"))?;
    if res.status().as_u16() != 200 {
        return Err(format!("Open VSX membalas HTTP {}", res.status().as_u16()));
    }
    res.body_mut()
        .with_config()
        .limit(MAX_VSIX_BYTES)
        .read_to_vec()
        .map_err(|e| format!("gagal membaca unduhan (maks 15 MB): {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Jaringan nyata: `cargo test --lib openvsx -- --ignored`. Menjaga agar https benar-benar bekerja (TLS native).
    #[test]
    #[ignore = "butuh internet"]
    fn live_search_and_download_work_over_https() {
        let items = search_themes("dracula").expect("pencarian https gagal");
        assert!(!items.is_empty(), "tidak ada hasil");
        let first = &items[0];
        assert!(is_allowed_url(&first.download));
        let bytes = download_vsix(&first.download).expect("unduhan gagal");
        assert!(bytes.starts_with(b"PK"), "bukan zip/vsix");
    }

    #[test]
    fn only_https_open_vsx_urls_are_allowed() {
        assert!(is_allowed_url(
            "https://open-vsx.org/api/a/b/1.0/file/a.b-1.0.vsix"
        ));
        for bad in [
            "http://open-vsx.org/api/x",
            "https://evil.com/https://open-vsx.org/",
            "https://open-vsx.org.evil.com/x",
            "https://open-vsx.org/../etc",
            "https://user@open-vsx.org/x",
            "file:///C:/x",
            "",
        ] {
            assert!(!is_allowed_url(bad), "{bad}");
        }
    }

    #[test]
    fn query_is_percent_encoded_and_limited() {
        assert_eq!(encode_query("dracula pro"), "dracula%20pro");
        assert_eq!(encode_query("a&b=c#"), "a%26b%3Dc%23");
        assert_eq!(encode_query("   "), "");
        assert_eq!(encode_query(&"x".repeat(200)).len(), 80);
    }

    #[test]
    fn search_response_is_parsed_and_unsafe_downloads_dropped() {
        let json = r#"{"extensions":[
          {"name":"dracula","namespace":"dr","displayName":"Dracula","version":"1.2.3","description":"tema","downloadCount":42,
           "files":{"download":"https://open-vsx.org/api/dr/dracula/1.2.3/file/dr.dracula-1.2.3.vsix"}},
          {"name":"jahat","namespace":"x","files":{"download":"https://evil.com/x.vsix"}},
          {"name":"tanpa-file","namespace":"x"}
        ]}"#;
        let items = parse_search(json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].display_name, "Dracula");
        assert_eq!(items[0].downloads, 42);
        assert!(parse_search("{}").is_err());
        assert!(parse_search("bukan json").is_err());
    }
}

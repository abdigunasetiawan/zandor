//! Generate gambar (ADR-077): memanggil API gambar provider dengan kunci dari keychain (tidak pernah sampai ke UI),
//! lalu menyimpan hasilnya di folder project. Gaya API yang didukung:
//! - OpenAI Images (`POST {base}/images/generations`): provider `openai` dan endpoint openai/openai-compatible.
//! - Google Gemini (`POST {base}/models/{model}:generateContent`, `responseModalities: IMAGE`).
//! - OpenRouter (`POST {base}/chat/completions` dengan `modalities: ["image","text"]`).
//!
//! Provider lain (mis. Anthropic) tidak punya API gambar → galat jujur.
use crate::providers::Provider;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(170);
const MAX_RESPONSE: u64 = 40 * 1024 * 1024;
const MAX_ERROR_CHARS: usize = 400;

#[derive(Debug, PartialEq)]
pub enum Style {
    OpenAiImages(String),
    Gemini(String),
    OpenRouter(String),
}

fn trim(base: &str) -> String {
    base.trim_end_matches('/').to_string()
}

/// Pilih gaya API dari provider. Galat bila provider tidak punya API gambar yang dikenal.
pub fn style_for(p: &Provider) -> Result<Style, String> {
    let base = p.base_url.as_deref().map(trim);
    let protocol = p.protocol.as_deref().unwrap_or("");
    if p.id == "openrouter" || base.as_deref().is_some_and(|b| b.contains("openrouter.ai")) {
        return Ok(Style::OpenRouter(
            base.unwrap_or_else(|| "https://openrouter.ai/api/v1".into()),
        ));
    }
    if p.id == "google" || protocol == "google" {
        return Ok(Style::Gemini(base.unwrap_or_else(|| {
            "https://generativelanguage.googleapis.com/v1beta".into()
        })));
    }
    if p.id == "openai" {
        return Ok(Style::OpenAiImages(
            base.unwrap_or_else(|| "https://api.openai.com/v1".into()),
        ));
    }
    match (protocol, base) {
        ("anthropic", _) => Err(format!(
            "provider \"{}\" (protokol Anthropic) tidak punya API generate gambar",
            p.name
        )),
        (_, Some(b)) => Ok(Style::OpenAiImages(b)),
        _ => Err(format!(
            "provider \"{}\" belum didukung untuk generate gambar (didukung: OpenAI, Google Gemini, OpenRouter, endpoint kompatibel OpenAI)",
            p.name
        )),
    }
}

/// URL, header (tanpa kunci), dan body permintaan.
pub fn build_request(style: &Style, model: &str, prompt: &str, size: &str) -> (String, Value) {
    match style {
        Style::OpenAiImages(base) => {
            let mut body = json!({ "model": model, "prompt": prompt, "n": 1, "size": size });
            // Model DALL·E mengembalikan URL kecuali diminta base64; model gpt-image selalu base64 (dan menolak opsi ini).
            if model.starts_with("dall-e") {
                body["response_format"] = json!("b64_json");
            }
            (format!("{base}/images/generations"), body)
        }
        Style::Gemini(base) => (
            format!("{base}/models/{model}:generateContent"),
            json!({
                "contents": [{ "parts": [{ "text": prompt }] }],
                "generationConfig": { "responseModalities": ["IMAGE", "TEXT"] }
            }),
        ),
        Style::OpenRouter(base) => (
            format!("{base}/chat/completions"),
            json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "modalities": ["image", "text"]
            }),
        ),
    }
}

#[derive(Debug, PartialEq)]
pub enum ImageData {
    Bytes(Vec<u8>),
    Url(String),
}

fn from_data_url(s: &str) -> Option<ImageData> {
    let (_, b64) = s.strip_prefix("data:image/")?.split_once(";base64,")?;
    decode_b64(b64).map(ImageData::Bytes)
}

/// Ambil gambar pertama dari respons sukses. `None` → respons tidak memuat gambar.
pub fn parse_response(style: &Style, v: &Value) -> Option<ImageData> {
    match style {
        Style::OpenAiImages(_) => {
            let first = v.get("data")?.get(0)?;
            if let Some(b) = first.get("b64_json").and_then(Value::as_str) {
                return decode_b64(b).map(ImageData::Bytes);
            }
            first
                .get("url")
                .and_then(Value::as_str)
                .map(|u| ImageData::Url(u.to_string()))
        }
        Style::Gemini(_) => v
            .get("candidates")?
            .as_array()?
            .iter()
            .filter_map(|c| c.get("content")?.get("parts")?.as_array())
            .flatten()
            .find_map(|p| {
                let d = p.get("inlineData").or_else(|| p.get("inline_data"))?;
                decode_b64(d.get("data")?.as_str()?).map(ImageData::Bytes)
            }),
        Style::OpenRouter(_) => {
            let msg = v.get("choices")?.get(0)?.get("message")?;
            let url = msg
                .get("images")?
                .get(0)?
                .get("image_url")?
                .get("url")?
                .as_str()?;
            from_data_url(url).or_else(|| {
                url.starts_with("https://")
                    .then(|| ImageData::Url(url.to_string()))
            })
        }
    }
}

/// Pesan galat dari body respons gagal (bentuk umum `{error: {message}}` atau `{error: "..."}`).
pub fn error_message(status: u16, v: &Value, raw: &str) -> String {
    let msg = v
        .get("error")
        .and_then(|e| {
            e.get("message")
                .and_then(Value::as_str)
                .or_else(|| e.as_str())
        })
        .map(str::to_string)
        .unwrap_or_else(|| raw.chars().take(MAX_ERROR_CHARS).collect());
    let hint = match status {
        401 | 403 => " (kunci API ditolak)",
        404 => " (model atau endpoint tidak ditemukan — apakah ini model gambar?)",
        429 => " (batas pemakaian/kuota)",
        _ => "",
    };
    format!(
        "HTTP {status}{hint}: {}",
        msg.chars().take(MAX_ERROR_CHARS).collect::<String>().trim()
    )
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Dekode base64 baku/URL-safe (spasi diabaikan). `None` bila tidak valid.
pub fn decode_b64(s: &str) -> Option<Vec<u8>> {
    let mut val = [255u8; 256];
    for (i, c) in B64.iter().enumerate() {
        val[*c as usize] = i as u8;
    }
    val[b'-' as usize] = 62;
    val[b'_' as usize] = 63;
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for b in s.bytes() {
        if b == b'=' {
            break;
        }
        if b.is_ascii_whitespace() {
            continue;
        }
        let v = val[b as usize];
        if v == 255 {
            return None;
        }
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Ekstensi dari magic bytes; `None` → bukan gambar yang dikenal.
pub fn sniff_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Path tujuan relatif yang aman (tanpa `..`/absolut), dengan ekstensi mengikuti isi dan tanpa menimpa berkas lama.
pub fn target_path(root: &Path, rel: &str, ext: &str) -> Result<(PathBuf, String), String> {
    let rel = rel.trim().replace('\\', "/");
    let p = Path::new(&rel);
    if rel.is_empty() || !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("path gambar tidak valid: {rel}"));
    }
    let stem = rel
        .rsplit_once('.')
        .filter(|(_, e)| !e.contains('/'))
        .map(|(s, _)| s.to_string())
        .unwrap_or(rel.clone());
    for n in 0..1000 {
        let candidate = if n == 0 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem}-{n}.{ext}")
        };
        let full = root.join(&candidate);
        if !full.exists() {
            return Ok((full, candidate));
        }
    }
    Err("terlalu banyak berkas dengan nama yang sama".into())
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

fn read_text(res: &mut ureq::http::Response<ureq::Body>) -> String {
    res.body_mut()
        .with_config()
        .limit(MAX_RESPONSE)
        .read_to_string()
        .unwrap_or_default()
}

/// Panggil API gambar dan kembalikan byte gambar. Blocking (jalankan di thread terpisah).
pub fn generate(
    provider: &Provider,
    key: Option<&str>,
    model: &str,
    prompt: &str,
    size: &str,
) -> Result<Vec<u8>, String> {
    let style = style_for(provider)?;
    let (url, body) = build_request(&style, model, prompt, size);
    let mut req = agent()
        .post(&url)
        .header("content-type", "application/json");
    if let Some(k) = key.filter(|k| !k.is_empty()) {
        req = match style {
            Style::Gemini(_) => req.header("x-goog-api-key", k),
            _ => req.header("authorization", &format!("Bearer {k}")),
        };
    }
    let mut res = req
        .send(body.to_string().as_bytes())
        .map_err(|e| format!("{} tidak terjangkau: {e}", provider.name))?;
    let status = res.status().as_u16();
    let text = read_text(&mut res);
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !(200..300).contains(&status) {
        return Err(error_message(status, &v, &text));
    }
    match parse_response(&style, &v) {
        Some(ImageData::Bytes(b)) => Ok(b),
        Some(ImageData::Url(u)) if u.starts_with("https://") => {
            let mut r = agent()
                .get(&u)
                .call()
                .map_err(|e| format!("gambar tidak bisa diunduh: {e}"))?;
            if !r.status().is_success() {
                return Err(format!("gambar tidak bisa diunduh: HTTP {}", r.status()));
            }
            r.body_mut()
                .with_config()
                .limit(MAX_RESPONSE)
                .read_to_vec()
                .map_err(|e| format!("gambar tidak bisa diunduh: {e}"))
        }
        Some(ImageData::Url(_)) => Err("respons memuat URL gambar yang tidak aman (bukan https)".into()),
        None => Err(
            "respons tidak memuat gambar (model ini mungkin hanya teks, atau permintaan ditolak filter konten)".into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prov(id: &str, base: Option<&str>, protocol: Option<&str>) -> Provider {
        Provider {
            id: id.into(),
            name: id.into(),
            base_url: base.map(str::to_string),
            protocol: protocol.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn gaya_api_dipilih_dari_provider() {
        assert_eq!(
            style_for(&prov("openai", None, None)).unwrap(),
            Style::OpenAiImages("https://api.openai.com/v1".into())
        );
        assert_eq!(
            style_for(&prov("google", None, None)).unwrap(),
            Style::Gemini("https://generativelanguage.googleapis.com/v1beta".into())
        );
        assert_eq!(
            style_for(&prov("openrouter", None, None)).unwrap(),
            Style::OpenRouter("https://openrouter.ai/api/v1".into())
        );
        assert_eq!(
            style_for(&prov("lokal", Some("http://localhost:8000/v1/"), None)).unwrap(),
            Style::OpenAiImages("http://localhost:8000/v1".into())
        );
        assert!(style_for(&prov("anthropic", None, None)).is_err());
        assert!(style_for(&prov("x", Some("https://a"), Some("anthropic"))).is_err());
    }

    #[test]
    fn body_openai_dan_respons_tiap_gaya() {
        let s = Style::OpenAiImages("https://api.openai.com/v1".into());
        let (url, body) = build_request(&s, "dall-e-3", "kucing", "1024x1024");
        assert_eq!(url, "https://api.openai.com/v1/images/generations");
        assert_eq!(body["response_format"], "b64_json");
        let (_, body) = build_request(&s, "gpt-image-1", "kucing", "1024x1024");
        assert!(body.get("response_format").is_none());
        assert_eq!(
            parse_response(&s, &json!({"data":[{"b64_json":"aGFp"}]})),
            Some(ImageData::Bytes(b"hai".to_vec()))
        );
        assert_eq!(
            parse_response(&s, &json!({"data":[{"url":"https://x/y.png"}]})),
            Some(ImageData::Url("https://x/y.png".into()))
        );
        let g = Style::Gemini("https://g".into());
        let (url, _) = build_request(&g, "gemini-2.5-flash-image", "p", "1024x1024");
        assert_eq!(
            url,
            "https://g/models/gemini-2.5-flash-image:generateContent"
        );
        let gv = json!({"candidates":[{"content":{"parts":[{"text":"ok"},{"inlineData":{"mimeType":"image/png","data":"aGFp"}}]}}]});
        assert_eq!(
            parse_response(&g, &gv),
            Some(ImageData::Bytes(b"hai".to_vec()))
        );
        let o = Style::OpenRouter("https://o".into());
        let ov = json!({"choices":[{"message":{"images":[{"image_url":{"url":"data:image/png;base64,aGFp"}}]}}]});
        assert_eq!(
            parse_response(&o, &ov),
            Some(ImageData::Bytes(b"hai".to_vec()))
        );
        assert_eq!(
            parse_response(&o, &json!({"choices":[{"message":{"content":"x"}}]})),
            None
        );
    }

    #[test]
    fn galat_dan_base64() {
        let m = error_message(401, &json!({"error":{"message":"bad key"}}), "");
        assert!(m.contains("401") && m.contains("bad key") && m.contains("ditolak"));
        assert_eq!(decode_b64("aGVsbG8="), Some(b"hello".to_vec()));
        assert_eq!(decode_b64("a-_b"), Some(vec![0x6b, 0xef, 0xdb]));
        assert_eq!(decode_b64("@@"), None);
    }

    #[test]
    fn path_tujuan_aman_dan_tidak_menimpa() {
        let root = std::env::temp_dir().join(format!("zandor-img-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("images")).unwrap();
        let (_, rel) = target_path(&root, "images/a.png", "jpg").unwrap();
        assert_eq!(rel, "images/a.jpg");
        std::fs::write(root.join("images/a.jpg"), b"x").unwrap();
        let (_, rel) = target_path(&root, "images/a.png", "jpg").unwrap();
        assert_eq!(rel, "images/a-1.jpg");
        assert!(target_path(&root, "../a.png", "png").is_err());
        assert!(target_path(&root, "C:/a.png", "png").is_err());
        assert_eq!(sniff_ext(&[0x89, b'P', b'N', b'G', 0]), Some("png"));
        assert_eq!(sniff_ext(b"hai"), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}

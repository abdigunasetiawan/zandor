//! Port `gradient.ts` + validasi gambar gaya project: spesifikasi gradien ⇄ string CSS, validasi ketat string CSS
//! gradien, templat, serta data URL gambar (base64) dan cache gambar untuk dirender GPUI.
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub color: String,
    pub pos: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub radial: bool,
    pub angle: f32,
    pub stops: Vec<Stop>,
}

pub const MIN_STOPS: usize = 2;
pub const MAX_STOPS: usize = 5;
pub const MAX_STYLE_IMAGE_CHARS: usize = 700_000;
pub const MAX_LOGO_IMAGE_CHARS: usize = 120_000;

fn is_hex(s: &str) -> bool {
    let h = s.strip_prefix('#').unwrap_or("");
    s.starts_with('#') && [3, 4, 6, 8].contains(&h.len()) && h.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn gradient_css(s: &Spec) -> String {
    let stops = s.stops.iter().take(MAX_STOPS).map(|x| format!("{} {}%", x.color, x.pos.clamp(0., 100.).round())).collect::<Vec<_>>().join(", ");
    if s.radial {
        format!("radial-gradient(circle at center, {stops})")
    } else {
        format!("linear-gradient({}deg, {stops})", s.angle.clamp(0., 360.).round())
    }
}

pub fn validate_spec(s: &Spec) -> Option<&'static str> {
    if !s.angle.is_finite() {
        return Some("sudut harus angka");
    }
    if s.stops.len() < MIN_STOPS || s.stops.len() > MAX_STOPS {
        return Some("gradien butuh 2–5 titik warna");
    }
    for x in &s.stops {
        if !is_hex(&x.color) {
            return Some("warna titik harus berupa hex (#rrggbb)");
        }
        if !(0. ..=100.).contains(&x.pos) {
            return Some("posisi titik harus 0–100");
        }
    }
    None
}

/// Port parseGradient: hanya bentuk buatan generator.
pub fn parse_gradient(css: &str) -> Option<Spec> {
    let (radial, angle, body) = if let Some(r) = css.strip_prefix("linear-gradient(").and_then(|r| r.strip_suffix(')')) {
        let (a, b) = r.split_once("deg,")?;
        if a.is_empty() || a.len() > 3 || !a.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        (false, a.parse::<f32>().ok()?, b.trim_start())
    } else if let Some(r) = css.strip_prefix("radial-gradient(circle at center,").and_then(|r| r.strip_suffix(')')) {
        (true, 0., r.trim_start())
    } else {
        return None;
    };
    let mut stops = vec![];
    for part in body.split(',') {
        let (c, p) = part.trim().split_once(char::is_whitespace)?;
        let p = p.trim().strip_suffix('%')?;
        if p.is_empty() || p.len() > 3 || !p.chars().all(|c| c.is_ascii_digit()) || !is_hex(c) {
            return None;
        }
        stops.push(Stop { color: c.to_string(), pos: p.parse().ok()? });
    }
    let s = Spec { radial, angle, stops };
    validate_spec(&s).is_none().then_some(s)
}

/// Port validateGradientCss.
pub fn validate_gradient_css(v: &str) -> Option<String> {
    if v.is_empty() || v.chars().count() > 400 {
        return Some("gradien tidak valid".into());
    }
    let re = regex::Regex::new(r"(?s)^(linear|radial)-gradient\((.*)\)$").expect("regex");
    let Some(m) = re.captures(v).and_then(|c| c.get(2)).filter(|m| !m.as_str().is_empty()) else {
        return Some("gradien harus linear-gradient(...) atau radial-gradient(...)".into());
    };
    let f = regex::Regex::new(r"(?i)(?:rgb|rgba|hsl|hsla)\(\s*[\d.,%\s/-]+\)").expect("regex");
    let stripped = f.replace_all(m.as_str(), "0");
    if stripped.contains('(') || stripped.contains(')') {
        return Some("gradien mengandung fungsi yang tidak diizinkan".into());
    }
    let num = regex::Regex::new(r"^-?\d{1,4}(\.\d+)?(deg|%|px|rem)?$").expect("regex");
    let kw = regex::Regex::new(r"(?i)^(circle|ellipse|at|to|center|top|bottom|left|right|closest-side|farthest-side|closest-corner|farthest-corner)$").expect("regex");
    let word = regex::Regex::new(r"(?i)^[a-z]{3,20}$").expect("regex");
    for tok in stripped.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()) {
        if is_hex(tok) || num.is_match(tok) || kw.is_match(tok) || word.is_match(tok) {
            continue;
        }
        return Some(format!("token gradien tidak diizinkan: {tok}"));
    }
    None
}

/// Templat gradien (sama dengan aplikasi TS): (id, nama, radial, sudut, titik).
pub const TEMPLATES: [(&str, &str, bool, f32, &[(&str, f32)]); 12] = [
    ("aurora", "Aurora", false, 135., &[("#0f766e", 0.), ("#2563eb", 55.), ("#7c3aed", 100.)]),
    ("sunset", "Senja", false, 120., &[("#f97316", 0.), ("#ec4899", 60.), ("#7c3aed", 100.)]),
    ("ocean", "Samudra", false, 160., &[("#0ea5e9", 0.), ("#1d4ed8", 100.)]),
    ("forest", "Hutan", false, 150., &[("#14532d", 0.), ("#65a30d", 100.)]),
    ("ember", "Bara", false, 135., &[("#7f1d1d", 0.), ("#ea580c", 100.)]),
    ("graphite", "Grafit", false, 160., &[("#18181b", 0.), ("#52525b", 100.)]),
    ("mint", "Mint", false, 120., &[("#99f6e4", 0.), ("#a7f3d0", 50.), ("#fef9c3", 100.)]),
    ("peach", "Persik", false, 135., &[("#fecaca", 0.), ("#fed7aa", 100.)]),
    ("lavender", "Lavender", false, 145., &[("#ddd6fe", 0.), ("#fbcfe8", 100.)]),
    ("midnight", "Tengah malam", false, 180., &[("#020617", 0.), ("#1e3a8a", 100.)]),
    ("spotlight", "Sorot", true, 0., &[("#6366f1", 0.), ("#1e1b4b", 100.)]),
    ("glow", "Cahaya", true, 0., &[("#fde68a", 0.), ("#f59e0b", 55.), ("#7c2d12", 100.)]),
];

pub fn template(idx: usize) -> Option<Spec> {
    let (_, _, radial, angle, stops) = TEMPLATES.get(idx)?;
    Some(Spec { radial: *radial, angle: *angle, stops: stops.iter().map(|(c, p)| Stop { color: (*c).into(), pos: *p }).collect() })
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for ch in data.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= ch.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0);
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let v = B64.iter().position(|&b| b == c)? as u32;
        buf = buf << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

fn mime_of(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG") {
        Some("png")
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        Some("jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("gif")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Format keluaran perkecilan gambar (port opsi mime/quality imageToDataUrl).
#[derive(Clone, Copy)]
pub enum OutFmt {
    Jpeg(u8),
    Png,
}

/// Port imageToDataUrl: dekode png/jpeg/webp/gif, perkecil agar sisi terpanjang <= max_side, lalu enkode.
pub fn resize_image_bytes(bytes: &[u8], max_side: u32, fmt: OutFmt) -> Result<(Vec<u8>, &'static str), String> {
    if mime_of(bytes).is_none() {
        return Err("hanya png, jpeg, webp, atau gif".into());
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("gambar tidak bisa dibaca: {e}"))?;
    let (w, h) = (img.width().max(1), img.height().max(1));
    let scale = (max_side as f64 / w.max(h) as f64).min(1.);
    let (nw, nh) = (((w as f64 * scale).round() as u32).max(1), ((h as f64 * scale).round() as u32).max(1));
    let img = if (nw, nh) == (w, h) { img } else { img.resize_exact(nw, nh, image::imageops::FilterType::Triangle) };
    let mut out = Vec::new();
    match fmt {
        OutFmt::Jpeg(q) => {
            // JPEG tanpa alfa: komposit di atas putih (seperti canvas → toDataURL jpeg yang mengisi hitam/putih)
            let rgba = img.to_rgba8();
            let mut rgb = image::RgbImage::new(nw, nh);
            for (x, y, p) in rgba.enumerate_pixels() {
                let a = p[3] as u32;
                let c = |v: u8| ((v as u32 * a + 255 * (255 - a)) / 255) as u8;
                rgb.put_pixel(x, y, image::Rgb([c(p[0]), c(p[1]), c(p[2])]));
            }
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q).encode_image(&rgb).map_err(|e| e.to_string())?;
            Ok((out, "jpeg"))
        }
        OutFmt::Png => {
            img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
            Ok((out, "png"))
        }
    }
}

/// Berkas gambar → data URL kecil (diperkecil dulu seperti aplikasi TS).
pub fn image_to_data_url(path: &std::path::Path, max_side: u32, fmt: OutFmt, max_chars: usize) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (out, m) = resize_image_bytes(&bytes, max_side, fmt)?;
    let url = format!("data:image/{m};base64,{}", b64_encode(&out));
    if url.len() > max_chars {
        return Err("gambar terlalu besar setelah diperkecil; pilih gambar lain".into());
    }
    Ok(url)
}

/// Port validateImageDataUrl.
pub fn validate_image_data_url(v: &str, max_chars: usize) -> Option<&'static str> {
    let re = regex::Regex::new(r"^data:image/(?:png|jpeg|webp|gif);base64,[A-Za-z0-9+/=]+$").expect("regex");
    if !re.is_match(v) {
        return Some("gambar harus berupa data URL png/jpeg/webp/gif");
    }
    (v.len() > max_chars).then_some("gambar terlalu besar")
}

thread_local! {
    static CACHE: std::cell::RefCell<std::collections::HashMap<u64, Arc<gpui::Image>>> = Default::default();
}

/// Data URL → gambar GPUI (di-cache per isi).
pub fn image_of(url: &str) -> Option<Arc<gpui::Image>> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    let key = h.finish();
    if let Some(i) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Some(i);
    }
    let (head, data) = url.split_once(";base64,")?;
    let fmt = match head.strip_prefix("data:image/")? {
        "png" => gpui::ImageFormat::Png,
        "jpeg" => gpui::ImageFormat::Jpeg,
        "gif" => gpui::ImageFormat::Gif,
        "webp" => gpui::ImageFormat::Webp,
        _ => return None,
    };
    let img = Arc::new(gpui::Image::from_bytes(fmt, b64_decode(data)?));
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 64 {
            c.clear();
        }
        c.insert(key, img.clone());
    });
    Some(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for i in 0..TEMPLATES.len() {
            let s = template(i).unwrap();
            let css = gradient_css(&s);
            assert_eq!(validate_gradient_css(&css), None, "{css}");
            assert_eq!(parse_gradient(&css), Some(s));
        }
        assert!(validate_gradient_css("linear-gradient(url(x), red)").is_some());
        assert!(validate_gradient_css("conic-gradient(red, blue)").is_some());
    }

    #[test]
    fn base64() {
        for s in ["", "a", "ab", "abc", "abcd", "hello world!"] {
            assert_eq!(b64_decode(&b64_encode(s.as_bytes())).unwrap(), s.as_bytes());
        }
        assert_eq!(b64_encode(b"hi"), "aGk=");
        assert_eq!(validate_image_data_url("data:image/png;base64,aGk=", 100), None);
        assert!(validate_image_data_url("data:text/html;base64,aGk=", 100).is_some());
    }
}

#[cfg(test)]
mod resize_tests {
    use super::*;

    #[test]
    fn downscales_large_png() {
        let img = image::RgbaImage::from_pixel(3000, 1500, image::Rgba([10, 200, 30, 128]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let (out, m) = resize_image_bytes(&png, 1280, OutFmt::Jpeg(80)).unwrap();
        assert_eq!(m, "jpeg");
        let d = image::load_from_memory(&out).unwrap();
        assert_eq!((d.width(), d.height()), (1280, 640));
        let (out, _) = resize_image_bytes(&png, 64, OutFmt::Png).unwrap();
        let d = image::load_from_memory(&out).unwrap();
        assert_eq!((d.width(), d.height()), (64, 32));
        assert!(resize_image_bytes(b"<svg/>", 64, OutFmt::Png).is_err());
    }
}

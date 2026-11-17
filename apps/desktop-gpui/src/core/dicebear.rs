//! Avatar ilustrasi DiceBear — dirender LOKAL dengan pustaka DiceBear yang sama persis dengan apps/desktop (paket
//! @dicebear/* dibundel oleh scripts/export-dicebear-gpui.ts ke assets/dicebear.js) di interpreter QuickJS tertanam.
//! Tidak ada permintaan jaringan; skrip hanya menghasilkan teks SVG yang lalu dirender GPUI sebagai gambar.
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub const AVATAR_STYLES: [&str; 8] = ["lorelei", "notionists", "openPeeps", "adventurer", "bigSmile", "funEmoji", "pixelArt", "thumbs"];
const BUNDLE: &str = include_str!("../../assets/dicebear.js");

struct Js {
    _rt: rquickjs::Runtime,
    ctx: rquickjs::Context,
}

thread_local! {
    static JS: RefCell<Option<Result<Js, String>>> = const { RefCell::new(None) };
    static IMAGES: RefCell<HashMap<String, Arc<gpui::Image>>> = RefCell::new(HashMap::new());
}

fn with_js<T>(f: impl FnOnce(&rquickjs::Context) -> Result<T, String>) -> Result<T, String> {
    JS.with(|j| {
        let mut j = j.borrow_mut();
        if j.is_none() {
            let init = (|| {
                let rt = rquickjs::Runtime::new().map_err(|e| e.to_string())?;
                let ctx = rquickjs::Context::full(&rt).map_err(|e| e.to_string())?;
                ctx.with(|c| c.eval::<(), _>(BUNDLE).map_err(|e| format!("DiceBear: {e}")))?;
                Ok(Js { _rt: rt, ctx })
            })();
            *j = Some(init);
        }
        match j.as_ref().expect("init") {
            Ok(js) => f(&js.ctx),
            Err(e) => Err(e.clone()),
        }
    })
}

/// Port validateProfileAvatar (bagian ilustrasi).
pub fn valid(style: &str, seed: &str) -> bool {
    AVATAR_STYLES.contains(&style) && !seed.is_empty() && seed.len() <= 64 && seed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// SVG untuk gaya + seed (sama dengan createAvatar(style, { seed, size: 128 })).
pub fn svg(style: &str, seed: &str) -> Result<String, String> {
    if !valid(style, seed) {
        return Err("avatar tidak valid".into());
    }
    with_js(|ctx| {
        ctx.with(|c| {
            let f: rquickjs::Function = c.globals().get("zandorAvatar").map_err(|e| e.to_string())?;
            f.call::<_, String>((style, seed)).map_err(|e| e.to_string())
        })
    })
}

/// Gambar GPUI (di-cache per gaya+seed).
pub fn image(style: &str, seed: &str) -> Option<Arc<gpui::Image>> {
    let key = format!("{style}:{seed}");
    if let Some(i) = IMAGES.with(|m| m.borrow().get(&key).cloned()) {
        return Some(i);
    }
    let s = svg(style, seed).ok().filter(|s| !s.is_empty())?;
    let img = Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Svg, s.into_bytes()));
    IMAGES.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 200 {
            m.clear();
        }
        m.insert(key, img.clone());
    });
    Some(img)
}

#[derive(Clone, Debug, Default)]
pub struct Credit {
    pub title: String,
    pub creator: String,
    pub license: String,
}

/// Atribusi gaya dari metadata paket.
pub fn credit(style: &str) -> Credit {
    let raw = with_js(|ctx| {
        ctx.with(|c| {
            let f: rquickjs::Function = c.globals().get("zandorAvatarCredit").map_err(|e| e.to_string())?;
            f.call::<_, String>((style,)).map_err(|e| e.to_string())
        })
    })
    .unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
    let g = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Credit { title: Some(g("title")).filter(|s| !s.is_empty()).unwrap_or_else(|| style.to_string()), creator: g("creator"), license: g("license") }
}

/// Seed acak 10 karakter (port randomSeed).
pub fn random_seed() -> String {
    let mut b = [0u8; 8];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect::<String>()[..10].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_all_styles_deterministically() {
        for s in AVATAR_STYLES {
            let a = svg(s, "abc123").unwrap();
            assert!(a.starts_with("<svg"), "{s}: {}", &a[..a.len().min(60)]);
            assert_eq!(a, svg(s, "abc123").unwrap());
            assert_ne!(a, svg(s, "zzz999").unwrap());
            assert!(!credit(s).title.is_empty());
        }
        assert!(svg("x", "a").is_err());
        assert!(svg("lorelei", "a b").is_err());
        assert_eq!(random_seed().len(), 10);
        if let Ok(dir) = std::env::var("ZANDOR_DICEBEAR_DUMP") {
            for s in AVATAR_STYLES {
                std::fs::write(format!("{dir}/rs-{s}.svg"), svg(s, "abc123").unwrap()).unwrap();
            }
        }
    }
}

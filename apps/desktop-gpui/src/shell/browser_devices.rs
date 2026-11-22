//! Templat perangkat untuk simulasi lebar layar di panel Browser (viewport CSS, bukan piksel fisik).
//! Ukuran = viewport portrait dalam piksel CSS; laptop/desktop dibuat landscape. UA hanya untuk perangkat seluler.

pub struct Device {
    pub id: &'static str,
    /// android | iphone | pixel | tablet | laptop | desktop (kunci i18n `br.grp.<grup>`)
    pub group: &'static str,
    pub name: &'static str,
    pub w: u32,
    pub h: u32,
    pub mobile: bool,
}

pub const DEVICES: &[Device] = &[
    Device { id: "galaxy-s24", group: "android", name: "Samsung Galaxy S24", w: 360, h: 780, mobile: true },
    Device { id: "redmi-note-13", group: "android", name: "Xiaomi Redmi Note 13", w: 393, h: 851, mobile: true },
    Device { id: "reno-11", group: "android", name: "Oppo Reno 11", w: 412, h: 915, mobile: true },
    Device { id: "iphone-se", group: "iphone", name: "iPhone SE", w: 375, h: 667, mobile: true },
    Device { id: "iphone-15", group: "iphone", name: "iPhone 15", w: 393, h: 852, mobile: true },
    Device { id: "iphone-15-pro-max", group: "iphone", name: "iPhone 15 Pro Max", w: 430, h: 932, mobile: true },
    Device { id: "pixel-8", group: "pixel", name: "Google Pixel 8", w: 412, h: 915, mobile: true },
    Device { id: "pixel-fold", group: "pixel", name: "Google Pixel Fold (terlipat)", w: 360, h: 800, mobile: true },
    Device { id: "ipad-mini", group: "tablet", name: "iPad Mini", w: 744, h: 1133, mobile: true },
    Device { id: "ipad-pro-11", group: "tablet", name: "iPad Pro 11\"", w: 834, h: 1194, mobile: true },
    Device { id: "galaxy-tab-s9", group: "tablet", name: "Samsung Galaxy Tab S9", w: 800, h: 1280, mobile: true },
    Device { id: "laptop-1280", group: "laptop", name: "Laptop 13\" (1280×720)", w: 1280, h: 720, mobile: false },
    Device { id: "laptop-1366", group: "laptop", name: "Laptop 14\" (1366×768)", w: 1366, h: 768, mobile: false },
    Device { id: "macbook-air", group: "laptop", name: "MacBook Air (1440×900)", w: 1440, h: 900, mobile: false },
    Device { id: "laptop-1536", group: "laptop", name: "Laptop 15\" (1536×864)", w: 1536, h: 864, mobile: false },
    Device { id: "desktop-fhd", group: "desktop", name: "Desktop Full HD", w: 1920, h: 1080, mobile: false },
    Device { id: "desktop-qhd", group: "desktop", name: "Desktop QHD", w: 2560, h: 1440, mobile: false },
];

/// Nilai pemilih untuk "tanpa simulasi" (viewport = area panel) dan ukuran bebas.
pub const RESPONSIVE: &str = "responsive";
pub const CUSTOM: &str = "custom";

pub const MIN_DIM: u32 = 200;
pub const MAX_DIM: u32 = 3840;

/// Pilihan zoom: "fit" (muat ke panel, tidak memperbesar) atau persen tetap.
pub const ZOOMS: &[&str] = &["fit", "50", "75", "100", "125", "150"];

pub fn find(id: &str) -> Option<&'static Device> {
    DEVICES.iter().find(|d| d.id == id)
}

pub fn parse_dim(s: &str) -> Option<u32> {
    s.trim().parse::<u32>().ok().map(|n| n.clamp(MIN_DIM, MAX_DIM))
}

/// Faktor skala tampilan. `fit` memuat viewport ke area (maks 100%); tanpa ukuran viewport (mode responsif) `fit` = 100%.
pub fn scale(mode: &str, viewport: Option<(u32, u32)>, avail: (f32, f32)) -> f64 {
    match (mode, viewport) {
        ("fit", Some((w, h))) => {
            let s = (avail.0 / w as f32).min(avail.1 / h as f32).min(1.);
            (s as f64).max(0.1)
        }
        ("fit", None) => 1.,
        (p, _) => p.parse::<f64>().map(|n| (n / 100.).clamp(0.25, 3.)).unwrap_or(1.),
    }
}

/// Bingkai standar per jenis perangkat: (tebal bezel, radius sudut) dalam piksel; None = tanpa bingkai.
pub fn frame(key: &str) -> Option<(f32, f32)> {
    Some(match find(key)?.group {
        "android" | "iphone" | "pixel" => (12., 30.),
        "tablet" => (14., 22.),
        "laptop" => (10., 22.),
        _ => (8., 18.),
    })
}

/// User-agent seluler untuk perangkat (None = pakai UA bawaan WebView2).
pub fn user_agent(d: &Device) -> Option<&'static str> {
    if !d.mobile {
        return None;
    }
    Some(match d.group {
        "iphone" => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
        "tablet" if d.name.starts_with("iPad") => "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
        "tablet" => "Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
        _ => "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dims_and_scale() {
        assert_eq!(parse_dim(" 390 "), Some(390));
        assert_eq!(parse_dim("50"), Some(MIN_DIM));
        assert_eq!(parse_dim("99999"), Some(MAX_DIM));
        assert_eq!(parse_dim("abc"), None);
        assert_eq!(scale("fit", Some((1920, 1080)), (960., 1080.)), 0.5);
        assert_eq!(scale("fit", Some((360, 780)), (960., 1000.)), 1.);
        assert_eq!(scale("fit", None, (960., 600.)), 1.);
        assert_eq!(scale("75", Some((360, 780)), (960., 600.)), 0.75);
    }

    #[test]
    fn templates_are_valid() {
        let mut ids = std::collections::HashSet::new();
        for d in DEVICES {
            assert!(ids.insert(d.id), "id ganda {}", d.id);
            assert!((MIN_DIM..=MAX_DIM).contains(&d.w) && (MIN_DIM..=MAX_DIM).contains(&d.h));
            assert_eq!(user_agent(d).is_some(), d.mobile);
        }
        assert!(find("iphone-15").is_some() && find("nope").is_none());
    }
}

//! Tampilan — port packages/core/src/theme/{tokens,presets,appearance}.ts (bagian tema): preset bawaan, tema
//! kustom (impor/ekspor/hapus), validasi warna, kontras WCAG, penerapan ke `Colors`. Disimpan di appearance.json.
use crate::theme::{mix, Colors};
use gpui::{App, AppContext, Context, Entity, Global, Hsla, Rgba};
use serde_json::{json, Map, Value};
use std::path::PathBuf;

pub const COLOR_KEYS: [&str; 18] = [
    "bg", "surface", "raised", "hover", "active", "border", "border-strong", "fg", "fg-muted", "fg-subtle", "accent",
    "on-accent", "focus", "backdrop", "danger", "success", "warning", "shadow-color",
];
pub const SWATCH_KEYS: [&str; 5] = ["bg", "raised", "fg", "accent", "border-strong"];
pub const DEFAULT_PRESET: &str = "zandor";
pub const TYPE_PROPS: [&str; 4] = ["size", "weight", "lh", "tracking"];
pub const FONT_SLOTS: [&str; 3] = ["ui", "code", "terminal"];
/// Kunci gaya project yang diisi lewat kolom teks.
pub const PROJECT_STYLE_KEYS: [&str; 9] = ["logo", "border", "background", "fg", "gradient", "overlayColor", "overlayOpacity", "imageY", "imageZoom"];
pub const PROJECT_OVERLAYS: [&str; 5] = ["full", "gradient", "gradient-rtl", "gradient-ttb", "gradient-btt"];

fn int_in(v: &str, min: u32, max: u32) -> Option<&'static str> {
    let ok = !v.is_empty() && v.len() <= 3 && v.chars().all(|c| c.is_ascii_digit()) && v.parse::<u32>().is_ok_and(|n| (min..=max).contains(&n));
    (!ok).then_some("angka di luar rentang")
}

/// Port validateLogo.
pub fn validate_logo(v: &str) -> Option<&'static str> {
    if v.trim().is_empty() {
        return Some("logo kosong");
    }
    if v.chars().count() > 4 {
        return Some("logo maksimal 4 karakter");
    }
    if v.chars().any(|c| "<>&\"'`\\".contains(c) || (c as u32) < 32) {
        return Some("logo mengandung karakter terlarang");
    }
    None
}

pub fn validate_project_style(key: &str, v: &str) -> Option<&'static str> {
    use crate::core::gradient as g;
    match key {
        "logo" => validate_logo(v),
        "gradient" => g::validate_gradient_css(v).map(|_| "gradien tidak valid"),
        "image" => g::validate_image_data_url(v, g::MAX_STYLE_IMAGE_CHARS),
        "logoImage" => g::validate_image_data_url(v, g::MAX_LOGO_IMAGE_CHARS),
        "imageY" | "overlayOpacity" => int_in(v, 0, 100),
        "imageZoom" => int_in(v, 100, 300),
        "overlay" => (!PROJECT_OVERLAYS.contains(&v)).then_some("overlay tidak dikenal"),
        _ => (!is_color(v)).then_some("warna tidak valid"),
    }
}

pub const CHAT_STYLE_KEYS: [&str; 10] = ["width", "composerRatio", "font", "size", "weight", "opacity", "lineHeight", "tracking", "gap", "codeSize"];

/// Port validateChat. None = valid.
pub fn validate_chat(key: &str, v: &Value) -> Option<&'static str> {
    let num = v.as_f64();
    match key {
        "opacity" => (!num.is_some_and(|n| (0.3..=1.).contains(&n))).then_some("opasitas harus 0.3–1"),
        "weight" => (!num.is_some_and(|n| n.fract() == 0. && (100. ..=900.).contains(&n))).then_some("ketebalan harus 100–900"),
        "font" => match v.as_str() {
            Some(s) => validate_font_family(s),
            None => Some("nama font kosong"),
        },
        "lineHeight" => (!num.is_some_and(|n| (1. ..=2.4).contains(&n))).then_some("tinggi baris harus 1–2.4"),
        "composerRatio" => (!num.is_some_and(|n| (1. ..=1.6).contains(&n))).then_some("rasio composer harus 1–1.6"),
        "tracking" => {
            let Some(n) = v.as_str().and_then(|s| s.strip_suffix("em")).filter(|n| is_dec(n.strip_prefix('-').unwrap_or(n))) else {
                return Some("nilai harus dalam em, mis. 0.01em");
            };
            let n: f32 = n.parse().unwrap_or(9.);
            (!(-0.05..=0.2).contains(&n)).then_some("jarak huruf harus -0.05em–0.2em")
        }
        _ => {
            let Some(n) = v.as_str().and_then(|s| s.strip_suffix("rem")).filter(|n| is_dec(n)) else {
                return Some("nilai harus dalam rem, mis. 0.9rem");
            };
            let n: f32 = n.parse().unwrap_or(-1.);
            match key {
                "size" => (!(0.6..=2.).contains(&n)).then_some("ukuran harus 0.6rem–2rem"),
                "codeSize" => (!(0.6..=1.6).contains(&n)).then_some("ukuran kode harus 0.6rem–1.6rem"),
                "width" => (!(28. ..=120.).contains(&n)).then_some("lebar harus 28rem–120rem"),
                _ => (!(0. ..=3.).contains(&n)).then_some("jarak harus 0rem–3rem"),
            }
        }
    }
}

fn is_dec(s: &str) -> bool {
    // ^\d*\.?\d+$
    let (a, b) = s.split_once('.').unwrap_or(("", s));
    let b_ok = !b.is_empty() && b.chars().all(|c| c.is_ascii_digit());
    let a_ok = a.chars().all(|c| c.is_ascii_digit());
    if s.contains('.') { a_ok && b_ok } else { b_ok }
}

/// Validasi satu properti tipografi (port validateTypeValue). None = valid.
pub fn validate_type_value(prop: &str, v: &str) -> Option<&'static str> {
    match prop {
        "size" => {
            let Some(n) = v.strip_suffix("rem").filter(|n| is_dec(n)) else { return Some("ukuran harus dalam rem, mis. 0.875rem") };
            let n: f32 = n.parse().unwrap_or(0.);
            (!(0.5..=4.).contains(&n)).then_some("ukuran harus 0.5rem–4rem")
        }
        "tracking" => {
            if v == "0" {
                return None;
            }
            let Some(n) = v.strip_suffix("em").filter(|n| is_dec(n.strip_prefix('-').unwrap_or(n))) else {
                return Some("jarak huruf harus dalam em, mis. -0.01em");
            };
            let n: f32 = n.parse().unwrap_or(9.);
            (!(-0.1..=0.5).contains(&n)).then_some("jarak huruf harus -0.1em–0.5em")
        }
        "weight" => {
            if v.len() != 3 || !v.chars().all(|c| c.is_ascii_digit()) {
                return Some("ketebalan harus 100–900");
            }
            let n: u32 = v.parse().unwrap_or(0);
            (!(100..=900).contains(&n)).then_some("ketebalan harus 100–900")
        }
        _ => {
            if !is_dec(v) {
                return Some("tinggi baris harus angka, mis. 1.5");
            }
            let n: f32 = v.parse().unwrap_or(0.);
            (!(1. ..=3.).contains(&n)).then_some("tinggi baris harus 1–3")
        }
    }
}

/// Daftar font: nama dan koma saja (port validateFontFamily).
pub fn validate_font_family(v: &str) -> Option<&'static str> {
    if v.trim().is_empty() {
        return Some("nama font kosong");
    }
    if v.chars().count() > 200 {
        return Some("nama font terlalu panjang");
    }
    if !v.chars().all(|c| c.is_alphanumeric() || " ,\"'._-".contains(c)) {
        return Some("nama font hanya boleh huruf, angka, spasi, koma, tanda kutip");
    }
    let l = v.to_lowercase();
    if l.contains("url") || l.contains("expression") || l.contains("import") {
        return Some("nama font tidak valid");
    }
    None
}

/// Font pertama dari daftar CSS (GPUI memakai satu keluarga).
pub fn first_family(v: &str) -> String {
    v.split(',').next().unwrap_or("").trim().trim_matches(['"', '\'']).to_string()
}

pub type ColorMap = Vec<(String, String)>;

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeDef {
    pub id: String,
    pub name: String,
    pub dark: Option<ColorMap>,
    pub light: Option<ColorMap>,
    pub builtin: bool,
}

impl ThemeDef {
    pub fn colors(&self, dark: bool) -> &[(String, String)] {
        let m = if dark { &self.dark } else { &self.light };
        m.as_deref().unwrap_or(&[])
    }
    pub fn get(&self, dark: bool, key: &str) -> Option<&str> {
        self.colors(dark).iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    fn to_json(&self) -> Value {
        let m = |c: &Option<ColorMap>| c.as_ref().map(|c| Value::Object(c.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
        let mut o = json!({ "id": self.id, "name": self.name });
        if let Some(d) = m(&self.dark) {
            o["dark"] = d;
        }
        if let Some(l) = m(&self.light) {
            o["light"] = l;
        }
        o
    }
}

fn map_of(v: Option<&Value>) -> Option<ColorMap> {
    v.and_then(Value::as_object).map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
}

pub fn presets() -> &'static [ThemeDef] {
    static P: std::sync::LazyLock<Vec<ThemeDef>> = std::sync::LazyLock::new(|| {
        let v: Value = serde_json::from_str(include_str!("../../assets/presets.json")).unwrap_or(json!([]));
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|p| ThemeDef {
                        id: p["id"].as_str().unwrap_or("").into(),
                        name: p["name"].as_str().unwrap_or("").into(),
                        dark: map_of(p.get("dark")),
                        light: map_of(p.get("light")),
                        builtin: true,
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
    &P
}

// ---------- warna ----------

fn hex_digits(s: &str) -> Option<Vec<u8>> {
    let h = s.strip_prefix('#')?;
    if ![3, 4, 6, 8].contains(&h.len()) || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = if h.len() <= 4 { h.chars().flat_map(|c| [c, c]).collect() } else { h.into() };
    (0..full.len()).step_by(2).map(|i| u8::from_str_radix(&full[i..i + 2], 16).ok()).collect()
}

/// Warna CSS yang diizinkan: heks 3/4/6/8 digit, rgb()/rgba()/hsl()/hsla().
pub fn parse_color(s: &str) -> Option<Hsla> {
    let s = s.trim();
    if let Some(b) = hex_digits(s) {
        let a = if b.len() == 4 { b[3] as f32 / 255. } else { 1. };
        return Some(Rgba { r: b[0] as f32 / 255., g: b[1] as f32 / 255., b: b[2] as f32 / 255., a }.into());
    }
    let lower = s.to_lowercase();
    let (func, rest) = lower.split_once('(')?;
    let inner = rest.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(|c: char| c == ',' || c == '/' || c.is_whitespace()).filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 || parts.len() > 4 {
        return None;
    }
    let num = |p: &str| -> Option<(f32, bool)> {
        let pct = p.ends_with('%');
        p.trim_end_matches('%').parse::<f32>().ok().filter(|n| n.is_finite() && *n >= 0.).map(|n| (n, pct))
    };
    let vals: Vec<(f32, bool)> = parts.iter().map(|p| num(p)).collect::<Option<_>>()?;
    let alpha = vals.get(3).map(|(n, pct)| if *pct { n / 100. } else { *n }).unwrap_or(1.).clamp(0., 1.);
    match func {
        "rgb" | "rgba" => {
            let ch = |(n, pct): (f32, bool)| (if pct { n / 100. } else { n / 255. }).clamp(0., 1.);
            Some(Rgba { r: ch(vals[0]), g: ch(vals[1]), b: ch(vals[2]), a: alpha }.into())
        }
        "hsl" | "hsla" => Some(gpui::hsla((vals[0].0 % 360.) / 360., (vals[1].0 / 100.).clamp(0., 1.), (vals[2].0 / 100.).clamp(0., 1.), alpha)),
        _ => None,
    }
}

pub fn is_color(s: &str) -> bool {
    parse_color(s).is_some()
}

fn luminance(b: &[u8]) -> f32 {
    let f = |v: u8| {
        let s = v as f32 / 255.;
        if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(b[0]) + 0.7152 * f(b[1]) + 0.0722 * f(b[2])
}

/// Rasio kontras WCAG 2 dua warna heks; None bila bukan heks.
pub fn contrast_ratio(a: &str, b: &str) -> Option<f32> {
    let (x, y) = (luminance(&hex_digits(a)?), luminance(&hex_digits(b)?));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    Some((hi + 0.05) / (lo + 0.05))
}

/// Terapkan token warna ke palet, lalu hitung ulang token turunan (frame, bubble) seperti tokens.css.
pub fn apply_colors(c: &mut Colors, map: &[(String, String)], dark: bool) {
    for (k, v) in map {
        let Some(col) = parse_color(v) else { continue };
        let slot = match k.as_str() {
            "bg" => &mut c.bg,
            "surface" => &mut c.surface,
            "raised" => &mut c.raised,
            "hover" => &mut c.hover,
            "active" => &mut c.active,
            "border" => &mut c.border,
            "border-strong" => &mut c.border_strong,
            "fg" => &mut c.fg,
            "fg-muted" => &mut c.fg_muted,
            "fg-subtle" => &mut c.fg_subtle,
            "accent" => &mut c.accent,
            "on-accent" => &mut c.on_accent,
            "focus" => &mut c.focus,
            "backdrop" => &mut c.backdrop,
            "danger" => &mut c.danger,
            "success" => &mut c.success,
            "warning" => &mut c.warning,
            "shadow-color" => &mut c.shadow,
            _ => continue,
        };
        *slot = col;
    }
    c.frame = mix(c.bg, if dark { 0.5 } else { 0.9 }, gpui::black());
    c.bubble = mix(c.fg, 0.08, c.bg);
}

/// Validasi tema mentah (impor). Nilai salah ditolak, tidak diperbaiki diam-diam.
pub fn sanitize_theme(raw: &Value) -> Result<ThemeDef, Vec<String>> {
    let Some(o) = raw.as_object() else { return Err(vec!["tema harus berupa objek JSON".into()]) };
    let mut errors = vec![];
    let id = o.get("id").and_then(Value::as_str).unwrap_or("");
    let id_ok = !id.is_empty()
        && id.len() <= 40
        && id.split('-').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    if !id_ok {
        errors.push("id harus kebab-case (maks 40)".to_string());
    }
    let name = o.get("name").and_then(Value::as_str).unwrap_or("");
    if name.trim().is_empty() || name.chars().count() > 40 {
        errors.push("name wajib (maks 40 karakter)".into());
    }
    let mut modes: [Option<ColorMap>; 2] = [None, None];
    for (n, mode) in ["dark", "light"].iter().enumerate() {
        let Some(m) = o.get(*mode) else { continue };
        let Some(m) = m.as_object() else {
            errors.push(format!("{mode}: harus objek"));
            continue;
        };
        let mut out = vec![];
        for (k, v) in m {
            if !COLOR_KEYS.contains(&k.as_str()) {
                errors.push(format!("{mode}.{k}: token tidak dikenal"));
            } else if !v.as_str().is_some_and(is_color) {
                errors.push(format!("{mode}.{k}: warna tidak valid"));
            } else {
                out.push((k.clone(), v.as_str().unwrap().to_string()));
            }
        }
        modes[n] = Some(out);
    }
    if modes[0].is_none() && modes[1].is_none() {
        errors.push("tema harus punya `dark` dan/atau `light`".into());
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let [dark, light] = modes;
    Ok(ThemeDef { id: id.into(), name: name.trim().into(), dark, light, builtin: false })
}

// ---------- store ----------

pub struct AppearanceStore {
    path: PathBuf,
    /// Isi appearance.json apa adanya (kunci yang belum diport dipertahankan).
    raw: Map<String, Value>,
    pub preset: String,
    pub custom: Vec<ThemeDef>,
    /// "h1" → { "size": "1.5rem", … }
    pub typography: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    pub fonts: std::collections::BTreeMap<String, String>,
    pub chat: Map<String, Value>,
    /// Override pintasan: id perintah → combo ("Ctrl+Shift+K").
    pub shortcuts: std::collections::BTreeMap<String, String>,
    /// Gaya project: id project → { logo, border, background, fg, … }.
    pub projects: std::collections::BTreeMap<String, Map<String, Value>>,
    /// Tema editor terpilih ("auto", bawaan Monaco, atau id tema terpasang).
    pub editor_theme: String,
    pub editor_themes: Vec<crate::core::editor_theme::EditorTheme>,
}
struct Handle(Entity<AppearanceStore>);
impl Global for Handle {}

const MAX_CUSTOM: usize = 50;

impl AppearanceStore {
    pub fn init(path: PathBuf, cx: &mut App) -> Entity<Self> {
        let raw: Map<String, Value> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let custom: Vec<ThemeDef> = raw
            .get("customThemes")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|t| sanitize_theme(t).ok()).filter(|t| !presets().iter().any(|p| p.id == t.id)).collect())
            .unwrap_or_default();
        let mut preset = raw.get("preset").and_then(Value::as_str).unwrap_or(DEFAULT_PRESET).to_string();
        if !presets().iter().any(|p| p.id == preset) && !custom.iter().any(|c| c.id == preset) {
            preset = DEFAULT_PRESET.into();
        }
        let mut typography: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> = Default::default();
        if let Some(o) = raw.get("typography").and_then(Value::as_object) {
            for v in crate::theme::TYPE_VARS {
                for p in TYPE_PROPS {
                    if let Some(x) = o.get(v).and_then(|m| m.get(p)).and_then(Value::as_str) {
                        if validate_type_value(p, x).is_none() {
                            typography.entry(v.to_string()).or_default().insert(p.to_string(), x.to_string());
                        }
                    }
                }
            }
        }
        let fonts = raw
            .get("fonts")
            .and_then(Value::as_object)
            .map(|o| {
                FONT_SLOTS
                    .iter()
                    .filter_map(|k| o.get(*k).and_then(Value::as_str).filter(|v| validate_font_family(v).is_none()).map(|v| (k.to_string(), v.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        let chat: Map<String, Value> = raw
            .get("chat")
            .and_then(Value::as_object)
            .map(|o| o.iter().filter(|(k, v)| CHAT_STYLE_KEYS.contains(&k.as_str()) && validate_chat(k, v).is_none()).map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let shortcuts = raw
            .get("shortcuts")
            .and_then(Value::as_object)
            .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
            .unwrap_or_default();
        let projects = raw
            .get("projects")
            .and_then(Value::as_object)
            .map(|o| o.iter().filter_map(|(k, v)| v.as_object().map(|m| (k.clone(), m.clone()))).collect())
            .unwrap_or_default();
        let editor_theme = raw.get("editorTheme").and_then(Value::as_str).unwrap_or("auto").to_string();
        let editor_themes = raw
            .get("editorThemes")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(crate::core::editor_theme::sanitize_editor_theme).collect())
            .unwrap_or_default();
        let e = cx.new(|_| Self { path, raw, preset, custom, typography, fonts, chat, shortcuts, projects, editor_theme, editor_themes });
        cx.set_global(Handle(e.clone()));
        e
    }
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<Handle>().0.clone()
    }

    pub fn themes(&self) -> Vec<ThemeDef> {
        presets().iter().cloned().chain(self.custom.iter().cloned()).collect()
    }
    pub fn find(&self, id: &str) -> Option<ThemeDef> {
        presets().iter().find(|p| p.id == id).cloned().or_else(|| self.custom.iter().find(|c| c.id == id).cloned())
    }
    /// Terapkan tema aktif ke palet.
    pub fn apply(&self, c: &mut Colors, dark: bool) {
        if let Some(t) = self.find(&self.preset) {
            apply_colors(c, t.colors(dark), dark);
        }
    }

    /// Terapkan tipografi & font ke tema.
    pub fn apply_type(&self, t: &mut crate::theme::Theme) {
        for (n, v) in crate::theme::TYPE_VARS.iter().enumerate() {
            let Some(m) = self.typography.get(*v) else { continue };
            let ty = &mut t.types[n];
            if let Some(x) = m.get("size").and_then(|s| s.trim_end_matches("rem").parse().ok()) {
                ty.size = x;
            }
            if let Some(x) = m.get("weight").and_then(|s| s.parse().ok()) {
                ty.weight = x;
            }
            if let Some(x) = m.get("lh").and_then(|s| s.parse().ok()) {
                ty.lh = x;
            }
            if let Some(x) = m.get("tracking").and_then(|s| s.trim_end_matches("em").parse().ok()) {
                ty.tracking = x;
            }
        }
        if let Some(f) = self.fonts.get("ui") {
            t.font_ui = first_family(f).into();
        }
        if let Some(f) = self.fonts.get("code") {
            t.font_code = first_family(f).into();
            t.font_terminal = t.font_code.clone();
        }
        if let Some(f) = self.fonts.get("terminal") {
            t.font_terminal = first_family(f).into();
        }
        let n = |k: &str| self.chat.get(k).and_then(Value::as_f64).map(|x| x as f32);
        let u = |k: &str, unit: &str| self.chat.get(k).and_then(Value::as_str).and_then(|s| s.strip_suffix(unit)).and_then(|s| s.parse::<f32>().ok());
        t.chat = crate::theme::ChatVars {
            opacity: n("opacity"),
            weight: n("weight"),
            size: u("size", "rem"),
            gap: u("gap", "rem"),
            font: self.chat.get("font").and_then(Value::as_str).map(|f| first_family(f).into()),
            width: u("width", "rem"),
            line: n("lineHeight"),
            tracking: u("tracking", "em"),
            code_size: u("codeSize", "rem"),
            composer_ratio: n("composerRatio"),
        };
    }

    pub fn project_style(&self, id: &str, key: &str) -> Option<String> {
        let v = self.projects.get(id)?.get(key)?.as_str()?;
        validate_project_style(key, v).is_none().then(|| v.to_string())
    }

    /// None = hapus. Kunci lain (mis. gradien dari aplikasi TS) dipertahankan.
    pub fn set_project_style(&mut self, id: &str, key: &str, value: Option<&str>, cx: &mut Context<Self>) -> Option<&'static str> {
        if let Some(v) = value {
            if let Some(e) = validate_project_style(key, v) {
                return Some(e);
            }
        }
        let m = self.projects.entry(id.to_string()).or_default();
        match value {
            Some(v) => {
                m.insert(key.into(), json!(v));
            }
            None => {
                m.remove(key);
            }
        }
        if m.is_empty() {
            self.projects.remove(id);
        }
        self.save(cx);
        None
    }

    /// Simpan override pintasan (None = bawaan). Validasi bentrok dilakukan pemanggil.
    pub fn set_shortcut(&mut self, id: &str, combo: Option<&str>, cx: &mut Context<Self>) {
        match combo {
            Some(c) => {
                self.shortcuts.insert(id.into(), c.into());
            }
            None => {
                self.shortcuts.remove(id);
            }
        }
        self.save(cx);
    }

    /// None = kembali ke bawaan.
    pub fn set_chat(&mut self, key: &str, value: Option<Value>, cx: &mut Context<Self>) -> Option<&'static str> {
        match value {
            Some(v) => {
                if let Some(e) = validate_chat(key, &v) {
                    return Some(e);
                }
                self.chat.insert(key.into(), v);
            }
            None => {
                self.chat.remove(key);
            }
        }
        self.save(cx);
        None
    }

    /// None = kembali ke default. Mengembalikan galat.
    pub fn set_typography(&mut self, var: &str, prop: &str, value: Option<&str>, cx: &mut Context<Self>) -> Option<&'static str> {
        if let Some(v) = value {
            if let Some(e) = validate_type_value(prop, v) {
                return Some(e);
            }
        }
        let cur = self.typography.entry(var.to_string()).or_default();
        match value {
            Some(v) => {
                cur.insert(prop.into(), v.into());
            }
            None => {
                cur.remove(prop);
            }
        }
        if cur.is_empty() {
            self.typography.remove(var);
        }
        self.save(cx);
        None
    }

    pub fn set_font(&mut self, slot: &str, value: Option<&str>, cx: &mut Context<Self>) -> Option<&'static str> {
        match value {
            Some(v) => {
                if let Some(e) = validate_font_family(v) {
                    return Some(e);
                }
                self.fonts.insert(slot.into(), v.into());
            }
            None => {
                self.fonts.remove(slot);
            }
        }
        self.save(cx);
        None
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.raw.insert("typography".into(), serde_json::to_value(&self.typography).unwrap_or(json!({})));
        self.raw.insert("fonts".into(), serde_json::to_value(&self.fonts).unwrap_or(json!({})));
        self.raw.insert("chat".into(), Value::Object(self.chat.clone()));
        self.raw.insert("projects".into(), serde_json::to_value(&self.projects).unwrap_or(json!({})));
        self.raw.insert("shortcuts".into(), serde_json::to_value(&self.shortcuts).unwrap_or(json!({})));
        self.raw.insert("editorTheme".into(), json!(self.editor_theme));
        self.raw.insert("editorThemes".into(), Value::Array(self.editor_themes.iter().map(|t| t.to_json()).collect()));
        self.raw.insert("version".into(), json!(1));
        self.raw.insert("preset".into(), json!(self.preset));
        self.raw.insert("customThemes".into(), Value::Array(self.custom.iter().map(ThemeDef::to_json).collect()));
        if let Some(d) = self.path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Err(e) = std::fs::write(&self.path, serde_json::to_string_pretty(&self.raw).unwrap_or_default()) {
            eprintln!("[zandor] gagal menyimpan tampilan: {e}");
        }
        cx.notify();
    }

    pub fn set_preset(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.find(id).is_some() {
            self.preset = id.into();
            self.save(cx);
        }
    }

    pub fn save_custom(&mut self, theme: ThemeDef, cx: &mut Context<Self>) -> Result<(), Vec<String>> {
        let t = sanitize_theme(&theme.to_json())?;
        if presets().iter().any(|p| p.id == t.id) {
            return Err(vec!["id bentrok dengan preset bawaan".into()]);
        }
        match self.custom.iter().position(|c| c.id == t.id) {
            Some(i) => self.custom[i] = t,
            None if self.custom.len() >= MAX_CUSTOM => return Err(vec![format!("maksimal {MAX_CUSTOM} tema kustom")]),
            None => self.custom.push(t),
        }
        self.save(cx);
        Ok(())
    }

    pub fn import_theme(&mut self, text: &str, cx: &mut Context<Self>) -> Result<String, Vec<String>> {
        let raw: Value = serde_json::from_str(text).map_err(|_| vec!["bukan JSON yang valid".to_string()])?;
        if text.len() > 100_000 {
            return Err(vec!["berkas terlalu besar (maks 100 KB)".into()]);
        }
        let mut t = sanitize_theme(&raw)?;
        if presets().iter().any(|p| p.id == t.id) {
            t.id = format!("{}-impor", t.id);
        }
        let id = t.id.clone();
        self.save_custom(t, cx)?;
        Ok(id)
    }

    pub fn export_theme(&self, id: &str) -> Option<String> {
        let t = self.find(id)?;
        Some(format!("{}\n", serde_json::to_string_pretty(&t.to_json()).ok()?))
    }

    pub fn remove_custom(&mut self, id: &str, cx: &mut Context<Self>) {
        self.custom.retain(|c| c.id != id);
        if self.preset == id {
            self.preset = DEFAULT_PRESET.into();
        }
        self.save(cx);
    }

    pub fn reset_all(&mut self, cx: &mut Context<Self>) {
        let profile_keep: Vec<(String, Value)> = self.raw.iter().filter(|(k, _)| k.as_str() == "profile").map(|(k, v)| (k.clone(), v.clone())).collect();
        self.raw = profile_keep.into_iter().collect();
        self.preset = DEFAULT_PRESET.into();
        self.custom.clear();
        self.typography.clear();
        self.fonts.clear();
        self.chat.clear();
        self.shortcuts.clear();
        self.projects.clear();
        self.editor_theme = "auto".into();
        self.editor_themes.clear();
        self.save(cx);
    }

    /// Terapkan warna sintaks dari tema editor terpilih.
    pub fn apply_editor(&self, c: &mut crate::theme::Colors) {
        let list = crate::core::editor_theme::code_colors_for(&self.editor_theme, &self.editor_themes);
        crate::core::editor_theme::apply_code_colors(c, &list);
    }

    pub fn set_editor_theme(&mut self, id: &str, cx: &mut Context<Self>) {
        self.editor_theme = id.to_string();
        self.save(cx);
    }

    /// Pasang/ganti tema editor (maks 30).
    pub fn install_editor_theme(&mut self, t: crate::core::editor_theme::EditorTheme, cx: &mut Context<Self>) -> Result<(), String> {
        match self.editor_themes.iter().position(|x| x.id == t.id) {
            Some(i) => self.editor_themes[i] = t,
            None if self.editor_themes.len() >= crate::core::editor_theme::MAX_EDITOR_THEMES => return Err("terlalu banyak tema editor".into()),
            None => self.editor_themes.push(t),
        }
        self.save(cx);
        Ok(())
    }

    pub fn remove_editor_theme(&mut self, id: &str, cx: &mut Context<Self>) {
        self.editor_themes.retain(|t| t.id != id);
        if self.editor_theme == id {
            self.editor_theme = "auto".into();
        }
        self.save(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        assert!(is_color("#abc") && is_color("#aabbccdd") && is_color("rgb(1, 2, 3)") && is_color("hsl(120 50% 50% / 0.5)"));
        assert!(!is_color("red") && !is_color("#12") && !is_color("url(x)") && !is_color("rgb(1,2)"));
        let r = contrast_ratio("#000000", "#ffffff").unwrap();
        assert!((r - 21.).abs() < 0.01);
    }

    #[test]
    fn presets_load() {
        let p = presets();
        assert!(p.len() >= 7);
        assert_eq!(p[0].id, DEFAULT_PRESET);
        assert!(p.iter().all(|t| t.colors(true).iter().all(|(_, v)| is_color(v))));
    }

    #[test]
    fn sanitize() {
        let ok = sanitize_theme(&json!({"id": "laut-biru", "name": "Laut", "dark": {"bg": "#001122"}})).unwrap();
        assert_eq!(ok.get(true, "bg"), Some("#001122"));
        let e = sanitize_theme(&json!({"id": "Bad Id", "name": "", "dark": {"nope": "#000", "fg": "red"}})).unwrap_err();
        assert_eq!(e.len(), 4);
        assert!(sanitize_theme(&json!({"id": "a", "name": "A"})).is_err());
    }

    #[test]
    fn type_values() {
        assert_eq!(validate_type_value("size", "0.875rem"), None);
        assert!(validate_type_value("size", "5rem").is_some());
        assert!(validate_type_value("size", "12px").is_some());
        assert_eq!(validate_type_value("weight", "600"), None);
        assert!(validate_type_value("weight", "60").is_some());
        assert_eq!(validate_type_value("lh", "1.5"), None);
        assert!(validate_type_value("lh", "4").is_some());
        assert_eq!(validate_type_value("tracking", "-0.01em"), None);
        assert_eq!(validate_type_value("tracking", "0"), None);
        assert_eq!(validate_font_family("Inter, sans-serif"), None);
        assert!(validate_font_family("x; url(a)").is_some());
        assert_eq!(first_family("\"Fira Code\", monospace"), "Fira Code");
    }

    #[test]
    fn chat_values() {
        assert_eq!(validate_chat("opacity", &json!(0.5)), None);
        assert!(validate_chat("opacity", &json!(0.1)).is_some());
        assert_eq!(validate_chat("width", &json!("46rem")), None);
        assert!(validate_chat("width", &json!("10rem")).is_some());
        assert_eq!(validate_chat("tracking", &json!("0.01em")), None);
        assert!(validate_chat("weight", &json!(450.5)).is_some());
    }

    #[test]
    fn logos() {
        assert_eq!(validate_logo("🚀"), None);
        assert!(validate_logo("abcde").is_some());
        assert!(validate_logo("<a>").is_some());
        assert_eq!(validate_project_style("border", "#ff0000"), None);
        assert!(validate_project_style("background", "red").is_some());
    }

    #[test]
    fn apply() {
        let mut c = Colors::dark();
        apply_colors(&mut c, &[("bg".into(), "#ff0000".into())], true);
        assert!((c.bg.to_rgb().r - 1.).abs() < 1e-3);
    }
}

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
        let e = cx.new(|_| Self { path, raw, preset, custom });
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

    fn save(&mut self, cx: &mut Context<Self>) {
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
    fn apply() {
        let mut c = Colors::dark();
        apply_colors(&mut c, &[("bg".into(), "#ff0000".into())], true);
        assert!((c.bg.to_rgb().r - 1.).abs() < 1e-3);
    }
}

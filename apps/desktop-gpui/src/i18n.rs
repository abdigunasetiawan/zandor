//! i18n — port dari packages/i18n. Katalog diekspor ke JSON oleh scripts/export-i18n-gpui.ts
//! (sumber kebenaran tetap packages/i18n/src/locales/*.ts).
#![allow(dead_code)]
use gpui::Global;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    Id,
    En,
}
pub const DEFAULT_LOCALE: Locale = Locale::Id;

/// Pilih locale dari tag bahasa (mis. "en-US"), jatuh ke default.
pub fn detect_locale(tag: Option<&str>) -> Locale {
    let base = tag.unwrap_or("").to_lowercase();
    match base.split(['-', '_']).next().unwrap_or("") {
        "en" => Locale::En,
        "id" => Locale::Id,
        _ => DEFAULT_LOCALE,
    }
}

#[derive(Clone)]
pub struct I18n {
    pub locale: Locale,
    msgs: std::sync::Arc<HashMap<String, String>>,
}
impl Global for I18n {}

impl I18n {
    pub fn new(locale: Locale) -> Self {
        let src = match locale {
            Locale::Id => include_str!("../assets/i18n/id.json"),
            Locale::En => include_str!("../assets/i18n/en.json"),
        };
        Self {
            locale,
            msgs: std::sync::Arc::new(serde_json::from_str(src).expect("katalog i18n valid")),
        }
    }
    pub fn t(&self, key: &str) -> String {
        self.msgs.get(key).cloned().unwrap_or_else(|| key.to_string())
    }
    /// Ganti `{nama}`; placeholder tanpa nilai dibiarkan apa adanya (sama dengan versi TS).
    pub fn tp(&self, key: &str, params: &[(&str, &str)]) -> String {
        let mut out = self.t(key);
        for (k, v) in params {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Nomor panel itu internal (tidak tampil di UI dan bergeser saat panel dibuka/ditutup): teks untuk pengguna dan model
    /// harus memakai nama panel (Sidebar, Chat, Dokumen, Alat, Browser). Pintasan Ctrl+1..5 dikecualikan (memang bernomor).
    #[test]
    fn tanpa_nomor_panel_di_teks_pengguna_dan_model() {
        let nomor = |t: &str| {
            let l = t.to_lowercase();
            l.match_indices("panel ").any(|(i, m)| l[i + m.len()..].chars().next().is_some_and(|c| c.is_ascii_digit()))
        };
        for (nama, src) in [("id.json", include_str!("../assets/i18n/id.json")), ("en.json", include_str!("../assets/i18n/en.json"))] {
            let v: std::collections::BTreeMap<String, String> = serde_json::from_str(src).unwrap();
            for (k, t) in v {
                assert!(k == "tour.keys.panels" || !nomor(&t), "{nama}:{k} menyebut nomor panel: {t}");
            }
        }
        assert!(!nomor(include_str!("../assets/settings.schema.json")), "settings.schema.json menyebut nomor panel");
        // Hasil tool yang dibaca model lalu diulang ke pengguna: abaikan komentar (// ///).
        for (nama, src) in [("agent_control.rs", include_str!("shell/agent_control.rs")), ("browser_agent.rs", include_str!("shell/browser_agent.rs")), ("root.rs", include_str!("shell/root.rs"))] {
            for (n, line) in src.lines().enumerate() {
                if !line.trim_start().starts_with("//") {
                    assert!(!nomor(line), "{nama}:{} menyebut nomor panel: {line}", n + 1);
                }
            }
        }
    }

    #[test]
    fn katalog_sama() {
        let (id, en) = (I18n::new(Locale::Id), I18n::new(Locale::En));
        let mut a: Vec<_> = id.msgs.keys().collect();
        let mut b: Vec<_> = en.msgs.keys().collect();
        a.sort();
        b.sort();
        assert_eq!(a, b);
        assert_eq!(en.tp("app.version", &[("version", "1.0")]), "Version 1.0");
        assert_eq!(detect_locale(Some("en-US")), Locale::En);
    }
}

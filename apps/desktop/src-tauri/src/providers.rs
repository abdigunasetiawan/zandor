// Provider model kustom (endpoint kompatibel OpenAI). Daftar disimpan Zandor di `app_data/providers.json`
// (di luar repo). Kunci API TIDAK pernah masuk `OPENCODE_CONFIG_CONTENT`: konfigurasi hanya memuat rujukan
// `{env:ZANDOR_SECRET_PROVIDER_<ID>}`, nilainya dikirim lewat environment proses engine. Variabel berawalan
// `ZANDOR_SECRET_` dibersihkan plugin guard dari proses anak engine (ADR-022).
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

const MAX_BYTES: u64 = 256 * 1024;
const VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
pub struct ProviderModel {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<ProviderModel>,
}

#[derive(Deserialize)]
struct File {
    version: u32,
    providers: Vec<Provider>,
}

impl Provider {
    /// Nama variabel lingkungan pembawa kunci: `ZANDOR_SECRET_PROVIDER_<ID>` (id huruf besar, `-` → `_`).
    pub fn env_name(&self) -> String {
        format!(
            "ZANDOR_SECRET_PROVIDER_{}",
            self.id.to_ascii_uppercase().replace('-', "_")
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        let ok_id = !self.id.is_empty()
            && self.id.len() <= 40
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !ok_id {
            return Err(format!("id provider tidak valid: {:?}", self.id));
        }
        if !(self.base_url.starts_with("http://") || self.base_url.starts_with("https://")) {
            return Err(format!("baseUrl harus http(s): {:?}", self.base_url));
        }
        if self.api_key.is_empty() || self.api_key.contains(['\n', '\r', '\0']) {
            return Err("apiKey kosong atau berisi karakter kontrol".into());
        }
        if self.models.is_empty() {
            return Err(format!("provider {:?} tanpa model", self.id));
        }
        Ok(())
    }
}

/// Muat `providers.json`. Berkas tidak ada → daftar kosong. Entri tidak valid dilewati dan dilaporkan.
pub fn load(path: &Path) -> (Vec<Provider>, Vec<String>) {
    let mut warnings = Vec::new();
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return (Vec::new(), warnings),
    };
    if meta.len() > MAX_BYTES {
        warnings.push("providers.json terlalu besar; diabaikan".into());
        return (Vec::new(), warnings);
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            warnings.push(format!("providers.json tidak terbaca: {e}"));
            return (Vec::new(), warnings);
        }
    };
    let file: File = match serde_json::from_str(&text) {
        Ok(f) => f,
        Err(e) => {
            warnings.push(format!("providers.json rusak: {e}"));
            return (Vec::new(), warnings);
        }
    };
    if file.version != VERSION {
        warnings.push(format!(
            "versi providers.json {} tidak dikenal",
            file.version
        ));
        return (Vec::new(), warnings);
    }
    let mut out: Vec<Provider> = Vec::new();
    for p in file.providers {
        match p.validate() {
            Ok(()) if out.iter().any(|o| o.id == p.id) => {
                warnings.push(format!("provider ganda dilewati: {}", p.id))
            }
            Ok(()) => out.push(p),
            Err(e) => warnings.push(e),
        }
    }
    (out, warnings)
}

/// Bagian `provider` untuk konfigurasi opencode (tanpa nilai kunci).
pub fn config_section(providers: &[Provider]) -> Value {
    let mut map = serde_json::Map::new();
    for p in providers {
        let models: serde_json::Map<String, Value> = p
            .models
            .iter()
            .map(|m| {
                (
                    m.id.clone(),
                    json!({ "name": m.name.clone().unwrap_or_else(|| m.id.clone()) }),
                )
            })
            .collect();
        map.insert(
            p.id.clone(),
            json!({
                "npm": "@ai-sdk/openai-compatible",
                "name": p.name,
                "options": {
                    "baseURL": p.base_url,
                    "apiKey": format!("{{env:{}}}", p.env_name()),
                },
                "models": models,
            }),
        );
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Provider {
        Provider {
            id: "antigravity".into(),
            name: "Antigravity Tools".into(),
            base_url: "http://localhost:8045/v1".into(),
            api_key: "sk-rahasia".into(),
            models: vec![ProviderModel {
                id: "m1".into(),
                name: None,
            }],
        }
    }

    #[test]
    fn config_never_contains_the_key() {
        let s = config_section(&[sample()]).to_string();
        assert!(!s.contains("sk-rahasia"));
        assert!(s.contains("{env:ZANDOR_SECRET_PROVIDER_ANTIGRAVITY}"));
        assert!(s.contains("@ai-sdk/openai-compatible"));
    }

    #[test]
    fn env_name_is_sanitized() {
        let mut p = sample();
        p.id = "my-prov-2".into();
        assert_eq!(p.env_name(), "ZANDOR_SECRET_PROVIDER_MY_PROV_2");
    }

    #[test]
    fn validation_rejects_bad_entries() {
        let mut p = sample();
        p.id = "Bad Id".into();
        assert!(p.validate().is_err());
        let mut p = sample();
        p.base_url = "file:///x".into();
        assert!(p.validate().is_err());
        let mut p = sample();
        p.api_key = String::new();
        assert!(p.validate().is_err());
        let mut p = sample();
        p.models.clear();
        assert!(p.validate().is_err());
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn load_handles_missing_corrupt_and_duplicates() {
        let dir = std::env::temp_dir().join(format!("zandor-prov-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("providers.json");
        assert!(load(&f).0.is_empty());
        std::fs::write(&f, "{rusak").unwrap();
        let (p, w) = load(&f);
        assert!(p.is_empty() && w.len() == 1);
        let one =
            r#"{"id":"a","name":"A","baseUrl":"http://x/v1","apiKey":"k","models":[{"id":"m"}]}"#;
        std::fs::write(&f, format!(r#"{{"version":1,"providers":[{one},{one}]}}"#)).unwrap();
        let (p, w) = load(&f);
        assert_eq!(p.len(), 1);
        assert_eq!(w.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

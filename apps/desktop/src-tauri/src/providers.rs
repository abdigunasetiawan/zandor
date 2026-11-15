// Provider model yang dikelola Zandor (Fase 6). Daftar (tanpa kunci) di `app_data/providers.json` versi 2; kunci API
// di keychain OS (secrets.rs). Konfigurasi engine hanya memuat rujukan `{env:ZANDOR_SECRET_PROVIDER_<ID>}`; nilai kunci
// dikirim lewat environment proses engine dan dibersihkan plugin guard dari proses anak (ADR-022).
//
// Jenis:
// - custom : endpoint kompatibel OpenAI (base URL + model yang didaftarkan)
// - local  : server lokal tanpa kunci wajib (Ollama, LM Studio) — sama dengan custom di sisi engine
// - builtin: provider katalog opencode/models.dev; hanya kunci yang kita simpan (model datang dari katalog)
use crate::secrets::{provider_account, SecretBackend};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 256 * 1024;
const VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Custom,
    Local,
    Builtin,
}

fn default_kind() -> Kind {
    Kind::Custom
}

/// Satu akun/kunci pada sebuah provider (ADR-056). Kunci tiap akun di keychain (`provider:<id>#<akun>`); kunci akun
/// aktif disalin ke `provider:<id>` yang dibaca engine. Akun lama (satu kunci tanpa daftar) tampil sebagai akun `main`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AccountView {
    pub id: String,
    pub label: String,
    pub active: bool,
}

pub const LEGACY_ACCOUNT: &str = "main";
const MAX_ACCOUNTS: usize = 20;

/// Kemampuan model kustom (opsional). Diteruskan ke konfigurasi engine agar batas konteks dan fitur dikenali.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Jendela konteks (token).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<u64>,
    /// Batas token keluaran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment: Option<bool>,
}

/// Protokol API endpoint kustom → adapter AI SDK yang dipakai engine.
pub const PROTOCOLS: [(&str, &str); 4] = [
    ("openai-compatible", "@ai-sdk/openai-compatible"),
    ("openai", "@ai-sdk/openai"),
    ("anthropic", "@ai-sdk/anthropic"),
    ("google", "@ai-sdk/google"),
];

fn adapter_for(protocol: Option<&str>) -> &'static str {
    let p = protocol.unwrap_or("openai-compatible");
    PROTOCOLS
        .iter()
        .find(|(k, _)| *k == p)
        .map(|(_, v)| *v)
        .unwrap_or("@ai-sdk/openai-compatible")
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(default = "default_kind")]
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Protokol API (lihat `PROTOCOLS`); kosong = openai-compatible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(default)]
    pub models: Vec<ProviderModel>,
    /// Akun/kunci tambahan (ADR-056). Kosong = hanya satu kunci lama di `provider:<id>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<Account>,
    /// Id akun yang kuncinya dipakai engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,
    /// Hanya ada pada berkas versi 1 (kunci di berkas); dibaca untuk migrasi dan tidak pernah ditulis lagi.
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
}

/// Bentuk untuk UI: tanpa kunci, dengan penanda apakah kunci tersimpan.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub base_url: Option<String>,
    pub protocol: Option<String>,
    pub models: Vec<ProviderModel>,
    pub has_key: bool,
    pub accounts: Vec<AccountView>,
}

#[derive(Serialize, Deserialize)]
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
        if self.name.trim().is_empty() {
            return Err("nama provider kosong".into());
        }
        let needs_url = self.kind != Kind::Builtin;
        match &self.base_url {
            Some(u) if !(u.starts_with("http://") || u.starts_with("https://")) => {
                return Err(format!("baseUrl harus http(s): {u:?}"));
            }
            None if needs_url => return Err("baseUrl wajib untuk endpoint kustom/lokal".into()),
            _ => {}
        }
        if needs_url && self.models.is_empty() {
            return Err(format!("provider {:?} tanpa model", self.id));
        }
        if self.models.iter().any(|m| m.id.trim().is_empty()) {
            return Err("ada model dengan id kosong".into());
        }
        if let Some(p) = &self.protocol {
            if !PROTOCOLS.iter().any(|(k, _)| k == p) {
                return Err(format!("protokol tidak dikenal: {p:?}"));
            }
        }
        if self
            .models
            .iter()
            .any(|m| m.context == Some(0) || m.output == Some(0))
        {
            return Err("batas konteks/keluaran harus lebih dari 0".into());
        }
        Ok(())
    }
}

fn account_key(provider_id: &str, account_id: &str) -> String {
    format!("{}#{account_id}", provider_account(provider_id))
}

fn clean_label(label: &str) -> Result<String, String> {
    let l = label.trim();
    if l.chars().count() > 80 || l.chars().any(char::is_control) {
        return Err("nama akun terlalu panjang atau berisi karakter kontrol".into());
    }
    Ok(l.to_string())
}

/// Daftar akun untuk UI; akun lama (kunci tanpa daftar) tampil sebagai satu akun `main` aktif.
fn account_views(p: &Provider, has_key: bool) -> Vec<AccountView> {
    if p.accounts.is_empty() {
        return if has_key {
            vec![AccountView {
                id: LEGACY_ACCOUNT.into(),
                label: String::new(),
                active: true,
            }]
        } else {
            Vec::new()
        };
    }
    p.accounts
        .iter()
        .map(|a| AccountView {
            id: a.id.clone(),
            label: a.label.clone(),
            active: p.active_account.as_deref() == Some(a.id.as_str()),
        })
        .collect()
}

pub fn validate_api_key(key: &str) -> Result<(), String> {
    if key.is_empty() || key.len() > 8192 || key.contains(['\n', '\r', '\0']) {
        return Err("kunci API kosong, terlalu panjang, atau berisi karakter kontrol".into());
    }
    Ok(())
}

pub struct ProviderStore {
    file: PathBuf,
    secrets: Box<dyn SecretBackend>,
}

impl ProviderStore {
    pub fn new(file: &Path, secrets: Box<dyn SecretBackend>) -> Self {
        Self {
            file: file.to_path_buf(),
            secrets,
        }
    }

    fn write(&self, providers: &[Provider]) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&File {
            version: VERSION,
            providers: providers.to_vec(),
        })
        .map_err(|e| e.to_string())?;
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("gagal membuat direktori: {e}"))?;
        }
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("gagal menulis providers.json: {e}"))?;
        std::fs::rename(&tmp, &self.file)
            .map_err(|e| format!("gagal mengganti providers.json: {e}"))
    }

    /// Muat daftar. Berkas versi 1 (kunci di berkas) dimigrasikan: kunci dipindah ke keychain, berkas ditulis ulang
    /// tanpa kunci. Bila keychain gagal, berkas lama TIDAK diubah dan peringatan dikembalikan.
    pub fn load(&self) -> (Vec<Provider>, Vec<String>) {
        let mut warnings = Vec::new();
        let Ok(meta) = std::fs::metadata(&self.file) else {
            return (Vec::new(), warnings);
        };
        if meta.len() > MAX_BYTES {
            warnings.push("providers.json terlalu besar; diabaikan".into());
            return (Vec::new(), warnings);
        }
        let text = match std::fs::read_to_string(&self.file) {
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
        if file.version != 1 && file.version != VERSION {
            warnings.push(format!(
                "versi providers.json {} tidak dikenal",
                file.version
            ));
            return (Vec::new(), warnings);
        }
        let mut out: Vec<Provider> = Vec::new();
        let mut to_migrate: Vec<(String, String)> = Vec::new();
        for mut p in file.providers {
            if let Some(key) = p.api_key.take() {
                to_migrate.push((p.id.clone(), key));
            }
            match p.validate() {
                Ok(()) if out.iter().any(|o| o.id == p.id) => {
                    warnings.push(format!("provider ganda dilewati: {}", p.id));
                }
                Ok(()) => out.push(p),
                Err(e) => warnings.push(e),
            }
        }
        if file.version == 1 || !to_migrate.is_empty() {
            let mut all_ok = true;
            for (id, key) in &to_migrate {
                if let Err(e) = self.secrets.set(&provider_account(id), key) {
                    warnings.push(format!("gagal memindahkan kunci {id} ke keychain: {e}"));
                    all_ok = false;
                }
            }
            if all_ok {
                if let Err(e) = self.write(&out) {
                    warnings.push(e);
                } else if !to_migrate.is_empty() {
                    warnings.push(format!(
                        "{} kunci dipindahkan dari providers.json ke keychain",
                        to_migrate.len()
                    ));
                }
            }
        }
        (out, warnings)
    }

    pub fn views(&self) -> Result<Vec<ProviderView>, String> {
        let (providers, _) = self.load();
        providers
            .into_iter()
            .map(|p| {
                let has_key = self.secrets.get(&provider_account(&p.id))?.is_some();
                let accounts = account_views(&p, has_key);
                Ok(ProviderView {
                    id: p.id,
                    name: p.name,
                    kind: p.kind,
                    base_url: p.base_url,
                    protocol: p.protocol,
                    models: p.models,
                    has_key,
                    accounts,
                })
            })
            .collect()
    }

    /// Simpan/perbarui provider. `api_key = Some` menimpa kunci di keychain; `None` mempertahankan kunci lama.
    pub fn save(&self, mut provider: Provider, api_key: Option<String>) -> Result<(), String> {
        provider.api_key = None;
        provider.validate()?;
        if let Some(key) = &api_key {
            validate_api_key(key)?;
        }
        let (mut all, _) = self.load();
        // Akun tidak diubah lewat `save` (UI tidak mengirimnya): pertahankan yang sudah ada.
        if let Some(old) = all.iter().find(|p| p.id == provider.id) {
            provider.accounts = old.accounts.clone();
            provider.active_account = old.active_account.clone();
        }
        // Kunci disimpan lebih dulu: bila gagal, daftar tidak berubah.
        if let Some(key) = &api_key {
            self.secrets.set(&provider_account(&provider.id), key)?;
            if let Some(a) = &provider.active_account {
                self.secrets.set(&account_key(&provider.id, a), key)?;
            }
        }
        match all.iter().position(|p| p.id == provider.id) {
            Some(i) => all[i] = provider,
            None => all.push(provider),
        }
        self.write(&all)
    }

    fn find(all: &[Provider], id: &str) -> Result<usize, String> {
        all.iter()
            .position(|p| p.id == id)
            .ok_or_else(|| format!("provider {id:?} belum disimpan"))
    }

    /// Ubah akun lama (satu kunci tanpa daftar) menjadi akun `main` agar bisa dikelola bersama akun baru.
    fn materialize_legacy(&self, p: &mut Provider) -> Result<(), String> {
        if !p.accounts.is_empty() {
            return Ok(());
        }
        if let Some(key) = self.secrets.get(&provider_account(&p.id))? {
            self.secrets
                .set(&account_key(&p.id, LEGACY_ACCOUNT), &key)?;
            p.accounts.push(Account {
                id: LEGACY_ACCOUNT.into(),
                label: String::new(),
            });
            p.active_account = Some(LEGACY_ACCOUNT.into());
        }
        Ok(())
    }

    /// Tambah akun (kunci baru) pada provider yang sudah disimpan. Akun baru langsung aktif bila `activate`
    /// (atau belum ada yang aktif). Mengembalikan id akun.
    pub fn add_account(
        &self,
        provider_id: &str,
        label: &str,
        api_key: &str,
        activate: bool,
    ) -> Result<String, String> {
        validate_api_key(api_key)?;
        let label = clean_label(label)?;
        let (mut all, _) = self.load();
        let i = Self::find(&all, provider_id)?;
        let mut p = all[i].clone();
        self.materialize_legacy(&mut p)?;
        if p.accounts.len() >= MAX_ACCOUNTS {
            return Err(format!("maksimal {MAX_ACCOUNTS} akun per provider"));
        }
        let mut n = p.accounts.len() + 1;
        let id = loop {
            let c = format!("a{n}");
            if !p.accounts.iter().any(|a| a.id == c) {
                break c;
            }
            n += 1;
        };
        self.secrets.set(&account_key(provider_id, &id), api_key)?;
        p.accounts.push(Account {
            id: id.clone(),
            label,
        });
        if activate || p.active_account.is_none() {
            self.secrets.set(&provider_account(provider_id), api_key)?;
            p.active_account = Some(id.clone());
        }
        all[i] = p;
        self.write(&all)?;
        Ok(id)
    }

    /// Pakai kunci akun ini untuk engine.
    pub fn activate_account(&self, provider_id: &str, account_id: &str) -> Result<(), String> {
        let (mut all, _) = self.load();
        let i = Self::find(&all, provider_id)?;
        let mut p = all[i].clone();
        self.materialize_legacy(&mut p)?;
        if !p.accounts.iter().any(|a| a.id == account_id) {
            return Err(format!("akun {account_id:?} tidak ada"));
        }
        let key = self
            .secrets
            .get(&account_key(provider_id, account_id))?
            .ok_or("kunci akun tidak ditemukan di keychain")?;
        self.secrets.set(&provider_account(provider_id), &key)?;
        p.active_account = Some(account_id.into());
        all[i] = p;
        self.write(&all)
    }

    pub fn rename_account(
        &self,
        provider_id: &str,
        account_id: &str,
        label: &str,
    ) -> Result<(), String> {
        let label = clean_label(label)?;
        let (mut all, _) = self.load();
        let i = Self::find(&all, provider_id)?;
        let mut p = all[i].clone();
        self.materialize_legacy(&mut p)?;
        let a = p
            .accounts
            .iter_mut()
            .find(|a| a.id == account_id)
            .ok_or_else(|| format!("akun {account_id:?} tidak ada"))?;
        a.label = label;
        all[i] = p;
        self.write(&all)
    }

    /// Hapus akun. Bila yang dihapus aktif, akun pertama yang tersisa diaktifkan; bila habis, kunci provider dihapus.
    /// Mengembalikan jumlah akun tersisa.
    pub fn remove_account(&self, provider_id: &str, account_id: &str) -> Result<usize, String> {
        let (mut all, _) = self.load();
        let i = Self::find(&all, provider_id)?;
        let mut p = all[i].clone();
        self.materialize_legacy(&mut p)?;
        let before = p.accounts.len();
        p.accounts.retain(|a| a.id != account_id);
        if p.accounts.len() == before {
            return Err(format!("akun {account_id:?} tidak ada"));
        }
        self.secrets.delete(&account_key(provider_id, account_id))?;
        if p.active_account.as_deref() == Some(account_id) {
            match p.accounts.first().map(|a| a.id.clone()) {
                Some(next) => {
                    let key = self
                        .secrets
                        .get(&account_key(provider_id, &next))?
                        .ok_or("kunci akun berikutnya tidak ditemukan di keychain")?;
                    self.secrets.set(&provider_account(provider_id), &key)?;
                    p.active_account = Some(next);
                }
                None => {
                    self.secrets.delete(&provider_account(provider_id))?;
                    p.active_account = None;
                }
            }
        }
        let left = p.accounts.len();
        all[i] = p;
        self.write(&all)?;
        Ok(left)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let (mut all, _) = self.load();
        let accounts: Vec<Account> = all
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.accounts.clone())
            .unwrap_or_default();
        all.retain(|p| p.id != id);
        self.write(&all)?;
        self.secrets.delete(&provider_account(id))?;
        for a in accounts {
            self.secrets.delete(&account_key(id, &a.id))?;
        }
        Ok(())
    }

    pub fn api_key(&self, id: &str) -> Result<Option<String>, String> {
        self.secrets.get(&provider_account(id))
    }

    /// Provider + pasangan (nama env, kunci) untuk proses engine.
    pub fn for_engine(&self) -> (Vec<Provider>, Vec<(String, String)>, Vec<String>) {
        let (providers, mut warnings) = self.load();
        let mut env = Vec::new();
        for p in &providers {
            match self.secrets.get(&provider_account(&p.id)) {
                Ok(Some(k)) => env.push((p.env_name(), k)),
                Ok(None) => {}
                Err(e) => warnings.push(format!("kunci {} tidak terbaca: {e}", p.id)),
            }
        }
        (providers, env, warnings)
    }
}

/// Bagian `provider` untuk konfigurasi opencode (tanpa nilai kunci). `with_key`: id provider yang punya kunci.
pub fn config_section(providers: &[Provider], with_key: &dyn Fn(&str) -> bool) -> Value {
    let mut map = serde_json::Map::new();
    for p in providers {
        let mut options = serde_json::Map::new();
        if let Some(u) = &p.base_url {
            options.insert("baseURL".into(), json!(u));
        }
        if with_key(&p.id) {
            options.insert("apiKey".into(), json!(format!("{{env:{}}}", p.env_name())));
        }
        let entry = if p.kind == Kind::Builtin {
            json!({ "options": options })
        } else {
            let models: serde_json::Map<String, Value> = p
                .models
                .iter()
                .map(|m| {
                    let mut e = serde_json::Map::new();
                    e.insert(
                        "name".into(),
                        json!(m.name.clone().unwrap_or_else(|| m.id.clone())),
                    );
                    if let Some(ctx) = m.context {
                        // engine mewajibkan context dan output bersama-sama
                        let out = m.output.unwrap_or_else(|| ctx.min(16_384));
                        e.insert("limit".into(), json!({ "context": ctx, "output": out }));
                    }
                    if let Some(v) = m.reasoning {
                        e.insert("reasoning".into(), json!(v));
                    }
                    if let Some(v) = m.tool_call {
                        e.insert("tool_call".into(), json!(v));
                    }
                    if let Some(v) = m.attachment {
                        e.insert("attachment".into(), json!(v));
                    }
                    (m.id.clone(), Value::Object(e))
                })
                .collect();
            json!({
                "npm": adapter_for(p.protocol.as_deref()),
                "name": p.name,
                "options": options,
                "models": models,
            })
        };
        map.insert(p.id.clone(), entry);
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecrets;
    use std::sync::Arc;

    /// Backend bersama agar test bisa memeriksa isi keychain setelah ProviderStore dibuat ulang.
    struct Shared(Arc<MemorySecrets>);
    impl SecretBackend for Shared {
        fn get(&self, a: &str) -> Result<Option<String>, String> {
            self.0.get(a)
        }
        fn set(&self, a: &str, s: &str) -> Result<(), String> {
            self.0.set(a, s)
        }
        fn delete(&self, a: &str) -> Result<bool, String> {
            self.0.delete(a)
        }
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zandor-prov-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("providers.json")
    }

    fn sample() -> Provider {
        Provider {
            id: "antigravity".into(),
            name: "Antigravity Tools".into(),
            kind: Kind::Custom,
            base_url: Some("http://localhost:8045/v1".into()),
            protocol: None,
            models: vec![ProviderModel {
                id: "m1".into(),
                ..Default::default()
            }],
            api_key: None,
            ..Default::default()
        }
    }

    #[test]
    fn config_never_contains_the_key() {
        let s = config_section(&[sample()], &|_| true).to_string();
        assert!(s.contains("{env:ZANDOR_SECRET_PROVIDER_ANTIGRAVITY}"));
        assert!(s.contains("@ai-sdk/openai-compatible"));
        let none = config_section(&[sample()], &|_| false).to_string();
        assert!(!none.contains("apiKey"), "tanpa kunci → tanpa apiKey");
    }

    #[test]
    fn builtin_provider_only_carries_options() {
        let p = Provider {
            id: "openrouter".into(),
            name: "OpenRouter".into(),
            kind: Kind::Builtin,
            base_url: None,
            protocol: None,
            models: vec![],
            api_key: None,
            ..Default::default()
        };
        let v = config_section(&[p], &|_| true);
        assert!(v["openrouter"].get("npm").is_none());
        assert!(v["openrouter"].get("models").is_none());
        assert_eq!(
            v["openrouter"]["options"]["apiKey"],
            "{env:ZANDOR_SECRET_PROVIDER_OPENROUTER}"
        );
    }

    #[test]
    fn protocol_picks_the_adapter_and_caps_reach_the_config() {
        let mut p = sample();
        p.protocol = Some("anthropic".into());
        p.models = vec![ProviderModel {
            id: "m1".into(),
            name: Some("Model Satu".into()),
            context: Some(200_000),
            output: None,
            reasoning: Some(true),
            tool_call: Some(false),
            attachment: Some(true),
        }];
        assert!(p.validate().is_ok());
        let v = config_section(&[p], &|_| true);
        let e = &v["antigravity"];
        assert_eq!(e["npm"], "@ai-sdk/anthropic");
        let m = &e["models"]["m1"];
        assert_eq!(m["name"], "Model Satu");
        assert_eq!(m["limit"]["context"], 200_000);
        assert_eq!(m["limit"]["output"], 16_384);
        assert_eq!(m["reasoning"], true);
        assert_eq!(m["tool_call"], false);
        assert_eq!(m["attachment"], true);
    }

    #[test]
    fn unknown_protocol_or_zero_limit_is_rejected() {
        let mut p = sample();
        p.protocol = Some("ftp".into());
        assert!(p.validate().is_err());
        let mut p = sample();
        p.models[0].context = Some(0);
        assert!(p.validate().is_err());
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
        p.base_url = Some("file:///x".into());
        assert!(p.validate().is_err());
        let mut p = sample();
        p.base_url = None;
        assert!(p.validate().is_err());
        let mut p = sample();
        p.models.clear();
        assert!(p.validate().is_err());
        assert!(sample().validate().is_ok());
        assert!(validate_api_key("").is_err());
        assert!(validate_api_key("a\nb").is_err());
        assert!(validate_api_key("sk-ok").is_ok());
    }

    #[test]
    fn save_keeps_key_out_of_the_file_and_in_the_keychain() {
        let f = tmp("save");
        let mem = Arc::new(MemorySecrets::default());
        let store = ProviderStore::new(&f, Box::new(Shared(mem.clone())));
        store.save(sample(), Some("sk-rahasia".into())).unwrap();
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(!text.contains("sk-rahasia"));
        assert_eq!(
            mem.get("provider:antigravity").unwrap().as_deref(),
            Some("sk-rahasia")
        );
        // simpan ulang tanpa kunci → kunci lama dipertahankan
        let mut edited = sample();
        edited.name = "Nama Baru".into();
        store.save(edited, None).unwrap();
        assert_eq!(
            store.api_key("antigravity").unwrap().as_deref(),
            Some("sk-rahasia")
        );
        let views = store.views().unwrap();
        assert_eq!(views.len(), 1);
        assert!(views[0].has_key);
        assert_eq!(views[0].name, "Nama Baru");
        // hapus → entri dan kunci hilang
        store.delete("antigravity").unwrap();
        assert!(store.views().unwrap().is_empty());
        assert_eq!(mem.get("provider:antigravity").unwrap(), None);
    }

    #[test]
    fn multiple_accounts_keep_keys_apart_and_the_active_key_feeds_the_engine() {
        let f = tmp("accounts");
        let mem = Arc::new(MemorySecrets::default());
        let store = ProviderStore::new(&f, Box::new(Shared(mem.clone())));
        // akun lama: satu kunci tanpa daftar → tampil sebagai akun "main"
        store.save(sample(), Some("sk-lama".into())).unwrap();
        let v = store.views().unwrap();
        assert_eq!(v[0].accounts.len(), 1);
        assert!(v[0].accounts[0].active);
        // tambah akun kedua → akun lama ikut menjadi entri daftar, akun baru aktif
        let b = store
            .add_account("antigravity", "Kantor", "sk-kantor", true)
            .unwrap();
        let v = store.views().unwrap();
        assert_eq!(v[0].accounts.len(), 2);
        assert_eq!(v[0].accounts.iter().find(|a| a.active).unwrap().id, b);
        assert_eq!(
            store.api_key("antigravity").unwrap().as_deref(),
            Some("sk-kantor")
        );
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(!text.contains("sk-lama") && !text.contains("sk-kantor"));
        // simpan ulang dari UI (tanpa akun) tidak menghapus daftar akun
        store.save(sample(), None).unwrap();
        assert_eq!(store.views().unwrap()[0].accounts.len(), 2);
        // beralih kembali ke akun lama
        store
            .activate_account("antigravity", LEGACY_ACCOUNT)
            .unwrap();
        assert_eq!(
            store.api_key("antigravity").unwrap().as_deref(),
            Some("sk-lama")
        );
        store.rename_account("antigravity", &b, "Kerja").unwrap();
        assert_eq!(
            store.views().unwrap()[0]
                .accounts
                .iter()
                .find(|a| a.id == b)
                .unwrap()
                .label,
            "Kerja"
        );
        // hapus akun aktif → akun lain diaktifkan; hapus yang terakhir → kunci hilang
        assert_eq!(
            store.remove_account("antigravity", LEGACY_ACCOUNT).unwrap(),
            1
        );
        assert_eq!(
            store.api_key("antigravity").unwrap().as_deref(),
            Some("sk-kantor")
        );
        assert_eq!(store.remove_account("antigravity", &b).unwrap(), 0);
        assert_eq!(store.api_key("antigravity").unwrap(), None);
        assert!(store.views().unwrap()[0].accounts.is_empty());
        assert!(store.remove_account("antigravity", "nope").is_err());
        assert!(store.add_account("tidak-ada", "x", "k", true).is_err());
        // hapus provider membersihkan semua kunci akun
        store.add_account("antigravity", "A", "k1", true).unwrap();
        store.delete("antigravity").unwrap();
        assert_eq!(mem.get("provider:antigravity#a1").unwrap(), None);
    }

    #[test]
    fn v1_file_with_plaintext_key_is_migrated_to_the_keychain() {
        let f = tmp("migrate");
        std::fs::write(
            &f,
            r#"{"version":1,"providers":[{"id":"a","name":"A","baseUrl":"http://x/v1","apiKey":"sk-lama","models":[{"id":"m"}]}]}"#,
        )
        .unwrap();
        let mem = Arc::new(MemorySecrets::default());
        let store = ProviderStore::new(&f, Box::new(Shared(mem.clone())));
        let (providers, warnings) = store.load();
        assert_eq!(providers.len(), 1);
        assert!(warnings.iter().any(|w| w.contains("dipindahkan")));
        assert_eq!(mem.get("provider:a").unwrap().as_deref(), Some("sk-lama"));
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(
            !text.contains("sk-lama"),
            "berkas ditulis ulang tanpa kunci"
        );
        assert!(text.contains("\"version\": 2"));
    }

    #[test]
    fn migration_failure_leaves_the_old_file_untouched() {
        struct Broken;
        impl SecretBackend for Broken {
            fn get(&self, _: &str) -> Result<Option<String>, String> {
                Ok(None)
            }
            fn set(&self, _: &str, _: &str) -> Result<(), String> {
                Err("keychain terkunci".into())
            }
            fn delete(&self, _: &str) -> Result<bool, String> {
                Ok(false)
            }
        }
        let f = tmp("broken");
        let original = r#"{"version":1,"providers":[{"id":"a","name":"A","baseUrl":"http://x/v1","apiKey":"sk-lama","models":[{"id":"m"}]}]}"#;
        std::fs::write(&f, original).unwrap();
        let store = ProviderStore::new(&f, Box::new(Broken));
        let (_, warnings) = store.load();
        assert!(warnings.iter().any(|w| w.contains("gagal memindahkan")));
        assert_eq!(std::fs::read_to_string(&f).unwrap(), original);
    }

    #[test]
    fn load_handles_missing_corrupt_and_duplicates() {
        let f = tmp("load");
        let store = ProviderStore::new(&f, Box::new(MemorySecrets::default()));
        assert!(store.load().0.is_empty());
        std::fs::write(&f, "{rusak").unwrap();
        let (p, w) = store.load();
        assert!(p.is_empty() && w.len() == 1);
        let one = r#"{"id":"a","name":"A","baseUrl":"http://x/v1","models":[{"id":"m"}]}"#;
        std::fs::write(&f, format!(r#"{{"version":2,"providers":[{one},{one}]}}"#)).unwrap();
        let (p, w) = store.load();
        assert_eq!(p.len(), 1);
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn for_engine_yields_env_pairs_only_for_providers_with_keys() {
        let f = tmp("engine");
        let store = ProviderStore::new(&f, Box::new(MemorySecrets::default()));
        store.save(sample(), Some("sk-x".into())).unwrap();
        let mut local = sample();
        local.id = "ollama".into();
        local.kind = Kind::Local;
        store.save(local, None).unwrap();
        let (providers, env, _) = store.for_engine();
        assert_eq!(providers.len(), 2);
        assert_eq!(
            env,
            vec![(
                "ZANDOR_SECRET_PROVIDER_ANTIGRAVITY".to_string(),
                "sk-x".to_string()
            )]
        );
    }
}

//! Spike S4 — secret Zandor di keychain OS + cara memberikannya ke `opencode serve`.
//!
//! - Penyimpanan: crate `keyring` 4.2 (mode v1) → Windows Credential Manager, macOS Keychain,
//!   Linux Secret Service. Service = `id.web.zandor`, account = `provider:<id>`.
//! - Penyuntikan (lihat ADR-022): secret hanya hidup di memori proses Zandor dan dikirim ke
//!   engine saat spawn lewat env var `ZANDOR_SECRET_<ID>`; config provider memakai
//!   placeholder `{env:ZANDOR_SECRET_<ID>}` lewat `OPENCODE_CONFIG_CONTENT`. Tidak ada file
//!   berisi secret yang ditulis oleh Zandor.

use keyring::Entry;

pub const SERVICE: &str = "id.web.zandor";

#[derive(Debug)]
pub struct SecretError(pub String);
impl std::fmt::Display for SecretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "keychain: {}", self.0)
    }
}
impl std::error::Error for SecretError {}

/// Akses keychain OS untuk satu "service".
pub struct SecretStore {
    service: String,
}

impl SecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into() }
    }

    fn entry(&self, account: &str) -> Result<Entry, SecretError> {
        Entry::new(&self.service, account).map_err(|e| SecretError(e.to_string()))
    }

    fn raw_set(&self, account: &str, value: &str) -> Result<(), SecretError> {
        self.entry(account)?.set_password(value).map_err(|e| SecretError(e.to_string()))
    }

    fn raw_get(&self, account: &str) -> Result<Option<String>, SecretError> {
        match self.entry(account)?.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    fn raw_delete(&self, account: &str) -> Result<bool, SecretError> {
        match self.entry(account)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    /// Simpan secret. Windows Credential Manager membatasi 2560 byte (1280 unit UTF-16),
    /// padahal token OAuth/JWT bisa lebih panjang → secret panjang dipecah ke beberapa entri:
    /// `account` berisi penanda `zandor-chunked:v1:<n>`, potongan di `account#1..n`.
    pub fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        let old_chunks = self.chunk_count(account)?;
        let chunks = split_utf16(secret, CHUNK_UNITS);
        if chunks.len() <= 1 {
            self.raw_set(account, secret)?;
        } else {
            for (i, c) in chunks.iter().enumerate() {
                self.raw_set(&format!("{account}#{}", i + 1), c)?;
            }
            self.raw_set(account, &format!("{CHUNK_MARK}{}", chunks.len()))?;
        }
        // Bersihkan potongan sisa dari nilai lama yang lebih panjang.
        let new_n = if chunks.len() <= 1 { 0 } else { chunks.len() };
        for i in new_n + 1..=old_chunks {
            self.raw_delete(&format!("{account}#{i}"))?;
        }
        Ok(())
    }

    /// `Ok(None)` bila belum ada.
    pub fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        let Some(head) = self.raw_get(account)? else { return Ok(None) };
        let Some(n) = head.strip_prefix(CHUNK_MARK).and_then(|n| n.parse::<usize>().ok()) else {
            return Ok(Some(head));
        };
        let mut out = String::new();
        for i in 1..=n {
            out += &self
                .raw_get(&format!("{account}#{i}"))?
                .ok_or_else(|| SecretError(format!("potongan {i}/{n} hilang")))?;
        }
        Ok(Some(out))
    }

    /// `Ok(true)` bila ada yang dihapus.
    pub fn delete(&self, account: &str) -> Result<bool, SecretError> {
        let n = self.chunk_count(account)?;
        for i in 1..=n {
            self.raw_delete(&format!("{account}#{i}"))?;
        }
        self.raw_delete(account)
    }

    fn chunk_count(&self, account: &str) -> Result<usize, SecretError> {
        Ok(self
            .raw_get(account)?
            .and_then(|h| h.strip_prefix(CHUNK_MARK).and_then(|n| n.parse().ok()))
            .unwrap_or(0))
    }
}

const CHUNK_MARK: &str = "zandor-chunked:v1:";
/// Batas aman per entri (unit UTF-16); batas Windows 1280.
const CHUNK_UNITS: usize = 1200;

/// Pecah string per maksimal `max` unit UTF-16 tanpa memotong karakter.
fn split_utf16(s: &str, max: usize) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut units = 0;
    for ch in s.chars() {
        let n = ch.len_utf16();
        if units + n > max {
            out.push(String::new());
            units = 0;
        }
        out.last_mut().unwrap().push(ch);
        units += n;
    }
    out
}

pub fn provider_account(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

/// Nama env var untuk secret provider: `ZANDOR_SECRET_<ID>` (A-Z, 0-9, _).
pub fn env_var_name(provider_id: &str) -> String {
    let id: String = provider_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .collect();
    format!("ZANDOR_SECRET_{id}")
}

/// Provider kustom OpenAI-compatible yang dikelola Zandor.
pub struct CustomProvider<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub base_url: &'a str,
    pub models: &'a [&'a str],
}

/// Fragmen config opencode untuk provider kustom. `apiKey` hanya berupa placeholder
/// `{env:...}`; nilai sebenarnya disubstitusi opencode dari env proses engine.
pub fn provider_config(p: &CustomProvider) -> serde_json::Value {
    let models: serde_json::Map<String, serde_json::Value> = p
        .models
        .iter()
        .map(|m| (m.to_string(), serde_json::json!({ "name": m })))
        .collect();
    serde_json::json!({
        "npm": "@ai-sdk/openai-compatible",
        "name": p.name,
        "options": {
            "baseURL": p.base_url,
            "apiKey": format!("{{env:{}}}", env_var_name(p.id)),
        },
        "models": models,
    })
}

/// Env yang diberikan ke proses engine: `OPENCODE_CONFIG_CONTENT` (tanpa secret) +
/// satu `ZANDOR_SECRET_<ID>` per provider yang secret-nya ada di keychain.
pub fn engine_env(
    store: &SecretStore,
    providers: &[CustomProvider],
) -> Result<Vec<(String, String)>, SecretError> {
    let mut env = Vec::new();
    let mut provider_map = serde_json::Map::new();
    for p in providers {
        if let Some(secret) = store.get(&provider_account(p.id))? {
            env.push((env_var_name(p.id), secret));
        }
        provider_map.insert(p.id.to_string(), provider_config(p));
    }
    let content = serde_json::json!({ "provider": provider_map });
    env.push(("OPENCODE_CONFIG_CONTENT".into(), content.to_string()));
    Ok(env)
}

/// Token acak untuk test.
pub fn random_secret(prefix: &str) -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("rng");
    format!("{prefix}{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_names_are_safe() {
        assert_eq!(env_var_name("my-proxy.local"), "ZANDOR_SECRET_MY_PROXY_LOCAL");
    }

    #[test]
    fn config_contains_placeholder_not_secret() {
        let p = CustomProvider { id: "zmock", name: "Mock", base_url: "http://x/v1", models: &["m1"] };
        let v = provider_config(&p);
        assert_eq!(v["options"]["apiKey"], "{env:ZANDOR_SECRET_ZMOCK}");
    }
}

// Penyimpanan secret di keychain OS (hasil spike S4, ADR-022). Windows: Credential Manager; macOS/Linux menyusul
// (ADR-027). Service = `id.web.zandor`, account = `provider:<id>`. Secret panjang (token OAuth) dipecah karena
// Credential Manager membatasi ±1280 unit UTF-16 per entri.
use keyring::Entry;
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;

pub const SERVICE: &str = "id.web.zandor";

/// Antarmuka penyimpanan secret; test memakai `MemorySecrets`, aplikasi memakai `Keychain`.
pub trait SecretBackend: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    /// `Ok(true)` bila ada yang dihapus.
    fn delete(&self, account: &str) -> Result<bool, String>;
}

pub fn provider_account(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

const CHUNK_MARK: &str = "zandor-chunked:v1:";
/// Batas aman per entri (unit UTF-16); batas Windows 1280.
const CHUNK_UNITS: usize = 1200;

/// Pecah string per maksimal `max` unit UTF-16 tanpa memotong karakter.
pub fn split_utf16(s: &str, max: usize) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut units = 0;
    for ch in s.chars() {
        let n = ch.len_utf16();
        if units + n > max {
            out.push(String::new());
            units = 0;
        }
        if let Some(last) = out.last_mut() {
            last.push(ch);
        }
        units += n;
    }
    out
}

pub struct Keychain {
    service: String,
}

impl Keychain {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, account: &str) -> Result<Entry, String> {
        Entry::new(&self.service, account).map_err(|e| format!("keychain: {e}"))
    }

    fn raw_set(&self, account: &str, value: &str) -> Result<(), String> {
        self.entry(account)?
            .set_password(value)
            .map_err(|e| format!("keychain: {e}"))
    }

    fn raw_get(&self, account: &str) -> Result<Option<String>, String> {
        match self.entry(account)?.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("keychain: {e}")),
        }
    }

    fn raw_delete(&self, account: &str) -> Result<bool, String> {
        match self.entry(account)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(format!("keychain: {e}")),
        }
    }

    fn chunk_count(&self, account: &str) -> Result<usize, String> {
        Ok(self
            .raw_get(account)?
            .and_then(|h| h.strip_prefix(CHUNK_MARK).and_then(|n| n.parse().ok()))
            .unwrap_or(0))
    }
}

impl SecretBackend for Keychain {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        let Some(head) = self.raw_get(account)? else {
            return Ok(None);
        };
        let Some(n) = head
            .strip_prefix(CHUNK_MARK)
            .and_then(|n| n.parse::<usize>().ok())
        else {
            return Ok(Some(head));
        };
        let mut out = String::new();
        for i in 1..=n {
            out += &self
                .raw_get(&format!("{account}#{i}"))?
                .ok_or_else(|| format!("keychain: potongan {i}/{n} hilang"))?;
        }
        Ok(Some(out))
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
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
        let new_n = if chunks.len() <= 1 { 0 } else { chunks.len() };
        for i in new_n + 1..=old_chunks {
            self.raw_delete(&format!("{account}#{i}"))?;
        }
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<bool, String> {
        let n = self.chunk_count(account)?;
        for i in 1..=n {
            self.raw_delete(&format!("{account}#{i}"))?;
        }
        self.raw_delete(account)
    }
}

/// Penyimpanan di memori untuk test.
#[cfg(test)]
#[derive(Default)]
pub struct MemorySecrets(Mutex<HashMap<String, String>>);

#[cfg(test)]
impl SecretBackend for MemorySecrets {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .get(account)
            .cloned())
    }
    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .insert(account.into(), secret.into());
        Ok(())
    }
    fn delete(&self, account: &str) -> Result<bool, String> {
        Ok(self
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .remove(account)
            .is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_respects_utf16_units_and_never_cuts_a_char() {
        let s = "a😀".repeat(1000); // 😀 = 2 unit UTF-16
        let parts = split_utf16(&s, 1200);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|p| p.encode_utf16().count() <= 1200));
        assert_eq!(parts.concat(), s);
        assert_eq!(split_utf16("pendek", 1200), vec!["pendek".to_string()]);
    }

    #[test]
    fn memory_backend_roundtrip() {
        let m = MemorySecrets::default();
        assert_eq!(m.get("a").unwrap(), None);
        m.set("a", "x").unwrap();
        assert_eq!(m.get("a").unwrap().as_deref(), Some("x"));
        assert!(m.delete("a").unwrap());
        assert!(!m.delete("a").unwrap());
    }

    /// Menyentuh Credential Manager sungguhan dengan akun unik; selalu dibersihkan.
    #[test]
    fn real_keychain_roundtrip_with_long_secret() {
        let k = Keychain::new("id.web.zandor.test");
        let account = format!("test:{}-{}", std::process::id(), line!());
        let long = "😀x".repeat(1500);
        k.set(&account, "pendek").unwrap();
        assert_eq!(k.get(&account).unwrap().as_deref(), Some("pendek"));
        k.set(&account, &long).unwrap();
        assert_eq!(k.get(&account).unwrap().as_deref(), Some(long.as_str()));
        k.set(&account, "kembali pendek").unwrap();
        assert_eq!(k.get(&account).unwrap().as_deref(), Some("kembali pendek"));
        assert!(k.delete(&account).unwrap());
        assert_eq!(k.get(&account).unwrap(), None);
    }
}

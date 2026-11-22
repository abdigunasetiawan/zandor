//! Rekaman system prompt per sesi yang ditulis plugin Zandor (`packages/plugin/src/system-trace.ts`) dari hook
//! `experimental.chat.system.transform`. Engine sendiri tidak menyimpan system prompt di riwayat pesan.
//! Berkas: `<app_data>/system-prompts/<sessionID>.jsonl`, satu baris `{ "t": ms, "system": [string] }` setiap kali
//! isinya berubah. Secret sudah disamarkan oleh plugin sebelum ditulis.
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const DIR_NAME: &str = "system-prompts";

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Snapshot {
    /// Waktu (ms epoch) saat isi ini pertama kali dikirim ke model.
    pub t: f64,
    pub system: Vec<String>,
}

impl Snapshot {
    /// Teks utuh seperti yang dilihat model (bagian-bagian dipisah baris kosong).
    pub fn text(&self) -> String {
        self.system.join("\n\n")
    }
}

pub fn dir() -> PathBuf {
    super::services::app_data_dir().join(DIR_NAME)
}

fn file_for(dir: &Path, sid: &str) -> Option<PathBuf> {
    let safe = !sid.is_empty() && sid.len() <= 128 && sid.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    safe.then(|| dir.join(format!("{sid}.jsonl")))
}

/// Urai isi JSONL; baris rusak dilewati. Hasil terurut menurut waktu.
pub fn parse(text: &str) -> Vec<Snapshot> {
    let mut out: Vec<Snapshot> = text.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect();
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
}

/// Cache kecil per panel: baca ulang berkas hanya bila sesi atau waktu-ubah berkasnya berganti.
#[derive(Default)]
pub struct Cache {
    key: Option<(String, Option<SystemTime>)>,
    snaps: Vec<Snapshot>,
}

impl Cache {
    pub fn get(&mut self, sid: &str) -> &[Snapshot] {
        self.get_in(&dir(), sid)
    }

    pub fn get_in(&mut self, dir: &Path, sid: &str) -> &[Snapshot] {
        let Some(file) = file_for(dir, sid) else {
            self.key = None;
            self.snaps.clear();
            return &self.snaps;
        };
        let mtime = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
        let key = (sid.to_string(), mtime);
        if self.key.as_ref() != Some(&key) {
            self.snaps = std::fs::read_to_string(&file).map(|s| parse(&s)).unwrap_or_default();
            self.key = Some(key);
        }
        &self.snaps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urai_dan_lewati_baris_rusak() {
        let s = parse("{\"t\":2,\"system\":[\"b\"]}\nbukan json\n\n{\"t\":1,\"system\":[\"a\",\"c\"]}\n");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].t, 1.);
        assert_eq!(s[0].text(), "a\n\nc");
    }

    #[test]
    fn id_tidak_aman_ditolak() {
        let d = Path::new("x");
        assert!(file_for(d, "../rahasia").is_none());
        assert!(file_for(d, "").is_none());
        assert_eq!(file_for(d, "ses_AB-1"), Some(d.join("ses_AB-1.jsonl")));
    }

    #[test]
    fn cache_membaca_ulang_saat_berkas_berubah() {
        let dir = std::env::temp_dir().join(format!("zandor-sys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("ses_x.jsonl");
        std::fs::write(&f, "{\"t\":1,\"system\":[\"a\"]}\n").unwrap();
        let mut c = Cache::default();
        assert_eq!(c.get_in(&dir, "ses_x").len(), 1);
        assert!(c.get_in(&dir, "ses_lain").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

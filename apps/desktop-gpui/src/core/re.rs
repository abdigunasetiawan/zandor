//! Cache regex global: pola literal dikompilasi sekali lalu dipakai ulang. Regex dipanggil di jalur render
//! (logo model per baris, indikator sesi, gradien project); mengompilasi ulang tiap frame membuat UI patah-patah.
use regex::Regex;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

static CACHE: LazyLock<Mutex<HashMap<&'static str, Regex>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Regex untuk pola literal (klon murah: Regex berbagi program terkompilasi).
pub fn re(pattern: &'static str) -> Regex {
    let mut m = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    m.entry(pattern).or_insert_with(|| Regex::new(pattern).expect("regex")).clone()
}

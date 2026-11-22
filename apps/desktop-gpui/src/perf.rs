//! Pengukuran ringan: `ZANDOR_PERF=1` mencetak ke stderr tiap bagian yang melewati ambang waktu.
//! Tanpa env, biayanya satu pengecekan `OnceLock` per bagian (nyaris nol).
use std::sync::OnceLock;
use std::time::{Duration, Instant};

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ZANDOR_PERF").is_some())
}

/// Ukur sampai di-drop; cetak bila lebih lama dari `budget` (anggaran frame 60 Hz ≈ 16 ms).
pub struct Scope {
    name: &'static str,
    start: Option<Instant>,
    budget: Duration,
}

impl Scope {
    pub fn new(name: &'static str) -> Self {
        Self::with_budget(name, Duration::from_millis(8))
    }
    pub fn with_budget(name: &'static str, budget: Duration) -> Self {
        Self { name, start: enabled().then(Instant::now), budget }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(s) = self.start {
            let d = s.elapsed();
            if d >= self.budget {
                eprintln!("[perf] {} {:.1} ms", self.name, d.as_secs_f64() * 1000.);
            }
        }
    }
}

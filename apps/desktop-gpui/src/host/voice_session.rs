//! Sesi dikte: satu thread kerja yang memuat model, membuka mikrofon pilihan, lalu menjalankan pipeline
//! (VAD -> STT) dan mengirim `VoiceEvent` ke UI lewat channel. Berhenti lewat `stop()`: sisa ujaran tetap
//! dikirim sebagai final, lalu `Stopped`. UI tidak pernah menunggu thread ini (tidak ada `join` di UI).

use crate::voice_stt::{load_engine, Pipeline, VoiceConfig, VoiceEvent};
use futures::channel::mpsc::UnboundedSender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct VoiceSession {
    stop: Arc<AtomicBool>,
}

impl VoiceSession {
    /// Mulai sesi. Galat memuat model/membuka mikrofon dikirim sebagai `VoiceEvent::Error` lalu `Stopped`.
    pub fn start(cfg: VoiceConfig, tx: UnboundedSender<VoiceEvent>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let spawned = std::thread::Builder::new().name("voice-session".into()).spawn({
            let tx = tx.clone();
            move || {
                run(cfg, &tx, &flag);
                let _ = tx.unbounded_send(VoiceEvent::Stopped);
            }
        });
        if let Err(e) = spawned {
            let _ = tx.unbounded_send(VoiceEvent::Error(e.to_string()));
            let _ = tx.unbounded_send(VoiceEvent::Stopped);
        }
        Self { stop }
    }

    /// Minta berhenti. Hasil terakhir tetap datang sebelum `Stopped`.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for VoiceSession {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(cfg: VoiceConfig, tx: &UnboundedSender<VoiceEvent>, stop: &AtomicBool) {
    let send = |e: VoiceEvent| tx.unbounded_send(e).is_ok();
    send(VoiceEvent::Loading);
    let engine = match load_engine(&cfg) {
        Ok(e) => e,
        Err(e) => {
            send(VoiceEvent::Error(e));
            return;
        }
    };
    if stop.load(Ordering::SeqCst) {
        return;
    }
    let (atx, arx) = std::sync::mpsc::channel::<Vec<f32>>();
    let capture = match crate::voice_capture::start(&cfg.device, atx) {
        Ok(c) => c,
        Err(e) => {
            send(VoiceEvent::Error(e));
            return;
        }
    };
    send(VoiceEvent::Listening);
    let mut pipe = Pipeline::new(cfg, engine);
    while !stop.load(Ordering::SeqCst) {
        match arx.recv_timeout(Duration::from_millis(100)) {
            Ok(mut buf) => {
                // Gabungkan potongan yang menumpuk; `backlog` memberi tahu pipeline bila STT tertinggal.
                let mut backlog = 0;
                while let Ok(more) = arx.try_recv() {
                    buf.extend(more);
                    backlog += 1;
                }
                for e in pipe.feed(&buf, backlog) {
                    if !send(e) {
                        return; // UI sudah tidak mendengarkan
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                send(VoiceEvent::Error("perekaman mikrofon berhenti tiba-tiba".into()));
                break;
            }
        }
    }
    drop(capture); // menghentikan perekaman sebelum menyelesaikan ujaran terakhir
    let mut tail = Vec::new();
    while let Ok(more) = arx.try_recv() {
        tail.extend(more);
    }
    if !tail.is_empty() {
        for e in pipe.feed(&tail, 0) {
            send(e);
        }
    }
    for e in pipe.finish() {
        send(e);
    }
}

/// Susun isi composer dari teks sebelum kursor (`prefix`), ujaran yang sudah final, teks sementara, dan teks
/// sesudah kursor (`suffix`). Mengembalikan (teks, posisi kursor di akhir hasil dikte). Spasi pemisah
/// ditambahkan hanya bila perlu.
pub fn compose_text(prefix: &str, finals: &[String], tentative: &str, suffix: &str) -> (String, usize) {
    let mut s = String::from(prefix);
    let mut added = false;
    for part in finals.iter().map(String::as_str).chain(std::iter::once(tentative)) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if !s.is_empty() && !s.ends_with(char::is_whitespace) {
            s.push(' ');
        }
        s.push_str(part);
        added = true;
    }
    let caret = s.len();
    if added && !suffix.is_empty() && !suffix.starts_with(char::is_whitespace) {
        s.push(' ');
    }
    s.push_str(suffix);
    (s, caret)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_composer_gets_text_without_leading_space() {
        assert_eq!(compose_text("", &f(&["halo dunia"]), "", ""), ("halo dunia".to_string(), 10));
    }

    #[test]
    fn adds_space_only_when_needed() {
        assert_eq!(compose_text("tolong", &f(&["buka berkas"]), "", "").0, "tolong buka berkas");
        assert_eq!(compose_text("tolong ", &f(&["buka berkas"]), "", "").0, "tolong buka berkas");
        assert_eq!(compose_text("baris\n", &f(&["baru"]), "", "").0, "baris\nbaru");
    }

    #[test]
    fn finals_then_tentative_in_order_and_caret_after_dictation() {
        let (s, caret) = compose_text("a", &f(&["satu", "dua"]), "tiga", "");
        assert_eq!(s, "a satu dua tiga");
        assert_eq!(caret, s.len());
    }

    #[test]
    fn suffix_stays_after_caret() {
        let (s, caret) = compose_text("awal ", &f(&["tengah"]), "", "akhir");
        assert_eq!(s, "awal tengah akhir");
        assert_eq!(&s[..caret], "awal tengah");
        let (s2, _) = compose_text("awal ", &f(&["tengah"]), "", " akhir");
        assert_eq!(s2, "awal tengah akhir");
    }

    #[test]
    fn nothing_dictated_leaves_text_untouched() {
        let (s, caret) = compose_text("awal ", &[], "", "akhir");
        assert_eq!(s, "awal akhir");
        assert_eq!(caret, 5);
        assert_eq!(compose_text("x", &f(&["  ", ""]), "  ", "").0, "x");
    }

    #[test]
    fn tentative_is_replaced_not_accumulated() {
        let a = compose_text("", &[], "halo du", "").0;
        let b = compose_text("", &[], "halo dunia", "").0;
        assert_eq!(a, "halo du");
        assert_eq!(b, "halo dunia");
    }
}

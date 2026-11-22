//! Unduh model Whisper (ggml) dari Hugging Face ke `<data>/models/`. Dipicu pengguna dari Pengaturan (tombol Unduh);
//! berjalan di thread sendiri dan melapor lewat `DlEvent`. Aman: ditulis ke `.part` lalu diganti nama hanya bila
//! ukuran masuk akal dan sama dengan `Content-Length`; dapat dibatalkan; batas keras 3 GiB.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Batas keras byte yang diterima (cegah server nakal memenuhi disk).
const MAX_BYTES: u64 = 3 << 30;
const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

pub struct ModelInfo {
    pub name: &'static str,
    /// Perkiraan ukuran untuk label tombol (MB); ukuran sebenarnya diambil dari server.
    pub approx_mb: u64,
    /// Di bawah ini berkas dianggap rusak/terpotong.
    pub min_bytes: u64,
}

pub const MODELS: [ModelInfo; 4] = [
    ModelInfo { name: "base", approx_mb: 148, min_bytes: 100_000_000 },
    ModelInfo { name: "small", approx_mb: 488, min_bytes: 400_000_000 },
    ModelInfo { name: "medium", approx_mb: 1534, min_bytes: 1_000_000_000 },
    // large-v3-turbo terkuantisasi q5_0 (574.041.195 byte): akurasi mendekati large-v3, lebih cepat dari medium.
    ModelInfo { name: "large-v3-turbo-q5_0", approx_mb: 574, min_bytes: 500_000_000 },
];

pub fn info(model: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.name == model)
}

pub fn url(model: &str) -> Option<String> {
    info(model).map(|m| format!("{BASE_URL}/ggml-{}.bin", m.name))
}

#[derive(Clone, Debug, PartialEq)]
pub enum DlEvent {
    Progress { done: u64, total: Option<u64> },
    Done,
    Cancelled,
    Failed(String),
}

/// Unduh model terdaftar ke folder model aplikasi. Tepat satu peristiwa akhir (`Done`/`Cancelled`/`Failed`).
pub fn download(model: &str, cancel: &AtomicBool, mut on_event: impl FnMut(DlEvent)) {
    let (Some(info), Some(url), Some(dest)) = (info(model), url(model), crate::voice_stt::model_path(model)) else {
        on_event(DlEvent::Failed(format!("model tidak dikenal: {model}")));
        return;
    };
    download_to(&url, &dest, info.min_bytes, cancel, on_event);
}

fn agent() -> ureq::Agent {
    // Hanya fitur native-tls yang aktif: penyedia TLS harus dipilih eksplisit (sama dengan `engine.rs`).
    ureq::Agent::config_builder()
        .timeout_global(None)
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
        .build()
        .into()
}

/// Inti unduhan (dipisah dari URL Hugging Face agar bisa diuji dengan server lokal).
pub fn download_to(url: &str, dest: &Path, min_bytes: u64, cancel: &AtomicBool, mut on_event: impl FnMut(DlEvent)) {
    let part = dest.with_extension("bin.part");
    let result = run(url, dest, &part, min_bytes, cancel, &mut on_event);
    match result {
        Ok(true) => on_event(DlEvent::Done),
        Ok(false) => {
            let _ = std::fs::remove_file(&part);
            on_event(DlEvent::Cancelled);
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            on_event(DlEvent::Failed(e));
        }
    }
}

/// Ok(true) = selesai, Ok(false) = dibatalkan.
fn run(url: &str, dest: &Path, part: &Path, min_bytes: u64, cancel: &AtomicBool, on_event: &mut impl FnMut(DlEvent)) -> Result<bool, String> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("tidak bisa membuat folder model: {e}"))?;
    }
    let _ = std::fs::remove_file(part); // tanpa lanjut-unduh: selalu mulai bersih
    let resp = agent().get(url).call().map_err(|e| e.to_string())?;
    let total = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok());
    if total.is_some_and(|t| t > MAX_BYTES) {
        return Err("ukuran berkas melebihi batas yang diizinkan".into());
    }
    let (_, body) = resp.into_parts();
    let mut reader = body.into_with_config().limit(MAX_BYTES).reader();
    let mut file = std::fs::File::create(part).map_err(|e| format!("tidak bisa menulis berkas: {e}"))?;
    let mut last = Instant::now() - Duration::from_secs(1);
    let end = copy_with_progress(&mut reader, &mut file, cancel, |done| {
        if last.elapsed() >= Duration::from_millis(250) {
            last = Instant::now();
            on_event(DlEvent::Progress { done, total });
        }
    })
    .map_err(|e| format!("unduhan terputus: {e}"))?;
    let CopyEnd::Done(done) = end else { return Ok(false) };
    file.flush().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    size_ok(done, total, min_bytes)?;
    on_event(DlEvent::Progress { done, total });
    let _ = std::fs::remove_file(dest);
    std::fs::rename(part, dest).map_err(|e| format!("tidak bisa menyimpan model: {e}"))?;
    Ok(true)
}

#[derive(Debug, PartialEq)]
enum CopyEnd {
    Done(u64),
    Cancelled,
}

fn copy_with_progress(r: &mut impl Read, w: &mut impl Write, cancel: &AtomicBool, mut progress: impl FnMut(u64)) -> std::io::Result<CopyEnd> {
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Ok(CopyEnd::Cancelled);
        }
        let n = r.read(&mut buf)?;
        if n == 0 {
            return Ok(CopyEnd::Done(done));
        }
        w.write_all(&buf[..n])?;
        done += n as u64;
        progress(done);
    }
}

fn size_ok(done: u64, total: Option<u64>, min_bytes: u64) -> Result<(), String> {
    if done < min_bytes {
        return Err(format!("berkas terlalu kecil ({done} byte), kemungkinan rusak"));
    }
    if let Some(t) = total {
        if done != t {
            return Err(format!("ukuran tidak sesuai: diterima {done} dari {t} byte"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::TcpListener;

    #[test]
    fn known_models_and_urls() {
        assert_eq!(info("small").unwrap().approx_mb, 488);
        assert!(info("../x").is_none() && info("tiny").is_none());
        assert_eq!(url("base").unwrap(), "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin");
        assert!(url("a/b").is_none());
    }

    #[test]
    fn size_validation() {
        assert!(size_ok(500, Some(500), 100).is_ok());
        assert!(size_ok(500, None, 100).is_ok());
        assert!(size_ok(50, Some(50), 100).unwrap_err().contains("terlalu kecil"));
        assert!(size_ok(500, Some(600), 100).unwrap_err().contains("tidak sesuai"));
    }

    #[test]
    fn copy_reports_progress_and_honours_cancel() {
        let data = vec![7u8; 700_000];
        let mut out = Vec::new();
        let mut seen = vec![];
        let end = copy_with_progress(&mut Cursor::new(&data), &mut out, &AtomicBool::new(false), |d| seen.push(d)).unwrap();
        assert_eq!(end, CopyEnd::Done(700_000));
        assert_eq!(out.len(), 700_000);
        assert!(seen.windows(2).all(|w| w[0] < w[1]) && *seen.last().unwrap() == 700_000);
        let mut out2 = Vec::new();
        let end2 = copy_with_progress(&mut Cursor::new(&data), &mut out2, &AtomicBool::new(true), |_| {}).unwrap();
        assert_eq!(end2, CopyEnd::Cancelled);
        assert!(out2.is_empty());
    }

    /// Server HTTP satu-kali: mengirim `declared` sebagai Content-Length tetapi hanya `body` byte.
    fn serve_once(declared: usize, body: Vec<u8>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut req = [0u8; 2048];
                let _ = s.read(&mut req);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n");
                let _ = s.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}/m.bin")
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("zandor-dl-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn collect(url: &str, dest: &Path, min: u64, cancel: &AtomicBool) -> Vec<DlEvent> {
        let mut ev = vec![];
        download_to(url, dest, min, cancel, |e| ev.push(e));
        ev
    }

    #[test]
    fn downloads_and_renames_on_success() {
        let dir = tmp("ok");
        let dest = dir.join("models").join("ggml-x.bin");
        let url = serve_once(300_000, vec![1u8; 300_000]);
        let ev = collect(&url, &dest, 1000, &AtomicBool::new(false));
        assert_eq!(ev.last(), Some(&DlEvent::Done), "{ev:?}");
        assert_eq!(std::fs::metadata(&dest).unwrap().len(), 300_000);
        assert!(!dest.with_extension("bin.part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncated_download_fails_and_leaves_nothing() {
        let dir = tmp("trunc");
        let dest = dir.join("ggml-x.bin");
        let url = serve_once(500_000, vec![1u8; 100_000]);
        let ev = collect(&url, &dest, 1000, &AtomicBool::new(false));
        assert!(matches!(ev.last(), Some(DlEvent::Failed(_))), "{ev:?}");
        assert!(!dest.exists() && !dest.with_extension("bin.part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn too_small_file_is_rejected() {
        let dir = tmp("small");
        let dest = dir.join("ggml-x.bin");
        let url = serve_once(2000, vec![1u8; 2000]);
        let ev = collect(&url, &dest, 1_000_000, &AtomicBool::new(false));
        assert!(matches!(ev.last(), Some(DlEvent::Failed(m)) if m.contains("terlalu kecil")), "{ev:?}");
        assert!(!dest.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_removes_partial_file() {
        let dir = tmp("cancel");
        let dest = dir.join("ggml-x.bin");
        let url = serve_once(300_000, vec![1u8; 300_000]);
        let ev = collect(&url, &dest, 1000, &AtomicBool::new(true));
        assert_eq!(ev.last(), Some(&DlEvent::Cancelled), "{ev:?}");
        assert!(!dest.exists() && !dest.with_extension("bin.part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn http_error_is_reported() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut req = [0u8; 2048];
                let _ = s.read(&mut req);
                let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
        });
        let dir = tmp("404");
        let dest = dir.join("ggml-x.bin");
        let ev = collect(&format!("http://127.0.0.1:{port}/x"), &dest, 1000, &AtomicBool::new(false));
        assert!(matches!(ev.last(), Some(DlEvent::Failed(_))), "{ev:?}");
        assert!(!dest.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Suara pemberitahuan — port apps/desktop/src/shell/sound.ts. Nada bawaan DISINTESIS (pola sama: frekuensi, mulai,
//! durasi, bentuk gelombang, envelope eksponensial) menjadi WAV di memori lalu diputar lewat PlaySound (winmm). Berkas
//! kustom per peristiwa (maks 1 MB) disalin ke data aplikasi dan diputar lewat MCI. Gagal memutar tidak mengganggu aplikasi.
use std::path::PathBuf;

pub const EVENTS: [&str; 3] = ["done", "failed", "action"];
pub const MAX_CUSTOM_BYTES: u64 = 1024 * 1024;
const RATE: u32 = 44100;

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Triangle,
    Square,
}
struct Note {
    f: f64,
    at: f64,
    dur: f64,
    wave: Wave,
    gain: f64,
    to: Option<f64>,
}
const fn n(f: f64, at: f64, dur: f64) -> Note {
    Note { f, at, dur, wave: Wave::Sine, gain: 1., to: None }
}

fn pattern(choice: &str) -> Option<Vec<Note>> {
    Some(match choice {
        "chime" => vec![n(1046.5, 0., 0.22), n(1318.5, 0.11, 0.22), n(1568., 0.22, 0.4)],
        "pop" => vec![Note { to: Some(180.), gain: 0.9, ..n(620., 0., 0.14) }],
        "bell" => vec![n(880., 0., 1.0), Note { gain: 0.35, ..n(1760., 0., 0.6) }, Note { gain: 0.15, ..n(2640., 0., 0.3) }],
        "soft" => vec![Note { wave: Wave::Triangle, ..n(523.25, 0., 0.28) }, Note { wave: Wave::Triangle, ..n(659.25, 0.14, 0.34) }],
        "alert" => vec![Note { wave: Wave::Square, gain: 0.35, ..n(880., 0., 0.11) }, Note { wave: Wave::Square, gain: 0.35, ..n(880., 0.18, 0.11) }],
        "low" => vec![Note { gain: 0.9, ..n(233., 0., 0.3) }, Note { gain: 0.9, ..n(174.6, 0.2, 0.45) }],
        _ => return None,
    })
}

/// Sintesis pola menjadi WAV PCM 16-bit mono.
pub fn synth_wav(choice: &str, volume: f64) -> Option<Vec<u8>> {
    let notes = pattern(choice)?;
    let vol = volume.clamp(0., 1.);
    let end = notes.iter().map(|n| n.at + n.dur + 0.05).fold(0., f64::max);
    let len = (end * RATE as f64) as usize;
    let mut buf = vec![0f64; len];
    for nt in &notes {
        let peak = (vol * 0.4 * nt.gain).max(0.0002);
        let (start, attack) = (nt.at, 0.015);
        let mut phase = 0f64;
        let s0 = (start * RATE as f64) as usize;
        let s1 = (((start + nt.dur) * RATE as f64) as usize).min(len);
        for (k, out) in buf.iter_mut().enumerate().take(s1).skip(s0) {
            let t = (k - s0) as f64 / RATE as f64;
            // frekuensi: tetap, atau turun eksponensial ke `to`
            let f = match nt.to {
                Some(to) => nt.f * (to / nt.f).powf(t / nt.dur),
                None => nt.f,
            };
            phase += f / RATE as f64;
            let p = phase.fract();
            let w = match nt.wave {
                Wave::Sine => (p * std::f64::consts::TAU).sin(),
                Wave::Triangle => 1. - 4. * (p - 0.5).abs(),
                Wave::Square => if p < 0.5 { 1. } else { -1. },
            };
            // envelope eksponensial 0.0001 → peak (15 ms) → 0.0001
            let g = if t < attack { 0.0001 * (peak / 0.0001).powf(t / attack) } else { peak * (0.0001 / peak).powf((t - attack) / (nt.dur - attack).max(1e-3)) };
            *out += w * g;
        }
    }
    let mut wav = Vec::with_capacity(44 + len * 2);
    let data_len = (len * 2) as u32;
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in buf {
        wav.extend_from_slice(&((s.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
    }
    Some(wav)
}

fn sounds_dir() -> PathBuf {
    crate::core::services::app_data_dir().join("sounds")
}

/// Berkas kustom tersimpan untuk peristiwa: (nama asli, path).
pub fn custom_sound(ev: &str) -> Option<(String, PathBuf)> {
    let meta = std::fs::read_to_string(sounds_dir().join(format!("{ev}.json"))).ok()?;
    let v: serde_json::Value = serde_json::from_str(&meta).ok()?;
    let name = v["name"].as_str()?.to_string();
    let path = PathBuf::from(v["path"].as_str()?);
    path.exists().then_some((name, path))
}

pub fn clear_custom_sound(ev: &str) {
    if let Some((_, p)) = custom_sound(ev) {
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_file(sounds_dir().join(format!("{ev}.json")));
}

/// Simpan berkas suara pilihan pengguna (audio, maks 1 MB). Galat berbahasa Indonesia.
pub fn save_custom_sound(ev: &str, src: &std::path::Path) -> Result<String, String> {
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !["wav", "mp3", "wma", "aac", "m4a", "ogg", "flac", "mid", "midi"].contains(&ext.as_str()) {
        return Err("berkas bukan audio".into());
    }
    let size = std::fs::metadata(src).map_err(|_| "gagal membaca berkas".to_string())?.len();
    if size > MAX_CUSTOM_BYTES {
        return Err(format!("berkas terlalu besar (maks {} MB)", MAX_CUSTOM_BYTES / 1024 / 1024));
    }
    let dir = sounds_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    clear_custom_sound(ev);
    let dst = dir.join(format!("{ev}.{ext}"));
    std::fs::copy(src, &dst).map_err(|_| "gagal membaca berkas".to_string())?;
    let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    std::fs::write(dir.join(format!("{ev}.json")), serde_json::json!({ "name": name, "path": dst.to_string_lossy() }).to_string()).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Putar suara untuk satu peristiwa; false bila tidak ada yang diputar.
pub fn play(choice: &str, volume: f64, ev: &str) -> bool {
    let vol = volume.clamp(0., 1.);
    if choice == "none" || vol == 0. {
        return false;
    }
    if choice == "custom" {
        return custom_sound(ev).is_some_and(|(_, p)| play_file(&p, vol));
    }
    synth_wav(choice, vol).is_some_and(play_wav)
}

#[cfg(windows)]
fn play_wav(wav: Vec<u8>) -> bool {
    use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
    // SND_MEMORY + ASYNC butuh buffer hidup selama pemutaran: putar sinkron di utas sendiri.
    std::thread::spawn(move || unsafe {
        let _ = PlaySoundW(windows::core::PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY | SND_SYNC | SND_NODEFAULT);
    });
    true
}
#[cfg(not(windows))]
fn play_wav(_: Vec<u8>) -> bool {
    false
}

#[cfg(windows)]
fn play_file(path: &std::path::Path, vol: f64) -> bool {
    use windows::Win32::Media::Multimedia::mciSendStringW;
    let p = path.to_string_lossy().into_owned();
    std::thread::spawn(move || unsafe {
        let send = |cmd: String| {
            let w: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            mciSendStringW(windows::core::PCWSTR(w.as_ptr()), None, None)
        };
        let alias = "zandor_snd";
        send(format!("close {alias}"));
        if send(format!("open \"{p}\" type mpegvideo alias {alias}")) != 0 {
            return;
        }
        send(format!("setaudio {alias} volume to {}", (vol * 1000.) as u32));
        send(format!("play {alias} wait"));
        send(format!("close {alias}"));
    });
    true
}
#[cfg(not(windows))]
fn play_file(_: &std::path::Path, _: f64) -> bool {
    false
}

/// Notifikasi sistem (toast Windows). Mengembalikan galat bila tidak bisa ditampilkan.
#[cfg(windows)]
pub fn system_notify(title: &str, body: &str) -> Result<(), String> {
    use tauri_winrt_notification::Toast;
    Toast::new(Toast::POWERSHELL_APP_ID).title(title).text1(body).show().map_err(|e| e.to_string())
}
#[cfg(not(windows))]
pub fn system_notify(_: &str, _: &str) -> Result<(), String> {
    Err("tidak tersedia".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn wav() {
        let w = super::synth_wav("chime", 0.5).unwrap();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(w.len(), 44 + ((0.67f64 * 44100.) as usize) * 2);
        assert!(super::synth_wav("none", 1.).is_none());
        assert!(!super::play("none", 1., "done"));
    }
}

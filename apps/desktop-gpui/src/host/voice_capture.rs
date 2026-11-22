//! Perekaman mikrofon lewat WASAPI (mode shared, polling) pada endpoint yang dipilih pengguna.
//! Keluaran: potongan sampel 16 kHz mono f32 ke `Sender`. Berjalan di thread OS sendiri; berhenti saat
//! `CaptureHandle` dilepas. Di OS lain tidak tersedia (galat jujur, tidak dipalsukan). TODO(port): CoreAudio/ALSA.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Mulai merekam `device_id` (kosong = mikrofon bawaan sistem). Galat pembukaan perangkat dikembalikan
/// sinkron (izin ditolak, perangkat hilang, dipakai eksklusif), bukan ditelan di thread.
#[cfg(windows)]
pub fn start(device_id: &str, tx: Sender<Vec<f32>>) -> Result<CaptureHandle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let stop2 = stop.clone();
    let id = device_id.to_string();
    let thread = std::thread::Builder::new()
        .name("voice-capture".into())
        .spawn(move || win::run(&id, tx, stop2, ready_tx))
        .map_err(|e| e.to_string())?;
    match ready_rx.recv() {
        Ok(Ok(())) => Ok(CaptureHandle { stop, thread: Some(thread) }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => Err("thread perekam berhenti sebelum siap".into()),
    }
}

#[cfg(not(windows))]
pub fn start(_device_id: &str, _tx: Sender<Vec<f32>>) -> Result<CaptureHandle, String> {
    Err("perekaman mikrofon belum tersedia di platform ini".into())
}

// Seluruh isi modul ini panggilan COM/WASAPI yang sengaja `unsafe`; invarian dijelaskan di tiap fungsi.
#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
mod win {
    use super::*;
    use crate::voice_dsp::Resampler;
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
        AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, WAVEFORMATEX,
        WAVEFORMATEXTENSIBLE,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

    const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
    const WAVE_FORMAT_PCM: u16 = 1;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
    const BUFFERFLAGS_SILENT: u32 = 2;

    /// Log diagnostik 20 detik pertama tiap sesi dikte: `<app_data>/voice-capture.log` (untuk mencari mic senyap).
    fn diag(msg: &str) {
        use std::io::Write;
        let path = crate::core::services::app_data_dir().join("voice-capture.log");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{msg}");
        }
    }

    pub fn run(id: &str, tx: Sender<Vec<f32>>, stop: Arc<AtomicBool>, ready: Sender<Result<(), String>>) {
        // SAFETY: panggilan COM standar di thread sendiri; semua penunjuk berasal dari API dan hanya dipakai selama hidupnya.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            match open(id) {
                Err(e) => {
                    let _ = ready.send(Err(e));
                }
                Ok((client, capture, channels, hz, float)) => {
                    let _ = ready.send(Ok(()));
                    pump(&client, &capture, channels, hz, float, &tx, &stop);
                    let _ = client.Stop();
                }
            }
        }
    }

    unsafe fn open(id: &str) -> Result<(IAudioClient, IAudioCaptureClient, usize, u32, bool), String> {
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| format!("enumerator: {e}"))?;
        let dev = if id.is_empty() {
            en.GetDefaultAudioEndpoint(eCapture, eConsole).map_err(|e| format!("tidak ada mikrofon bawaan: {e}"))?
        } else {
            let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
            en.GetDevice(PCWSTR(wide.as_ptr())).map_err(|e| format!("mikrofon tidak ditemukan: {e}"))?
        };
        let client: IAudioClient = dev.Activate(CLSCTX_ALL, None).map_err(|e| format!("tidak bisa membuka mikrofon: {e}"))?;
        let pf = client.GetMixFormat().map_err(|e| format!("format: {e}"))?;
        let w: WAVEFORMATEX = std::ptr::read_unaligned(pf);
        let (tag, channels, hz, bits) = (w.wFormatTag, w.nChannels as usize, w.nSamplesPerSec, w.wBitsPerSample);
        let float = match tag {
            WAVE_FORMAT_IEEE_FLOAT => true,
            WAVE_FORMAT_PCM => false,
            WAVE_FORMAT_EXTENSIBLE => {
                let x: WAVEFORMATEXTENSIBLE = std::ptr::read_unaligned(pf as *const WAVEFORMATEXTENSIBLE);
                x.SubFormat.data1 == WAVE_FORMAT_IEEE_FLOAT as u32
            }
            _ => false,
        };
        if !float && bits != 16 {
            CoTaskMemFree(Some(pf as _));
            return Err(format!("format audio tidak didukung ({bits} bit)"));
        }
        // Penyangga 1 dtk (satuan 100 ns).
        let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, 10_000_000, 0, pf, None);
        CoTaskMemFree(Some(pf as _));
        init.map_err(|e| format!("mikrofon ditolak/dipakai aplikasi lain: {e}"))?;
        let capture: IAudioCaptureClient = client.GetService().map_err(|e| format!("capture: {e}"))?;
        client.Start().map_err(|e| format!("start: {e}"))?;
        Ok((client, capture, channels.max(1), hz, float))
    }

    unsafe fn pump(_client: &IAudioClient, cap: &IAudioCaptureClient, channels: usize, hz: u32, float: bool, tx: &Sender<Vec<f32>>, stop: &AtomicBool) {
        let mut rs = Resampler::new(hz);
        let t0 = std::time::Instant::now();
        let (mut packets, mut silent, mut peak, mut next_log) = (0u32, 0u32, 0f32, 1u64);
        diag(&format!("mulai: {hz} Hz, {channels} kanal, float={float}"));
        while !stop.load(Ordering::SeqCst) {
            if t0.elapsed().as_secs() >= next_log && next_log <= 20 {
                diag(&format!("t={next_log}s paket={packets} senyap={silent} puncak={peak:.5}"));
                next_log += 1;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            loop {
                let Ok(n) = cap.GetNextPacketSize() else { return };
                if n == 0 {
                    break;
                }
                let mut data: *mut u8 = std::ptr::null_mut();
                let (mut frames, mut flags) = (0u32, 0u32);
                if cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None).is_err() {
                    return;
                }
                packets += 1;
                if flags & BUFFERFLAGS_SILENT != 0 {
                    silent += 1;
                }
                let samples = frames as usize * channels;
                let buf: Vec<f32> = if flags & BUFFERFLAGS_SILENT != 0 || data.is_null() {
                    vec![0.0; samples]
                } else if float {
                    std::slice::from_raw_parts(data as *const f32, samples).to_vec()
                } else {
                    std::slice::from_raw_parts(data as *const i16, samples).iter().map(|s| *s as f32 / 32768.0).collect()
                };
                peak = buf.iter().fold(peak, |m, v| m.max(v.abs()));
                let _ = cap.ReleaseBuffer(frames);
                let out = rs.push(&buf, channels);
                if !out.is_empty() && tx.send(out).is_err() {
                    return; // penerima sudah tidak ada
                }
            }
        }
    }
}

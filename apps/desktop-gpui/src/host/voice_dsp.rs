//! DSP dikte: resample ke 16 kHz mono (format masukan Whisper) dan segmentasi ujaran berbasis energi (VAD).
//! Murni (tanpa perangkat/OS) sehingga bisa diuji. Segmenter memotong ujaran di jeda, memberi cuplikan
//! parsial berkala (untuk teks tentatif saat bicara) dan segmen final (untuk hasil akhir yang akurat).

pub const SAMPLE_RATE: u32 = 16_000;
const FRAME: usize = 320; // 20 md pada 16 kHz

/// Resampler mono berstatus (aman dipanggil per potongan audio): saring boxcar lalu interpolasi linear.
pub struct Resampler {
    step: f64,
    win: usize,
    acc: Vec<f32>,
    pos: f64,
    prev: f32,
    primed: bool,
}

impl Resampler {
    pub fn new(from_hz: u32) -> Self {
        let step = from_hz.max(1) as f64 / SAMPLE_RATE as f64;
        Self { step, win: (step.round() as usize).max(1), acc: Vec::new(), pos: 0.0, prev: 0.0, primed: false }
    }

    /// Campur kanal jadi mono lalu resample. `interleaved` berisi `channels` sampel per frame.
    pub fn push(&mut self, interleaved: &[f32], channels: usize) -> Vec<f32> {
        let ch = channels.max(1);
        let mut mono = Vec::with_capacity(interleaved.len() / ch);
        for fr in interleaved.chunks_exact(ch) {
            mono.push(fr.iter().sum::<f32>() / ch as f32);
        }
        // Saring boxcar (anti-aliasing kasar) bila turun-sampel.
        let filtered: Vec<f32> = if self.win > 1 {
            self.acc.extend_from_slice(&mono);
            let n = self.acc.len() / self.win;
            let out: Vec<f32> = (0..n).map(|k| self.acc[k * self.win..(k + 1) * self.win].iter().sum::<f32>() / self.win as f32).collect();
            self.acc.drain(..n * self.win);
            out
        } else {
            mono
        };
        // Setelah boxcar, laju efektif = from_hz / win; sisa rasio diinterpolasi.
        let step = self.step / self.win as f64;
        let mut out = Vec::new();
        if filtered.is_empty() {
            return out;
        }
        if !self.primed {
            self.prev = filtered[0];
            self.primed = true;
        }
        // Ruang indeks: 0 = self.prev, i+1 = filtered[i].
        let last = filtered.len() as f64;
        while self.pos < last {
            let i = self.pos.floor() as usize;
            let frac = (self.pos - i as f64) as f32;
            let a = if i == 0 { self.prev } else { filtered[i - 1] };
            let b = filtered[i];
            out.push(a + (b - a) * frac);
            self.pos += step;
        }
        self.pos -= last;
        self.prev = *filtered.last().unwrap();
        out
    }
}

pub fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt()
}

#[derive(Clone, Debug)]
pub struct SegConfig {
    pub end_silence_ms: u32,
    pub min_speech_ms: u32,
    pub max_segment_ms: u32,
    pub partial_every_ms: u32,
    pub preroll_ms: u32,
}

impl Default for SegConfig {
    fn default() -> Self {
        Self { end_silence_ms: 600, min_speech_ms: 240, max_segment_ms: 15_000, partial_every_ms: 1_000, preroll_ms: 200 }
    }
}

#[derive(Debug, PartialEq)]
pub enum SegEvent {
    /// Ujaran dimulai (untuk indikator).
    Start,
    /// Cuplikan sementara dari awal ujaran sampai sekarang.
    Partial(Vec<f32>),
    /// Ujaran selesai (jeda atau mencapai batas panjang).
    Final(Vec<f32>),
}

pub struct Segmenter {
    cfg: SegConfig,
    pending: Vec<f32>,
    preroll: Vec<f32>,
    seg: Vec<f32>,
    in_speech: bool,
    speech_ms: u32,
    silence_ms: u32,
    since_partial_ms: u32,
    noise: f32,
    /// Level terakhir (0..1) untuk indikator mikrofon.
    pub level: f32,
}

impl Segmenter {
    pub fn new(cfg: SegConfig) -> Self {
        Self { cfg, pending: vec![], preroll: vec![], seg: vec![], in_speech: false, speech_ms: 0, silence_ms: 0, since_partial_ms: 0, noise: 0.004, level: 0.0 }
    }

    fn threshold(&self) -> f32 {
        (self.noise * 3.0).max(0.012)
    }

    /// Masukkan sampel 16 kHz mono; kembalikan peristiwa yang terjadi.
    pub fn push(&mut self, samples: &[f32]) -> Vec<SegEvent> {
        self.pending.extend_from_slice(samples);
        let mut ev = Vec::new();
        while self.pending.len() >= FRAME {
            let frame: Vec<f32> = self.pending.drain(..FRAME).collect();
            self.frame(&frame, &mut ev);
        }
        ev
    }

    /// Akhiri paksa (mis. pengguna menekan berhenti): keluarkan sisa ujaran sebagai final.
    pub fn flush(&mut self) -> Vec<SegEvent> {
        let mut ev = Vec::new();
        self.finish(&mut ev);
        ev
    }

    fn finish(&mut self, ev: &mut Vec<SegEvent>) {
        if self.in_speech && self.speech_ms >= self.cfg.min_speech_ms {
            ev.push(SegEvent::Final(std::mem::take(&mut self.seg)));
        }
        self.seg.clear();
        self.in_speech = false;
        self.speech_ms = 0;
        self.silence_ms = 0;
        self.since_partial_ms = 0;
    }

    fn frame(&mut self, f: &[f32], ev: &mut Vec<SegEvent>) {
        let e = rms(f);
        self.level = (e * 6.0).min(1.0);
        let speech = e > self.threshold();
        let ms = (FRAME as u32 * 1000) / SAMPLE_RATE;
        if !self.in_speech {
            if speech {
                self.in_speech = true;
                self.speech_ms = ms;
                self.silence_ms = 0;
                self.since_partial_ms = 0;
                self.seg = std::mem::take(&mut self.preroll);
                self.seg.extend_from_slice(f);
                ev.push(SegEvent::Start);
            } else {
                // Perbarui noise floor hanya saat hening, lalu simpan pra-gulir.
                self.noise = self.noise * 0.95 + e * 0.05;
                self.preroll.extend_from_slice(f);
                let keep = (SAMPLE_RATE * self.cfg.preroll_ms / 1000) as usize;
                if self.preroll.len() > keep {
                    let cut = self.preroll.len() - keep;
                    self.preroll.drain(..cut);
                }
            }
            return;
        }
        self.seg.extend_from_slice(f);
        if speech {
            self.speech_ms += ms;
            self.silence_ms = 0;
        } else {
            self.silence_ms += ms;
        }
        self.since_partial_ms += ms;
        let seg_ms = (self.seg.len() as u32 * 1000) / SAMPLE_RATE;
        if self.silence_ms >= self.cfg.end_silence_ms || seg_ms >= self.cfg.max_segment_ms {
            self.finish(ev);
        } else if self.since_partial_ms >= self.cfg.partial_every_ms && self.speech_ms >= self.cfg.min_speech_ms {
            self.since_partial_ms = 0;
            ev.push(SegEvent::Partial(self.seg.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: u32, amp: f32) -> Vec<f32> {
        (0..(SAMPLE_RATE * ms / 1000) as usize).map(|n| amp * (n as f32 * 0.3).sin()).collect()
    }
    fn silence(ms: u32) -> Vec<f32> {
        vec![0.0; (SAMPLE_RATE * ms / 1000) as usize]
    }

    #[test]
    fn resample_48k_stereo_to_16k_mono() {
        let mut r = Resampler::new(48_000);
        let stereo: Vec<f32> = (0..48_000 * 2).map(|_| 0.5).collect(); // 1 dtk
        let out = r.push(&stereo, 2);
        assert!((out.len() as i64 - 16_000).abs() <= 2, "len {}", out.len());
        assert!(out.iter().skip(2).all(|s| (s - 0.5).abs() < 1e-3));
    }

    #[test]
    fn resample_is_continuous_across_chunks() {
        let data: Vec<f32> = (0..44_100).map(|n| (n as f32 * 0.01).sin()).collect();
        let whole = Resampler::new(44_100).push(&data, 1);
        let mut r = Resampler::new(44_100);
        let mut parts = vec![];
        for c in data.chunks(1000) {
            parts.extend(r.push(c, 1));
        }
        assert!((whole.len() as i64 - parts.len() as i64).abs() <= 2);
    }

    #[test]
    fn rms_of_silence_and_tone() {
        assert_eq!(rms(&silence(20)), 0.0);
        assert!(rms(&tone(20, 0.5)) > 0.3);
    }

    #[test]
    fn segments_speech_between_silences() {
        let mut s = Segmenter::new(SegConfig::default());
        let mut ev = s.push(&silence(500));
        ev.extend(s.push(&tone(800, 0.3)));
        ev.extend(s.push(&silence(900)));
        assert_eq!(ev.first(), Some(&SegEvent::Start));
        let finals: Vec<_> = ev.iter().filter(|e| matches!(e, SegEvent::Final(_))).collect();
        assert_eq!(finals.len(), 1);
        if let SegEvent::Final(a) = finals[0] {
            assert!(a.len() >= (SAMPLE_RATE * 800 / 1000) as usize);
        }
    }

    #[test]
    fn emits_partials_during_long_speech() {
        let mut s = Segmenter::new(SegConfig::default());
        let ev = s.push(&tone(3500, 0.3));
        assert!(ev.iter().filter(|e| matches!(e, SegEvent::Partial(_))).count() >= 2);
    }

    #[test]
    fn ignores_short_blips_and_noise() {
        let mut s = Segmenter::new(SegConfig::default());
        let mut ev = s.push(&tone(60, 0.3));
        ev.extend(s.push(&silence(900)));
        assert!(!ev.iter().any(|e| matches!(e, SegEvent::Final(_))));
        let mut n = Segmenter::new(SegConfig::default());
        assert!(n.push(&tone(2000, 0.002)).is_empty());
    }

    #[test]
    fn forces_split_at_max_length_and_flush_returns_tail() {
        let mut s = Segmenter::new(SegConfig { max_segment_ms: 2000, ..Default::default() });
        let ev = s.push(&tone(2500, 0.3));
        assert!(ev.iter().any(|e| matches!(e, SegEvent::Final(_))));
        let mut t = Segmenter::new(SegConfig::default());
        t.push(&tone(700, 0.3));
        assert!(matches!(t.flush().first(), Some(SegEvent::Final(_))));
    }
}

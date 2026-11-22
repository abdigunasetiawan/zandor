//! Pipeline dikte: audio 16 kHz mono -> segmenter (VAD) -> mesin STT -> peristiwa untuk UI.
//! Mesin STT berada di balik trait `Transcriber` agar logika teruji tanpa model, dan agar mesin
//! (Whisper lokal/cloud) bisa diganti tanpa menyentuh UI. Selama mesin belum terpasang, `load_engine`
//! mengembalikan galat jujur dan UI menonaktifkan tombol dengan alasannya (tanpa UI palsu, ADR-018).

use crate::voice_dsp::{rms, SegConfig, SegEvent, Segmenter};

#[derive(Clone, Debug, PartialEq)]
pub enum VoiceEvent {
    /// Model sedang dimuat (sebelum mikrofon dibuka).
    Loading,
    /// Mikrofon terbuka dan mendengarkan.
    Listening,
    /// Sesi selesai (setelah hasil terakhir terkirim, atau setelah galat).
    Stopped,
    /// Tingkat suara 0..1 (indikator mikrofon).
    Level(f32),
    /// Teks sementara (akan diganti oleh `Final` untuk ujaran yang sama).
    Partial(String),
    /// Teks akhir satu ujaran.
    Final(String),
    /// Hasil transkripsi ulang seluruh rekaman saat Stop (konteks utuh, lebih akurat); menggantikan semua `Final`.
    Refined(String),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct VoiceConfig {
    /// ID endpoint mikrofon; kosong = bawaan sistem.
    pub device: String,
    /// Kode bahasa ("id", "en", ...) atau "auto".
    pub language: String,
    /// Kosakata/ejaan yang dibisikkan ke model (initial prompt).
    pub vocabulary: String,
    /// Ukuran model ("base", "small", "medium").
    pub model: String,
    /// Kamus koreksi pengguna (salah, benar), diterapkan setelah transkripsi.
    pub corrections: Vec<(String, String)>,
}

/// Urai kamus koreksi: satu aturan per baris, `salah = benar`. Baris kosong, tanpa `=`, berawalan `#`,
/// atau dengan sisi kiri kosong diabaikan.
pub fn parse_corrections(src: &str) -> Vec<(String, String)> {
    src.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('#') {
                return None;
            }
            let (a, b) = l.split_once('=')?;
            let (a, b) = (a.trim(), b.trim());
            (!a.is_empty()).then(|| (a.to_string(), b.to_string()))
        })
        .collect()
}

/// Terapkan kamus koreksi: hanya pada kata utuh dan tanpa membedakan huruf besar-kecil.
pub fn apply_corrections(text: &str, rules: &[(String, String)]) -> String {
    let mut cur = text.to_string();
    for (from, to) in rules {
        let f: Vec<char> = from.chars().collect();
        let t: Vec<char> = cur.chars().collect();
        let mut out = String::with_capacity(cur.len());
        let mut i = 0;
        while i < t.len() {
            let end = i + f.len();
            let hit = end <= t.len()
                && (i == 0 || !t[i - 1].is_alphanumeric())
                && (end == t.len() || !t[end].is_alphanumeric())
                && t[i..end].iter().zip(&f).all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()));
            if hit {
                out.push_str(to);
                i = end;
            } else {
                out.push(t[i]);
                i += 1;
            }
        }
        cur = out;
    }
    cur
}

pub trait Transcriber: Send {
    /// `partial` = true untuk cuplikan sementara (boleh lebih cepat/kasar), false untuk ujaran final.
    fn transcribe(&mut self, audio: &[f32], language: &str, prompt: &str, partial: bool) -> Result<String, String>;
}

/// Lokasi berkas model ggml (`<data>/models/ggml-<ukuran>.bin`). Nama model hanya boleh `[a-z0-9._-]` supaya
/// tidak bisa dipakai keluar dari folder model.
pub fn model_path(model: &str) -> Option<std::path::PathBuf> {
    let ok = !model.is_empty() && model.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    ok.then(|| crate::core::services::app_data_dir().join("models").join(format!("ggml-{model}.bin")))
}

/// true bila berkas model sudah diunduh (dipakai UI untuk mengaktifkan tombol / menawarkan unduhan).
pub fn model_ready(model: &str) -> bool {
    model_path(model).is_some_and(|p| p.is_file())
}

/// Mesin Whisper lokal (whisper.cpp). Satu `WhisperState` dipakai ulang antar ujaran.
struct WhisperEngine {
    state: whisper_rs::WhisperState,
    threads: i32,
}

/// Muat mesin STT dari berkas model yang sudah diunduh. Galat disebutkan apa adanya (model belum ada, rusak).
pub fn load_engine(cfg: &VoiceConfig) -> Result<Box<dyn Transcriber>, String> {
    use whisper_rs::{WhisperContext, WhisperContextParameters};
    let path = model_path(&cfg.model).ok_or_else(|| format!("nama model tidak valid: {}", cfg.model))?;
    if !path.is_file() {
        return Err(format!("model '{}' belum diunduh", cfg.model));
    }
    let ctx = WhisperContext::new_with_params(&path, WhisperContextParameters::default()).map_err(|e| format!("model gagal dimuat: {e}"))?;
    let state = ctx.create_state().map_err(|e| format!("gagal menyiapkan mesin: {e}"))?;
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    Ok(Box::new(WhisperEngine { state, threads: cores.saturating_sub(1).clamp(2, 8) as i32 }))
}

impl Transcriber for WhisperEngine {
    fn transcribe(&mut self, audio: &[f32], language: &str, prompt: &str, partial: bool) -> Result<String, String> {
        use whisper_rs::{FullParams, SamplingStrategy};
        // Sementara = Greedy (cepat); final = BeamSearch (lebih akurat, hasil inilah yang masuk composer).
        let strategy = if partial { SamplingStrategy::Greedy { best_of: 1 } } else { SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 } };
        let mut p = FullParams::new(strategy);
        p.set_n_threads(self.threads);
        p.set_translate(false);
        p.set_language(if language.is_empty() || language == "auto" { None } else { Some(language) });
        p.set_no_context(true);
        // Satu ujaran = satu segmen tanpa timestamp. Tanpa ini ujaran pendek ("Halo.") berakhir dengan satu token
        // timestamp tunggal dan whisper.cpp membuang seluruh potongannya ("single timestamp ending - skip entire chunk").
        p.set_single_segment(true);
        p.set_no_timestamps(true);
        p.set_temperature(0.0);
        p.set_suppress_blank(true);
        p.set_suppress_nst(true);
        p.set_no_speech_thold(0.6);
        p.set_print_special(false);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_timestamps(false);
        if !prompt.is_empty() {
            p.set_initial_prompt(prompt);
        }
        // Whisper menolak/kacau pada audio < 1 dtk: isi dengan hening.
        let mut padded;
        let data = if audio.len() < crate::voice_dsp::SAMPLE_RATE as usize {
            padded = audio.to_vec();
            padded.resize(crate::voice_dsp::SAMPLE_RATE as usize, 0.0);
            &padded[..]
        } else {
            audio
        };
        self.state.full(p, data).map_err(|e| format!("transkripsi gagal: {e}"))?;
        let mut out = String::new();
        for seg in self.state.as_iter() {
            if let Ok(t) = seg.to_str_lossy() {
                out.push_str(&t);
            }
        }
        Ok(out.trim().to_string())
    }
}

/// Kalimat yang sering "dihalusinasikan" Whisper pada audio hening/derau; dibuang agar tidak mengotori composer.
const HALLUCINATIONS: &[&str] = &[
    "terima kasih telah menonton",
    "terima kasih sudah menonton",
    "terima kasih",
    "thanks for watching",
    "thank you for watching",
    "thank you",
    "subtitles by",
    "amara.org",
    "you",
];

/// Bersihkan keluaran model: rapikan spasi, buang penanda non-ujaran seperti `[musik]`/`(tepuk tangan)`,
/// dan tolak halusinasi umum.
pub fn clean_text(raw: &str) -> String {
    let mut s = String::new();
    let mut depth = 0u32;
    for ch in raw.chars() {
        match ch {
            '[' | '(' | '♪' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => s.push(ch),
            _ => {}
        }
    }
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let norm = s.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    if norm.is_empty() || HALLUCINATIONS.iter().any(|h| norm == *h) {
        return String::new();
    }
    s
}

pub struct Pipeline {
    seg: Segmenter,
    cfg: VoiceConfig,
    engine: Box<dyn Transcriber>,
    last_level: f32,
    /// Seluruh audio sesi (sudah dikuatkan) untuk transkripsi ulang saat Stop; berhenti terisi di `REFINE_MAX_SAMPLES`.
    recording: Vec<f32>,
    recording_overflow: bool,
    /// Jumlah ujaran final; transkripsi ulang hanya bila ada >= 2 (satu ujaran sudah utuh).
    segments: u32,
    /// Puncak sinyal terkini (meluruh perlahan) untuk penguat otomatis mic yang pelan.
    peak_est: f32,
}

/// Sasaran puncak setelah dikuatkan, lantai estimasi puncak (menahan penguatan pada keheningan), dan batas penguatan.
/// Batas rekaman yang ditranskripsi ulang (60 dtk pada 16 kHz); lebih panjang dari ini hasil per ujaran dipakai apa adanya.
const REFINE_MAX_SAMPLES: usize = 60 * 16_000;
const AGC_TARGET: f32 = 0.3;
const AGC_FLOOR: f32 = 0.008;
const AGC_MAX_GAIN: f32 = 40.0;

/// Penguat otomatis sederhana: mic pelan (puncak ~0,01 pada laptop dengan gain rendah) dikuatkan sampai ~0,3 supaya
/// VAD dan Whisper bekerja; mic normal (gain 1) tidak berubah. Penguatan tidak turun di bawah 1.
pub fn agc_gain(peak_est: f32) -> f32 {
    (AGC_TARGET / peak_est.max(AGC_FLOOR)).clamp(1.0, AGC_MAX_GAIN)
}

impl Pipeline {
    pub fn new(cfg: VoiceConfig, engine: Box<dyn Transcriber>) -> Self {
        Self { seg: Segmenter::new(SegConfig::default()), cfg, engine, last_level: 0.0, recording: Vec::new(), recording_overflow: false, segments: 0, peak_est: 0.0 }
    }

    /// Masukkan audio dan kembalikan peristiwa. Cuplikan sementara dilewati bila ada pekerjaan yang menumpuk
    /// (caller memberi `backlog` = jumlah potongan audio yang masih antre) supaya teks tidak tertinggal dari suara.
    pub fn feed(&mut self, audio: &[f32], backlog: usize) -> Vec<VoiceEvent> {
        let chunk_peak = audio.iter().fold(0f32, |m, v| m.max(v.abs()));
        self.peak_est = chunk_peak.max(self.peak_est * 0.99999f32.powi(audio.len().min(1 << 20) as i32));
        let gain = agc_gain(self.peak_est);
        let boosted: Option<Vec<f32>> = (gain > 1.0).then(|| audio.iter().map(|v| (v * gain).clamp(-1.0, 1.0)).collect());
        let used: &[f32] = boosted.as_deref().unwrap_or(audio);
        if !self.recording_overflow {
            if self.recording.len() + used.len() > REFINE_MAX_SAMPLES {
                self.recording_overflow = true;
                self.recording = Vec::new();
            } else {
                self.recording.extend_from_slice(used);
            }
        }
        let events = self.seg.push(used);
        self.handle(events, backlog)
    }

    /// Selesaikan ujaran yang sedang berjalan (pengguna menekan berhenti).
    pub fn finish(&mut self) -> Vec<VoiceEvent> {
        let events = self.seg.flush();
        let mut out = self.handle(events, 0);
        // Rekaman beberapa ujaran (jeda memecahnya, tiap potongan tanpa konteks): transkripsi ulang utuh seperti
        // aplikasi referensi (rekam lalu transkripsi sekali) supaya kata, ejaan, dan tanda baca lebih akurat.
        if self.segments >= 2 && !self.recording_overflow && !self.recording.is_empty() {
            let all = std::mem::take(&mut self.recording);
            if let Ok(t) = self.engine.transcribe(&all, &self.cfg.language, &self.cfg.vocabulary, false) {
                let t = apply_corrections(&clean_text(&t), &self.cfg.corrections);
                if !t.is_empty() {
                    out.push(VoiceEvent::Refined(t));
                }
            }
        }
        out
    }

    fn handle(&mut self, events: Vec<SegEvent>, backlog: usize) -> Vec<VoiceEvent> {
        let mut out = Vec::new();
        let level = self.seg.level;
        if (level - self.last_level).abs() > 0.02 {
            self.last_level = level;
            out.push(VoiceEvent::Level(level));
        }
        for e in events {
            match e {
                SegEvent::Start => {}
                SegEvent::Partial(a) => {
                    if backlog > 2 || rms(&a) < 0.005 {
                        continue;
                    }
                    if let Ok(t) = self.engine.transcribe(&a, &self.cfg.language, &self.cfg.vocabulary, true) {
                        let t = apply_corrections(&clean_text(&t), &self.cfg.corrections);
                        if !t.is_empty() {
                            out.push(VoiceEvent::Partial(t));
                        }
                    }
                }
                SegEvent::Final(a) => {
                    self.segments += 1;
                    match self.engine.transcribe(&a, &self.cfg.language, &self.cfg.vocabulary, false) {
                        Ok(t) => out.push(VoiceEvent::Final(apply_corrections(&clean_text(&t), &self.cfg.corrections))),
                        Err(e) => out.push(VoiceEvent::Error(e)),
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice_dsp::SAMPLE_RATE;

    struct Fake;
    impl Transcriber for Fake {
        fn transcribe(&mut self, audio: &[f32], lang: &str, prompt: &str, partial: bool) -> Result<String, String> {
            Ok(format!("{}|{}|{}|{}", lang, prompt, if partial { "p" } else { "f" }, audio.len() > 0))
        }
    }

    fn cfg() -> VoiceConfig {
        VoiceConfig { device: String::new(), language: "id".into(), vocabulary: "Zandor".into(), model: "small".into(), corrections: vec![] }
    }

    #[test]
    fn parses_correction_lines() {
        let r = parse_corrections("xander = Zandor\n# komentar\n\nbaris tanpa sama\n= kosong\n  g pui=GPUI  \nhapus =");
        assert_eq!(r, vec![("xander".into(), "Zandor".into()), ("g pui".into(), "GPUI".into()), ("hapus".into(), "".into())]);
    }

    #[test]
    fn applies_corrections_on_whole_words_case_insensitively() {
        let r = parse_corrections("xander = Zandor\ng pui = GPUI");
        assert_eq!(apply_corrections("Test of the Xander voice feature", &r), "Test of the Zandor voice feature");
        assert_eq!(apply_corrections("XANDER, and g pui!", &r), "Zandor, and GPUI!");
        // bukan kata utuh: tidak diubah
        assert_eq!(apply_corrections("xanderson", &r), "xanderson");
        assert_eq!(apply_corrections("tanpa aturan", &[]), "tanpa aturan");
    }

    #[test]
    fn pipeline_applies_corrections_to_final_text() {
        struct Says;
        impl Transcriber for Says {
            fn transcribe(&mut self, _: &[f32], _: &str, _: &str, _: bool) -> Result<String, String> {
                Ok("buka proyek Xander".into())
            }
        }
        let c = VoiceConfig { corrections: parse_corrections("xander = Zandor"), ..cfg() };
        let mut p = Pipeline::new(c, Box::new(Says));
        let mut ev = p.feed(&tone(1500), 0);
        ev.extend(p.finish());
        assert!(ev.contains(&VoiceEvent::Final("buka proyek Zandor".into())), "{ev:?}");
    }
    fn tone(ms: u32) -> Vec<f32> {
        (0..(SAMPLE_RATE * ms / 1000) as usize).map(|n| 0.3 * (n as f32 * 0.3).sin()).collect()
    }

    fn quiet(ms: u32) -> Vec<f32> {
        vec![0.0; (SAMPLE_RATE * ms / 1000) as usize]
    }

    #[test]
    fn refines_whole_recording_when_several_utterances() {
        let mut p = Pipeline::new(cfg(), Box::new(Fake));
        let mut ev = p.feed(&tone(1500), 0);
        ev.extend(p.feed(&quiet(1200), 0));
        ev.extend(p.feed(&tone(1500), 0));
        ev.extend(p.finish());
        let finals = ev.iter().filter(|e| matches!(e, VoiceEvent::Final(_))).count();
        assert!(finals >= 2, "{ev:?}");
        assert!(ev.iter().any(|e| matches!(e, VoiceEvent::Refined(t) if t.starts_with("id|Zandor|f"))), "{ev:?}");
    }

    #[test]
    fn no_refine_for_single_utterance() {
        let mut p = Pipeline::new(cfg(), Box::new(Fake));
        let mut ev = p.feed(&tone(1500), 0);
        ev.extend(p.finish());
        assert!(!ev.iter().any(|e| matches!(e, VoiceEvent::Refined(_))), "{ev:?}");
    }

    #[test]
    fn clean_text_drops_markers_and_hallucinations() {
        assert_eq!(clean_text("  halo   dunia [musik] "), "halo dunia");
        assert_eq!(clean_text("(tepuk tangan)"), "");
        assert_eq!(clean_text("Terima kasih telah menonton!"), "");
        assert_eq!(clean_text("Thank you."), "");
        assert_eq!(clean_text("Terima kasih atas bantuannya"), "Terima kasih atas bantuannya");
    }

    #[test]
    fn passes_language_and_vocabulary_to_engine() {
        let mut p = Pipeline::new(cfg(), Box::new(Fake));
        let mut ev = p.feed(&tone(2500), 0);
        ev.extend(p.finish());
        assert!(ev.iter().any(|e| matches!(e, VoiceEvent::Partial(t) if t.starts_with("id|Zandor|p"))));
        assert!(ev.iter().any(|e| matches!(e, VoiceEvent::Final(t) if t.starts_with("id|Zandor|f"))));
    }

    #[test]
    fn skips_partials_under_backlog_but_keeps_final() {
        let mut p = Pipeline::new(cfg(), Box::new(Fake));
        let mut ev = p.feed(&tone(2500), 5);
        ev.extend(p.finish());
        assert!(!ev.iter().any(|e| matches!(e, VoiceEvent::Partial(_))));
        assert!(ev.iter().any(|e| matches!(e, VoiceEvent::Final(_))));
    }

    #[test]
    fn engine_error_is_reported_not_swallowed() {
        struct Bad;
        impl Transcriber for Bad {
            fn transcribe(&mut self, _: &[f32], _: &str, _: &str, _: bool) -> Result<String, String> {
                Err("model rusak".into())
            }
        }
        let mut p = Pipeline::new(cfg(), Box::new(Bad));
        let mut ev = p.feed(&tone(1500), 0);
        ev.extend(p.finish());
        assert!(ev.contains(&VoiceEvent::Error("model rusak".into())));
    }

    #[test]
    fn missing_model_is_reported_honestly() {
        let c = VoiceConfig { model: "zz-tidak-ada".into(), ..cfg() };
        assert!(!model_ready("zz-tidak-ada"));
        let e = load_engine(&c).err().expect("harus galat");
        assert!(e.contains("belum diunduh"), "{e}");
    }

    /// Uji sungguhan (butuh model terunduh): `ZANDOR_TEST_WAV=<wav 16 kHz mono 16-bit> ZANDOR_TEST_LANG=en
    /// cargo test real_model -- --ignored --nocapture`. Tidak jalan di `cargo test` biasa.
    #[test]
    #[ignore]
    fn real_model_transcribes_wav() {
        let wav = std::env::var("ZANDOR_TEST_WAV").expect("set ZANDOR_TEST_WAV");
        let lang = std::env::var("ZANDOR_TEST_LANG").unwrap_or_else(|_| "en".into());
        let bytes = std::fs::read(wav).unwrap();
        let at = bytes.windows(4).position(|w| w == b"data").expect("chunk data") + 8;
        let audio: Vec<f32> = bytes[at..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
        let c = VoiceConfig { language: lang.clone(), ..cfg() };
        assert!(model_ready(&c.model), "model {} belum diunduh", c.model);
        let mut e = load_engine(&c).expect("muat model");
        let t0 = std::time::Instant::now();
        let plain = e.transcribe(&audio, &lang, "", false).unwrap();
        let t_plain = t0.elapsed();
        let hinted = e.transcribe(&audio, &lang, "Zandor, GPUI, commit, repository.", false).unwrap();
        let partial = e.transcribe(&audio[..audio.len() / 2], &lang, "", true).unwrap();
        println!("audio {:.1}s | final tanpa prompt ({:?}): {plain}", audio.len() as f32 / 16000.0, t_plain);
        println!("final dengan prompt: {hinted}");
        println!("partial (separuh audio): {partial}");
        assert!(!clean_text(&plain).is_empty(), "keluaran kosong");
    }

    #[test]
    fn agc_boosts_quiet_mic_but_leaves_normal_mic_alone() {
        assert_eq!(agc_gain(0.3), 1.0);
        assert_eq!(agc_gain(0.9), 1.0);
        let g = agc_gain(0.008);
        assert!(g > 30.0 && g <= AGC_MAX_GAIN, "{g}");
        // Keheningan murni tidak menaikkan penguatan melewati batas.
        assert_eq!(agc_gain(0.0), agc_gain(AGC_FLOOR));
    }

    #[test]
    fn model_name_cannot_escape_models_folder() {
        assert!(model_path("small").is_some());
        assert!(model_path("../../etc/passwd").is_none());
        assert!(model_path("a/b").is_none());
        assert!(model_path("").is_none());
        assert!(model_path("Small").is_none());
        let p = model_path("small").unwrap();
        assert!(p.ends_with("models/ggml-small.bin") || p.ends_with("models\\ggml-small.bin"));
    }
}

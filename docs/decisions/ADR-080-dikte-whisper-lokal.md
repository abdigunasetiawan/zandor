# ADR-080 — Dikte realtime dengan Whisper lokal (menggantikan Win+H)

Tanggal: 20-11-2026 · Status: diterima (sebagian teruji, lihat Verifikasi) · Mengubah: bagian "Input suara" ADR-042

## Konteks

Tombol suara di composer GPUI hanya mengirim pintasan Win+H. Aplikasi tidak merekam audio dan tidak punya hasil
transkripsi; pilihan mikrofon (`zandor.voice.device`) tidak berpengaruh karena dikte Windows selalu memakai mikrofon
bawaan sistem, dan bahasa mengikuti pengaturan Windows. Pemilik meminta: pilih perangkat input, pilih bahasa, akurat
untuk bahasa Indonesia, dan realtime.

## Keputusan

1. **Rekam sendiri** lewat WASAPI (mode shared, polling) pada endpoint pilihan pengguna (`host/voice_capture.rs`);
   keluaran 16 kHz mono f32 (`host/voice_dsp.rs`: resampler berstatus + saring boxcar).
2. **VAD berbasis energi** dengan noise floor adaptif memotong ucapan di jeda ~600 md, memberi cuplikan sementara tiap
   ~1 dtk, dan segmen final (maks 15 dtk).
3. **Whisper lokal** (`whisper-rs` 0.16, whisper.cpp; lisensi Unlicense/MIT, lolos `deny.toml`) di balik trait
   `Transcriber` (`host/voice_stt.rs`). Hasil sementara memakai Greedy (cepat), hasil final memakai BeamSearch 5.
   Model ggml diunduh pengguna ke `<data>/models/ggml-<ukuran>.bin`; nama model divalidasi (`[a-z0-9._-]`).
4. **Akurasi:** bahasa dipilih eksplisit (`voiceLanguage`: id/en/auto, bawaan `id`), ukuran model (`voiceModel`:
   base/small/medium, bawaan small), kosakata sebagai initial prompt, filter halusinasi Whisper pada hening, dan
   **kamus koreksi** `salah = benar` (kata utuh, tanpa membedakan huruf besar-kecil) yang diterapkan setelah transkripsi.
5. **Sesi** (`host/voice_session.rs`): satu thread memuat model, membuka mikrofon, menjalankan pipeline, dan mengirim
   `VoiceEvent` lewat channel ke UI. `compose_text` menyisipkan hasil di posisi kursor; teks sementara diganti, bukan
   ditumpuk. Tombol mic: siap → memuat → merekam (tombol stop + titik level). Menu perangkat dikunci saat merekam.
6. Setting: `voiceLanguage` dan `voiceModel` lewat skema (grup Chat); kosakata dan kamus koreksi (teks bebas) di
   `prefs.json` (`zandor.voice.vocabulary`, `zandor.voice.corrections`) lewat `shell/voice_section.rs`.
7. **Unduh model dari aplikasi** (`host/voice_models.rs`, tombol di Pengaturan → Chat): sumber
   `huggingface.co/ggerganov/whisper.cpp`, dipicu klik pengguna dengan perkiraan ukuran di label (base ±148 MB,
   small ±488 MB, medium ±1,5 GB); ditulis ke `.part`, diganti nama hanya bila ukuran ≥ batas minimum model dan sama
   dengan `Content-Length`; progres, tombol Batal, batas keras 3 GiB; tidak ada lanjut-unduh. Checksum tidak
   diperiksa (hash resmi belum diverifikasi terhadap sumber independen).
8. Jujur bila tidak tersedia: model belum diunduh → toast peringatan dengan lokasi berkas; bukan Windows → tombol
   nonaktif dengan alasan. Kode Win+H dihapus.

## Konsekuensi

- **Build butuh toolchain tambahan** di Windows: CMake, Build Tools C++ (MSVC), dan LLVM (libclang untuk bindgen).
  Pada mesin pengembangan, `bindgen` perlu `BINDGEN_EXTRA_CLANG_ARGS` yang menunjuk header clang (`stdbool.h`) dan header
  MSVC (`vcruntime.h`), `LIBCLANG_PATH`, dan generator `CMAKE_GENERATOR=NMake Makefiles`. Tanpa itu `whisper-rs-sys`
  jatuh ke `bindings.rs` bawaan buatan Linux dan gagal dikompilasi. Belum dipasang permanen di proyek.
- `cargo test --offline` memerlukan crate `whisper-rs` ada di cache (diunduh sekali dengan jaringan).
- Binary lebih besar dan build pertama lama (whisper.cpp dikompilasi dari sumber; ±6 menit pada mesin pengembangan).
- Whisper bukan streaming asli: teks sementara adalah decode ulang buffer berjalan; latensinya bergantung CPU.
- Keterbatasan: bila pengguna mengedit composer saat dikte, teks sementara yang sedang tampil ikut menjadi bagian
  jangkar baru.

## Verifikasi

Lulus: tes unit DSP/VAD/resampler, pipeline dengan mesin palsu (bahasa, kosakata, backlog, galat), `clean_text`,
kamus koreksi, `compose_text`, validasi nama model, skema + label i18n. Uji manual mesin dengan model `small` sungguhan
pada suara TTS bahasa Inggris 7,6 dtk: teks benar kecuali nama proyek ("Xander"; dengan prompt "Zandoor") — alasan
adanya kamus koreksi; decode final 3,6 dtk (satu pengukuran).

**Belum teruji:** akurasi bahasa Indonesia, perekaman WASAPI dengan mikrofon sungguhan, perilaku UI (tombol, titik
level, penyisipan teks, tombol unduh) di jendela aplikasi, unduhan nyata dari Hugging Face lewat tombol, latensi
streaming, model `base`/`medium`. Logika unduh teruji dengan server HTTP lokal (sukses, terpotong, terlalu kecil,
dibatalkan, HTTP 404; tanpa berkas sisa).

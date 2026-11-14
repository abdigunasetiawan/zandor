# ADR-040 — Render respons kaya, form pertanyaan agent, info pemakaian, Masuk dengan OpenWork

- **Tanggal:** 14-11-2026
- **Status:** Diterima
- **Permintaan:** pemilik (14-11-2026): respons model dirender lengkap seperti harness (teks, blok kode, Mermaid, tabel, form
  pertanyaan), provider sign-in OpenWork (mengacu implementasi `my-harness`), dan elemen limit/usage/context di composer.
- **Terkait:** ADR-033 (chat), ADR-034 (provider), ADR-035 (Mermaid/KaTeX), ADR-038 (TLS native)

## Keputusan

1. **Renderer Markdown kaya** (`shell/rich/RichMarkdown.tsx`, dimuat malas): GFM (tabel, daftar tugas, coret), KaTeX,
   **blok kode berwarna** (`rehype-highlight` + `lowlight`/highlight.js, bahasa umum + PowerShell/Dockerfile; label bahasa,
   tombol **Salin**, blok > 30 baris diciutkan), **Mermaid** (strict + dagre; selama jawaban masih mengalir sumbernya
   ditampilkan dan diagram dirender setelah selesai; diagram tidak valid → galat + sumber), tabel dalam area gulir.
   Warna sintaks adalah token `--z-code-*` di `tokens.css` (gelap/terang). **Keamanan tidak berubah**: HTML mentah tidak
   dirender, tautan hanya http(s)/mailto/#, **gambar luar tidak dimuat** (mencegah eksfiltrasi lewat URL; ditampilkan sebagai teks alt).
2. **Form pertanyaan agent**: tool `question` opencode diaktifkan lewat env `OPENCODE_ENABLE_QUESTION_TOOL=true`
   (hanya klien resmi yang bawaannya aktif). Core: tipe `QuestionRequest`, event `question.asked/replied/rejected`, API
   `listQuestions/replyQuestion/rejectQuestion`, aksi `answerQuestion/rejectQuestion`. UI `QuestionCard`: radio (tunggal) atau
   checkbox (jamak), jawaban sendiri bila `custom`, tombol Kirim aktif hanya bila semua pertanyaan terjawab, **Lewati**
   menolak. **Daftar tugas agent** (`todo.updated`/`session.todo`) tampil di atas composer.
3. **Info pemakaian di composer** (`ComposerInfo`): model aktif, jendela konteks terpakai/batas (peringatan ≥ 80% dan ≥ 95%),
   token masuk/keluar, biaya (hanya bila dilaporkan), dan rincian (sisa konteks, batas keluaran, reasoning, jumlah pesan,
   tool call, waktu kerja, mode izin). Semua dihitung dari pesan sesi nyata; batas dari metadata model engine
   (`limit.context/output`). **Kuota/limit tarif provider tidak diekspos engine** sehingga tidak ditampilkan (dinyatakan di UI).
4. **Masuk dengan OpenWork** (`openwork.rs`, mengikuti `my-harness`): OAuth 2.0 **Device Authorization Grant (RFC 8628)** ke
   `api.openworklabs.com` (client id `openwork-opencode-plugin`, cadangan `openwork-cli`) → tukar sesi akun menjadi kunci
   **AI Gateway** (`ow_gw_…` dari `/v1/inference-providers/*/connect`, atau kredensial gratis `ow_inf_…`) → simpan provider
   `openwork` (endpoint OpenAI-compatible `https://inference.openworklabs.com/api/v1`, model dari gateway atau `/models`) dengan
   kunci di **keychain**. Token sesi dan kunci **hanya ada di Rust** (tidak pernah ke JS/UI/berkas); URL verifikasi dari server
   divalidasi (https + host `openworklabs.com`, tanpa kredensial/port) sebelum dibuka; kode kedaluwarsa dan `slow_down`
   ditangani; akun tanpa kredensial gateway dilaporkan jelas (tidak memakai token sesi sebagai kunci seperti `my-harness`).
   Perbedaan dari `my-harness` yang disengaja: tidak ada pemetaan model ke provider lain (hack routing spesifik proyek itu).
5. **TLS**: ureq memakai `native-tls` dengan **verifikasi platform (OS)** (`RootCerts::PlatformVerifier`) untuk Open VSX,
   probe provider, dan OpenWork. Sebelumnya akar bawaan WebPki membuat sebagian situs (mis. OpenWork) gagal verifikasi.

## Bukti

- Unit: `usage-format.test.ts`, `questions.test.ts` (7), `openwork.test.ts` (7), Rust `openwork` (11 + 1 jaringan nyata),
  komponen `OpenWorkLogin.test.tsx` (4).
- E2E: `rich.spec.ts` (3, termasuk konten berbahaya), `questions.spec.ts` (4), `composer-info.spec.ts` (3).
- Jaringan nyata: `cargo test --lib openwork -- --ignored` meminta kode perangkat ke server OpenWork asli dan memastikan
  status menunggu. Login penuh **tidak** diuji otomatis (butuh persetujuan manusia di browser).

## Batas / belum

- Login OpenWork penuh (persetujuan → kunci gateway → model) belum pernah dijalankan dengan akun sungguhan di sesi ini; logika
  diuji dengan server tiruan dan sebagian dengan server asli (kode perangkat).
- Kepatuhan model memanggil tool `question` bergantung pada model; mekanisme diuji e2e dengan engine tiruan dan dicoba di smoke.
- Gambar dalam jawaban tidak dimuat (keputusan keamanan); diagram selain Mermaid (mis. PlantUML) tidak ada.
- Tidak ada pembaruan kuota/limit tarif provider (tidak tersedia dari engine); gateway OpenWork mungkin menyediakan endpoint
  usage, belum dipelajari.

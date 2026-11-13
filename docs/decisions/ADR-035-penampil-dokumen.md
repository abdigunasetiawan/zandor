# ADR-035 — Penampil dokumen Panel 3 (Fase 7)

- **Tanggal:** 14-11-2026
- **Status:** Diterima
- **Terkait:** ADR-026 (spike renderer), ADR-028 (MPL transitif), ADR-032/033 (engine, chat)

## Keputusan

1. **Pembacaan berkas lewat Rust, byte persis** (`fs_read`, `apps/desktop/src-tauri/src/files.rs`), bukan lewat engine.
   Alasan: `GET /file/content` milik engine **memangkas newline akhir** dan memutuskan teks/biner dengan heuristik
   (PDF asli dan berkas ASCII bisa dianggap teks). Di luar Tauri (dev/e2e) dipakai engine sebagai cadangan (lossy,
   dijelaskan di kode). Batas baca 100 MB; teks ditampilkan maks 5 MB (ditandai terpotong).
2. **Keamanan path:** `root` harus terdaftar (project di `projects.json` atau folder Chat); path harus relatif, tanpa `..`,
   dikanonisasi, dan hasilnya harus berada di dalam root (menolak symlink keluar). "Buka di aplikasi lain" memakai
   `rundll32 url.dll,FileProtocolHandler` dengan argumen terpisah (bukan lewat shell) dan **menolak berkas yang dapat
   dieksekusi** (`.exe .bat .ps1 .lnk .msi …`). "Simpan salinan" lewat dialog sistem lalu `fs_copy_to`.
   Terbukti pada aplikasi asli: `..`, path absolut, dan root tak terdaftar ditolak.
3. **Satu `DocStore` (core, murni)**: tab (maks 16, tersimpan dan dipulihkan setelah engine/Tauri siap), jenis dari nama lalu
   isi (NUL → biner), mode render/mentah, bungkus baris, cache isi hanya 4 tab terakhir, penolak respons basi.
4. **Penampil per jenis:** kode = **Monaco 0.57** read-only (hanya fitur inti, cari, lipat, salin; tokenizer per bahasa
   dimuat malas; tanpa layanan bahasa; worker editor lokal); Markdown = `react-markdown` + GFM + **KaTeX** + **Mermaid**
   (`securityLevel: "strict"`); gambar/SVG lewat `<img>` blob (skrip SVG tidak berjalan); PDF = **pdf.js 6.5** (worker,
   render per halaman malas, zoom); CSV/TSV = tabel tervirtualisasi (Papa Parse, maks 200.000 baris); JSON = pohon lipat;
   HTML = `<iframe sandbox="">` (tanpa skrip/formulir/origin); audio/video = elemen media blob; selain itu pesan jelas +
   "buka di aplikasi lain". DOCX/XLSX/PPTX/ipynb **belum** (Fase 11, sesuai ADR-026).
5. **Toolbar:** render/mentah, cari (Monaco), bungkus, salin isi (byte asli, bukan model editor yang menormalkan EOL), salin
   path, simpan salinan, buka di aplikasi lain (nonaktif di luar Tauri, tidak ada tombol palsu), muat ulang, tutup.
6. **Pintu masuk:** klik berkas di tab Files, atau tombol "Buka" pada kartu tool chat (read/edit/write) — path absolut dari
   engine dipetakan ke project terdaftar; di luar project ditolak dengan pesan.
7. **CSP** ditambah `blob:` untuk gambar/media/worker dan `frame-src 'self' about:` (sebelumnya gambar blob diblokir —
   ditemukan oleh smoke native, bukan e2e browser).
8. **Lisensi:** `mermaid` menarik `elkjs` (EPL-2.0 OR GPL-3.0) dan `khroma` (MIT, field lisensi kosong). `khroma` masuk
   `licenses-allow.json`; `elkjs` **tidak dibundel**: Vite meng-alias `elkjs/lib/elk.bundled.js` ke stub, dan `dist`
   diperiksa bebas dari ELK. Konsekuensi: Mermaid 12 menganggap ELK sebagai tata letak bawaan, jadi Zandor **memaksa
   `layout: "dagre"`**; diagram yang meminta ELK menampilkan galat yang jelas.

## "Snapshot" (kriteria keluar)

- **Snapshot ARIA** (Playwright `toMatchAriaSnapshot`, berkas di `e2e/docs.spec.ts-snapshots/`): Markdown dan TSV.
- **Verifikasi piksel/DOM** per format: Monaco (teks + token berwarna), KaTeX (`.katex`), Mermaid (SVG berisi teks
  node), PDF (canvas tidak kosong, 3 halaman, zoom, render malas), PNG/SVG (`naturalWidth`), CSV (DOM < 80 baris untuk
  2.000 baris, baris terakhir terjangkau), JSON (lipat/buka), HTML (sandbox kosong; skrip tidak jalan), audio (durasi),
  video WebM (lebar video).
- Fixture sintetis dibuat di kode (`e2e/fixtures.ts`); satu-satunya biner adalah WebM 0,6 KB (ffmpeg) karena
  `MediaRecorder` tidak menghasilkan data di lingkungan uji.

## Batasan yang diketahui (jujur)

- Belum: edit berkas, cari di PDF/Markdown render (tombol cari hanya untuk tampilan teks), PDF text layer (tidak bisa
  memilih/menyalin teks PDF), highlight blok kode di Markdown, DOCX/XLSX/PPTX/ipynb.
- Fidelitas visual belum dinilai oleh manusia; semua fixture sintetis (ADR-026 risiko #1 tetap berlaku).
- `dist` membengkak (≈17 MB, termasuk KaTeX/Mermaid/Monaco/pdf.js; semua dimuat malas per penampil). Belum dioptimalkan.
- Video besar dibaca penuh ke memori (batas 100 MB); belum streaming.
- Diuji hanya di Windows/WebView2 (ADR-027). Hanya `<input>` browser untuk lampiran di luar Tauri.
- Smoke native menyisipkan project uji sementara ke registry aplikasi dan memulihkannya di akhir (`finally`).

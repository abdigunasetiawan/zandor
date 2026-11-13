# Fitur: Panel 3 — penampil dokumen

- **Status:** selesai untuk lingkup Fase 7 (format "Penuh" PRD) dengan batasan jujur. Keputusan: ADR-035.
- **Kode:** `packages/core/src/docs/` (jenis, dekode teks, path, `DocStore`), `apps/desktop/src/shell/docs/`
  (`DocPanel`, `CodeViewer`, `MarkdownViewer`, `PdfViewer`, `simple-viewers`), `apps/desktop/src-tauri/src/files.rs`.

## Spek

- **Tab dokumen** (maks 16; tersimpan dan dipulihkan), path berkas, dan toolbar: render/mentah, cari, bungkus baris,
  salin isi, salin path, simpan salinan, buka di aplikasi lain, muat ulang, tutup.
- **Format:** kode (Monaco), Markdown (GFM + KaTeX + Mermaid), SVG, gambar (PNG/JPG/GIF/WebP/…), PDF (zoom, render malas),
  CSV/TSV (tabel tervirtualisasi), JSON (pohon), HTML (sandbox), audio, video. Format lain: pesan "tidak punya penampil"
  dan tombol buka di aplikasi lain.
- **Cara membuka:** klik berkas di tab **Files**, atau tombol **Buka** pada kartu tool di chat. Panel Dokumen terbuka
  otomatis bila sedang terlipat.
- **Keamanan:** hanya berkas di dalam project terdaftar atau folder Chat; `..`, path absolut, dan symlink keluar ditolak;
  berkas yang dapat dieksekusi tidak dibuka; HTML dalam sandbox tanpa skrip; Markdown tanpa HTML mentah; Mermaid strict.

## Belum / terbatas

Edit berkas, seleksi/cari teks di PDF, DOCX/XLSX/PPTX/ipynb (Fase 11), pewarnaan blok kode di Markdown, video streaming.

## Cara test

- Otomatis: `bun run test` (jenis, dekode teks, path, `DocStore`), `cargo test -p zandor-desktop files` (keamanan path),
  `bun x playwright test docs` (15 skenario + snapshot ARIA), smoke native (Markdown/kode/CSV/PNG/PDF dari disk dan
  penolakan akses luar project).
- Manual: buka project berisi PDF/Markdown/CSV sungguhan; coba file besar (> 5 MB teks → ada catatan terpotong); coba
  `Ctrl+F` di tampilan teks; klik "Buka di aplikasi lain" pada `.docx`.

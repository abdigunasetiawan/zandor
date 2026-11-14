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

## Fase 11 (ADR-039)

- **Edit berkas di Monaco** (aplikasi desktop): tombol Edit → Ctrl+S menulis atomik ke berkas yang sudah ada; penanda ● pada
  tab yang belum disimpan; konfirmasi saat menutup/memuat ulang/selesai edit dengan perubahan; **deteksi konflik** bila berkas
  diubah program lain (tanya "Timpa?"). Hanya UTF-8 valid (BOM dipertahankan); di build web tombol Edit nonaktif dengan alasan.
- **DOCX, XLSX, PPTX, ipynb** (best-effort, hanya baca; tanpa makro/skrip/HTML mentah). Format lama `.doc/.xls/.ppt` tetap
  "buka di aplikasi lain".
- **Saran prompt dinamis** di composer (lihat `docs/features/chat.md` dan ADR-039).

## Belum / terbatas

Seleksi/cari teks di PDF, pewarnaan blok kode di Markdown, video streaming; DOCX/XLSX/PPTX tidak bisa diedit dan hanya
menampilkan isi/struktur (PPTX tanpa tata letak/animasi/catatan; XLSX tanpa grafik/gambar/format angka kustom selain tanggal);
notebook: output HTML/JS tidak ditampilkan; edit hanya UTF-8; fidelitas visual terhadap Office asli belum diukur manusia.

## Cara test

- Otomatis: `bun run test` (jenis, dekode teks, path, `DocStore`), `cargo test -p zandor-desktop files` (keamanan path),
  `bun x playwright test docs office` (skenario dokumen + Office), smoke native (Markdown/kode/CSV/PNG/PDF, **edit + konflik
  nyata**, DOCX/XLSX/PPTX/ipynb asli dari disk, penolakan akses luar project). Fixture Office dibuat
  `scripts/make-office-fixtures.py` (python-docx/openpyxl/python-pptx).
- Manual: buka project berisi PDF/Markdown/CSV sungguhan; coba file besar (> 5 MB teks → ada catatan terpotong); coba
  `Ctrl+F` di tampilan teks; klik "Buka di aplikasi lain" pada `.docx`.

## Tab, "Buka dengan", HTML sumber luar (ADR-048)

Tab dokumen berikon jenis berkas, tanpa scrollbar, dengan menu klik kanan (tutup / lainnya / kanan / kiri / semua; perubahan belum tersimpan dikonfirmasi sekali). Tombol **Buka di aplikasi lain** membuka popover: aplikasi terdaftar untuk ekstensi (ikon asli, lencana Bawaan), Aplikasi bawaan sistem, Tampilkan di File Explorer; berkas teks juga menawarkan VS Code/Notepad bila terpasang. Pratinjau HTML default memblokir skrip dan sumber luar; saklar **Izinkan skrip dan sumber luar** (hanya desktop) memuatnya lewat skema `zpreview://` di iframe sandbox tanpa same-origin. Tes: `e2e/doc-tabs.spec.ts`, `docs/OpenWithMenu.test.tsx`, `openwith::tests`. Belum diuji di Tauri sungguhan: daftar aplikasi, peluncuran, `zpreview`.

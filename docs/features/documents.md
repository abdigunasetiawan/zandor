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

Tab dokumen berikon jenis berkas, tanpa scrollbar, dengan menu klik kanan (tutup / lainnya / kanan / kiri / semua; perubahan belum tersimpan dikonfirmasi sekali). Tombol **Buka di aplikasi lain** membuka popover: aplikasi terdaftar untuk ekstensi (ikon asli, lencana Bawaan), Aplikasi bawaan sistem, Tampilkan di File Explorer; berkas teks juga menawarkan VS Code/Notepad bila terpasang. Pratinjau HTML di Panel 3 selalu sandbox penuh (tanpa skrip, formulir, sumber luar) tanpa bilah opsi. Tombol **Buka di browser** (ikon globe di toolbar, hanya desktop) menjalankan **live server** lokal (`liveserver.rs`: 127.0.0.1 port acak, satu per folder project, akses bertoken + cookie `SameSite=Strict`, path dikunci di dalam folder) dan membuka berkas di Browser Panel 4: aset relatif/absolut-root dan skrip berfungsi, halaman dimuat ulang otomatis saat berkas di folder berubah (polling ~0,8 dtk; mengabaikan `.git`, `node_modules`, `target`, `dist`, `.next`). Server hidup sampai aplikasi ditutup. Tes: `e2e/doc-tabs.spec.ts`, `docs/OpenWithMenu.test.tsx`, `openwith::tests`. Live server: `liveserver::tests` (path traversal, token/403, injeksi skrip). Belum diuji di Tauri sungguhan: daftar aplikasi, peluncuran, live server di Browser Panel 4.

## Kustomisasi tampilan Panel 3

Tombol **Kustomisasi** (ikon slider) di bilah atas Panel 3, di kiri tombol tutup, membuka popover `DocStyleControls`
yang hanya menampilkan opsi untuk jenis dokumen aktif (`groupsForViewer`):

| Kelompok             | Penampil                             | Opsi                                                                                              |
| -------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------- |
| Kode dan teks mentah | kode, mode mentah, notebook          | tema editor, font kode, ukuran (px), tinggi baris, nomor baris, minimap, ligatur, tampilkan spasi |
| Dokumen              | Markdown, Word, PowerPoint, notebook | tema baca (Ikut aplikasi/Kertas/Sepia/Malam), font, ukuran, tinggi baris, lebar kolom baca        |
| Tabel dan data       | CSV/TSV, Excel, JSON                 | ukuran huruf tabel                                                                                |
| Gambar               | gambar, SVG                          | latar (kotak-kotak/terang/gelap)                                                                  |

Semua kelompok juga tersedia di **Pengaturan → Dokumen** (`#/settings/documents`). Nilai tersimpan di `appearance.docs`
(AppearanceStore, divalidasi) dan diterapkan lewat token `--z-doc-*` serta opsi Monaco.

**Zoom teks mentah:** Ctrl + roda mouse di atas editor kode/mentah mengubah ukuran huruf editor (8–40px) dan menyimpannya;
skala UI tidak berubah. Tes: `e2e/docs.spec.ts` ("kustomisasi Panel 3"), `packages/core/src/theme/theme.test.ts`.

## Edit dokumen oleh agent (ADR-070)

Agent memakai **skill bawaan** `zandor-docx`, `zandor-xlsx`, `zandor-pptx`, `zandor-pdf` (sumber:
`apps/desktop/src-tauri/bundled-skills/`, dipasang oleh `bundled_skills.rs` ke folder skill global engine setiap engine
dimulai). Alurnya: baca struktur → buat cadangan → ubah dengan python-docx/openpyxl/python-pptx/pypdf → verifikasi dengan
`office_check.py` (`--expect "teks"`) → laporkan hasil beserta batasannya.

- **Syarat:** Python 3 terpasang. Pustaka dipasang `pip --user` setelah user memberi izin.
- **Batasan:** tampilan visual dan hitung ulang rumus hanya diperiksa bila LibreOffice tersedia. PDF tidak diedit teksnya
  di tempat. Panel 3 belum memuat ulang otomatis setelah berkas Office diubah.
- **Tes:** `cargo test -p zandor-desktop bundled_skills` (nama/frontmatter, instalasi idempoten, pemulihan berkas rusak),
  `python -m unittest discover -s apps/desktop/src-tauri/bundled-skills` (alur edit DOCX, rumus XLSX, slide/catatan PPTX,
  PDF utuh/terpotong, ZIP rusak, relasi rusak). Belum diuji: model nyata memilih dan mengikuti skill di aplikasi native.

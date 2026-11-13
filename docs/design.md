# Preferensi Visual — Zandor

> Ringkasan arah visual. Token lengkap dan halaman design system dibuat di Fase 2.

- **Tema default monokrom** tanpa warna primary. Dark: `#121212` (latar), `#141414` (permukaan),
  `#1f1f1f` (permukaan menonjol). Light: padanan monokrom.
- **Font:** Plus Jakarta Sans untuk UI (OFL); font kode dan terminal bisa diatur.
- **Ketebalan & ukuran per variabel** (h1–h4, p, link, tombol, code), semuanya lewat token.
- **Zoom UI** lewat token `--ui-scale` (75–150%), bukan zoom webview. Semua ukuran memakai `rem`
  (ADR-014, ADR-025). Konten browser bawaan tidak ikut ter-zoom.
- **Layout 5 panel** (ADR-015, ADR-025): P1 sidebar, P2 chat (tidak bisa dilipat), P3 dokumen,
  P4 tools, P5 rail. Lebar minimum dalam `rem`. Di layar 1366 px dengan zoom > 112,5%, minimal satu
  panel terlipat otomatis.
- **Overlay** (menu, popover, modal) yang menimpa Panel 4 harus menyembunyikan browser bawaan (ADR-025).
- **Tidak ada dialog bawaan OS** (`alert`/`confirm`); semua lewat komponen.
- **Aksesibilitas dasar:** kontras, fokus terlihat, label pada kontrol.

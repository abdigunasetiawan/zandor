# ADR-059 — Semua pengaturan dalam satu halaman Pengaturan

Tanggal: 15-11-2026 · Status: diterima

## Keputusan

- Tampilan lanjutan, Memori dan skill, dan Automasi bukan lagi halaman terpisah yang dibuka lewat kartu "Lainnya".
  Tampilan lanjutan digabung ke dalam kategori Tampilan (di bawah baris pengaturannya); Memori dan skill serta Automasi
  menjadi kategori sendiri (topiknya memang berbeda). Semuanya dirender tertanam (`<X embedded />`) lewat
  `PageFrame` (`<main>` + tombol kembali bila berdiri sendiri; bagian dengan h2 bila tertanam).
- Rute lama dipertahankan sebagai alias supaya menu, palet perintah, tombol Automasi di sidebar, dan tautan lama tetap
  berfungsi: `/memory`, `/appearance` (kategori Tampilan), `/automation` → `/settings` dengan kategori yang sesuai.
- Kategori "Lainnya" dihapus; pencarian pengaturan ikut mencocokkan judulnya.

## Konsekuensi

- Satu tempat untuk semua yang bisa diatur; tidak ada halaman pengaturan yatim.
- Ikon baru `PaletteIcon`; tes e2e daftar kategori diperbarui (nama tombol `exact`).

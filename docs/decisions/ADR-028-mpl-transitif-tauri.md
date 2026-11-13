# ADR-028 — MPL-2.0 dari dependensi transitif Tauri dikecualikan per crate

- **Tanggal:** 13-11-2026
- **Status:** Diterima (konsekuensi langsung ADR-002) · **perlu ditinjau pemilik**
- **Terkait:** PRD §4.8 (lisensi bersih), ADR-002 (Tauri), ADR-009, ADR-026 (PPTX/MPL)

## Konteks

Audit lisensi Rust (`cargo deny check licenses`) di Fase 1 menemukan bahwa **Tauri 2.12.5 sendiri**
membawa crate berlisensi **MPL-2.0** sebagai dependensi transitif:
`cssparser`, `cssparser-macros`, `selectors` (dari proyek Servo, dipakai pemrosesan HTML/CSP Tauri),
`dtoa-short`, dan `option-ext` (lewat `dirs`).

Artinya aturan PRD §4.8 "hanya lisensi permisif" sudah tidak bisa dipenuhi 100% sejak Tauri dipilih
(ADR-002). Mengganti Tauri bukan opsi realistis: hasil Fase 0 (S1, S3, S7, S8) bergantung padanya.

## Keputusan

1. MPL-2.0 **diizinkan hanya sebagai pengecualian per crate** di `deny.toml`, untuk lima crate di atas,
   dengan syarat:
   - file berlisensi MPL **tidak dimodifikasi**;
   - dicantumkan di Third-Party Notices beserta tautan ke sumbernya (Fase 12).
2. **Tidak ada izin MPL-2.0 global.** Crate MPL baru (termasuk yang masuk lewat upgrade Tauri) membuat CI
   gagal dan wajib dibahas lewat ADR baru.
3. Kasus PPTX (`mtx-decompressor`, ADR-026) **tidak** otomatis tercakup. Itu pilihan fitur, bukan bawaan
   framework, dan tetap menunggu keputusan pemilik sebelum Fase 11.

## Catatan

MPL-2.0 adalah copyleft lemah per file: produk tertutup/komersial boleh memakainya tanpa membuka kode
sendiri, selama file MPL yang dipakai tetap tersedia sumbernya. Ini bukan nasihat hukum. Pemilik
sebaiknya meninjaunya sebelum rilis berbayar.

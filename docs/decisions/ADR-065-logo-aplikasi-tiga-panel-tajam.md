# ADR-065 — Logo aplikasi: "Tiga Panel — Tajam"

Tanggal: 16-11-2026 · Status: diterima

## Konteks

Zandor belum punya logo. Header sidebar menampilkan huruf "Z" berupa teks di ubin aksen, favicon kosong (`data:,`), dan
ikon aplikasi Tauri masih ikon bawaan sejak Fase 1. Pemilik meninjau 10 konsep logo, memilih konsep 02 "Tiga Panel" (huruf
Z dari tiga bagian terpisah, merujuk panel Chat, Dokumen, dan Tools), lalu dari 30 variasinya memilih varian **Tajam**:
tanpa radius, ujung diagonal sejajar tepi balok.

## Keputusan

- **Bentuk** (viewBox 64×64): balok atas `rect 10,10 44×10`, diagonal `polygon 40,23 54,23 24,41 10,41`, balok bawah
  `rect 10,44 44×10`. Monokrom, sesuai aturan tema bawaan tanpa warna aksen.
- **Atom `AppLogo`** (`packages/ui/src/atoms/AppLogo.tsx`): SVG `currentColor`, ukuran `sm|md|lg` lewat token
  `--z-icon-*`; tanpa `label` dianggap dekoratif (`aria-hidden`), dengan `label` menjadi `role="img"` bernama. Terdaftar di
  katalog, demo design system, dan bertes.
- **Sidebar**: ubin `.sidebar__logo` (latar `accent`, warna `on-accent`) kini berisi `AppLogo size="lg"`, bukan huruf teks.
- **Favicon**: `apps/desktop/public/favicon.svg`, mengikuti `prefers-color-scheme` (gelap di tema terang, terang di tema gelap).
- **Ikon aplikasi**: sumber `apps/desktop/src-tauri/icons/app-icon.svg` (ubin #161616 radius 224/1024, Z #ececec). Berkas
  yang dipakai `tauri.conf.json` (`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.ico`, `icon.png`) dihasilkan dengan:

  ```bash
  cd apps/desktop && bun x tauri icon src-tauri/icons/app-icon.svg -o <folder-sementara>
  ```

  lalu hanya lima berkas tersebut yang disalin ke `src-tauri/icons/` (aset Android/iOS/macOS tidak dipakai).

## Akibat

- Satu bentuk dipakai di tiga tempat; bila logo berubah, ubah `AppLogo.tsx`, `favicon.svg`, dan `app-icon.svg` bersamaan,
  lalu regenerasi ikon.
- Nilai warna mentah hanya ada di dua berkas SVG aset (di luar cakupan `design-tokens`), bukan di kode UI.
- Ikon jendela/taskbar baru terlihat setelah build Tauri berikutnya; belum diperiksa pada build native.

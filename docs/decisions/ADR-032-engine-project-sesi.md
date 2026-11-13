# ADR-032 — Engine produksi, project, dan sesi (Fase 4)

- **Tanggal:** 13-11-2026
- **Status:** Diterima
- **Terkait:** ADR-019 (sidecar), ADR-021 (engine pin), ADR-022 (env guard), ADR-023 (plugin), ADR-031 (ikon)

## Keputusan

1. **Supervisor produksi** = crate `zandor-engine-supervisor` (hasil spike S1) + `engine.rs` di app: thread
   khusus, event `engine-state` ke UI, restart otomatis maksimal 3 kali per 60 detik dengan backoff, dan
   mati bersama aplikasi (Job Object di Windows → tidak ada proses yatim).
2. **Engine di-pin dan diverifikasi:** `engine.json` memuat versi + sha256 paket npm per target.
   `scripts/prepare-engine.ts --fetch` mengunduh, memverifikasi hash dan versi, lalu menaruh sidecar. Binary tidak
   di-commit.
3. **Isolasi XDG:** engine dijalankan dengan `XDG_{DATA,CONFIG,CACHE,STATE}_HOME` di `app_data/engine`, sehingga
   tidak mewarisi konfigurasi/auth opencode milik pengguna dan datanya terpisah dari instalasi opencode lain.
4. **Plugin guard wajib dan diverifikasi:** konfigurasi (`OPENCODE_CONFIG_CONTENT`) hanya memuat plugin Zandor
   dengan opsi `markerFile`; plugin menulis penanda saat termuat. Supervisor menghapus penanda sebelum start,
   **memancing instance** (opencode memuat plugin per-instance saat request berdirektori pertama, bukan saat
   server start), lalu menunggu penanda. Tanpa penanda engine dihentikan dan status = error (ADR-022).
5. **Project = folder, daftar dimiliki Zandor** (`projects.json` di `app_data`, penulisan atomik tmp→rename,
   maksimal 5 MB, versi 1). Alasan: opencode hanya mengenal repositori git dan menggabungkan semua folder
   non-git ke satu project "global", sehingga tidak bisa dipakai sebagai daftar project buatan pengguna.
   Hapus project = lepas dari daftar; folder dan sesinya di engine tidak dihapus.
6. **Sesi dimiliki engine**, terikat ke direktori (`GET /session?directory=`). "Chat" (sesi tanpa project) memakai
   direktori khusus `app_data/chats`; folder itu tidak bisa ditambahkan sebagai project.
7. **Klien engine = SDK resmi** `@opencode-ai/sdk@1.19.5` (client v2) dibungkus antarmuka `EngineApi`; test memakai
   engine palsu (`createFakeEngine`). Pohon file memakai `GET /file?path=` dan menghormati `ignored`.
8. **Connection manager:** `ConnectionStore` menyimpan status engine; sumber saat ini hanya `local`. Struktur
   `source: "local" | "remote"` sudah ada; remote URL sungguhan menyusul di Fase 11.
9. **Ikon file:** Material Icon Theme dimuat malas (manifest + SVG per ikon), dengan ikon Lucide sebagai cadangan.
   Pemilihan ikon adalah fungsi murni di `@zandor/core` (`resolveIconName`).

## Temuan selama implementasi (penting untuk fase berikutnya)

- **Plugin dimuat per-instance**, bukan saat start (lihat butir 4).
- **`resource_dir()` Tauri di Windows berawalan verbatim `\\?\`.** Jika dimasukkan apa adanya ke URL `file://`,
  opencode melewati plugin **secara diam-diam** (guard tidak aktif). `file_url` membuang awalan itu dan diuji.
- **Direktori kerja dan data XDG harus ada sebelum spawn** (`os error 267`); dibuat di `EngineManager::start`.
- **Tes paralel**: dua engine berbagi data XDG → "database is locked"; tiap engine tes diberi data sementara unik.

## Risiko dan batasan yang diketahui

- **Dua instance Zandor** memakai data engine yang sama → "database is locked". Belum ditangani; kandidat:
  `tauri-plugin-single-instance`.
- **Kustomisasi warna project**: model data dan validasi (`PROJECT_COLORS`) sudah ada, tetapi **belum ada UI**
  karena token warna kategori belum ada di design system. Rename project sudah berfungsi.
- **Daftar project hanya di perangkat ini** (bukan di engine); sinkronisasi lintas perangkat/remote belum ada.
- **Ukuran**: semua SVG Material Icon Theme ikut dibuild (~3,7 MB) walau dimuat malas; dapat dipangkas nanti.
- **Hanya Windows** yang diuji (ADR-027).

## Verifikasi

Tes unit core (fake engine), e2e Playwright dengan engine tiruan (`apps/desktop/e2e/projects.spec.ts`), tes
integrasi supervisor terhadap opencode asli, dan smoke native (`apps/desktop/e2e-native/smoke.ts`): engine siap
dengan guard, buat sesi, **restart aplikasi → sesi tetap ada**, lalu dihapus.

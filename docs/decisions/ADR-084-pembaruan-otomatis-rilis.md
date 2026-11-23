# ADR-084 — Pembaruan otomatis dan rilis beta lewat GitHub Releases

Tanggal: 23-11-2026 · Status: diterima (tes unit lulus; alur installer + pembaruan diuji lokal, lihat
`docs/features/release.md`) · Menambah: ADR-078 (GPUI)

## Konteks

Zandor GPUI belum punya cara distribusi. Pemilik ingin:

- satu saluran **beta**;
- aplikasi yang **memperbarui dirinya sendiri** tanpa langkah manual;
- folder data bernama **`Zandor`**, bukan identifier Tauri `id.web.zandor.app`.

Repo `abdigunasetiawan/zandor` kini publik, jadi aset GitHub Releases bisa diunduh tanpa token.

## Keputusan

1. **Hosting:** GitHub Releases. Tiap rilis bertag `vX.Y.Z` dan berisi dua aset:
   - `Zandor-Setup-X.Y.Z.exe`;
   - `latest.json`.

   App membaca `releases/latest/download/latest.json`. Rilis dibuat **bukan** sebagai prerelease, agar `/latest/`
   menunjuk ke sana. Status beta hanya ditulis di judul rilis.
2. **Manifes bertanda tangan.** Isi `latest.json`: `version`, `notes`, `pub_date`, `url`, `sha256`, `size`,
   `signature`.
   - Tanda tangannya Ed25519 (hex) atas `"zandor-update\n{version}\n{sha256}\n{size}"`.
   - Kunci publik tertanam di `src/core/update_key.pub` dan diverifikasi dengan `ring`.
   - Kunci privat hanya ada di `%USERPROFILE%\.zandor\update-signing-key.pem`, di luar repo.
   - Unduhan diverifikasi ulang terhadap SHA-256 dan ukuran. Manifes atau aset yang ditukar akan ditolak.
3. **Alur di app** (`core/update.rs`):
   - **Cek:** 30 dtk setelah start, lalu tiap 6 jam, atau lewat tombol "Periksa sekarang".
   - **Unduh:** di thread latar ke `%LOCALAPPDATA%\Zandor\updates\`.
   - **Siap:** status menjadi Siap, `pending.json` ditulis, dan muncul toast sekali dengan tombol "Mulai ulang sekarang".
   - **Pasang:** saat Zandor ditutup, atau segera bila pengguna memilih "Mulai ulang sekarang".
     - Skrip `apply.ps1` berjalan terlepas: menunggu PID keluar, menghentikan `opencode.exe` milik folder instalasi,
       lalu menjalankan `installer /S /UPDATE /D=<folder instalasi>`.
     - Pada mode mulai ulang, Zandor dibuka lagi setelah terpasang.
     - `pending.json` dihapus sebelum skrip berjalan, jadi pemasangan yang gagal tidak diulang terus-menerus.
   - **Pemulihan:** bila Zandor dihentikan paksa sebelum sempat memasang, pembaruan tertunda yang masih sah (versi
     lebih baru, SHA cocok) dipasang saat start berikutnya, sebelum jendela dibuka.
4. **Updater hanya aktif pada build rilis yang dipasang.** Dua syarat:
   - `ZANDOR_RELEASE_VERSION` di-set saat build (oleh skrip rilis); nilainya juga menjadi versi yang tampil;
   - `uninstall.exe` ada di samping exe.

   Bila salah satu tidak terpenuhi, Pengaturan > Umum > Pembaruan menyatakan dengan jujur bahwa pembaruan otomatis
   tidak aktif, beserta sebabnya. Build dev dan `target/release` tidak pernah memperbarui dirinya.
5. **Installer NSIS** (`apps/desktop-gpui/installer/zandor.nsi`):
   - per pengguna, tanpa UAC, di `%LOCALAPPDATA%\Programs\Zandor`;
   - berisi `Zandor.exe`, `opencode.exe`, dan `resources\zandor-plugin.js`; aset lain sudah tertanam di exe;
   - membuat shortcut Start Menu (dan desktop hanya pada pemasangan pertama);
   - mendaftarkan uninstaller di HKCU;
   - tidak menyentuh data pengguna.
6. **Folder data:**
   - Folder data menjadi `%APPDATA%\Zandor`; build debug memakai `%APPDATA%\ZandorDev`.
   - Saat start pertama, folder lama (`id.web.zandor.app` / `.dev`) di-*rename* ke nama baru, lalu sebuah junction
     dipasang di jalur lama. Junction diperlukan karena engine menyimpan direktori kerja sesi Chat sebagai jalur
     absolut (`…\id.web.zandor.app\chats\chat-…`).
   - Daftar Chat juga menanyakan jalur lama (`ProjectHost.legacy_chats`), jadi sesi lama tetap tampil.
   - Bila rename gagal (mis. Zandor Tauri masih berjalan), sesi itu tetap memakai folder lama dan migrasi dicoba lagi
     pada start berikutnya.
7. **Skrip rilis:** `bun scripts/rilis.ts X.Y.Z [--notes …] [--dry-run] [--fast]`. Urutannya:
   - memeriksa tree bersih, tag belum ada, dan HEAD sudah di GitHub;
   - build profil `dist` dengan versi rilis;
   - menyusun berkas, menjalankan makensis, menghitung SHA-256 dan menandatangani;
   - menulis `latest.json`;
   - menjalankan `gh release create`.

   Pembuatan kunci: `--keygen` (sekali; menolak menimpa tanpa `--force`). Branch kerja rilis: `zandor/release`.

## Akibat

- **Kunci privat wajib dicadangkan.** Bila hilang, instalasi yang ada tidak bisa menerima pembaruan lagi; pengguna
  harus memasang ulang installer yang ditandatangani kunci baru.
- **Installer belum ditandatangani Authenticode,** jadi SmartScreen memperingatkan saat pemasangan pertama.
  Pembaruan otomatis tidak terkena peringatan itu karena dijalankan oleh Zandor sendiri.
- **Rilis dibangun di mesin pemilik,** bukan di CI. Alasannya, kunci privat tidak disimpan di GitHub. Bila nanti
  diinginkan, kunci bisa dipindah ke secret Actions.
- **Junction di jalur lama bergantung pada `mklink /J`.** Bila gagal, folder baru tetap dipakai tetapi sesi Chat lama
  bisa tidak terbuka. Hal ini tercatat di stderr.

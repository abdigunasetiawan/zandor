# Rilis dan pembaruan otomatis (GPUI)

Keputusan desain: [ADR-084](../decisions/ADR-084-pembaruan-otomatis-rilis.md). Branch kerja rilis: `zandor/release`.

## Untuk pengguna

- **Pemasangan pertama:** unduh `Zandor-Setup-X.Y.Z.exe` dari
  [GitHub Releases](https://github.com/abdigunasetiawan/zandor/releases) dan jalankan.
  - Tidak perlu hak admin.
  - Lokasi pemasangan: `%LOCALAPPDATA%\Programs\Zandor`.
  - Windows SmartScreen bisa memperingatkan karena installer belum ditandatangani. Pilih "More info" → "Run anyway".
  - Pemasangan manual menampilkan halaman **Ketentuan Penggunaan (beta)** (`installer/terms-id.txt`/`terms-en.txt`,
    UTF-8 ber-BOM) dengan kotak persetujuan; Berikutnya aktif setelah dicentang. Pembaruan otomatis (`/S /UPDATE`)
    melewati halaman ini.
- **Pembaruan:** berjalan otomatis.
  - Zandor memeriksa 30 detik setelah dibuka, lalu tiap 6 jam.
  - Bila ada versi baru, Zandor mengunduh dan memverifikasinya di latar, lalu memasangnya saat Zandor ditutup.
  - "Mulai ulang sekarang" (di toast atau Pengaturan > Umum > Pembaruan) memasangnya segera.
- **Data:** disimpan di `%APPDATA%\Zandor` dan tidak dihapus oleh uninstaller.

## Merilis versi baru

Prasyarat:

- Rust;
- NSIS (`makensis`);
- GitHub CLI (`gh auth login`);
- engine sudah disiapkan (`bun run prepare:engine`);
- kunci penanda tangan ada di `%USERPROFILE%\.zandor\update-signing-key.pem`.

Langkah:

1. Commit semua perubahan, lalu push branch.
2. Uji tanpa mengunggah:

   ```powershell
   bun scripts/rilis.ts 0.1.0 --dry-run
   ```

   Hasilnya ada di `dist/rilis/`: installer, `latest.json`, dan `stage/`.
3. Rilis:

   ```powershell
   bun scripts/rilis.ts 0.1.0 --notes "Ringkasan perubahan"
   ```

   Skrip ini melakukan build profil `dist`, membuat installer, menandatangani, menulis `latest.json`, lalu menjalankan
   `gh release create v0.1.0`.

Versi harus naik (semver). App hanya memasang versi yang **lebih baru** dari versinya sendiri.

## Kunci penanda tangan

- Dibuat sekali dengan `bun scripts/rilis.ts --keygen`.
  - Kunci publik masuk repo: `apps/desktop-gpui/src/core/update_key.pub`.
  - Kunci privat tetap di luar repo.
- **Cadangkan `%USERPROFILE%\.zandor\update-signing-key.pem`,** misalnya di pengelola sandi atau penyimpanan
  terenkripsi. Tanpa kunci yang sama, Zandor yang sudah terpasang menolak semua rilis berikutnya.

## Cara menguji

- **Tes unit:** `cargo test --offline update::` dan `services::tests`. Keduanya mencakup:
  - tanda tangan sah dan tanda tangan yang diubah;
  - semver;
  - pembaruan tertunda yang berkasnya berubah;
  - migrasi folder data.
- **Installer:** jalankan `--dry-run --fast`, pasang `dist/rilis/Zandor-Setup-*.exe`, lalu buka Pengaturan > Umum >
  Pembaruan. Status yang benar: "Otomatis", lalu "Terbaru" atau sebuah galat yang jelas bila belum ada rilis.
- **Alur pembaruan menyeluruh tanpa GitHub (sudah diuji 23-11-2026, lulus):**
  1. Buat installer 0.1.0 dan 0.1.1 dengan `--dry-run --fast`.
  2. Pasang 0.1.0 dengan `/S /UPDATE /D=<folder uji>`.
  3. Sajikan installer 0.1.1 dan `latest.json` (dengan `url` diganti ke localhost) lewat `python -m http.server`.
  4. Jalankan Zandor dengan `ZANDOR_UPDATE_URL=http://127.0.0.1:<port>/latest.json`. Bila ingin data terpisah, set juga
     `APPDATA` dan `LOCALAPPDATA` ke folder uji.
  5. Tunggu toast "siap dipasang", lalu tutup Zandor. `Zandor.exe` harus berganti ke 0.1.1.

  Override `ZANDOR_UPDATE_URL` tetap mewajibkan tanda tangan kunci yang tertanam. Log penerap ditulis di
  `%LOCALAPPDATA%\Zandor\updates\apply.log`.

## Status

- Selesai:
  - modul pembaruan;
  - UI Pengaturan;
  - installer NSIS;
  - skrip rilis;
  - migrasi folder data.
- Belum:
  - tanda tangan Authenticode;
  - rilis lewat CI;
  - catatan rilis tampil di app (saat ini hanya teks `notes` di Pengaturan).

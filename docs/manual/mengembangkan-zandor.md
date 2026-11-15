# Mengembangkan Zandor memakai Zandor

Pakai **dua salinan**: satu stabil untuk bekerja, satu dev untuk menguji perubahan (ADR-061).

## 1. Pasang atau perbarui salinan stabil (sekali klik)

Klik dua kali **`Perbarui-Zandor.cmd`** di akar repo (atau `bun run install:stable`). Urutannya: bangun release → tutup
salinan stabil yang berjalan (hanya yang di `%LOCALAPPDATA%\ZandorStable`) → cadangkan versi lama ke `ZandorStable.bak`
(satu generasi) → salin → buka. Opsi: `--dest <folder>`, `--profile debug`, `--no-build`, `--no-launch`.
Kembali ke versi lama: tutup Zandor, hapus `ZandorStable`, ganti nama `ZandorStable.bak` menjadi `ZandorStable`.
Prod yang berjalan dari `target\debug` tidak ditutup otomatis (dan mengunci build debug); pindahkan ke `ZandorStable`.
Buka project Zandor dari sini dan minta agent mengubah kode.

## 2. Jalankan salinan dev untuk menguji

```bash
bun run --filter @zandor/desktop tauri:dev          # hot reload
bun run --filter @zandor/desktop tauri:build:dev    # exe debug terpaket di target/debug
```

Salinan dev bernama "Zandor Dev" dengan identifier `id.web.zandor.dev`: folder data (`%APPDATA%\id.web.zandor.dev`) dan
single-instance terpisah, jadi keduanya bisa terbuka bersamaan. Provider dan project harus ditambahkan sendiri di sana
(kunci API di keychain dipakai bersama).

## 3. Smoke native

```bash
bun run --filter @zandor/desktop e2e:native ../../target/debug/zandor-desktop.exe
```

Bawaannya memakai data salinan dev (terisolasi). Hanya proses yang diluncurkan smoke yang dimatikan. Untuk menguji data nyata:
`ZANDOR_SMOKE_APP_ID=id.web.zandor.app` (jangan jalankan saat salinan stabil terbuka).

## Hal yang tetap perlu diperhatikan

- Hot reload/`tauri:dev` hanya memuat ulang salinan dev, bukan salinan stabil tempat agent berjalan.
- Build Rust besar (`target/` bisa belasan GB): pastikan disk cukup; `cargo clean` bila perlu.
- Jangan jalankan salinan stabil dari `target/`; exe di sana terkunci saat dibangun ulang.
- **Run pertama salinan dev lebih lama:** di folder data baru, opencode memasang dependensinya lewat jaringan (belasan detik). Batas tunggu plugin guard dinaikkan dari 8 ke 90 detik karena ini (sebelumnya engine dimatikan "demi keamanan" di run pertama).
- **Build dev saat salinan lain terkunci:** bila `target/debug/zandor-desktop.exe` sedang berjalan, bangun dev ke folder lain: `CARGO_TARGET_DIR=<folder> bun run --filter @zandor/desktop tauri:build:dev`.

## Debug render: Inspect Element dan salin HTML Panel Chat

- **Inspect Element** aktif juga di salinan stabil: tekan **F12**, klik kanan → _Inspect_, atau menu **Dev Tools → Buka Inspect Element**.
- **Salin HTML Panel Chat** (menu **Dev Tools**, atau palet perintah): menyalin satu dokumen HTML mandiri ke clipboard. Isinya DOM
  Panel Chat saat ini, semua CSS, tema/skala, info build, dan **data pesan mentah** sesi aktif (termasuk langkah proses agent yang
  terlipat). Simpan sebagai `.html` untuk dibuka di browser, atau tempelkan ke percakapan dengan agent.
- Dump memuat isi percakapan Anda. Periksa dulu sebelum membagikannya.

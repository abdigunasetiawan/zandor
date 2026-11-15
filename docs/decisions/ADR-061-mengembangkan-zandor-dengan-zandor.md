# ADR-061 — Mengembangkan Zandor memakai Zandor (dua salinan terpisah)

Tanggal: 15-11-2026 · Status: diterima

## Masalah

Satu salinan yang dipakai sekaligus untuk mengembangkan dan menguji bentrok: single-instance membuat build uji langsung
keluar, data `%APPDATA%\id.web.zandor.app` dipakai bersama (smoke menimpa registry dan menghapus sesi), `taskkill /IM`
di smoke mematikan Zandor milik pengguna, exe di `target/` terkunci saat dibangun ulang, dan hot reload memutus sesi agent.

## Keputusan

- **Salinan stabil** (identifier `id.web.zandor.app`) dipasang di luar repo: `bun run install:stable` (build release lalu
  salin `zandor-desktop.exe`, `opencode.exe`, `resources/` ke `%LOCALAPPDATA%\ZandorStable`). Dipakai untuk bekerja.
- **Salinan dev** memakai `src-tauri/tauri.dev.conf.json` (identifier `id.web.zandor.dev`, nama "Zandor Dev"): folder data,
  webview data, dan kunci single-instance terpisah. `bun run --filter @zandor/desktop tauri:dev` / `tauri:build:dev`.
- **Smoke native** memakai identifier dev (bawaan) sehingga tidak menyentuh data nyata; `ZANDOR_SMOKE_APP_ID` mengubahnya.
  Akhir smoke hanya mematikan proses (dan anaknya) yang ia luncurkan sendiri.
- Kunci provider di keychain (`SERVICE=id.web.zandor`) tetap dipakai bersama; port engine `--port 0` (dipilih OS), jadi
  dua salinan tidak berebut port.

## Konsekuensi

- Salinan stabil tidak ikut berubah saat Anda membangun ulang; perbarui dengan `install:stable` bila ingin versi baru.
- Provider/proyek di salinan dev harus ditambahkan sendiri (datanya terpisah).

## Temuan saat verifikasi (15-11-2026)

- Dua salinan (identifier `.app` dan `.dev`) berjalan bersamaan dengan engine dan data terpisah; smoke terisolasi 50/50 dan tidak menyentuh proses Zandor lain.
- Run pertama di folder data baru gagal: dependensi opencode belum terpasang sehingga plugin guard tak termuat dalam 8 detik. `GUARD_WAIT` dinaikkan ke 90 detik (polling berhenti begitu penanda muncul).

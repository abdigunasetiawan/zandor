# Fitur: Panel 4 — Tools

- **Status:** selesai untuk lingkup Fase 8 dengan batasan jujur. Keputusan: ADR-036.
- **Kode:** `packages/core/src/{terminal,tools}/`, `apps/desktop/src/shell/tools/`, `apps/desktop/src-tauri/src/{git,browser}.rs`.

## Spek

- **Terminal:** xterm.js terhubung ke PTY engine; beberapa tab per project; cwd = folder project (sesi tanpa project =
  folder Chat); pilih shell; tab dipulihkan setelah muat ulang dengan riwayat; sambung ulang otomatis; penanda shell selesai.
- **Git:** cabang + tracking, perubahan staged/unstaged/konflik, diff berwarna, stage/unstage per berkas atau semua, commit,
  buat/pindah cabang, commit terbaru. Hanya di aplikasi desktop (git CLI); di luar itu ada pesan jelas.
- **Browser:** child webview native (default tanpa halaman; ketik alamat), mundur/maju/muat ulang, buka di browser sistem.
  Hanya http/https; konten remote tidak punya akses IPC. Disembunyikan saat overlay UI terbuka.
- **Info sesi:** project, direktori, model, jumlah pesan, konteks terpakai/batas, token (masuk/keluar/reasoning), biaya bila
  dilaporkan, panggilan tool per jenis, waktu kerja, ID sesi.
- **Log aktivitas:** tool call, izin, retry, galat, status engine, warn/error aplikasi; filter sumber; bersihkan.

## Belum / terbatas

Tab di browser, discard/stash/push/pull di Git, Git Bash/WSL di terminal belum diuji, airspace belum diuji otomatis,
login Google di browser belum diuji ulang.

## Cara test

- Otomatis: `cargo test -p zandor-desktop git::` dan `browser::`, `bun run test` (terminal, git store, usage, activity),
  `bun x playwright test tools`, smoke native (terminal, git, browser, isolasi IPC).
- Manual: buka Terminal di project berisi repo git, jalankan `git status`; ubah berkas lalu stage dan commit dari panel Git;
  buka Browser, kunjungi situs, lalu buka dialog/menu dan pastikan browser tersembunyi (tidak menutupi dialog).

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

## Panel alat sesi dan git monorepo (ADR-049)

Rel alat: Terminal, Git, **Perubahan sesi** (diff berkas yang diubah sesi, dari snapshot engine), **Berkas sesi** (berkas yang disentuh agent), Browser, Info sesi, **Aktivitas sesi** (garis waktu seluruh kejadian sesi: filter, rincian, lompat ke pesan), Log aktivitas aplikasi. Git: bila akar project bukan repo, repo di anak folder (maks 3 tingkat) ditemukan dan dipilih lewat pemilih repo. Tes: `e2e/panel4.spec.ts`, `session-timeline.test.ts`, `tools.test.ts` (monorepo), `git::tests::find_repos_*`, `files::tests::resolve_dir_within_*`. Belum diuji di Tauri sungguhan: pencarian repo dan `GET /session/:id/diff` engine nyata.

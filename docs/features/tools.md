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

## Git: push, pull, stash, discard (ADR-057)

- **Push** (tanpa `--force`; tanpa upstream memakai `push -u origin <cabang>` hanya bila remote `origin` ada) dan **Pull** (`--ff-only`: tidak pernah membuat merge/rebase otomatis; bila tidak bisa fast-forward, galat git ditampilkan). Prompt kredensial terminal dimatikan (`GIT_TERMINAL_PROMPT=0`), jadi remote yang butuh login harus lewat credential helper git; kegagalan ditampilkan apa adanya.
- **Stash** (termasuk berkas baru) dan **Pop stash** dengan jumlah stash.
- **Buang perubahan** per berkas atau semua perubahan belum di-stage, selalu dengan konfirmasi (destruktif): berkas terlacak kembali ke HEAD, berkas baru dihapus.
- Tombol disembunyikan bila host tidak mendukung. Tes: cargo `push_pull_stash_and_discard_on_real_repos`, vitest `GitStore: push, pull, stash, discard`.

## Tab browser (ADR-058)

- Browser Panel 4 punya **tab**: satu child webview native per tab (label `browser-<id>`), maksimal 8. Hanya tab aktif yang
  tampil (Rust menyembunyikan yang lain saat tab dibuka); menutup tab menutup webview-nya (membebaskan memori); menutup tab
  terakhir membuat satu tab kosong baru. Judul tab = host URL (tanpa `www.`), "Tab baru" sebelum dimuat.
- Daftar tab bertahan saat berpindah tab alat, **tidak** dipulihkan setelah aplikasi ditutup (webview tidak persisten).
- URL yang diminta bagian lain aplikasi (mis. "Buka di browser" di pratinjau HTML) dibuka di tab aktif. Perintah Rust menerima
  `tab` opsional (tanpa `tab` = label lama `browser`); id tab divalidasi.
- Tes: vitest `browser-tabs.test.ts` (state tab), cargo `tab_ids_become_safe_webview_labels`, smoke native (dua tab = dua
  webview, tutup tab membuang webview). Smoke native yang baru belum dijalankan.

## Belum / terbatas

Git Bash/WSL di terminal belum ada; push/pull/stash/discard di Git baru lulus tes Rust (repo nyata + remote bare lokal) dan belum dicoba di aplikasi Tauri atau ke remote sungguhan (butuh credential helper), airspace belum diuji otomatis,
login Google di browser belum diuji ulang.

## Cara test

- Otomatis: `cargo test -p zandor-desktop git::` dan `browser::`, `bun run test` (terminal, git store, usage, activity),
  `bun x playwright test tools`, smoke native (terminal, git, browser, isolasi IPC).
- Manual: buka Terminal di project berisi repo git, jalankan `git status`; ubah berkas lalu stage dan commit dari panel Git;
  buka Browser, kunjungi situs, lalu buka dialog/menu dan pastikan browser tersembunyi (tidak menutupi dialog).

## Panel alat sesi dan git monorepo (ADR-049)

Rel alat: Terminal, Git, **Perubahan sesi** (diff berkas yang diubah sesi, dari snapshot engine), **Berkas sesi** (berkas yang disentuh agent), Browser, Info sesi, **Aktivitas sesi** (garis waktu seluruh kejadian sesi: filter, rincian, lompat ke pesan), Log aktivitas aplikasi. Git: bila akar project bukan repo, repo di anak folder (maks 3 tingkat) ditemukan dan dipilih lewat pemilih repo. Tes: `e2e/panel4.spec.ts`, `session-timeline.test.ts`, `tools.test.ts` (monorepo), `git::tests::find_repos_*`, `files::tests::resolve_dir_within_*`. Belum diuji di Tauri sungguhan: pencarian repo dan `GET /session/:id/diff` engine nyata.

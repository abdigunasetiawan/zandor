# ADR-072 — Background job untuk agent (server, watcher) + panel Jobs

- **Tanggal:** 16-11-2026
- **Status:** diterima

## Konteks

Tool `bash` bawaan opencode menunggu perintah selesai (dengan timeout). Perintah yang tidak pernah selesai
(`npm run dev`, `vite`, watcher) membuat agent tertahan sampai timeout lalu prosesnya mati. Harness agent lain
(Claude Code, Codex) memberi agent cara menjalankan proses di latar, membaca outputnya belakangan, dan menghentikannya,
plus UI untuk melihatnya. Zandor belum punya itu.

Pilihan yang ditimbang:

1. **PTY engine (`/pty`)**: job otomatis tampil sebagai tab Terminal. Namun plugin harus membaca output lewat WebSocket
   ke server engine sendiri (butuh tiket/kredensial server di dalam plugin) dan output PTY penuh kode kontrol.
2. **Plugin mengelola proses anak sendiri** (dipilih): sederhana, bisa dites tanpa engine, output bersih (pipa).

## Keputusan

- Plugin Zandor (`packages/plugin/src/jobs.ts`) menambah tool **`job_start`, `job_output`, `job_list`, `job_stop`,
  `job_restart`**. Proses dijalankan dengan shell sistem (Windows: `cmd.exe`, `TODO(port)`: `sh -c` + grup proses),
  env **disaring `scrubEnv`** (ADR-022), cwd dibatasi di dalam folder project/sesi.
- `job_start` dan `job_restart` **meminta izin lewat `ctx.ask({ permission: "bash" })`**, jadi mengikuti mode izin
  dan kartu izin yang sama dengan `bash` (ADR-068). Tidak ada jalan pintas melewati izin.
- `job_start` menunggu singkat (bawaan 8 dtk, maks 60) sampai **port terdeteksi** di output, proses keluar, atau
  waktu habis, lalu mengembalikan status + port + ekor output. `job_output` bisa menunggu pola (`wait_for`).
- Tiap giliran, plugin menyuntikkan blok `<zandor-jobs>` (hanya bila ada job di sesi itu): daftar job, status, port,
  kode keluar. Dengan begitu agent tahu bila server crash tanpa harus bertanya.
- Status disimpan sebagai berkas di `<app_data>/jobs/`: `<id>.json` (meta) dan `<id>.log` (≤ 256 KB terakhir).
  UI membaca lewat perintah Tauri `jobs_list` / `job_log`; aksi **Stop/Restart** dari UI ditulis sebagai berkas kontrol
  `control/<id>.<aksi>` yang dipantau plugin (satu jalur, tidak ada dua pengelola proses).
- Hanya instance plugin pemilik (pid engine sama) yang mengelola job-nya. Meta `running` milik pid engine lain
  ditandai `lost` saat start. Proses anak ikut mati bersama engine karena engine berada di Job Object
  `KILL_ON_JOB_CLOSE` (supervisor); stop memakai `taskkill /T /F` agar pohon proses ikut mati.
- UI: alat **Jobs** baru di Panel 4 (daftar job, status, port → buka di Browser Panel, log, Stop/Restart/Hapus).

## Konsekuensi

- Agent bisa menjalankan server/watcher tanpa macet, dan pengguna melihat serta mengendalikannya.
- Job tidak bertahan setelah engine dimulai ulang (sengaja; ditandai `lost`).
- Job tidak tampil sebagai tab Terminal (input interaktif ke job tidak didukung).
- Polling berkas (UI ±1,5 dtk, kontrol ±0,7 dtk) — cukup untuk skala puluhan job.

# ADR-019 — Spike S1: `opencode serve` sebagai sidecar Tauri

- **Tanggal:** 13-11-2026
- **Status:** Diterima untuk Windows · macOS dan Linux **ditunda** (keputusan pemilik 13-11-2026:
  belum ada perangkat; diverifikasi lewat CI runner atau perangkat pinjaman nanti)
- **Hasil spike:** ✅ Lulus di Windows 11 x64 · ⚠️ macOS/Linux hanya lolos kompilasi (`cargo check`), belum dijalankan
- **Kode:** `spikes/s1-sidecar/` (crate `supervisor` + app Tauri `src-tauri`), `spikes/tools/fetch-opencode.mjs`

## Pertanyaan

Apakah binary `opencode` bisa dibundel sebagai sidecar di Windows/macOS/Linux, lalu dijalankan,
dipantau, dan dimatikan dengan bersih?
Kriteria lulus (roadmap): tidak ada proses yatim; port loopback + token; versi di-pin.

## Keputusan

1. **Sumber binary:** paket npm resmi `opencode-<os>-<arch>` (MIT), versi di-pin di
   `spikes/tools/opencode-version.json` (saat ini **1.19.5**). Skrip `fetch-opencode.mjs` mengunduh lewat
   `npm pack` (integritas tarball dicek npm), menaruhnya sebagai `binaries/opencode-<target-triple>`,
   menulis sha256, dan menolak bila `--version` tidak cocok.
2. **Bundel:** `bundle.externalBin = ["binaries/opencode"]`. Tauri menyalinnya ke sebelah executable
   sebagai `opencode[.exe]` (berlaku juga untuk `cargo build` biasa). Supervisor memanggil binary itu
   langsung lewat `std::process` — **bukan** `tauri-plugin-shell` — karena kita butuh kontrol flag
   pembuatan proses (Job Object, process group).
3. **Start:** `opencode serve --hostname 127.0.0.1 --port 0`, dengan env:
   - `OPENCODE_SERVER_PASSWORD=<64 hex acak per start>` → HTTP Basic `opencode:<token>`.
   - `OPENCODE_DISABLE_AUTOUPDATE=1` → engine tidak mengganti versinya sendiri.
     URL dibaca dari baris stdout `opencode server listening on http://…`, lalu `GET /global/health`
     wajib 200 dan versi wajib sama dengan pin. stdout/stderr terus dikuras ke ring buffer 500 baris.
4. **Monitor & restart:** thread monitor (`try_wait` tiap 250 ms) memanggil callback bila engine mati
   tanpa diminta. App merestart maksimal 3x dengan backoff; setelah itu error ditampilkan apa adanya.
   Semua start/restart lewat **satu thread supervisor yang hidup sepanjang app** (wajib di Linux,
   karena `PR_SET_PDEATHSIG` terikat ke thread pembuat).
5. **Stop:** best-effort `POST /global/dispose` (1,5 dtk), lalu bunuh seluruh pohon proses.
6. **Anti proses yatim:**
   - **Windows:** proses dibuat `CREATE_SUSPENDED | CREATE_NO_WINDOW`, dimasukkan ke Job Object dengan
     `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, baru di-resume (`NtResumeProcess`). Jika app crash atau
     di-kill, kernel menutup handle job dan seluruh pohon (engine + cucu: PTY, shell tool, LSP) ikut mati.
   - **Linux:** process group sendiri + `PR_SET_PDEATHSIG(SIGKILL)`; stop = `SIGTERM` ke group, 3 dtk, `SIGKILL`.
   - **macOS:** process group sendiri; stop sama seperti Linux. **Tidak ada mekanisme otomatis saat
     parent crash** (lihat risiko).
7. **Akses dari UI:** UI (webview lokal, origin `http://tauri.localhost`) memanggil engine langsung
   via `fetch` dengan header `Authorization`. CORS bawaan opencode sudah mengizinkan origin
   `http://tauri.localhost` dan `tauri://localhost`. **SSE wajib lewat `fetch` + stream**, karena
   `EventSource` tidak bisa mengirim header `Authorization` (penting untuk S2).

## Bukti (Windows 11 x64, Rust 1.99.1, Tauri 2.12.5, opencode 1.19.5)

`cargo test -p zandor-engine-supervisor -- --test-threads=1` → **10/10 lulus** (3 unit + 7 integrasi
memakai binary asli):

| Test                                                                   | Hasil                                    |
| ---------------------------------------------------------------------- | ---------------------------------------- |
| start + health + versi di-pin                                          | engine siap **1,1 dtk**; URL `127.0.0.1` |
| tanpa token / token salah / `/session` tanpa token                     | 401                                      |
| pin versi dipaksakan (`expected_version` salah)                        | start ditolak `VersionMismatch`          |
| dua engine bersamaan                                                   | port & token berbeda                     |
| `stop()` dengan proses cucu (`POST /pty` → `ping.exe` + `conhost.exe`) | semua mati, stop **82 ms**               |
| engine mati tak terduga                                                | callback dipanggil (code 1)              |
| `stop()` tidak memicu callback "mati tak terduga"                      | ✓                                        |
| **parent di-kill paksa** (TerminateProcess) saat engine + cucu hidup   | engine & semua cucu mati                 |

App Tauri (`ZANDOR_SELFTEST=<file>`), debug **dan** release → 6/6 lulus:
engine siap ±1,0 dtk; `fetch` health dari webview 200; tanpa token 401; daftar sesi 200;
SSE `/global/event` lewat fetch-stream menerima `server.connected`; crash engine → restart otomatis,
pulih **1,1–1,3 dtk**. Tambahan manual: app di-`taskkill /F` → tidak ada `opencode.exe` tersisa.

Bundel: `tauri build` → installer NSIS **58,5 MiB**; `installer.nsi` memuat
`File "/oname=opencode.exe" …` dan menghapusnya saat uninstall. Installer **tidak** dijalankan
(tidak ingin mengubah sistem), jadi jalur "setelah instal" belum diuji.

## Yang belum terbukti / risiko (jujur)

1. **macOS & Linux belum dijalankan sama sekali.** Kode unix hanya lolos `cargo check` untuk
   `x86_64-unknown-linux-gnu` dan `aarch64-apple-darwin`. Kriteria S1 menyebut tiga OS, jadi
   **S1 belum boleh dianggap lulus penuh** sampai test yang sama jalan di mac dan Linux (rencana: job CI
   matrix di Fase 1, atau mesin pemilik).
2. **macOS: proses yatim saat app crash.** Tidak ada padanan Job Object/PDEATHSIG. Test
   `parent_crash_leaves_no_orphans` diperkirakan **gagal** di macOS. Usulan mitigasi (belum dibuat):
   (a) pidfile + "reap" engine basi saat app start (semua OS), dan/atau (b) helper kecil yang memantau
   PID app via `kqueue EVFILT_PROC/NOTE_EXIT` lalu membunuh process group engine.
3. **Linux: cucu engine saat crash.** `PDEATHSIG` hanya mengenai engine; cucu (PTY/shell) bisa
   tersisa bila engine mati oleh SIGKILL. Mitigasi sama dengan poin 2 (reaper / pidfile, atau
   `PR_SET_CHILD_SUBREAPER`). Belum diuji.
4. **Ukuran:** binary opencode **211 MB** per platform (installer 58,5 MiB). Ini bertentangan dengan
   kesan "ringan" di ADR-002; perlu diterima secara sadar atau diganti opsi "unduh engine saat
   pertama kali dijalankan".
5. **Config global ikut terbaca.** Sidecar membaca `~/.config/opencode` dan data
   `~/.local/share/opencode` milik user (di mesin ini: plugin dan 7 sesi lama ikut muncul). Ini bisa
   berarti "warisan sign-in" (bagus) tapi juga plugin/konfigurasi asing ikut jalan di engine Zandor.
   **Keputusan tertunda → dibahas bersama S4.** Binary mengenal `OPENCODE_CONFIG_DIR`,
   `OPENCODE_CONFIG`, `OPENCODE_DISABLE_PROJECT_CONFIG`, `OPENCODE_DISABLE_DEFAULT_PLUGINS` (belum diuji).
6. `OPENCODE_DISABLE_AUTOUPDATE` ada di binary, tetapi **efeknya belum diverifikasi** (perlu versi
   lama + update yang tersedia untuk membuktikan).
7. Token engine dipegang JS di webview UI. Aman selama hanya webview `ui` yang punya IPC (dibuktikan
   S3: halaman remote ditolak). Alternatif bila perlu lebih ketat: proxy di Rust.
8. `NtResumeProcess` adalah API ntdll yang tidak terdokumentasi resmi. Alternatif terdokumentasi:
   `CreateProcessW` + `PROC_THREAD_ATTRIBUTE_JOB_LIST`. Ganti bila ada masalah AV/kompatibilitas.
9. `opencode serve --port 0` selalu mencoba 4096 dulu; aman (port lain dipilih bila terpakai —
   terbukti di test dua engine), tetapi URL tidak boleh di-hardcode.
10. **(Ditemukan di S4 / ADR-022)** Proses anak engine (tool bash, PTY) mewarisi
    `OPENCODE_SERVER_PASSWORD`, sehingga perintah yang dijalankan agent bisa memanggil API engine,
    termasuk menyetujui izinnya sendiri. Mitigasi wajib: plugin `zandor-env-guard` (hook `shell.env`).
11. Belum diuji: binary tak bertanda tangan 211 MB vs Windows Defender/SmartScreen dan Gatekeeper
    (notarisasi macOS wajib menandatangani sidecar juga).

## Konsekuensi

- Fase 4 (supervisor engine) memakai desain crate `zandor-engine-supervisor` ini sebagai dasar.
- S2 harus memakai SSE via `fetch` stream, bukan `EventSource`.
- Fase 1 CI wajib menjalankan test S1 di matrix Windows/macOS/Linux sebelum Fase 0 dinyatakan
  lulus penuh, atau PRD direvisi untuk risiko macOS.

## Cara mengulang

```
node spikes/tools/fetch-opencode.mjs
cd spikes
cargo test -p zandor-engine-supervisor -- --test-threads=1 --nocapture
cargo build -p zandor-spike-s1
ZANDOR_SELFTEST=$TEMP/s1.json ./target/debug/zandor-spike-s1   # lalu cek tidak ada proses opencode
```

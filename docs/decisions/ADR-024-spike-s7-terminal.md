# ADR-024 — Spike S7: terminal (xterm.js + PTY)

- **Tanggal:** 13-11-2026
- **Status:** Diterima untuk Windows · macOS/Linux ditunda (tidak ada perangkat)
- **Hasil spike:** ✅ Lulus di Windows 11 untuk **kedua** backend yang dibandingkan
- **Kode:** `spikes/s7-terminal/` (app Tauri: `src-tauri/src/main.rs`, `ui/index.html`)

## Pertanyaan
Apakah xterm.js + PTY menghasilkan shell interaktif yang berjalan dengan resize yang benar di tiga OS?
Roadmap menyebut `portable-pty`, padahal ternyata `opencode serve` juga punya API `/pty`. Mana yang
dipakai?

## Opsi yang diuji (UI xterm.js 6.0.0 + addon fit + unicode11 yang sama)
- **A — `portable-pty =0.9.0` di Rust Zandor.** Byte output dialirkan ke UI lewat
  `tauri::ipc::Channel` (raw, tanpa JSON). Shell dimasukkan ke Job Object Zandor.
- **B — API `/pty` milik `opencode serve` 1.19.5.** UI langsung memanggil `POST /pty` lalu
  `POST /pty/:id/connect-token` (header wajib `x-opencode-ticket: 1`) untuk mendapat tiket
  **sekali pakai, berlaku 60 dtk**, kemudian membuka WebSocket
  `/pty/:id/connect?ticket=…&cursor=N`.
  - Frame teks = output.
  - Frame biner `0x00` + `{"cursor":N}` = posisi untuk replay.
  - Input = teks.
  - Resize = `PUT /pty/:id {size:{cols,rows}}`.
  - Tiket diperlukan karena WebSocket di browser tidak bisa mengirim header `Authorization`.

## Keputusan
1. **Backend utama: B (`opencode /pty`).**
   - Terminal berada di **mesin engine**, jadi otomatis benar untuk engine remote (Fase 11) tanpa
     kode tambahan.
   - Ada **replay/reconnect** bawaan (UI di-reload → riwayat terminal kembali).
   - Env proses anak sudah dilindungi plugin `zandor-env-guard` (hook `shell.env` juga dipanggil
     saat PTY dibuat, lihat ADR-022/023).
   - Lebih sedikit kode Rust untuk dirawat. Throughput-nya bahkan lebih cepat di uji ini.
   - Daftar shell diambil dari `GET /pty/shells`. Di Windows, default-nya Git Bash bila terpasang.
2. **A tidak dipakai di MVP**, tetapi pola ini (Channel raw + Job Object) disimpan sebagai cadangan
   bila API `/pty` berubah atau bila nanti dibutuhkan "terminal lokal" yang tidak bergantung pada engine.
3. **Wajib di app (Windows):** panggil `SetConsoleCtrlHandler(NULL, FALSE)` saat start, sebelum
   membuat engine atau PTY. Status "abaikan Ctrl+C" **diwariskan dari proses induk**. Bila Zandor
   diluncurkan dari proses yang mengabaikan Ctrl+C, Ctrl+C di **semua** terminal (A maupun B) tidak
   berfungsi. Ini terbukti: tanpa reset `ping -t` tidak berhenti di kedua backend (juga di `cmd.exe`
   dan dengan format win32-input-mode); dengan reset, berhenti di keduanya.
4. xterm.js memakai addon `unicode11` (lebar sel CJK/emoji benar) dan `fit` + `ResizeObserver`.

## Bukti (Windows 11, PowerShell 5.1 `-NoProfile`, 2 run berturut-turut, semua lulus)

| Skenario | A portable-pty | B opencode /pty |
|---|---|---|
| Prompt siap sejak terminal dibuka | 0,9 dtk | 1,7 dtk |
| Echo perintah | ✓ | ✓ |
| Unicode `héllo 你好 🚀 ✓`, sel CJK lebar 2 | ✓ | ✓ |
| Resize 420×260 / 900×520 / 600×360 → `[Console]::WindowWidth/Height` = cols/rows xterm | ✓ | ✓ |
| Resize lalu langsung mengetik 5x → tidak ada input hilang | ✓ | ✓ |
| stdin interaktif (`Read-Host`) | ✓ | ✓ |
| Ctrl+C menghentikan `ping -t` (dengan reset poin 3) | ✓ | ✓ |
| 20.000 baris output | 4,3 dtk, jeda frame maks 13–14 ms | 2,8–3,2 dtk, jeda frame maks 12 ms |
| Reconnect WebSocket baru (`cursor=0`) → riwayat kembali, input lanjut | — | ✓ |
| Tutup terminal → proses shell mati | ✓ | ✓ |
| **App di-kill paksa** dengan dua shell terbuka → tidak ada proses tersisa (shell, conhost, engine, WebView2) | ✓ | ✓ |

## Temuan & risiko (jujur)
1. **B: tampilan echo rusak setelah resize** (konsisten di salah satu ukuran). Ini hanya masalah
   redraw tampilan, inputnya tetap utuh (diuji terpisah). Pada eksperimen awal dengan Git Bash, satu
   karakter ketikan sempat benar-benar hilang (`mode con` → `ode con`) setelah resize. Hal itu tidak
   terulang di PowerShell, tetapi **untuk Git Bash belum diuji ulang secara sistematis**. Perlu
   dipastikan sebelum Fase 8 selesai.
2. **Ctrl+C:** lihat keputusan poin 3. Ini akar masalah yang mudah terlewat. Gejalanya "terminal
   rusak" padahal penyebabnya proses induk.
3. **WebView2 yatim** teramati sekali: app yang macet lalu dimatikan paksa oleh `timeout` meninggalkan
   proses `msedgewebview2.exe` yang memegang profil, sehingga run berikutnya gagal start ("resource
   is in use"). Pada uji kill paksa yang terkontrol, WebView2 ikut mati. Perlu ditangani di Fase 3/4:
   deteksi profil terkunci saat start dan tampilkan pesan yang jujur.
4. Protokol `/pty` adalah "v1" di 1.19.5. Source opencode terbaru (2.2.7) memakai frame biner berbeda
   (`/api/pty`, v2). **Upgrade engine bisa mematahkan terminal.** Suite S7 wajib dijalankan saat
   upgrade.
5. B bergantung pada engine: kalau engine mati, terminal ikut mati. Supervisor ADR-019 akan
   merestart engine, tetapi PTY lama hilang (tidak diuji).
6. Belum diuji: macOS/Linux; renderer WebGL xterm; input IME; copy/paste & tautan; banyak terminal
   sekaligus (memori); HiDPI; Git Bash dan shell lain secara sistematis; shell dengan profil user
   (spike memakai `-NoProfile`).

## Konsekuensi
- Fase 8: Panel 4 Terminal = xterm.js + `/pty` engine (B), multi-tab, mengikuti `directory` project
  aktif.
- Fase 3/4: app memanggil `SetConsoleCtrlHandler(NULL, FALSE)` saat start (Windows), dan supervisor
  memastikan plugin guard termuat sebelum PTY pertama dibuat.
- `portable-pty` dikeluarkan dari rencana MVP. Roadmap S7 diperbarui bersamaan dengan revisi roadmap
  Fase 0.

## Cara mengulang
```
cd spikes
cargo build -p zandor-spike-s1            # menyalin sidecar opencode ke target/debug
cd s7-terminal && bun install && node tools/vendor.mjs && cd ..
cargo build -p zandor-spike-s7
ZANDOR_SELFTEST=$TEMP/s7.json ./target/debug/zandor-spike-s7    # laporan JSON + .log
./target/debug/zandor-spike-s7                                    # uji manual: A kiri, B kanan
```

# ADR-036 — Panel 4: Terminal, Git, Browser, Info sesi, Log aktivitas (Fase 8)

- **Tanggal:** 14-11-2026
- **Status:** Diterima
- **Terkait:** ADR-020 (browser child webview), ADR-022 (env guard), ADR-024 (terminal), ADR-032/033/035

## Keputusan

1. **Terminal = xterm.js 6 + API `/pty` engine** (ADR-024 butir 1), bukan `portable-pty`. Klien (`packages/core/src/terminal/pty.ts`):
   daftar shell, buat PTY dengan `cwd` = folder project, resize, hapus, dan WebSocket dengan tiket sekali pakai (header
   `x-opencode-ticket: 1`). PowerShell diutamakan (`-NoLogo`). Terminal **mengikuti project aktif** (cwd), tab terminal
   hidup di engine sehingga **muat ulang aplikasi memulihkannya dengan riwayat** (adopsi PTY yang `running`; engine
   memutar ulang output). Koneksi putus tetapi shell hidup → sambung ulang otomatis (maks 5×); shell selesai → tab
   ditandai ✕ dengan pesan "[proses selesai]".
2. **Git lewat `git` CLI di Rust** (`git.rs`), hanya di project terdaftar: status (porcelain v1 `-z`), diff (termasuk berkas
   baru lewat `--no-index`), stage/unstage (menangani repo tanpa commit), commit, cabang (buat/pindah, nama divalidasi
   `git check-ref-format`), log. Tanpa shell, argumen terpisah dan `--` sebelum path, path wajib relatif tanpa `..`,
   `core.fsmonitor=false` (cegah eksekusi kode dari konfigurasi repo saat membaca status), `GIT_TERMINAL_PROMPT=0`,
   keluaran dan waktu dibatasi (read 20 dtk, write 90 dtk), tanpa jendela konsol. Identitas commit memakai konfigurasi
   git pengguna; galat (mis. identitas kosong) ditampilkan apa adanya. UI: cabang, tracking ↑↓, staged/perubahan/konflik,
   diff berwarna, commit, buat/pindah cabang, commit terbaru.
3. **Browser = child webview Tauri** (`browser.rs`, fitur `unstable`, ADR-020): hanya http/https (`on_navigation` + parser
   yang diuji), bounds mengikuti elemen panel (ResizeObserver) dan **disembunyikan saat ada overlay UI** (dialog, menu,
   palet, toast) karena webview native selalu di atas UI (airspace). Konten remote **tanpa capability** apa pun.
   Di luar Tauri alamat dibuka di tab browser; di Linux Wayland fallback sistem (ADR-020).
4. **Info sesi** dihitung murni dari pesan sesi (`sessionUsage`): token, biaya (hanya bila provider melaporkan — `null`,
   bukan 0 palsu), konteks terpakai vs batas model (`limit.context` dari engine), panggilan tool per jenis, waktu kerja.
5. **Log aktivitas** (`ActivityLog`, ring buffer 500): tool call (sekali per status), izin diminta/dijawab, retry, galat sesi,
   transisi status engine, serta warn/error aplikasi dari logger (yang sudah disamarkan). Bisa disaring per sumber dan
   dibersihkan.

## Temuan penting

- **Perintah Tauri yang membuat/mengubah webview WAJIB `async`.** Sebagai fungsi sinkron, `add_child` berjalan di thread
  utama dan **child webview WebView2 macet di `about:blank`** selamanya (perintah sinkron lain yang menyentuh webview juga
  menggantung). Ditemukan lewat smoke native + screenshot layar; setelah `async`, halaman termuat dan ukuran webview sama
  persis dengan panel (selisih 0 px). Komentar peringatan ada di `browser.rs`.
- **Bug nyata ditangkap tes:** engine memutar ulang riwayat terminal _segera_ setelah WebSocket terbuka; klien awal memasang
  penerima data setelah `connect()` selesai sehingga riwayat hilang (terminal tampak kosong). Sekarang data awal
  ditampung sampai ada penerima (diuji unit dan e2e).
- Smoke yang dihentikan paksa tidak menjalankan `finally`, sehingga registry aplikasi pengguna bisa tertinggal project uji;
  smoke kini membersihkan sisa itu saat start. (Tetap risiko: smoke menyentuh data aplikasi asli.)

## Batasan yang diketahui (jujur)

- **Sembunyi saat overlay (airspace) belum diuji otomatis di aplikasi asli** — logikanya (MutationObserver) hanya diuji
  tidak langsung; verifikasi manual diperlukan.
- Login Google/persistensi sesi di browser tertanam belum diuji ulang (ADR-020 temuan 2 tetap berlaku). Browser berbagi
  profil WebView2 dengan UI aplikasi.
- Browser: tidak ada tab, riwayat/bookmark, unduhan, atau penanganan jendela baru (`target=_blank`) khusus.
- Git: tanpa discard/stash/merge/push/pull, tanpa UI resolusi konflik (konflik hanya ditampilkan), commit memakai hook repo.
- Terminal: Git Bash/WSL belum diuji (ADR-024 risiko #1: echo setelah resize di Git Bash perlu diuji sistematis);
  PowerShell dan cmd tersedia. Semua tab terminal tetap terpasang (disembunyikan) sehingga banyak tab = banyak xterm.
- Info sesi: biaya `null` untuk provider yang tidak melaporkan (mis. endpoint kustom).
- Hanya Windows (ADR-027).

## Verifikasi

Rust (45 tes): git pada repo asli sementara (alur lengkap, cabang, validasi, batas waktu), parser status, path aman, URL
browser. Core (466 tes): klien PTY, `TerminalStore`, `GitStore`, `sessionUsage`, `ActivityLog`, URL browser. E2E (95): terminal
(buat/ketik/tab/tutup/adopsi/sambung ulang/shell selesai/ikut project), git (pesan jelas di luar desktop), info, log, browser.
**Smoke native 30/30 pada aplikasi asli:** terminal PowerShell nyata di folder project, git nyata (status, diff, stage, commit
tercatat di repo), child webview memuat server lokal, ukuran webview sama dengan panel, dan halaman remote tidak bisa
memanggil perintah aplikasi (`git_status`, `fs_read`, `browser_close`, `providers_list` ditolak).

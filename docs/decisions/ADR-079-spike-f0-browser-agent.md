# ADR-079 — Browser-use untuk agent: spike F0 lulus (CDP lewat WebView2, dua profil, snapshot)

Tanggal: 19-11-2026 · Status: spike diterima; desain penuh = [`docs/features/browser-agent.md`](../features/browser-agent.md), **belum diimplementasi**

## Konteks
Agent hanya bisa membuka URL di Panel 4 (ADR-077). Untuk browser-use perlu klik/ketik tepercaya, baca halaman, screenshot,
dan profil terpisah dari login pengguna. Tiga asumsi teknis diuji sebelum desain dikomit.

## Spike
`spikes/s9-browser-agent` (crate mandiri, `tao` + `wry 0.57` + `webview2-com 0.39`): dua child webview di satu jendela,
masing-masing `WebContext` dengan data directory sendiri, memuat fixture HTTP lokal (`127.0.0.1`).
Jalankan: `cargo build` lalu `target/debug/zandor-spike-s9.exe` (hasil juga di `%TEMP%/zandor-s9/hasil.txt`).

**Hasil (Windows 11, WebView2): 15 PASS, 0 FAIL.**

| Asumsi | Hasil |
|---|---|
| S-B1 dua profil WebView2 berdampingan, cookie/localStorage terisolasi | **Lulus.** Cookie dan localStorage di profil A tidak terlihat di B (`"profil=A\|A"` vs `"\|null"`); CDP dan snapshot jalan di kedua profil |
| S-B2 CDP lewat `WebViewExtWindows::controller().CoreWebView2().CallDevToolsProtocolMethod` dari child webview wry | **Lulus.** `Input.dispatchMouseEvent` (klik), `Input.insertText`, `Input.dispatchKeyEvent` (Enter) sampai ke halaman dengan **`isTrusted=true`**; `Page.captureScreenshot` mengembalikan PNG (~40 ms). Satu klik (move+press+release) ~3–12 ms |
| S-B3 snapshot + ref lewat initialization script | **Lulus.** Role+nama+ref terbaca (5 ref = 164 byte untuk fixture), ~2 ms; nilai password disamarkan; teks `display:none` (umpan injeksi) tidak ikut `innerText`; halaman tidak bisa menimpa `__zsnap` (`defineProperty` non-writable, ref di closure); hit-test `elementFromPoint` + `scrollIntoView` bekerja |

## Temuan yang mengubah desain
1. **Webview tersembunyi (`set_visible(false)`):** `document.visibilityState` menjadi `"hidden"`; evaluasi JS dan **klik/ketik CDP tetap jalan**, tetapi **`Page.captureScreenshot` menggantung** (timeout 15 dtk). Konsekuensi: screenshot hanya saat webview tampil (Panel 4 terbuka, tanpa overlay); aksi tulis saat tersembunyi harus diperlakukan sebagai kondisi yang dilaporkan, bukan diam-diam dijalankan (lihat risiko "airspace" di dokumen fitur). Beri batas waktu pendek (≤5 dtk) pada screenshot.
2. **Versi crate bertabrakan:** `webview2-com 0.39` memakai `windows 0.62`, sedangkan `apps/desktop-gpui` memakai `windows 0.61`. Tipe tidak kompatibel; di app tambahkan `windows62 = { package = "windows", version = "0.62", features = ["Win32_Foundation"] }` + `webview2-com = "0.39"` (jangan menaikkan `windows 0.61` yang dipakai modul lain).
3. **Jebakan fixture/halaman:** id elemen seperti `name` menimpa global `window.name`; snapshot/aksi harus memakai ref dan `querySelector`, bukan global bernama.
4. **Callback CDP dan `evaluate_script_with_callback` datang di thread UI** (message loop). Di GPUI, panggil dari thread utama dan kirim hasil lewat channel; jangan memblokir thread UI menunggu hasil.

## Belum terbukti (tetap risiko)
- Dijalankan di jendela `tao`, **bukan** di child webview di bawah jendela GPUI; perilaku fokus/keyboard native dan airspace di GPUI perlu dites saat F1.
- `mouseWheel` terkirim (`{}`), tetapi efek scroll tidak diasersi.
- Halaman nyata (SPA, iframe lintas-origin, Shadow DOM) belum diuji; snapshot hanya DOM utama.
- `with_incognito` dan `Page.captureScreenshot` pada halaman besar belum diukur.
- Hanya Windows.

## Keputusan
Lanjut ke F1 sesuai rencana (CDP sebagai jalur utama aksi; event DOM sintetis tidak perlu sebagai cadangan awal). Keamanan, izin,
dan batasan di dokumen fitur tidak berubah; tambahan: screenshot tersembunyi = gagal cepat dengan pesan jelas.

## Implementasi F1 (19-11-2026)
Dikerjakan di `apps/desktop-gpui`: `shell/browser_agent.rs` (izin domain, navigation handler, snapshot, screenshot, kartu izin),
`shell/browser.rs` (tab Agent + profil terpisah, `eval_cb`, `cdp_call`, tombol Hentikan), `shell/agent_control.rs` (`browser_*`,
`browserAgent` dilindungi), `shell/root.rs` (polling cepat, dialog izin), `packages/plugin/src/control.ts` (`zandor_browser`),
pengaturan `browserAgent` (sumber `packages/core`, diekspor) dan teks i18n. `webview2-com 0.39` + alias `windows62` (0.62) ditambahkan
di app. Tes unit Rust dan vitest hijau; **belum** diuji native end-to-end di jendela GPUI dan belum `cargo build --release`.
Penyimpangan dari rencana: kartu izin hanya Sesi ini / Selalu / Tolak (tanpa "Sekali").

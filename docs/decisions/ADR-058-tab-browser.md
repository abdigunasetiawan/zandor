# ADR-058 — Tab browser (satu webview per tab)

Tanggal: 15-11-2026 · Status: diterima

## Keputusan

- Setiap tab browser Panel 4 adalah child webview Tauri sendiri (`browser-<id>`), bukan satu webview yang dinavigasi ulang:
  riwayat, scroll, dan sesi login tiap tab terjaga saat berpindah.
- Rust: perintah `browser_*` menerima `tab` opsional (kompatibel dengan label lama). `browser_open` menampilkan tab dan
  menyembunyikan webview browser lain (airspace tetap: UI menyembunyikan tab aktif saat ada overlay). Id tab divalidasi
  (huruf kecil/angka/`-`/`_`, maks 24) karena menjadi bagian label webview. Peristiwa `browser-load` membawa id tab.
- State tab murni di `@zandor/core` (`addBrowserTab`, `closeBrowserTab`, …), maksimal 8 tab; menutup tab terakhir membuat tab
  kosong baru dengan id lain. Daftar tab hidup di memori modul (bertahan antar-ganti tab alat), tidak disimpan ke disk.

## Batasan

- Judul tab hanya host URL (judul halaman tidak dibaca dari webview remote agar tidak menambah jalur IPC).
- Tidak dipulihkan setelah restart; belum diuji di aplikasi terpaket di sesi ini (lihat docs/features/tools.md).

# ADR-047 — Antrean pesan (stacked message)

Tanggal: 14-11-2026 · Status: diterima

## Konteks

Saat agent bekerja, composer menolak pesan baru ("sesi sedang berjalan"), sehingga pengguna harus menunggu atau menghentikan
agent untuk menambah instruksi.

## Keputusan

- Pesan yang dikirim saat agent sibuk **masuk antrean per sesi** (`ChatStore.queues`, maks 50, hanya di memori — lampiran
  bisa besar dan tidak ditulis ke disk). Tombol tambahan "Tambah ke antrean" muncul di composer saat sibuk dan ada teks.
- Antrean tampil sebagai **dok di atas composer** (`QueueDock`, bagian dari dok "Menunggu Anda"): urutan bisa diubah
  (seret-lepas HTML5, atau Alt+↑/↓ pada gagang agar bisa lewat keyboard), pesan bisa diedit, dihapus, atau **dikirim langsung**.
- **Pengiriman otomatis**: begitu sesi idle (tanpa galat, tanpa izin/pertanyaan tertunda, bukan sedang memuat ulang), pesan
  terdepan dikirim — satu per giliran (tidak merantai beberapa pesan sekaligus).
- **Stop menjeda antrean** (pesan tidak dikirim diam-diam setelah pengguna menghentikan agent); dok menampilkan "Antrean
  dijeda" + tombol Lanjutkan. Pengiriman otomatis yang gagal (mis. belum ada model) juga menjeda.
- **Kirim langsung**: item dipindah ke depan, giliran berjalan dihentikan (abort), lalu item itu dikirim begitu sesi idle;
  item lain tetap menunggu di belakangnya.

## Konsekuensi

- Antrean hilang bila aplikasi ditutup (disengaja; dicatat di dok fitur chat).
- Pengiriman memakai model/varian yang terpilih saat pesan benar-benar dikirim, bukan saat diantre.
- Tes: `chat-extra.test.ts` (antrean), `e2e/queue.spec.ts`.

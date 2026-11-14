# ADR-052 — Halaman Pengaturan

Tanggal: 14-11-2026 · Status: diterima

## Keputusan

- Pengaturan dirender dari `SETTINGS_SCHEMA` dengan **navigasi kategori** (Umum, Tampilan, Chat, Notifikasi dan suara, Alat, Tata
  letak, Lainnya, Data), **pencarian** lintas kategori, dan tombol reset per baris.
- Pengaturan baru: toast/sistem/suara/hanya-saat-tidak-dilihat, tiga peristiwa notifikasi, volume, suara per peristiwa, tombol kirim
  (Enter / Ctrl+Enter), lebar isi chat, konfirmasi hapus sesi, tampilkan sesi demo, ukuran huruf terminal.
- Bagian khusus: izin notifikasi sistem, uji dan berkas suara kustom, atur ulang tata letak, kartu ke Tampilan lanjutan / Provider /
  Memori dan skill / Automasi, **ekspor/impor JSON** (hanya nilai yang diubah; nilai tidak valid dilewati), dan **atur ulang semua**.
- Dokumentasi skema (`docs/manual/settings.*`) dihasilkan ulang dari skema.

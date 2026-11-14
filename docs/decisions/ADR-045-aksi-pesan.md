# ADR-045 — Aksi per pesan (tautan, salin, simpan sebagai, catatan, sematkan, cabang, ulangi)

Tanggal: 14-11-2026. Status: diterima.

## Konteks

ADR-044 hanya menampilkan tiga aksi dari gambar acuan (salin, cabangkan, ulangi) dan melewatkan sisanya dengan alasan fiturnya
belum ada. Itu keliru: fitur yang diminta pemilik harus dibangun.

## Keputusan

- **Salin tautan pesan:** format `zandor://chat/{key}/{sessionId}/{messageId}` (`chat/links.ts`, ada tes). Bukan tautan OS; dibuka
  dengan menempel di palet perintah (butir "Buka pesan dari tautan") → membuka sesi dan menggulir ke pesan (disorot).
- **Salin isi:** teks respons (Markdown) ke clipboard.
- **Simpan sebagai (popover):** Teks (Markdown), Gambar (PNG), SVG. Gambar/SVG dirender dari DOM jawaban dengan `html-to-image`
  (MIT); teks langsung. Disimpan lewat `DocHost.saveBytes` (dialog Simpan sebagai + perintah Rust `fs_save_as`, atau unduhan Blob di
  web). `fs_save_as` menolak berkas dapat-eksekusi, folder tujuan yang tidak ada, dan data > 32 MB (tes Rust).
- **Simpan ke catatan:** menambahkan blok bertanggal ke catatan project / global / sesi (berkas memori yang sama dengan halaman
  Memori). Isi yang mengandung kunci/rahasia ditolak.
- **Sematkan pesan:** per sesi, tersimpan (`ChatStore.pinnedMessages`, pref `zandor.pinnedMessages`), ditampilkan di bar "Pesan
  disematkan (n)" di atas transkrip (klik → gulir ke pesan, tombol lepas).
- **Cabangkan / Ulangi:** tetap (fork dari pesan; ulangi hanya untuk jawaban terakhir). Di sesi demo, aksi yang butuh engine
  (cabang, ulangi, revert) disembunyikan; sisanya berfungsi.

## Belum

Ekspor PNG/SVG dan Simpan sebagai belum diuji di Tauri nyata (jalur web teruji e2e). Tautan pesan hanya dikenali di dalam aplikasi.

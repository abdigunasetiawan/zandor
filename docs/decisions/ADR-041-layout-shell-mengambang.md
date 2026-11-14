# ADR-041 — Layout shell: panel mengambang, sidebar bertingkat, sapaan sesi baru

Tanggal: 14-11-2026. Status: diterima.

## Konteks

Pemilik meminta tampilan shell mengikuti aplikasi harness lama: panel terpisah berjarak dengan sudut membulat; sidebar
berurutan; panel chat dengan tab sesi dan sapaan; Panel Alat tanpa tab (dipilih dari rail).

## Keputusan

- Latar `--z-color-surface`, tiap panel kartu `--z-color-bg` + border + `--z-radius-lg`, separator transparan.
- Sidebar: logo + brand + cari (ikon membuka kolom filter) → tab Chat | Berkas → tombol Chat baru (Ctrl+N, perintah
  `chat.new`) dan Automasi → section Projects (+) → section Chat (+) → footer avatar, nama pengguna OS (perintah Rust
  `os_user`), versi, status engine, menu ⋯ (Pengaturan, Provider, Memori & skill, Tampilan).
- Chat: tab sesi selalu tampil dengan penanda model (Avatar inisial provider; tanpa logo berlisensi); model picker pindah
  ke baris aksi composer; sesi kosong menampilkan sapaan berdasar jam + composer di tengah, chip saran di bawah.
- Panel Alat: header (ikon, judul, tutup) tanpa tab; alat dipilih lewat rail.

## Belum

Tombol notifikasi (belum ada ikon/penyimpanan notifikasi; tidak dibuat sebagai UI palsu), nama pengguna yang bisa diubah
di pengaturan, kartu project berlatar gambar seperti di aplikasi lama.

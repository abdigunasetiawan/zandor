# ADR-068 — Mode izin "Otomatis, tanya yang berisiko" sebagai bawaan

- Status: diterima
- Tanggal: 16-11-2026

## Konteks

Mode izin sisi klien (ADR-034) hanya punya tiga pilihan: Tanya setiap kali (bawaan), Setujui edit otomatis, dan Setujui
semua otomatis. "Tanya setiap kali" membuat agent sering berhenti untuk aksi yang tidak berbahaya (mis. `bun test`),
sedangkan "Setujui semua" tidak memberi rem untuk aksi yang sulit dibatalkan. Pemilik meminta mode di antara keduanya
dan menjadikannya bawaan.

## Keputusan

- Mode baru `auto-safe` ("Otomatis, tanya yang berisiko"), urutan pilihan: `ask` < `auto-edit` < `auto-safe` < `auto-all`.
  Menjadi bawaan (`DEFAULT_PERMISSION_MODE`); preferensi yang sudah tersimpan tetap dihormati.
- `permissionRisk(req)` (`packages/core/src/chat/permission-risk.ts`) memeriksa jenis izin, pola, dan metadata
  (`command`, `filepath`) lalu mengembalikan alasan atau `null`:
  - selalu ditanya: `external_directory` (folder luar), `doom_loop`, jenis izin tidak dikenal;
  - `edit`/`read`/dll.: ditanya bila berkas rahasia (`.env*` kecuali contoh, kunci/sertifikat, `.ssh`, isi `.git/`,
    berkas kredensial);
  - `bash`: ditanya bila cocok pola hapus paksa/rekursif, ubah riwayat git, push/publish/rilis/merge, skrip dari
    internet (`curl | sh`, `iex`), hak admin/sistem/proses, pasang paket global, atau membaca berkas rahasia;
  - `webfetch`, todo, task, skill, dll.: aman.
- Kartu izin di mode ini menampilkan alasan (`chat.risk.*`) agar pengguna tahu mengapa ditanya.
- Popup mode izin menampilkan deskripsi tiap mode (`SelectItem.description`).

## Konsekuensi

- Jauh lebih sedikit interupsi; aksi yang sulit dibatalkan tetap berhenti di kartu izin.
- Ini **bukan sandbox**: deteksi berbasis pola dan bisa dilewati perintah yang dikaburkan atau skrip yang dipanggil
  (mis. `bun run x` yang di dalamnya menghapus berkas). Dokumentasi menyarankan mode Tanya bila butuh jaminan.
- Hanya berjalan selama Zandor terbuka (sama seperti mode otomatis lain).
- Belum: aturan kustom per pola dari pengguna, opsi "tolak" otomatis, mode hanya-baca/plan.

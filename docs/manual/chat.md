# Chat, izin, dan model

- **Kirim:** `Enter` (baris baru: `Shift+Enter`). **Hentikan** membatalkan giliran yang berjalan.
- **Lampiran:** ikon klip (berkas) atau tombol folder (aplikasi desktop). Berkas dari browser dibatasi 5 MB.
- **Model:** tombol model di header membuka pemilih (cari, sematkan, uji). **Reasoning** muncul bila model mendukung varian.
- **Izin:** perubahan berkas, perintah shell, dan pengambilan web meminta persetujuan lewat kartu izin:
  _Izinkan sekali_, _Selalu izinkan_, atau _Tolak_. **Mode izin** di composer: Tanya setiap kali (bawaan), Setujui edit
  otomatis, atau Setujui semua otomatis (hanya berjalan selama Zandor terbuka).
- **Aksi pesan:** salin, **Cabangkan dari sini** (sesi baru), **Kembalikan ke sebelum ini** (revert; _Batalkan revert_ untuk
  memulihkan), **Ulangi jawaban** (regenerate).
- **Kartu tool:** klik untuk melihat masukan/keluaran/diff; tombol **Buka** membuka berkas di panel Dokumen.
- Sesi sangat panjang divirtualisasi otomatis (≥ 40 pesan).

## Respons kaya, pertanyaan agent, dan info pemakaian

- Jawaban model dirender sebagai Markdown lengkap: tabel, daftar tugas, rumus (KaTeX), **blok kode berwarna** dengan tombol
  **Salin** (blok panjang diciutkan), dan **diagram Mermaid** (muncul setelah jawaban selesai). Gambar dari luar tidak dimuat.
- Bila agent perlu bertanya, muncul **form pertanyaan**: pilih jawaban (atau ketik sendiri) lalu **Kirim jawaban**, atau **Lewati**.
- **Rencana agent** (daftar tugas) tampil di atas kotak pesan.
- Di bawah kotak pesan ada bar info: model, **konteks terpakai/batas**, token, dan biaya. Klik untuk rincian. Bar berubah warna
  saat konteks hampir penuh (≥ 80%). Kuota/limit tarif provider tidak tersedia dari engine, jadi tidak ditampilkan.

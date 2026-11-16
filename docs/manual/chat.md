# Chat, izin, dan model

- **Kirim:** `Enter` (baris baru: `Shift+Enter`). **Hentikan** membatalkan giliran yang berjalan.
- **Lampiran:** ikon klip (berkas) atau tombol folder (aplikasi desktop). Anda juga bisa menempelkan file/gambar dari clipboard
  atau menyeret file ke composer. Klik chip lampiran untuk pratinjau di modal sesuai format (mis. gambar, PDF, teks/Markdown,
  tabel, JSON, audio/video, atau Office yang didukung). Thumbnail gambar PNG/JPEG/GIF/WebP menyediakan kontrol zoom dan pan.
  Berkas clipboard/browser tidak memiliki batas ukuran buatan dari Zandor; kapasitas nyata bergantung pada memori aplikasi,
  engine, dan batas provider/model.
- **Model:** tombol model di header membuka pemilih (cari, sematkan, uji). **Reasoning** muncul bila model mendukung varian.
- **Izin:** perubahan berkas, perintah shell, dan pengambilan web meminta persetujuan lewat kartu izin:
  _Izinkan sekali_, _Selalu izinkan_, atau _Tolak_. **Mode izin** di composer (popup menjelaskan tiap mode): Tanya setiap
  kali, Setujui edit otomatis, **Otomatis, tanya yang berisiko** (bawaan), atau Setujui semua otomatis. Mode bawaan
  menyetujui aksi biasa dan tetap bertanya untuk hapus paksa/rekursif, mengubah riwayat git, push/publish, skrip dari
  internet, hak admin/sistem, pasang paket global, berkas rahasia (`.env`, kunci, `.git`), dan folder di luar project;
  kartu izinnya menyebut alasannya. Mode otomatis hanya berjalan selama Zandor terbuka.
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

## Perintah `/` dan mention `@`

- Ketik **`/`** di awal pesan untuk daftar perintah (↑↓ pilih, Enter/Tab pakai, Esc tutup):
  - `/btw <pertanyaan>` — pertanyaan sampingan di tab cabang baru; sesi ini tidak terganggu (bisa saat agent bekerja).
  - `/new` sesi baru · `/compact` ringkas riwayat · `/undo` batalkan pesan terakhir (teksnya kembali ke kotak) · `/redo`
  - `/fork` cabangkan sesi · `/copy` salin jawaban terakhir · `/mode ask|auto-edit|auto-safe|auto-all` · `/stop`
  - Perintah dari engine/project (mis. `/init`, perintah kustom, prompt MCP, skill) ikut tampil di bawahnya.
  - Mulai dengan `//` untuk mengirim teks yang diawali garis miring.
- Ketik **`@`** lalu nama berkas untuk menyebut berkas project; berkas dilampirkan ke pesan agar dibaca agent.
- Tekan **↑** di kotak kosong untuk memanggil ulang pesan terakhir Anda.

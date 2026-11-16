# Dokumen (Panel 3)

- **Buka:** klik berkas di tab Files atau tombol **Buka** pada kartu tool di chat.
- **Format:** kode, Markdown (dengan rumus KaTeX dan diagram Mermaid), gambar, SVG, PDF, CSV/TSV, JSON, HTML (sandbox),
  audio, video, **DOCX, XLSX, PPTX, notebook (.ipynb)** (pratinjau best-effort: isi dan struktur, bukan tampilan Office asli).
  Format lain (mis. `.doc` lama) menampilkan pesan dan tombol **Buka di aplikasi lain**.
- **Toolbar:** render/mentah, cari (tampilan teks), bungkus baris, salin isi, salin path, simpan salinan, buka di aplikasi
  lain, muat ulang, tutup. Tab dipulihkan setelah aplikasi dibuka kembali.
- **Keamanan:** hanya berkas di dalam project terdaftar yang dibaca; HTML berjalan tanpa skrip; berkas yang dapat dieksekusi
  tidak dibuka dari Zandor.

## Edit berkas

- Di aplikasi desktop, tombol **Edit berkas** (ikon pensil) membuat editor bisa diubah. **Ctrl+S** menyimpan; ● pada tab
  berarti ada perubahan yang belum disimpan. **Selesai edit** / menutup tab bertanya dulu bila ada perubahan.
- Bila berkas diubah program lain setelah Anda membukanya, Zandor memberi tahu dan menanyakan apakah mau **menimpa**.
- Hanya berkas teks UTF-8 yang valid dan berukuran wajar yang bisa diedit; berkas yang dapat dieksekusi tidak diedit.
- DOCX/XLSX/PPTX hanya bisa dilihat. Notebook bisa diedit sebagai JSON lewat tampilan mentah.

## Kustomisasi tampilan

- Tombol **Kustomisasi** (ikon slider, di kiri tombol tutup Panel 3) menampilkan opsi sesuai jenis dokumen yang terbuka:
  kode/teks mentah (tema editor, font, ukuran, tinggi baris, nomor baris, minimap, ligatur, spasi), dokumen
  Markdown/Word/PowerPoint (tema baca Kertas/Sepia/Malam, font, ukuran, tinggi baris, lebar kolom), tabel (ukuran huruf),
  dan gambar (latar). Semua opsi juga ada di **Pengaturan → Dokumen**.
- Saat melihat **teks mentah** atau kode, **Ctrl + roda mouse** memperbesar/memperkecil teks (bukan tampilan aplikasi).

## Pratinjau HTML dan live server

- Panel 3 menampilkan HTML dalam sandbox aman (tanpa skrip dan sumber luar).
- Tombol **Buka di browser** (ikon globe di toolbar) menjalankan live server lokal dan membuka halaman di Browser
  Panel 4: CSS/JS/gambar di folder project ikut dimuat, dan halaman **dimuat ulang otomatis** saat Anda menyimpan berkas.
  Hanya di aplikasi desktop.

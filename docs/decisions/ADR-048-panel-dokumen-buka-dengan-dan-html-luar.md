# ADR-048 — Panel dokumen: "Buka dengan" dan pratinjau HTML dengan sumber luar

Tanggal: 14-11-2026 · Status: diterima (bagian native belum diuji di aplikasi Tauri sungguhan)

## Konteks

Panel 3 hanya punya satu tombol "Buka di aplikasi lain" (aplikasi bawaan sistem). Pratinjau HTML selalu dalam sandbox kosong dan
mewarisi CSP aplikasi sehingga skrip, formulir, dan **semua sumber luar** (CSS/font/gambar CDN) diblokir tanpa jalan keluar.

## Keputusan

- **Tab dokumen** memakai ikon jenis berkas (`FileTypeIcon`), bilah tanpa scrollbar, dan **menu klik kanan** yang sama dengan tab
  sesi (tutup tab, lainnya, kanan, kiri, semua). Tab yang belum disimpan dikonfirmasi **satu kali** untuk seluruh kelompok.
- **Buka dengan**: tombol membuka popover berisi aplikasi terdaftar untuk ekstensi itu (nama, ikon asli dari exe, lencana
  "Bawaan"), "Aplikasi bawaan sistem", dan "Tampilkan di File Explorer". Daftar dibaca Rust (`openwith.rs` + `openwith.ps1`,
  hanya BACA registri; ikon diekstrak dari exe jadi PNG). Berkas teks/kode juga menawarkan VS Code dan Notepad bila terpasang.
  Peluncuran (`fs_open_with`) hanya menerima aplikasi yang ada di daftar ekstensi itu. Hasil di-cache per ekstensi.
- **HTML dengan sumber luar** — opt-in per tab ("Izinkan skrip dan sumber luar"), default mati: HTML disajikan lewat skema
  Tauri `zpreview://` dengan CSP sendiri (https + skrip diizinkan, tanpa form/base) di iframe
  `sandbox="allow-scripts allow-popups"` **tanpa** `allow-same-origin` (origin buram, tidak menjangkau aplikasi/berkas).

- **Buka di browser**: tombol di pratinjau HTML membuka dokumen yang sama di Browser Panel 4 lewat URL `zpreview` (skrip dan sumber https berfungsi di sana; sumber relatif seperti CSS di sebelah berkas tidak ikut). Hanya desktop; belum diuji di Tauri sungguhan.

## Konsekuensi / batas

- Aplikasi UWP (mis. Foto) tidak punya jalur exe di registri → tidak muncul di daftar; tersedia lewat "Aplikasi bawaan sistem".
- Membaca daftar bisa makan beberapa detik pertama kali (PowerShell + ikon); ada spinner dan cache.
- Hanya Windows; platform lain mengembalikan daftar kosong (ADR-027). Di build web tombol dan opsi dinonaktifkan dengan alasan.
- Markdown tetap tidak memuat gambar luar (disengaja).
- Skema `zpreview`, daftar aplikasi, dan peluncuran **belum diuji di aplikasi Tauri sungguhan** (tes unit Rust + komponen saja).

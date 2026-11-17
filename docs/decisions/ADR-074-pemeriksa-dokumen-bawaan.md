# ADR-074 — Pemeriksa dokumen bawaan di biner (`Zandor office-check`)

- **Tanggal:** 17-11-2026
- **Status:** diterima (menggantikan bagian pemeriksa `office_check.py` di ADR-070)

## Konteks

Skill bawaan dokumen (ADR-070) memakai `office_check.py` untuk membaca dan memverifikasi DOCX/XLSX/PPTX/PDF. Python
tidak dibundel, jadi di mesin tanpa Python bahkan membaca isi dokumen pun gagal. Dua pilihan dipertimbangkan:
membundel Python portabel beserta python-docx/openpyxl/python-pptx (installer membesar puluhan MB, plus urusan lisensi
dan pembaruan), atau memindahkan pemeriksa ke biner aplikasi. Pemilik memilih yang kedua.

## Keputusan

- Pemeriksa ditulis ulang di Rust (`src-tauri/src/office_check.rs`, crate `zip` + `roxmltree`) dengan perilaku dan
  format laporan JSON yang sama: ZIP/XML/relasi internal OOXML, info per jenis (paragraf/tabel/section, lembar/sel/
  rumus, slide/catatan), teks untuk `--text` dan `--expect`; PDF: header, `%%EOF`, perkiraan halaman, peringatan
  enkripsi. Kode keluar 0 valid, 1 tidak valid/ekspektasi hilang, 2 pemakaian salah.
- Dijalankan sebagai subperintah biner aplikasi: `"<Zandor.exe>" office-check <berkas> ...`. `main.rs` menanganinya
  sebelum Tauri dan single-instance, jadi tidak membuka jendela dan aman saat aplikasi sedang terbuka.
- `SKILL.md` memakai placeholder `{{OFFICE_CHECK}}`; pemasang skill menggantinya dengan path `current_exe()`.
  `office_check.py` lama dihapus dari folder skill saat pemasangan.
- Bagian "Lingkungan" skill kini menjelaskan bahwa membaca dan verifikasi tidak butuh Python; Python hanya untuk
  mengubah/membuat berkas, dan bila tidak ada agent mengatakannya dengan jujur.
- Pengaman: batas 64 MB per anggota ZIP (zip bomb), DTD XML ditolak.

## Konsekuensi

- Mengubah dan membuat dokumen **masih butuh Python** + pustakanya. Membundel Python tetap terbuka bila nanti perlu.
- Path biner ditanam di `SKILL.md`; bila aplikasi dipindah, path diperbarui otomatis saat engine dimulai berikutnya.
- Teruji: build debug dan rilis (subsystem `windows`) mengeluarkan JSON dan kode keluar yang benar lewat pipa bash/PowerShell. Belum diuji: dipanggil agent dengan model nyata.

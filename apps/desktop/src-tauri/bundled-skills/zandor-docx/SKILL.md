---
name: zandor-docx
description: "Membuat, membaca, atau mengubah dokumen Word (.docx): ganti teks, tambah/ubah paragraf, tabel, gambar, header/footer, gaya. Pakai setiap kali user menyebut .docx, dokumen Word, atau meminta laporan/surat dalam format Word."
---

# Dokumen Word (.docx)

Skill bawaan Zandor. Folder skill: `{{SKILL_DIR}}`.

## Prinsip

- **Jangan menimpa berkas asli tanpa cadangan.** Sebelum mengubah, salin ke `<nama>.bak.docx` di folder yang sama
  (kecuali user menolak), atau tulis hasil ke berkas baru bila user tidak minta menimpa.
- Jangan pernah menulis `.docx` dengan editor teks biasa: berkas ini paket ZIP berisi XML.
- Pertahankan gaya yang ada: ubah teks di dalam _run_ yang sudah ada, jangan hapus lalu buat ulang paragraf.

## Lingkungan

Membaca struktur/isi dan memverifikasi hasil **tidak butuh Python**: pakai pemeriksa bawaan Zandor
`{{OFFICE_CHECK}} <berkas> [--text] [--expect "teks"]` (laporan JSON). Python hanya dibutuhkan untuk mengubah atau
membuat berkas; bila tugasnya hanya membaca, lewati langkah di bawah.

1. Cari Python: `python --version`, lalu `py -3 --version` (Windows), lalu `python3 --version`.
   Bila tidak ada, berhenti dan katakan jujur bahwa mengubah berkas butuh Python 3 (membaca dan verifikasi tetap
   bisa); sarankan python.org atau `winget install Python.Python.3.12`. Jangan memasang tanpa persetujuan user.
2. Cek pustaka: `python -c "import docx"`. Bila gagal, minta izin lalu `python -m pip install --user python-docx`.

## Alur

1. **Baca dulu**: `{{OFFICE_CHECK}} berkas.docx --text` untuk melihat struktur dan isi.
2. **Ubah** dengan skrip Python sementara (simpan di folder temp, bukan di project), memakai `python-docx`:

   ```python
   from docx import Document
   d = Document("in.docx")
   for p in d.paragraphs:
       for r in p.runs:
           if "LAMA" in r.text:
               r.text = r.text.replace("LAMA", "BARU")
   # tabel: d.tables[i].cell(r, c).text ; tambah baris: t.add_row()
   d.save("out.docx")
   ```

   Teks yang terpecah di beberapa _run_ tidak tertangkap penggantian per run: gabungkan run paragraf itu dengan
   hati-hati (pertahankan format run pertama) bila perlu. Header/footer: `d.sections[i].header.paragraphs`.
3. **Verifikasi wajib**: `{{OFFICE_CHECK}} out.docx --expect "BARU"`. Hasil harus
   `"valid": true` dan `expect_missing` kosong. Bila gagal, perbaiki; jangan menyerahkan berkas rusak.
4. **Laporkan**: path berkas hasil, apa yang berubah, path cadangan, dan batasan yang belum diverifikasi
   (tampilan visual tidak diperiksa kecuali dibuka di Word/LibreOffice).

## Konversi

- DOCX → PDF hanya bila LibreOffice ada (`soffice --headless --convert-to pdf berkas.docx`); jika tidak, katakan jujur.
- `.doc` lama: minta user menyimpan ulang sebagai `.docx`, atau konversi dengan LibreOffice bila tersedia.

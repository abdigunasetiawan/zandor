---
name: zandor-pptx
description: "Membuat, membaca, atau mengubah presentasi PowerPoint (.pptx): teks slide, tambah/hapus/urutkan slide, gambar, tabel, catatan pembicara. Pakai setiap kali user menyebut PowerPoint, .pptx, slide, atau presentasi sebagai berkas."
---

# Presentasi PowerPoint (.pptx)

Skill bawaan Zandor. Folder skill: `{{SKILL_DIR}}`.

## Prinsip

- **Cadangan dulu**: salin ke `<nama>.bak.pptx` sebelum menimpa, atau tulis ke berkas baru.
- Saat mengedit presentasi yang ada, **pakai tata letak (layout) dan placeholder milik template**, jangan
  menempatkan kotak teks dengan koordinat sembarang.
- Ubah teks di _run_ yang ada agar font/warna tetap.
- Jaga teks muat di slide: maks ±6 butir per slide, butir pendek. Bila teks panjang, pecah menjadi slide baru.

## Lingkungan

Membaca struktur/isi dan memverifikasi hasil **tidak butuh Python**: pakai pemeriksa bawaan Zandor
`{{OFFICE_CHECK}} <berkas> [--text] [--expect "teks"]` (laporan JSON). Python hanya dibutuhkan untuk mengubah atau
membuat berkas; bila tugasnya hanya membaca, lewati langkah di bawah.

1. Cari Python: `python --version`, `py -3 --version`, `python3 --version`. Bila tidak ada, berhenti dan katakan jujur bahwa
   mengubah berkas butuh Python 3 (membaca dan verifikasi tetap bisa).
2. `python -c "import pptx"`; bila gagal, minta izin lalu `python -m pip install --user python-pptx`.

## Alur

1. **Baca dulu**: `{{OFFICE_CHECK}} deck.pptx --text` (jumlah slide, catatan, teks).
2. **Ubah** (skrip sementara di folder temp):

   ```python
   from pptx import Presentation
   from pptx.util import Pt
   prs = Presentation("deck.pptx")
   s = prs.slides.add_slide(prs.slide_layouts[1])   # Judul + Isi
   s.shapes.title.text = "Ringkasan"
   body = s.placeholders[1].text_frame
   body.text = "Butir pertama"
   body.add_paragraph().text = "Butir kedua"
   s.notes_slide.notes_text_frame.text = "Catatan pembicara"
   prs.save("deck.pptx")
   ```

   Mengurutkan ulang slide: pindahkan elemen di `prs.slides._sldIdLst`. Menghapus slide: hapus `sldId` dan
   relasinya (`prs.part.drop_rel(rId)`).
3. **Verifikasi wajib**: `{{OFFICE_CHECK}} deck.pptx --expect "Ringkasan"` → `valid: true`.
   Bila LibreOffice ada dan model bisa melihat gambar, render ke PDF/PNG untuk memeriksa teks meluap:
   `soffice --headless --convert-to pdf --outdir <temp> deck.pptx`.
4. **Laporkan**: slide yang berubah, cadangan, dan bahwa tata letak visual belum diperiksa bila tidak dirender.

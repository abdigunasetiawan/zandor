---
name: zandor-pdf
description: "Membaca, mengekstrak teks/tabel, menggabung, memisah, memutar, memberi watermark, mengisi formulir, atau membuat PDF. Pakai setiap kali user menyebut berkas .pdf atau meminta hasil dalam format PDF."
---

# Dokumen PDF

Skill bawaan Zandor. Folder skill: `{{SKILL_DIR}}`.

## Prinsip

- PDF **bukan format yang bisa diedit bebas**. Mengganti kalimat di tengah halaman tidak andal; bila user minta
  itu, jelaskan dan tawarkan: edit sumbernya (DOCX/Markdown) lalu ekspor ulang, atau tambahkan anotasi/overlay.
- **Jangan menimpa berkas asli**: tulis hasil ke berkas baru (mis. `laporan-gabungan.pdf`) kecuali user minta.
- PDF terenkripsi: minta kata sandi kepada user; jangan mencoba membobolnya.

## Lingkungan

Membaca struktur/isi dan memverifikasi hasil **tidak butuh Python**: pakai pemeriksa bawaan Zandor
`{{OFFICE_CHECK}} <berkas> [--text] [--expect "teks"]` (laporan JSON). Python hanya dibutuhkan untuk mengubah atau
membuat berkas; bila tugasnya hanya membaca, lewati langkah di bawah.

1. Cari Python: `python --version`, `py -3 --version`, `python3 --version`. Bila tidak ada, berhenti dan katakan jujur bahwa
   mengubah berkas butuh Python 3 (membaca dan verifikasi tetap bisa).
2. Pustaka sesuai tugas (minta izin sebelum memasang dengan `python -m pip install --user ...`):
   - `pypdf` — gabung, pisah, putar, metadata, formulir, ekstrak teks sederhana.
   - `pdfplumber` — ekstrak teks dengan tata letak dan tabel.
   - `reportlab` — membuat PDF baru atau lapisan watermark.
   - PDF hasil pindaian (tanpa teks) butuh OCR (`ocrmypdf`/Tesseract); katakan jujur bila tidak tersedia.

## Resep

```python
from pypdf import PdfReader, PdfWriter
r = PdfReader("in.pdf"); print(len(r.pages)); print(r.pages[0].extract_text())

w = PdfWriter()                                  # gabung
for f in ["a.pdf", "b.pdf"]:
    for p in PdfReader(f).pages: w.add_page(p)
w.write("gabung.pdf")

w = PdfWriter(); w.append("in.pdf", pages=(0, 3)); w.write("hal-1-3.pdf")   # pisah
p = r.pages[0]; p.rotate(90)                                                # putar

w = PdfWriter(clone_from="form.pdf")                                         # isi formulir
print(r.get_fields().keys())
w.update_page_form_field_values(w.pages[0], {"nama": "Budi"}, auto_regenerate=False)
w.write("form-terisi.pdf")
```

Tabel: `pdfplumber.open("in.pdf").pages[0].extract_tables()`.

## Verifikasi wajib

1. `{{OFFICE_CHECK}} hasil.pdf` → `valid: true` (header, trailer, perkiraan halaman).
2. Buka dengan `pypdf` dan cocokkan jumlah halaman serta teks kunci yang diharapkan.
3. Laporkan path hasil, jumlah halaman, dan batasan (mis. tampilan visual belum diperiksa).

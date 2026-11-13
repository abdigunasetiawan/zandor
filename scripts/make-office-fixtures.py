"""Membuat fixture Office/ipynb untuk uji penampil dokumen (Fase 11). Berkas hasilnya DI-COMMIT di
packages/core/src/docs/fixtures/ (kecil). Jalankan ulang hanya bila fixture perlu diubah:
    pip install python-docx openpyxl python-pptx
    python scripts/make-office-fixtures.py
Berkas ditulis oleh pustaka sungguhan (bukan dirakit tangan) supaya parser diuji terhadap OOXML nyata."""
import datetime
import io
import json
import struct
import zlib
from pathlib import Path

import openpyxl
from docx import Document
from docx.shared import Inches
from pptx import Presentation
from pptx.util import Inches as PInches

OUT = Path(__file__).resolve().parent.parent / "packages" / "core" / "src" / "docs" / "fixtures"
OUT.mkdir(parents=True, exist_ok=True)


def png(w=4, h=4, rgb=(200, 30, 30)):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))

    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


# ---------- DOCX ----------
d = Document()
d.add_heading("Judul Laporan", level=1)
p = d.add_paragraph("Paragraf biasa dengan ")
p.add_run("tebal").bold = True
p.add_run(" dan ")
p.add_run("miring").italic = True
p.add_run(" serta ")
r = p.add_run("garis bawah")
r.underline = True
d.add_heading("Sub judul", level=2)
d.add_paragraph("butir satu", style="List Bullet")
d.add_paragraph("butir dua", style="List Bullet")
d.add_paragraph("langkah satu", style="List Number")
d.add_paragraph("langkah dua", style="List Number")
t = d.add_table(rows=2, cols=3)
for i, row in enumerate([["Nama", "Umur", "Kota"], ["Budi", "30", "Jakarta"]]):
    for j, v in enumerate(row):
        t.cell(i, j).text = v
d.add_picture(io.BytesIO(png()), width=Inches(1))
d.add_paragraph("Teks <b>bukan</b> markup & \"kutip\" aman?")
d.save(OUT / "contoh.docx")

# ---------- XLSX ----------
wb = openpyxl.Workbook()
ws = wb.active
ws.title = "Data"
ws.append(["Produk", "Jumlah", "Harga", "Aktif", "Tanggal"])
ws.append(["Apel", 3, 1500.5, True, datetime.date(2026, 11, 14)])
ws.append(["Jeruk <manis>", 10, 2000, False, datetime.date(2026, 1, 2)])
ws["F2"] = "=B2*C2"
ws2 = wb.create_sheet("Ringkasan")
ws2["A1"] = "Total"
ws2["B1"] = 42
wb.create_sheet("Kosong")
wb.save(OUT / "contoh.xlsx")

# ---------- PPTX ----------
prs = Presentation()
s1 = prs.slides.add_slide(prs.slide_layouts[0])
s1.shapes.title.text = "Presentasi Uji"
s1.placeholders[1].text = "Subjudul slide pertama"
s2 = prs.slides.add_slide(prs.slide_layouts[1])
s2.shapes.title.text = "Poin Penting"
tf = s2.placeholders[1].text_frame
tf.text = "Poin satu"
tf.add_paragraph().text = "Poin dua"
s3 = prs.slides.add_slide(prs.slide_layouts[5])
s3.shapes.title.text = "Gambar"
s3.shapes.add_picture(io.BytesIO(png(8, 8, (30, 60, 200))), PInches(1), PInches(2), width=PInches(1))
prs.save(OUT / "contoh.pptx")

# ---------- IPYNB ----------
nb = {
    "nbformat": 4,
    "nbformat_minor": 5,
    "metadata": {"kernelspec": {"name": "python3", "display_name": "Python 3", "language": "python"}, "language_info": {"name": "python"}},
    "cells": [
        {"cell_type": "markdown", "metadata": {}, "source": ["# Judul Notebook\n", "Teks **tebal**."]},
        {
            "cell_type": "code",
            "execution_count": 1,
            "metadata": {},
            "source": ["print('halo')\n", "1 + 1"],
            "outputs": [
                {"output_type": "stream", "name": "stdout", "text": ["halo\n"]},
                {"output_type": "execute_result", "execution_count": 1, "metadata": {}, "data": {"text/plain": ["2"]}},
            ],
        },
        {
            "cell_type": "code",
            "execution_count": 2,
            "metadata": {},
            "source": ["plot()"],
            "outputs": [
                {
                    "output_type": "display_data",
                    "metadata": {},
                    "data": {
                        "image/png": __import__("base64").b64encode(png()).decode(),
                        "text/html": ["<script>alert(1)</script><b>html</b>"],
                        "text/plain": ["<Figure>"],
                    },
                }
            ],
        },
        {
            "cell_type": "code",
            "execution_count": 3,
            "metadata": {},
            "source": ["1/0"],
            "outputs": [{"output_type": "error", "ename": "ZeroDivisionError", "evalue": "division by zero", "traceback": ["\u001b[31mTraceback\u001b[0m", "ZeroDivisionError: division by zero"]}],
        },
        {"cell_type": "raw", "metadata": {}, "source": ["catatan mentah"]},
    ],
}
(OUT / "contoh.ipynb").write_text(json.dumps(nb, indent=1), encoding="utf8")
print("ok", sorted(p.name for p in OUT.iterdir()))

"""Tes pemeriksa skill bawaan dokumen (ADR-070). Jalankan: python -m unittest discover apps/desktop/src-tauri/bundled-skills
Butuh python-docx, openpyxl, python-pptx, pypdf (dilewati bila tidak ada)."""
import json
import os
import subprocess
import sys
import tempfile
import unittest
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
CHECK = os.path.join(HERE, "office_check.py")


def run(*args):
    p = subprocess.run([sys.executable, CHECK, *args], capture_output=True, text=True, encoding="utf-8")
    return p.returncode, json.loads(p.stdout)


class OfficeCheck(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()

    def path(self, name):
        return os.path.join(self.tmp, name)

    def test_docx_edit_flow(self):
        docx = __import__("docx")
        d = docx.Document()
        d.add_paragraph("Halo LAMA")
        t = d.add_table(rows=1, cols=2)
        t.cell(0, 0).text = "sel"
        d.sections[0].header.paragraphs[0].text = "Kop"
        f = self.path("a.docx")
        d.save(f)
        d = docx.Document(f)
        for r in d.paragraphs[0].runs:
            r.text = r.text.replace("LAMA", "BARU")
        d.save(f)
        code, rep = run(f, "--expect", "Halo BARU", "--expect", "Kop", "--expect", "sel")
        self.assertEqual(code, 0, rep)
        self.assertEqual(rep["info"]["tables"], 1)
        code, rep = run(f, "--expect", "LAMA")
        self.assertEqual(code, 1)
        self.assertEqual(rep["expect_missing"], ["LAMA"])

    def test_xlsx_formula_and_strings(self):
        openpyxl = __import__("openpyxl")
        wb = openpyxl.Workbook()
        ws = wb.active
        ws.append(["Barang", "Qty", "Harga"])
        ws.append(["Pena", 2, 1500])
        ws["D2"] = "=B2*C2"
        f = self.path("b.xlsx")
        wb.save(f)
        code, rep = run(f, "--expect", "Pena")
        self.assertEqual(code, 0, rep)
        self.assertEqual(rep["info"]["formulas"], 1)
        self.assertTrue(any("rumus" in w for w in rep["warnings"]))

    def test_pptx_slides(self):
        pptx = __import__("pptx")
        prs = pptx.Presentation()
        s = prs.slides.add_slide(prs.slide_layouts[1])
        s.shapes.title.text = "Ringkasan"
        s.notes_slide.notes_text_frame.text = "catatan"
        f = self.path("c.pptx")
        prs.save(f)
        code, rep = run(f, "--expect", "Ringkasan")
        self.assertEqual(code, 0, rep)
        self.assertEqual(rep["info"]["slides"], 1)
        self.assertEqual(rep["info"]["notes"], 1)

    def test_pdf(self):
        pypdf = __import__("pypdf")
        w = pypdf.PdfWriter()
        w.add_blank_page(200, 200)
        w.add_blank_page(200, 200)
        f = self.path("d.pdf")
        with open(f, "wb") as fh:
            w.write(fh)
        code, rep = run(f)
        self.assertEqual(code, 0, rep)
        self.assertEqual(rep["info"]["pages_estimate"], 2)
        with open(f, "rb") as fh:
            data = fh.read()
        with open(f, "wb") as fh:
            fh.write(data[: len(data) // 2])
        code, rep = run(f)
        self.assertEqual(code, 1)

    def test_corrupt_and_broken_rel(self):
        f = self.path("x.docx")
        with open(f, "wb") as fh:
            fh.write(b"bukan zip")
        code, rep = run(f)
        self.assertEqual(code, 1)
        self.assertFalse(rep["valid"])
        g = self.path("y.docx")
        with zipfile.ZipFile(g, "w") as z:
            z.writestr("[Content_Types].xml", "<Types/>")
            z.writestr("word/document.xml", "<w:document xmlns:w='x'><w:body><w:p><w:t>a</w:t></w:p></w:body></w:document>")
            z.writestr(
                "word/_rels/document.xml.rels",
                "<Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'>"
                "<Relationship Id='r1' Type='t' Target='media/hilang.png'/></Relationships>",
            )
        code, rep = run(g)
        self.assertEqual(code, 1)
        self.assertTrue(any("hilang.png" in e for e in rep["errors"]), rep)


def _skip_missing():
    for mod, test in [("docx", "test_docx_edit_flow"), ("openpyxl", "test_xlsx_formula_and_strings"),
                      ("pptx", "test_pptx_slides"), ("pypdf", "test_pdf")]:
        try:
            __import__(mod)
        except ImportError:
            setattr(OfficeCheck, test, unittest.skip(f"{mod} tidak terpasang")(getattr(OfficeCheck, test)))


_skip_missing()

if __name__ == "__main__":
    unittest.main()

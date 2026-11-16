#!/usr/bin/env python3
"""Pemeriksa struktur dokumen Zandor (hanya pustaka standar Python).

Pemakaian:
    python office_check.py <berkas> [--expect "teks"]... [--text]

Memeriksa DOCX/XLSX/PPTX (ZIP + XML + relasi internal) dan PDF (header, trailer, jumlah halaman kasar).
Mencetak satu laporan JSON. Kode keluar 0 bila valid dan semua --expect ditemukan, 1 bila tidak.
Ini BUKAN pemeriksa tampilan visual dan tidak menghitung ulang rumus.
"""
import json
import posixpath
import re
import sys
import zipfile
import xml.etree.ElementTree as ET

NS_REL = "http://schemas.openxmlformats.org/package/2006/relationships"
MAIN = {
    "docx": "word/document.xml",
    "xlsx": "xl/workbook.xml",
    "pptx": "ppt/presentation.xml",
}


def local(tag):
    return tag.rsplit("}", 1)[-1]


def texts(root, name):
    return [el.text or "" for el in root.iter() if local(el.tag) == name]


def check_ooxml(path, kind, report):
    try:
        z = zipfile.ZipFile(path)
    except zipfile.BadZipFile:
        report["errors"].append("bukan paket ZIP (berkas rusak atau terenkripsi)")
        return ""
    with z:
        names = set(z.namelist())
        bad = z.testzip()
        if bad:
            report["errors"].append(f"anggota ZIP rusak: {bad}")
            return ""
        if "[Content_Types].xml" not in names:
            report["errors"].append("[Content_Types].xml tidak ada")
        if MAIN[kind] not in names:
            report["errors"].append(f"bagian utama {MAIN[kind]} tidak ada")
        parsed = {}
        for n in sorted(names):
            if n.endswith(".xml") or n.endswith(".rels"):
                try:
                    parsed[n] = ET.fromstring(z.read(n))
                except ET.ParseError as e:
                    report["errors"].append(f"XML tidak valid di {n}: {e}")
        # Relasi internal harus menunjuk bagian yang ada.
        for n, root in parsed.items():
            if not n.endswith(".rels"):
                continue
            base = posixpath.dirname(posixpath.dirname(n))
            for rel in root.iter(f"{{{NS_REL}}}Relationship"):
                if rel.get("TargetMode") == "External":
                    continue
                t = rel.get("Target", "")
                full = t.lstrip("/") if t.startswith("/") else posixpath.normpath(posixpath.join(base, t))
                if full not in names:
                    report["errors"].append(f"relasi rusak di {n}: {t}")
        text = []
        info = report["info"]
        if kind == "docx" and "word/document.xml" in parsed:
            doc = parsed["word/document.xml"]
            paras = [el for el in doc.iter() if local(el.tag) == "p"]
            info["paragraphs"] = len(paras)
            info["tables"] = sum(1 for el in doc.iter() if local(el.tag) == "tbl")
            info["sections"] = sum(1 for el in doc.iter() if local(el.tag) == "sectPr")
            for p in paras:
                text.append("".join(texts(p, "t")))
            for n in sorted(parsed):
                if re.match(r"word/(header|footer|footnotes|endnotes)\d*\.xml$", n):
                    text.extend(texts(parsed[n], "t"))
        elif kind == "xlsx" and "xl/workbook.xml" in parsed:
            sheets = [el.get("name") for el in parsed["xl/workbook.xml"].iter() if local(el.tag) == "sheet"]
            info["sheets"] = sheets
            shared = []
            if "xl/sharedStrings.xml" in parsed:
                for si in parsed["xl/sharedStrings.xml"]:
                    shared.append("".join(texts(si, "t")))
            formulas = cells = 0
            for n in sorted(parsed):
                if not re.match(r"xl/worksheets/sheet\d+\.xml$", n):
                    continue
                for c in parsed[n].iter():
                    if local(c.tag) != "c":
                        continue
                    cells += 1
                    v = next((x.text for x in c if local(x.tag) == "v"), None)
                    if any(local(x.tag) == "f" for x in c):
                        formulas += 1
                    if c.get("t") == "s" and v is not None and v.isdigit() and int(v) < len(shared):
                        text.append(shared[int(v)])
                    elif c.get("t") == "inlineStr":
                        text.append("".join(texts(c, "t")))
                    elif v is not None:
                        text.append(v)
            info["cells"] = cells
            info["formulas"] = formulas
            if formulas:
                report["warnings"].append(
                    "rumus tidak dihitung ulang oleh pemeriksa; nilai cache bisa usang sampai dibuka di Excel/LibreOffice"
                )
        elif kind == "pptx":
            slides = sorted(
                (n for n in parsed if re.match(r"ppt/slides/slide\d+\.xml$", n)),
                key=lambda s: int(re.findall(r"\d+", s)[-1]),
            )
            info["slides"] = len(slides)
            for n in slides:
                text.extend(texts(parsed[n], "t"))
            info["notes"] = sum(1 for n in parsed if re.match(r"ppt/notesSlides/notesSlide\d+\.xml$", n))
        return "\n".join(t for t in text if t)


def check_pdf(path, report):
    data = open(path, "rb").read()
    if not data.startswith(b"%PDF-"):
        report["errors"].append("header %PDF- tidak ada")
        return ""
    report["info"]["version"] = data[5:8].decode("latin-1", "replace")
    if b"%%EOF" not in data[-2048:]:
        report["errors"].append("penanda %%EOF tidak ada di akhir berkas (mungkin terpotong)")
    report["info"]["pages_estimate"] = len(re.findall(rb"/Type\s*/Page(?![a-zA-Z])", data))
    if b"/Encrypt" in data:
        report["warnings"].append("PDF terenkripsi")
    report["warnings"].append("teks PDF tidak diekstrak oleh pemeriksa stdlib; pakai pypdf/pdfplumber untuk isi")
    return ""


def main(argv):
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 2
    path = argv[0]
    expects, show_text, i = [], False, 1
    while i < len(argv):
        if argv[i] == "--expect" and i + 1 < len(argv):
            expects.append(argv[i + 1])
            i += 2
        elif argv[i] == "--text":
            show_text = True
            i += 1
        else:
            print(f"argumen tidak dikenal: {argv[i]}", file=sys.stderr)
            return 2
    ext = path.rsplit(".", 1)[-1].lower()
    kind = {"docx": "docx", "docm": "docx", "xlsx": "xlsx", "xlsm": "xlsx", "pptx": "pptx", "pptm": "pptx"}.get(ext, ext)
    report = {"file": path, "kind": kind, "valid": False, "errors": [], "warnings": [], "info": {}}
    try:
        if kind in MAIN:
            text = check_ooxml(path, kind, report)
        elif kind == "pdf":
            text = check_pdf(path, report)
        else:
            report["errors"].append(f"jenis tidak didukung: .{ext}")
            text = ""
    except OSError as e:
        report["errors"].append(f"gagal membaca berkas: {e}")
        text = ""
    missing = [e for e in expects if e not in text]
    if expects:
        report["expect_missing"] = missing
    if show_text:
        report["text"] = text[:20000]
    report["valid"] = not report["errors"]
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["valid"] and not missing else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

// @vitest-environment jsdom
// Parser diuji terhadap berkas yang ditulis pustaka sungguhan (python-docx/openpyxl/python-pptx; lihat
// scripts/make-office-fixtures.py), bukan XML rakitan tangan.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { strToU8, zipSync } from "fflate";
import { describe, expect, test } from "vitest";
import { parseNotebook } from "./notebook";
import { type DocxBlock, parseDocx, parsePptx, parseXlsx } from "./office";

const dir = join(import.meta.dirname, "fixtures");
const fixture = (name: string) => new Uint8Array(readFileSync(join(dir, name)));
const text = (b: DocxBlock) => (b.type === "p" ? b.runs.map((r) => r.text).join("") : "");

describe("parseDocx", () => {
  const r = parseDocx(fixture("contoh.docx"));
  const paras = r.blocks.filter((b) => b.type === "p");

  test("judul/heading dikenali per level", () => {
    expect(paras.find((b) => text(b) === "Judul Laporan")).toMatchObject({ style: "h1" });
    expect(paras.find((b) => text(b) === "Sub judul")).toMatchObject({ style: "h2" });
  });

  test("format run: tebal, miring, garis bawah tetap terpisah dari teks biasa", () => {
    const p = paras.find((b) => text(b).startsWith("Paragraf biasa"));
    expect(p?.type === "p" && p.runs.map((x) => [x.text, !!x.bold, !!x.italic, !!x.underline])).toEqual([
      ["Paragraf biasa dengan ", false, false, false],
      ["tebal", true, false, false],
      [" dan ", false, false, false],
      ["miring", false, true, false],
      [" serta ", false, false, false],
      ["garis bawah", false, false, true],
    ]);
  });

  test("daftar butir dan bernomor", () => {
    const li = paras.filter((b) => b.type === "p" && b.list);
    expect(li.map((b) => [text(b), b.type === "p" && b.list?.ordered])).toEqual([
      ["butir satu", false],
      ["butir dua", false],
      ["langkah satu", true],
      ["langkah dua", true],
    ]);
  });

  test("tabel: baris × sel × blok", () => {
    const t = r.blocks.find((b) => b.type === "table");
    expect(t?.type === "table" && t.rows.map((row) => row.map((cell) => cell.map(text).join("")))).toEqual([
      ["Nama", "Umur", "Kota"],
      ["Budi", "30", "Jakarta"],
    ]);
  });

  test("gambar tertanam menjadi data URL png", () => {
    const img = paras.flatMap((b) => (b.type === "p" ? b.runs : [])).find((x) => x.image);
    expect(img?.image).toMatch(/^data:image\/png;base64,/);
  });

  test("teks yang mirip markup tetap teks biasa (tidak ada HTML mentah)", () => {
    expect(paras.some((b) => text(b) === 'Teks <b>bukan</b> markup & "kutip" aman?')).toBe(true);
  });

  test("bukan zip → galat jelas; zip tanpa document.xml → galat", () => {
    expect(() => parseDocx(new Uint8Array([1, 2, 3]))).toThrow(/bukan paket Office/);
    expect(() => parseDocx(zipSync({ "x.txt": strToU8("a") }))).toThrow(/document.xml/);
  });
});

describe("parseXlsx", () => {
  const r = parseXlsx(fixture("contoh.xlsx"));

  test("sheet berurutan sesuai buku kerja", () => {
    expect(r.sheets.map((s) => s.name)).toEqual(["Data", "Ringkasan", "Kosong"]);
  });

  test("sel: teks (shared string), angka, boolean, tanggal, rumus tanpa nilai tersimpan", () => {
    const rows = r.sheets[0]?.rows ?? [];
    expect(rows[0]).toEqual(["Produk", "Jumlah", "Harga", "Aktif", "Tanggal", ""]);
    expect(rows[1]).toEqual(["Apel", "3", "1500.5", "TRUE", "2026-11-14", "=B2*C2"]);
    expect(rows[2]?.slice(0, 5)).toEqual(["Jeruk <manis>", "10", "2000", "FALSE", "2026-01-02"]);
  });

  test("sheet lain dan sheet kosong", () => {
    expect(r.sheets[1]?.rows).toEqual([["Total", "42"]]);
    expect(r.sheets[2]?.rows).toEqual([]);
    expect(r.sheets.every((s) => !s.truncated)).toBe(true);
  });

  test("galat: bukan zip, tanpa workbook", () => {
    expect(() => parseXlsx(new Uint8Array([1]))).toThrow(/bukan paket Office/);
    expect(() => parseXlsx(zipSync({ "a.txt": strToU8("a") }))).toThrow(/workbook.xml/);
  });

  test("batas baris/kolom ditandai truncated", () => {
    const rows = Array.from(
      { length: 2005 },
      (_, i) => `<row r="${i + 1}"><c r="A${i + 1}"><v>${i}</v></c></row>`,
    ).join("");
    const pkg = zipSync({
      "xl/workbook.xml": strToU8(
        '<workbook xmlns="x" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>',
      ),
      "xl/_rels/workbook.xml.rels": strToU8(
        '<Relationships xmlns="x"><Relationship Id="rId1" Target="worksheets/sheet1.xml" Type="t"/></Relationships>',
      ),
      "xl/worksheets/sheet1.xml": strToU8(`<worksheet xmlns="x"><sheetData>${rows}</sheetData></worksheet>`),
    });
    const s = parseXlsx(pkg).sheets[0];
    expect(s?.rows.length).toBe(2000);
    expect(s?.truncated).toBe(true);
  });
});

describe("parsePptx", () => {
  const r = parsePptx(fixture("contoh.pptx"));

  test("urutan slide, judul, dan teks isi", () => {
    expect(r.slides.map((s) => s.title)).toEqual(["Presentasi Uji", "Poin Penting", "Gambar"]);
    expect(r.slides[0]?.paragraphs).toEqual(["Subjudul slide pertama"]);
    expect(r.slides[1]?.paragraphs).toEqual(["Poin satu", "Poin dua"]);
  });

  test("gambar slide menjadi data URL", () => {
    expect(r.slides[2]?.images[0]).toMatch(/^data:image\/png;base64,/);
    expect(r.slides[0]?.images).toEqual([]);
  });

  test("galat: bukan zip, tanpa presentation.xml", () => {
    expect(() => parsePptx(new Uint8Array([9]))).toThrow(/bukan paket Office/);
    expect(() => parsePptx(zipSync({ "a.txt": strToU8("a") }))).toThrow(/presentation.xml/);
  });
});

describe("parseNotebook", () => {
  const nb = parseNotebook(readFileSync(join(dir, "contoh.ipynb"), "utf8"));

  test("sel markdown, kode, mentah dan bahasa", () => {
    expect(nb.language).toBe("python");
    expect(nb.cells.map((c) => c.type)).toEqual(["markdown", "code", "code", "code", "raw"]);
    expect(nb.cells[0]?.source).toBe("# Judul Notebook\nTeks **tebal**.");
    expect(nb.cells[1]?.source).toBe("print('halo')\n1 + 1");
    expect(nb.cells[1]?.executionCount).toBe(1);
  });

  test("output: stream, hasil, gambar png; HTML/JS tidak dirender", () => {
    expect(nb.cells[1]?.outputs).toEqual([
      { kind: "text", text: "halo\n", stream: "stdout" },
      { kind: "text", text: "2" },
    ]);
    const o = nb.cells[2]?.outputs ?? [];
    expect(o).toHaveLength(1);
    expect(o[0]).toMatchObject({ kind: "image" });
    expect((o[0] as { src: string }).src).toMatch(/^data:image\/png;base64,/);
    expect(JSON.stringify(nb)).not.toContain("<script>");
  });

  test("galat: kode ANSI dibuang dari traceback", () => {
    const e = nb.cells[3]?.outputs[0];
    expect(e).toMatchObject({ kind: "error", name: "ZeroDivisionError", message: "division by zero" });
    expect((e as { traceback: string }).traceback).toBe("TracebackZeroDivisionError: division by zero");
  });

  test("output HTML-saja dilewati dengan alasan; notebook rusak → galat", () => {
    const only = parseNotebook(
      JSON.stringify({
        cells: [
          {
            cell_type: "code",
            source: "x",
            outputs: [{ output_type: "display_data", data: { "text/html": "<b>x</b>" } }],
          },
        ],
      }),
    );
    expect(only.cells[0]?.outputs).toEqual([{ kind: "skipped", reason: "output interaktif/HTML tidak ditampilkan" }]);
    expect(() => parseNotebook("{rusak")).toThrow(/JSON rusak/);
    expect(() => parseNotebook("{}")).toThrow(/tidak ada `cells`/);
  });

  test("gambar base64 tidak valid ditolak", () => {
    const bad = parseNotebook(
      JSON.stringify({
        cells: [
          {
            cell_type: "code",
            source: "",
            outputs: [{ output_type: "display_data", data: { "image/png": "<<bukan base64>>" } }],
          },
        ],
      }),
    );
    expect(bad.cells[0]?.outputs[0]).toMatchObject({ kind: "skipped" });
  });
});

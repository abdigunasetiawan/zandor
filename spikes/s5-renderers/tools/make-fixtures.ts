// Spike S5 — buat file uji ±10 MB per format (PDF, DOCX, XLSX, PPTX) di public/fixtures/.
// Isi: teks banyak + gambar noise (tidak bisa dikompresi) agar ukuran realistis dan render berat.
import { mkdirSync, statSync, writeFileSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { randomBytes } from "node:crypto";
import { join } from "node:path";
import { PDFDocument, StandardFonts } from "pdf-lib";
import * as docx from "docx";
import * as XLSX from "xlsx";
import PptxGenJS from "pptxgenjs";

const out = join(import.meta.dir, "..", "public", "fixtures");
mkdirSync(out, { recursive: true });
const TARGET = 10 * 1024 * 1024;

let seed = 42;
const rnd = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
const words = "zandor agent panel memori sesi provider model engine browser terminal dokumen render tabel grafik project kode".split(" ");
const sentence = (n: number) => Array.from({ length: n }, () => words[Math.floor(rnd() * words.length)]).join(" ");

/** PNG RGB berisi noise acak (±w*h*3 byte, nyaris tidak terkompresi). */
function noisePng(w: number, h: number): Uint8Array {
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) randomBytes(w * 3).copy(raw, y * (w * 3 + 1) + 1);
  const crcT = new Int32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c; });
  const crc = (b: Uint8Array) => { let c = -1; for (const x of b) c = crcT[(c ^ x) & 255] ^ (c >>> 8); return (c ^ -1) >>> 0; };
  const chunk = (t: string, d: Uint8Array) => { const l = Buffer.alloc(4); l.writeUInt32BE(d.length); const td = Buffer.concat([Buffer.from(t), d]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  return new Uint8Array(Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]));
}
const IMG = noisePng(360, 300); // ±324 KB
const IMAGES = Math.round((TARGET * 0.85) / IMG.length); // ±85% ukuran dari gambar

// ---------- PDF ----------
{
  const pdf = await PDFDocument.create();
  const font = await pdf.embedFont(StandardFonts.Helvetica);
  const img = await pdf.embedPng(IMG); // satu objek gambar dipakai ulang → tambah variasi di bawah
  const pages = 400;
  const extra: Awaited<ReturnType<typeof pdf.embedPng>>[] = [];
  for (let i = 0; i < IMAGES; i++) extra.push(await pdf.embedPng(noisePng(360, 300)));
  for (let p = 0; p < pages; p++) {
    const page = pdf.addPage([595, 842]);
    page.drawText(`Halaman ${p + 1} — Zandor S5`, { x: 50, y: 800, size: 16, font });
    for (let l = 0; l < 40; l++) page.drawText(sentence(12), { x: 50, y: 770 - l * 14, size: 10, font });
    if (p < extra.length) page.drawImage(extra[p], { x: 50, y: 60, width: 240, height: 200 });
    else if (p % 10 === 0) page.drawImage(img, { x: 320, y: 60, width: 200, height: 160 });
  }
  writeFileSync(join(out, "sample.pdf"), await pdf.save());
}

// ---------- DOCX ----------
{
  const children: docx.Paragraph[] = [new docx.Paragraph({ text: "Zandor S5 — dokumen uji", heading: docx.HeadingLevel.TITLE })];
  for (let i = 0; i < 1500; i++) {
    if (i % 100 === 0) children.push(new docx.Paragraph({ text: `Bab ${i / 100 + 1}`, heading: docx.HeadingLevel.HEADING_1 }));
    children.push(new docx.Paragraph({ children: [new docx.TextRun({ text: sentence(25), bold: i % 7 === 0 }), new docx.TextRun({ text: " " + sentence(10), italics: true })] }));
    if (i < IMAGES * 2 && i % 2 === 0)
      children.push(new docx.Paragraph({ children: [new docx.ImageRun({ type: "png", data: noisePng(360, 300), transformation: { width: 240, height: 200 } })] }));
  }
  const rows = Array.from({ length: 60 }, (_, r) => new docx.TableRow({ children: Array.from({ length: 5 }, (_, c) => new docx.TableCell({ children: [new docx.Paragraph(r === 0 ? `Kolom ${c + 1}` : sentence(3))] })) }));
  const doc = new docx.Document({ sections: [{ children: [...children, new docx.Table({ rows }) as unknown as docx.Paragraph] }] });
  writeFileSync(join(out, "sample.docx"), await docx.Packer.toBuffer(doc));
}

// ---------- XLSX ----------
{
  const rows: (string | number)[][] = [["ID", "Nama", "Kategori", "Nilai", "Persen", "Tanggal", "Catatan", "Kode"]];
  for (let i = 1; i <= 62_000; i++)
    rows.push([i, sentence(2), words[i % words.length], Math.round(rnd() * 1e6) / 100, rnd(), new Date(2026, 0, 1 + (i % 365)).toISOString().slice(0, 10), sentence(4), Math.floor(rnd() * 1e9).toString(36)]);
  const wb = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(wb, XLSX.utils.aoa_to_sheet(rows), "Data");
  XLSX.utils.book_append_sheet(wb, XLSX.utils.aoa_to_sheet(rows.slice(0, 2000)), "Ringkas");
  writeFileSync(join(out, "sample.xlsx"), XLSX.write(wb, { type: "buffer", bookType: "xlsx", compression: true }));
}

// ---------- PPTX ----------
{
  const pptx = new PptxGenJS();
  const slides = 60;
  for (let s = 0; s < slides; s++) {
    const slide = pptx.addSlide();
    slide.addText(`Slide ${s + 1} — Zandor S5`, { x: 0.5, y: 0.3, w: 9, h: 0.8, fontSize: 28, bold: true, color: "202020" });
    slide.addText(sentence(30), { x: 0.5, y: 1.2, w: 5, h: 3, fontSize: 14 });
    slide.addShape(pptx.ShapeType.roundRect, { x: 6, y: 4, w: 3, h: 1, fill: { color: "3366CC" }, rectRadius: 0.2 });
    if (s < IMAGES) slide.addImage({ data: "data:image/png;base64," + Buffer.from(noisePng(360, 300)).toString("base64"), x: 5.8, y: 1.2, w: 3.6, h: 2.6 });
    if (s % 6 === 0) slide.addTable([["A", "B", "C"], ...Array.from({ length: 4 }, () => [sentence(1), sentence(1), sentence(1)])], { x: 0.5, y: 4.3, w: 5, fontSize: 10 });
  }
  writeFileSync(join(out, "sample.pptx"), (await pptx.write({ outputType: "nodebuffer" })) as Buffer);
}

for (const f of ["sample.pdf", "sample.docx", "sample.xlsx", "sample.pptx"])
  console.log(f.padEnd(12), (statSync(join(out, f)).size / 1024 / 1024).toFixed(2), "MB");

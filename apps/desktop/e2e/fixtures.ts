// Berkas uji sintetis untuk e2e penampil dokumen (dibuat di kode supaya repo tidak membawa biner).

/** PDF valid minimal dengan teks Helvetica; xref dihitung benar. Satu halaman per elemen `pages`. */
export function makePdf(pages: string[]): Uint8Array {
  const objs: string[] = [];
  const kids = pages.map((_, i) => `${3 + i * 2} 0 R`).join(" ");
  objs.push("<< /Type /Catalog /Pages 2 0 R >>");
  objs.push(`<< /Type /Pages /Kids [${kids}] /Count ${pages.length} >>`);
  const fontId = 3 + pages.length * 2;
  pages.forEach((text, i) => {
    const content = 4 + i * 2;
    objs.push(
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents ${content} 0 R /Resources << /Font << /F1 ${fontId} 0 R >> >> >>`,
    );
    const stream = `BT /F1 24 Tf 20 100 Td (${text}) Tj ET`;
    objs.push(`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`);
  });
  objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
  let out = "%PDF-1.4\n";
  const offsets: number[] = [];
  objs.forEach((o, i) => {
    offsets.push(out.length);
    out += `${i + 1} 0 obj\n${o}\nendobj\n`;
  });
  const xref = out.length;
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n`;
  for (const off of offsets) out += `${String(off).padStart(10, "0")} 00000 n \n`;
  out += `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return new TextEncoder().encode(out);
}

/** WAV mono 16-bit, sinus 440 Hz, ~0,3 dtk. */
export function makeWav(): Uint8Array {
  const rate = 8000;
  const n = Math.floor(rate * 0.3);
  const buf = new ArrayBuffer(44 + n * 2);
  const v = new DataView(buf);
  const w = (o: number, s: string) => {
    for (let i = 0; i < s.length; i++) v.setUint8(o + i, s.charCodeAt(i));
  };
  w(0, "RIFF");
  v.setUint32(4, 36 + n * 2, true);
  w(8, "WAVE");
  w(12, "fmt ");
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, rate, true);
  v.setUint32(28, rate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  w(36, "data");
  v.setUint32(40, n * 2, true);
  for (let i = 0; i < n; i++)
    v.setInt16(44 + i * 2, Math.round(Math.sin((2 * Math.PI * 440 * i) / rate) * 12000), true);
  return new Uint8Array(buf);
}

/** PNG RGB sintetis (piksel berwarna), valid: CRC dan zlib dihitung sungguhan. */
export function makePng(width = 2, height = 2): Uint8Array {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (buf: Uint8Array) => {
    let c = 0xffffffff;
    for (const byte of buf) c = (crcTable[(c ^ byte) & 0xff] as number) ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type: string, data: Uint8Array) => {
    const out = new Uint8Array(12 + data.length);
    const dv = new DataView(out.buffer);
    dv.setUint32(0, data.length);
    for (let i = 0; i < 4; i++) out[4 + i] = type.charCodeAt(i);
    out.set(data, 8);
    dv.setUint32(8 + data.length, crc(out.subarray(4, 8 + data.length)));
    return out;
  };
  const raw = new Uint8Array(height * (1 + width * 3));
  for (let y = 0; y < height; y++) {
    const row = y * (1 + width * 3);
    raw[row] = 0;
    for (let x = 0; x < width; x++) {
      const o = row + 1 + x * 3;
      raw[o] = (x * 255) / Math.max(1, width - 1);
      raw[o + 1] = (y * 255) / Math.max(1, height - 1);
      raw[o + 2] = 128;
    }
  }
  // zlib "stored" (tanpa kompresi) agar tidak butuh modul tambahan: header 78 01, blok stored, Adler-32
  const adler = (() => {
    let a = 1;
    let b2 = 0;
    for (const byte of raw) {
      a = (a + byte) % 65521;
      b2 = (b2 + a) % 65521;
    }
    return ((b2 << 16) | a) >>> 0;
  })();
  const z = new Uint8Array(2 + 5 + raw.length + 4);
  z[0] = 0x78;
  z[1] = 0x01;
  z[2] = 1;
  z[3] = raw.length & 0xff;
  z[4] = raw.length >> 8;
  z[5] = ~raw.length & 0xff;
  z[6] = (~raw.length >> 8) & 0xff;
  z.set(raw, 7);
  new DataView(z.buffer).setUint32(7 + raw.length, adler);
  const ihdr = new Uint8Array(13);
  const dv = new DataView(ihdr.buffer);
  dv.setUint32(0, width);
  dv.setUint32(4, height);
  ihdr[8] = 8;
  ihdr[9] = 2;
  const sig = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const parts = [sig, chunk("IHDR", ihdr), chunk("IDAT", z), chunk("IEND", new Uint8Array(0))];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

/** WebM VP8 64x48, 0,6 dtk (ffmpeg, ±0,6 KB): MediaRecorder tidak menghasilkan data di lingkungan uji. */
export const WEBM_BASE64 =
  "GkXfo59ChoEBQveBAULygQRC84EIQoKEd2VibUKHgQJChYECGFOAZwEAAAAAAAJDEU2bdLpNu4tTq4QVSalmU6yBoU27i1OrhBZUrmtTrIHWTbuMU6uEElTDZ1OsggEyTbuMU6uEHFO7a1OsggIt7AEAAAAAAABZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAVSalmsCrXsYMPQkBNgIxMYXZmNjMuMS4xMDFXQYxMYXZmNjMuMS4xMDFEiYhAgsAAAAAAABZUrmvXrgEAAAAAAABO14EBc8WI+jcr6VgZdWScgQAitZyDdW5kiIEAhoVWX1ZQOIOBASPjg4QL68IA4JCwgUC6gTCagQJVsIRVuYEBVe6BAOwBAAAAAAAAAgAAElTDZ/pzc59jwIBnyJlFo4dFTkNPREVSRIeMTGF2ZjYzLjEuMTAxc3PVY8CLY8WI+jcr6VgZdWRnyKBFo4dFTkNPREVSRIeTTGF2YzYzLjEuMTAxIGxpYnZweGfIoUWjiERVUkFUSU9ORIeTMDA6MDA6MDAuNjAwMDAwMDAwAB9DtnX354EAo8KBAACAUAMAnQEqQAAwAABHCIWFiIWEiAICAnWqA/gCBwjVZ5mPN9IA/vp5X/+p0/qdP6nT/6nT/9yAeyAeyAfv3ACjloEAyADRAQABEBAAGAAYWC/0AAiMAACjloEBkADRAQABEBAAGAAYWC/0AAiMAAAcU7trkbuPs4EAt4r3gQHxggGx8IED";

export const b64 = (bytes: Uint8Array): string => Buffer.from(bytes).toString("base64");

export const SAMPLE_MD = [
  "# Judul Dokumen",
  "",
  "Paragraf dengan **tebal**, _miring_, dan [tautan](https://example.com).",
  "",
  "- butir satu",
  "- butir dua",
  "",
  "| nama | nilai |",
  "| ---- | ----- |",
  "| a    | 1     |",
  "| b    | 2     |",
  "",
  "```ts",
  "const x: number = 1;",
  "```",
  "",
].join("\n");

export const MATH_MERMAID_MD = [
  "# Rumus dan diagram",
  "",
  "Inline $a^2 + b^2 = c^2$ dan blok:",
  "",
  "$$\\int_0^1 x\\,dx = \\frac{1}{2}$$",
  "",
  "```mermaid",
  "graph TD;",
  "  A[Mulai] --> B[Selesai];",
  "```",
  "",
  "```mermaid",
  "ini bukan diagram yang valid",
  "```",
  "",
].join("\n");

export const csvRows = (n: number): string =>
  ["id,nama,nilai", ...Array.from({ length: n }, (_, i) => `${i + 1},"Nama ${i + 1}",${(i * 7) % 100}`)].join("\n");

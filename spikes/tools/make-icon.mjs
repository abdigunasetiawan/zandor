// Membuat ikon placeholder spike (PNG 512x512 monokrom, huruf "Z" sederhana). Bukan aset produk.
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
const W = 512, H = 512;
const raw = Buffer.alloc((W * 4 + 1) * H);
for (let y = 0; y < H; y++) {
  raw[y * (W * 4 + 1)] = 0;
  for (let x = 0; x < W; x++) {
    const i = y * (W * 4 + 1) + 1 + x * 4;
    const inZ = (y > 100 && y < 150 && x > 110 && x < 400) || (y > 360 && y < 410 && x > 110 && x < 400) ||
      (Math.abs((x - 110) - (410 - y) * (290 / 260)) < 30 && y >= 150 && y <= 360);
    const v = inZ ? 0xf0 : 0x14;
    raw[i] = raw[i + 1] = raw[i + 2] = v; raw[i + 3] = 255;
  }
}
const crcT = new Int32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c; });
const crc = (b) => { let c = -1; for (const x of b) c = crcT[(c ^ x) & 255] ^ (c >>> 8); return (c ^ -1) >>> 0; };
const chunk = (t, d) => { const l = Buffer.alloc(4); l.writeUInt32BE(d.length); const td = Buffer.concat([Buffer.from(t), d]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(W, 0); ihdr.writeUInt32BE(H, 4); ihdr[8] = 8; ihdr[9] = 6;
writeFileSync(process.argv[2], Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]));

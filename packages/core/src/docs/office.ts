// Pembaca DOCX / XLSX / PPTX (Fase 11, ADR-039): best-effort, hanya MEMBACA (tanpa menjalankan makro/skrip) dan hasilnya
// data polos untuk dirender React (tidak ada HTML mentah). Pustaka: fflate (zip) + DOMParser bawaan WebView.
// Batas keras di setiap pembaca mencegah berkas besar/berbahaya membekukan UI.
import { unzipSync } from "fflate";

const NS_R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const MAX_IMAGE_BYTES = 2 * 1024 * 1024;
const MAX_IMAGES = 30;
const MIME: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
};

type Files = Record<string, Uint8Array>;

function open(bytes: Uint8Array, want: (name: string) => boolean): Files {
  try {
    return unzipSync(bytes, { filter: (f) => f.originalSize <= 40 * 1024 * 1024 && want(f.name) });
  } catch {
    throw new Error("berkas bukan paket Office yang valid (zip rusak)");
  }
}

function xml(files: Files, name: string): Document | null {
  const b = files[name];
  if (!b) return null;
  const doc = new DOMParser().parseFromString(new TextDecoder().decode(b), "application/xml");
  return doc.getElementsByTagName("parsererror").length ? null : doc;
}

const local = (n: Node) => (n as Element).localName;
/** Semua turunan dengan nama lokal tertentu (mengabaikan prefix namespace). */
const all = (root: Node, name: string): Element[] => [...(root as Element).getElementsByTagNameNS("*", name)];
/** Anak langsung dengan nama lokal tertentu. */
const kids = (root: Node, name: string): Element[] => [...(root as Element).children].filter((c) => local(c) === name);
const kid = (root: Node, name: string): Element | undefined => kids(root, name)[0];
const attr = (el: Element | undefined | null, name: string): string | null => {
  if (!el) return null;
  for (const a of [...el.attributes]) if (a.localName === name) return a.value;
  return null;
};

function relsOf(files: Files, path: string): Map<string, { target: string; type: string; external: boolean }> {
  const doc = xml(files, path);
  const out = new Map<string, { target: string; type: string; external: boolean }>();
  if (!doc) return out;
  for (const r of all(doc, "Relationship"))
    out.set(attr(r, "Id") ?? "", {
      target: attr(r, "Target") ?? "",
      type: attr(r, "Type") ?? "",
      external: attr(r, "TargetMode") === "External",
    });
  return out;
}

/** Selesaikan target relatif terhadap folder berkas pemilik rel (mis. `word/` + `media/a.png`). */
function resolve(baseDir: string, target: string): string {
  const parts = (target.startsWith("/") ? target.slice(1) : `${baseDir}${target}`).split("/");
  const out: string[] = [];
  for (const p of parts) {
    if (p === "..") out.pop();
    else if (p !== "." && p !== "") out.push(p);
  }
  return out.join("/");
}

function toBase64(b: Uint8Array): string {
  let s = "";
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return btoa(s);
}

function imageUrl(files: Files, path: string, budget: { n: number }): string | null {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const mime = MIME[ext];
  const data = files[path];
  if (!mime || !data || data.length > MAX_IMAGE_BYTES || budget.n >= MAX_IMAGES) return null;
  budget.n++;
  return `data:${mime};base64,${toBase64(data)}`;
}

// ====================== DOCX ======================

export type DocxRun = {
  text: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
  href?: string;
  image?: string;
};
export type DocxStyle = "title" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | null;
export type DocxBlock =
  | { type: "p"; style: DocxStyle; list: { level: number; ordered: boolean } | null; runs: DocxRun[] }
  | { type: "table"; rows: DocxBlock[][][] };
export type DocxResult = { blocks: DocxBlock[]; truncated: boolean };

const MAX_BLOCKS = 5000;

export function parseDocx(bytes: Uint8Array): DocxResult {
  const files = open(bytes, (n) =>
    /^word\/(document\.xml|numbering\.xml|_rels\/document\.xml\.rels|media\/[^/]+)$/.test(n),
  );
  const docXml = xml(files, "word/document.xml");
  if (!docXml) throw new Error("word/document.xml tidak ditemukan atau rusak");
  const rels = relsOf(files, "word/_rels/document.xml.rels");
  const budget = { n: 0 };

  // numbering: numId → ordered? (bullet bila numFmt bullet)
  const ordered = new Map<string, boolean>();
  const numDoc = xml(files, "word/numbering.xml");
  if (numDoc) {
    const abs = new Map<string, boolean>();
    for (const a of all(numDoc, "abstractNum")) {
      const lvl = kids(a, "lvl")[0];
      abs.set(attr(a, "abstractNumId") ?? "", attr(kid(lvl ?? a, "numFmt"), "val") !== "bullet");
    }
    for (const n of all(numDoc, "num"))
      ordered.set(attr(n, "numId") ?? "", abs.get(attr(kid(n, "abstractNumId"), "val") ?? "") ?? true);
  }

  let count = 0;
  let truncated = false;

  const headingOf = (style: string | null): DocxStyle => {
    if (!style) return null;
    const s = style.toLowerCase().replace(/\s+/g, "");
    if (s === "title") return "title";
    const m = /^(?:heading|titre|judul)(\d)$/.exec(s);
    if (m && Number(m[1]) >= 1 && Number(m[1]) <= 6) return `h${m[1]}` as DocxStyle;
    return null;
  };

  const runOf = (r: Element, href?: string): DocxRun | null => {
    const rpr = kid(r, "rPr");
    const flag = (name: string) => {
      const e = kid(rpr ?? r, name);
      if (!e || !rpr) return false;
      const v = attr(e, "val");
      return v === null || !["0", "false", "none"].includes(v);
    };
    let text = "";
    let image: string | undefined;
    for (const c of [...r.children]) {
      const n = local(c);
      if (n === "t") text += c.textContent ?? "";
      else if (n === "tab") text += "\t";
      else if (n === "br" || n === "cr") text += "\n";
      else if (n === "drawing" || n === "pict") {
        const blip = all(c, "blip")[0];
        const id = attr(blip, "embed");
        const rel = id ? rels.get(id) : undefined;
        if (rel && !rel.external) image = imageUrl(files, resolve("word/", rel.target), budget) ?? undefined;
      }
    }
    if (!text && !image) return null;
    return {
      text,
      ...(flag("b") && { bold: true }),
      ...(flag("i") && { italic: true }),
      ...(flag("u") && { underline: true }),
      ...(flag("strike") && { strike: true }),
      ...(href && { href }),
      ...(image && { image }),
    };
  };

  const para = (p: Element): DocxBlock => {
    const ppr = kid(p, "pPr");
    const style = attr(kid(ppr ?? p, "pStyle"), "val");
    const numPr = ppr ? kid(ppr, "numPr") : undefined;
    let list: { level: number; ordered: boolean } | null = null;
    if (numPr) {
      const id = attr(kid(numPr, "numId"), "val") ?? "";
      if (id !== "0")
        list = { level: Number(attr(kid(numPr, "ilvl"), "val") ?? 0) || 0, ordered: ordered.get(id) ?? false };
    } else if (style && /^list ?bullet/i.test(style)) list = { level: 0, ordered: false };
    else if (style && /^list ?number/i.test(style)) list = { level: 0, ordered: true };
    const runs: DocxRun[] = [];
    for (const c of [...p.children]) {
      if (local(c) === "r") {
        const r = runOf(c);
        if (r) runs.push(r);
      } else if (local(c) === "hyperlink") {
        const id = attr(c, "id");
        const rel = id ? rels.get(id) : undefined;
        const href = rel?.external && /^https?:\/\//i.test(rel.target) ? rel.target : undefined;
        for (const r of kids(c, "r")) {
          const run = runOf(r, href);
          if (run) runs.push(run);
        }
      }
    }
    return { type: "p", style: headingOf(style), list, runs };
  };

  const body = (container: Element, depth: number): DocxBlock[] => {
    const out: DocxBlock[] = [];
    for (const c of [...container.children]) {
      if (count >= MAX_BLOCKS) {
        truncated = true;
        break;
      }
      const n = local(c);
      if (n === "p") {
        out.push(para(c));
        count++;
      } else if (n === "tbl" && depth < 3) {
        const rows = kids(c, "tr").map((tr) => kids(tr, "tc").map((tc) => body(tc, depth + 1)));
        out.push({ type: "table", rows });
        count++;
      }
    }
    return out;
  };

  const bodyEl = all(docXml, "body")[0];
  if (!bodyEl) throw new Error("isi dokumen tidak ditemukan");
  return { blocks: body(bodyEl, 0), truncated };
}

// ====================== XLSX ======================

export type XlsxSheet = { name: string; rows: string[][]; truncated: boolean };
export type XlsxResult = { sheets: XlsxSheet[] };

const MAX_ROWS = 2000;
const MAX_COLS = 100;
const MAX_SHEETS = 20;
const BUILTIN_DATE_FMT = new Set([14, 15, 16, 17, 18, 19, 20, 21, 22, 45, 46, 47]);

function colIndex(ref: string): number {
  const m = /^([A-Z]+)/i.exec(ref);
  if (!m) return -1;
  let n = 0;
  for (const ch of (m[1] as string).toUpperCase()) n = n * 26 + (ch.charCodeAt(0) - 64);
  return n - 1;
}

function serialToText(serial: number): string {
  const ms = Math.round((serial - 25569) * 86400 * 1000);
  const d = new Date(ms);
  if (Number.isNaN(d.getTime())) return String(serial);
  const iso = d.toISOString();
  return Number.isInteger(serial) ? iso.slice(0, 10) : `${iso.slice(0, 10)} ${iso.slice(11, 19)}`;
}

export function parseXlsx(bytes: Uint8Array): XlsxResult {
  const files = open(bytes, (n) =>
    /^xl\/(workbook\.xml|styles\.xml|sharedStrings\.xml|_rels\/workbook\.xml\.rels|worksheets\/[^/]+\.xml)$/.test(n),
  );
  const wb = xml(files, "xl/workbook.xml");
  if (!wb) throw new Error("xl/workbook.xml tidak ditemukan atau rusak");
  const rels = relsOf(files, "xl/_rels/workbook.xml.rels");

  const shared: string[] = [];
  const ss = xml(files, "xl/sharedStrings.xml");
  if (ss)
    for (const si of all(ss, "si"))
      shared.push(
        all(si, "t")
          .map((t) => t.textContent ?? "")
          .join(""),
      );

  // gaya sel yang bertanggal
  const dateXf = new Set<number>();
  const st = xml(files, "xl/styles.xml");
  if (st) {
    const customDate = new Set<number>();
    for (const nf of all(st, "numFmt")) {
      const code = (attr(nf, "formatCode") ?? "").replace(/"[^"]*"|\[[^\]]*\]|\\./g, "");
      if (/[dmyhs]/i.test(code)) customDate.add(Number(attr(nf, "numFmtId")));
    }
    const xfs = all(st, "cellXfs")[0];
    if (xfs)
      kids(xfs, "xf").forEach((xf, i) => {
        const id = Number(attr(xf, "numFmtId"));
        if (BUILTIN_DATE_FMT.has(id) || customDate.has(id)) dateXf.add(i);
      });
  }

  const sheets: XlsxSheet[] = [];
  for (const sh of all(wb, "sheet").slice(0, MAX_SHEETS)) {
    if (attr(sh, "state") === "hidden" || attr(sh, "state") === "veryHidden") continue;
    const rel = rels.get(sh.getAttributeNS(NS_R, "id") ?? "");
    const doc = rel ? xml(files, resolve("xl/", rel.target)) : null;
    const name = attr(sh, "name") ?? "Sheet";
    if (!doc) {
      sheets.push({ name, rows: [], truncated: false });
      continue;
    }
    const rows: string[][] = [];
    let truncated = false;
    let maxCol = 0;
    for (const row of all(doc, "row")) {
      const r = Number(attr(row, "r")) - 1;
      if (!(r >= 0)) continue;
      if (r >= MAX_ROWS) {
        truncated = true;
        break;
      }
      const cells = rows[r] ?? [];
      rows[r] = cells;
      for (const c of kids(row, "c")) {
        const ci = colIndex(attr(c, "r") ?? "");
        if (ci < 0) continue;
        if (ci >= MAX_COLS) {
          truncated = true;
          continue;
        }
        const t = attr(c, "t");
        const v = kid(c, "v")?.textContent ?? "";
        const f = kid(c, "f")?.textContent;
        let text = "";
        if (t === "s") text = shared[Number(v)] ?? "";
        else if (t === "inlineStr")
          text = all(c, "t")
            .map((x) => x.textContent ?? "")
            .join("");
        else if (t === "b") text = v === "1" ? "TRUE" : v === "0" ? "FALSE" : "";
        else if (t === "str" || t === "e") text = v;
        else if (v !== "") {
          const n = Number(v);
          text = Number.isFinite(n) && dateXf.has(Number(attr(c, "s"))) ? serialToText(n) : v;
        } else if (f) text = `=${f}`; // tanpa nilai tersimpan (belum pernah dihitung): tampilkan rumusnya
        cells[ci] = text;
        maxCol = Math.max(maxCol, ci + 1);
      }
    }
    for (let i = 0; i < rows.length; i++) {
      const row = rows[i] ?? [];
      rows[i] = Array.from({ length: maxCol }, (_, j) => row[j] ?? "");
    }
    sheets.push({ name, rows, truncated });
  }
  return { sheets };
}

// ====================== PPTX ======================

export type PptxSlide = { index: number; title: string; paragraphs: string[]; images: string[]; tables: string[][][] };
export type PptxResult = { slides: PptxSlide[]; truncated: boolean };
const MAX_SLIDES = 200;

export function parsePptx(bytes: Uint8Array): PptxResult {
  const files = open(bytes, (n) =>
    /^ppt\/(presentation\.xml|_rels\/presentation\.xml\.rels|slides\/[^/]+\.xml|slides\/_rels\/[^/]+\.rels|media\/[^/]+)$/.test(
      n,
    ),
  );
  const pres = xml(files, "ppt/presentation.xml");
  if (!pres) throw new Error("ppt/presentation.xml tidak ditemukan atau rusak");
  const rels = relsOf(files, "ppt/_rels/presentation.xml.rels");
  const budget = { n: 0 };
  const slides: PptxSlide[] = [];
  let truncated = false;

  const textOf = (p: Element): string => {
    let s = "";
    for (const c of [...p.children]) {
      const n = local(c);
      if (n === "r" || n === "fld")
        s += all(c, "t")
          .map((t) => t.textContent ?? "")
          .join("");
      else if (n === "br") s += "\n";
    }
    return s;
  };

  // `<p:sldId id="256" r:id="rId2"/>`: yang dicari atribut ber-namespace r:, bukan `id` numerik.
  const rids = all(pres, "sldId").map((s) => s.getAttributeNS(NS_R, "id") ?? "");
  for (const rid of rids) {
    if (slides.length >= MAX_SLIDES) {
      truncated = true;
      break;
    }
    const rel = rels.get(rid);
    const path = rel ? resolve("ppt/", rel.target) : "";
    const doc = path ? xml(files, path) : null;
    const index = slides.length + 1;
    if (!doc) {
      slides.push({ index, title: "", paragraphs: [], images: [], tables: [] });
      continue;
    }
    const srels = relsOf(files, `${path.replace(/[^/]+$/, "")}_rels/${path.split("/").pop()}.rels`);
    let title = "";
    const paragraphs: string[] = [];
    for (const sp of all(doc, "sp")) {
      const ph = all(sp, "ph")[0];
      const type = attr(ph, "type");
      const tx = kid(sp, "txBody");
      if (!tx) continue;
      const lines = kids(tx, "p")
        .map(textOf)
        .filter((l) => l.trim() !== "");
      if (type === "title" || type === "ctrTitle") title = title || lines.join(" ");
      else paragraphs.push(...lines);
    }
    const tables = all(doc, "tbl").map((tbl) =>
      kids(tbl, "tr").map((tr) => kids(tr, "tc").map((tc) => all(tc, "p").map(textOf).join(" ").trim())),
    );
    const images: string[] = [];
    for (const blip of all(doc, "blip")) {
      const r = srels.get(attr(blip, "embed") ?? "");
      if (!r || r.external) continue;
      const url = imageUrl(files, resolve(`${path.replace(/[^/]+$/, "")}`, r.target), budget);
      if (url) images.push(url);
    }
    slides.push({ index, title, paragraphs, images, tables });
  }
  return { slides, truncated };
}

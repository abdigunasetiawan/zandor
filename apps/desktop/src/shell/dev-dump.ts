// Alat pengembang: membuat satu dokumen HTML mandiri dari isi sebuah panel (mis. Panel 2 Chat) untuk mereproduksi masalah
// render. Isinya: DOM panel apa adanya, semua CSS aplikasi (inline), atribut tema pada <html>/<body>, dan data mentah
// (mis. pesan sesi, termasuk langkah proses agent yang terlipat atau di luar jendela virtualisasi) sebagai JSON.
// Tidak menyentuh jaringan; dipanggil hanya atas aksi pengguna dan hasilnya masuk clipboard.

const escapeAttr = (v: string) => v.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

/** JSON aman di dalam <script type="application/json">: tanpa "</script>" dan pemisah baris unicode. */
const LS = String.fromCharCode(0x2028);
const PS = String.fromCharCode(0x2029);
const BS = String.fromCharCode(92);
export function jsonForScript(data: unknown): string {
  return (JSON.stringify(data, null, 1) ?? "null")
    .split("<")
    .join(`${BS}u003c`)
    .split(LS)
    .join(`${BS}u2028`)
    .split(PS)
    .join(`${BS}u2029`);
}

function attrsOf(el: Element): string {
  return [...el.attributes].map((a) => ` ${a.name}="${escapeAttr(a.value)}"`).join("");
}

/** Semua aturan CSS yang dapat dibaca; lembar lintas-origin dirujuk lewat <link>. */
export function collectCss(doc: Document): { css: string; links: string[] } {
  const parts: string[] = [];
  const links: string[] = [];
  for (const sheet of [...doc.styleSheets]) {
    try {
      parts.push([...sheet.cssRules].map((r) => r.cssText).join("\n"));
    } catch {
      if (sheet.href) links.push(sheet.href);
    }
  }
  return { css: parts.join("\n"), links };
}

export type PanelDumpInput = {
  doc: Document;
  /** Elemen panel yang disalin (outerHTML). */
  root: Element;
  title: string;
  /** Catatan keadaan saat disalin (waktu, build, sesi, ukuran, dll.). */
  meta: Record<string, unknown>;
  /** Data mentah untuk reproduksi (mis. pesan sesi). */
  data?: unknown;
};

export function buildPanelDump({ doc, root, title, meta, data }: PanelDumpInput): string {
  const { css, links } = collectCss(doc);
  const rect = root.getBoundingClientRect();
  const size = `width:${Math.round(rect.width)}px;height:${Math.round(rect.height)}px;`;
  const linkTags = links.map((h) => `<link rel="stylesheet" href="${escapeAttr(h)}">`).join("\n");
  return [
    "<!doctype html>",
    `<html${attrsOf(doc.documentElement)}>`,
    "<head>",
    '<meta charset="utf-8">',
    `<title>${escapeAttr(title)}</title>`,
    linkTags,
    `<style>\n${css.replace(/<\/style/gi, "<\\/style")}\n</style>`,
    "</head>",
    `<body${attrsOf(doc.body)}>`,
    `<!-- Dump Zandor (alat pengembang). Font dan aset relatif mungkin tidak termuat bila dibuka di luar aplikasi. -->`,
    `<script type="application/json" id="zandor-dump-meta">${jsonForScript(meta)}</script>`,
    `<div class="app" style="${size}display:flex;">`,
    root.outerHTML,
    "</div>",
    data === undefined ? "" : `<script type="application/json" id="zandor-dump-data">${jsonForScript(data)}</script>`,
    "</body>",
    "</html>",
  ].join("\n");
}

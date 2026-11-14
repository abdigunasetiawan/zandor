// Ekspor satu respons: teks (Markdown), gambar PNG, atau SVG. Teks dari isi pesan; gambar/SVG dirender dari DOM respons dengan
// html-to-image (MIT). Fungsi murni (nama berkas, dekode data URL) dipisah agar mudah diuji.
import { toPng, toSvg } from "html-to-image";

export type ExportKind = "text" | "png" | "svg";

const EXT: Record<ExportKind, string> = { text: "md", png: "png", svg: "svg" };

/** Nama berkas unik: `respons-YYYYMMDD-HHMMSS.ext`. */
export function exportFileName(kind: ExportKind, at: Date = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  const stamp = `${at.getFullYear()}${p(at.getMonth() + 1)}${p(at.getDate())}-${p(at.getHours())}${p(at.getMinutes())}${p(at.getSeconds())}`;
  return `respons-${stamp}.${EXT[kind]}`;
}

export const textBytes = (text: string): Uint8Array =>
  new TextEncoder().encode(text.endsWith("\n") ? text : `${text}\n`);

/** Dekode data URL (base64 atau persen-encoded) menjadi byte. */
export function dataUrlToBytes(url: string): Uint8Array {
  const comma = url.indexOf(",");
  if (!url.startsWith("data:") || comma < 0) throw new Error("data URL tidak valid");
  const meta = url.slice(5, comma);
  const body = url.slice(comma + 1);
  if (meta.includes(";base64")) {
    const bin = atob(body);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }
  return new TextEncoder().encode(decodeURIComponent(body));
}

/** Render node respons menjadi PNG atau SVG. Elemen bertanda `data-no-export` tidak ikut. */
export async function renderNode(node: HTMLElement, kind: "png" | "svg"): Promise<Uint8Array> {
  const bg = getComputedStyle(document.body).backgroundColor || undefined;
  const opts = {
    pixelRatio: 2,
    backgroundColor: bg,
    filter: (n: Node) => !(n instanceof HTMLElement && n.hasAttribute("data-no-export")),
  };
  const make = (skipFonts: boolean) =>
    kind === "png" ? toPng(node, { ...opts, skipFonts }) : toSvg(node, { ...opts, skipFonts });
  let url: string;
  try {
    url = await make(false);
  } catch {
    url = await make(true); // gagal menyematkan font → render tanpa font tersemat
  }
  return dataUrlToBytes(url);
}

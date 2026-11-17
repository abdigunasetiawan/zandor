// Lampiran prompt: URL file dari path lokal dan tebakan MIME dari nama. Murni (tanpa akses disk).
import type { FileAttachment } from "./engine/api";

const MIME: Record<string, string> = {
  txt: "text/plain",
  md: "text/markdown",
  json: "application/json",
  csv: "text/csv",
  html: "text/html",
  css: "text/css",
  js: "text/javascript",
  mjs: "text/javascript",
  ts: "text/plain",
  tsx: "text/plain",
  rs: "text/plain",
  py: "text/x-python",
  toml: "text/plain",
  yaml: "text/yaml",
  yml: "text/yaml",
  xml: "application/xml",
  pdf: "application/pdf",
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  svg: "image/svg+xml",
  docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  pptx: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  doc: "application/msword",
  xls: "application/vnd.ms-excel",
  ppt: "application/vnd.ms-powerpoint",
  odt: "application/vnd.oasis.opendocument.text",
  ods: "application/vnd.oasis.opendocument.spreadsheet",
  odp: "application/vnd.oasis.opendocument.presentation",
  ipynb: "application/x-ipynb+json",
  zip: "application/zip",
};

export const DIRECTORY_MIME = "application/x-directory";

export function mimeForName(name: string): string {
  const dot = name.lastIndexOf(".");
  const ext = dot === -1 ? "" : name.slice(dot + 1).toLowerCase();
  return MIME[ext] ?? "text/plain";
}

/** `C:\a b\c.txt` → `file:///C:/a%20b/c.txt`; awalan verbatim Windows `\\?\` dibuang. */
export function fileUrlFromPath(path: string): string {
  const raw = path.startsWith("\\\\?\\") ? path.slice(4) : path;
  const slashed = raw.replace(/\\/g, "/");
  const encoded = slashed
    .split("/")
    .map((seg, i) => (i === 0 && /^[A-Za-z]:$/.test(seg) ? seg : encodeURIComponent(seg)))
    .join("/");
  return encoded.startsWith("/") ? `file://${encoded}` : `file:///${encoded}`;
}

export const baseNameOf = (path: string): string => path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;

export function attachmentFromPath(path: string, isDirectory = false): FileAttachment {
  const filename = baseNameOf(path);
  return {
    mime: isDirectory ? DIRECTORY_MIME : mimeForName(filename),
    url: fileUrlFromPath(path),
    filename,
  };
}

/**
 * true bila model bisa menerima lampiran ini langsung sebagai bagian `file` (gambar, PDF, teks, folder). Jenis lain
 * (DOCX/XLSX/PPTX, arsip, biner) ditolak provider ("file part media type … not supported"), jadi dikirim sebagai path
 * agar agent membacanya dari disk dengan tool/skill (mis. `zandor-docx`).
 */
export function modelReadable(mime: string): boolean {
  const m = mime.toLowerCase().split(";")[0]?.trim() ?? "";
  return (
    m.startsWith("image/") ||
    m.startsWith("text/") ||
    m === "application/pdf" ||
    m === "application/json" ||
    m === "application/xml" ||
    m === DIRECTORY_MIME
  );
}

/** `file:///C:/a%20b/c.txt` → `C:/a b/c.txt`; bukan URL file → null. */
export function pathFromFileUrl(url: string): string | null {
  if (!url.startsWith("file://")) return null;
  let rest = url.slice("file://".length);
  if (/^\/[A-Za-z]:/.test(rest)) rest = rest.slice(1);
  try {
    return decodeURIComponent(rest);
  } catch {
    return null;
  }
}

export type PromptFilePart = { type: "file"; mime: string; url: string; filename?: string };
export type PromptTextPart = { type: "text"; text: string; synthetic?: boolean };

/** Bagian prompt untuk lampiran: yang terbaca model → `file`; sisanya → satu teks sintetis berisi path di disk. */
export function attachmentParts(files: readonly FileAttachment[]): Array<PromptFilePart | PromptTextPart> {
  const parts: Array<PromptFilePart | PromptTextPart> = [];
  const notes: string[] = [];
  for (const f of files) {
    if (modelReadable(f.mime)) {
      parts.push({ type: "file", mime: f.mime, url: f.url, ...(f.filename ? { filename: f.filename } : {}) });
      continue;
    }
    const path = pathFromFileUrl(f.url);
    const name = f.filename ?? (path ? baseNameOf(path) : "lampiran");
    notes.push(
      path
        ? `- ${name}: ${path}`
        : `- ${name}: (tidak tersimpan di disk; minta pengguna menyimpannya ke folder project)`,
    );
  }
  if (notes.length)
    parts.push({
      type: "text",
      synthetic: true,
      text: [
        "<zandor-attachments>",
        "Pengguna melampirkan berkas yang tidak bisa dikirim langsung ke model. Baca dari disk dengan tool/skill yang sesuai (DOCX/XLSX/PPTX/PDF: skill zandor-docx/zandor-xlsx/zandor-pptx/zandor-pdf):",
        ...notes,
        "</zandor-attachments>",
      ].join("\n"),
    });
  return parts;
}

/** Kebalikan `attachmentParts`: daftar lampiran (nama + path) dari teks sintetis `<zandor-attachments>`. */
export function attachmentsFromNote(text: string): Array<{ name: string; path: string | null }> {
  if (!text.startsWith("<zandor-attachments>")) return [];
  return text
    .split("\n")
    .filter((l) => l.startsWith("- "))
    .map((l) => {
      const body = l.slice(2);
      const at = body.indexOf(": ");
      if (at === -1) return { name: body, path: null };
      const rest = body.slice(at + 2);
      return { name: body.slice(0, at), path: rest.startsWith("(") ? null : rest };
    });
}

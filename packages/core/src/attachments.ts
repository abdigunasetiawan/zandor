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

/** Batas lampiran dari browser (data URL) agar prompt tidak membengkak. */
export const MAX_INLINE_BYTES = 5 * 1024 * 1024;

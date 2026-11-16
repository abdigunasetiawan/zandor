import type { FileAttachment } from "@zandor/core";

/** Ambang "teks panjang": lebih dari ini ditempel sebagai lampiran, bukan ke textarea. */
export const PASTE_MAX_LINES = 30;
export const PASTE_MAX_CHARS = 2000;
const PREFIX = "teks-ditempel-";

export const lineCount = (text: string) => (text === "" ? 0 : text.split(/\r\n|\r|\n/).length);

export const isLongPaste = (text: string) => text.length > PASTE_MAX_CHARS || lineCount(text) > PASTE_MAX_LINES;

/** Lampiran teks ditempel: data URL text/plain (UTF-8), dikirim ke engine seperti berkas teks biasa. */
export function pastedAttachment(text: string, existing: readonly FileAttachment[]): FileAttachment {
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  let n = 1;
  while (existing.some((f) => f.filename === `${PREFIX}${n}.txt`)) n++;
  return {
    mime: "text/plain",
    url: `data:text/plain;charset=utf-8;base64,${btoa(binary)}`,
    filename: `${PREFIX}${n}.txt`,
  };
}

export const isPasted = (file: FileAttachment) =>
  file.url.startsWith("data:text/plain") && (file.filename ?? "").startsWith(PREFIX);

/** Isi teks dari lampiran teks ditempel (null bila bukan/gagal didekode). */
export function pastedText(file: FileAttachment): string | null {
  if (!isPasted(file)) return null;
  try {
    const binary = atob(file.url.slice(file.url.indexOf(",") + 1));
    return new TextDecoder().decode(Uint8Array.from(binary, (c) => c.charCodeAt(0)));
  } catch {
    return null;
  }
}

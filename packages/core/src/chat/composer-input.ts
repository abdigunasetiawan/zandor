// Logika murni composer: deteksi token yang sedang diketik (`/perintah` atau `@berkas`), parsing perintah slash,
// daftar perintah bawaan aplikasi, dan lampiran untuk berkas yang di-mention.
import type { FileAttachment } from "../engine/api";

export type ComposerToken = {
  kind: "slash" | "mention";
  /** Teks setelah `/` atau `@` sampai kursor. */
  query: string;
  /** Indeks karakter pemicu (`/` atau `@`). */
  start: number;
  /** Indeks akhir token (kursor). */
  end: number;
};

/** Token yang sedang diketik di posisi kursor, atau null. */
export function activeToken(text: string, caret: number): ComposerToken | null {
  const before = text.slice(0, caret);
  // perintah slash hanya di awal pesan dan sebelum spasi pertama
  const slash = /^\/([^\s/]*)$/.exec(before);
  if (slash) return { kind: "slash", query: slash[1] ?? "", start: 0, end: caret };
  const mention = /(^|\s)@([^\s@]*)$/.exec(before);
  if (mention) {
    const query = mention[2] ?? "";
    return { kind: "mention", query, start: caret - query.length - 1, end: caret };
  }
  return null;
}

/** Ganti token dengan teks pengganti; mengembalikan teks baru dan posisi kursor. */
export function replaceToken(text: string, token: ComposerToken, replacement: string) {
  const after = text.slice(token.end);
  const sep = after.startsWith(" ") ? "" : " ";
  const next = text.slice(0, token.start) + replacement + sep + after;
  // kursor selalu setelah spasi pemisah (baru atau yang sudah ada)
  return { text: next, caret: token.start + replacement.length + 1 };
}

export type ParsedSlash = { name: string; args: string };

/** `/nama argumen…` → { name, args }. Bukan perintah (atau `//` untuk menulis garis miring literal) → null. */
export function parseSlash(text: string): ParsedSlash | null {
  const m = /^\/([A-Za-z0-9][\w:.-]*)(?:\s+([\s\S]*))?$/.exec(text.trim());
  if (!m?.[1]) return null;
  return { name: m[1], args: (m[2] ?? "").trim() };
}

/** Perintah bawaan aplikasi (dijalankan klien). Perintah engine dengan nama sama kalah dari ini. */
export const BUILTIN_COMMANDS = ["btw", "new", "compact", "undo", "redo", "fork", "copy", "mode", "stop"] as const;
export type BuiltinCommand = (typeof BUILTIN_COMMANDS)[number];
/** Perintah bawaan yang wajib diberi argumen. */
export const BUILTIN_NEEDS_ARGS: readonly BuiltinCommand[] = ["btw", "mode"];

export const isBuiltin = (name: string): name is BuiltinCommand =>
  (BUILTIN_COMMANDS as readonly string[]).includes(name);

/** Saring berdasarkan query: awalan dulu, lalu yang mengandung; urutan asli dipertahankan di tiap kelompok. */
export function rankByQuery<T>(items: readonly T[], query: string, key: (item: T) => string): T[] {
  const q = query.toLowerCase();
  if (!q) return [...items];
  const prefix: T[] = [];
  const contains: T[] = [];
  for (const item of items) {
    const k = key(item).toLowerCase();
    if (k.startsWith(q)) prefix.push(item);
    else if (k.includes(q)) contains.push(item);
  }
  return [...prefix, ...contains];
}

const IMAGE_MIME: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  pdf: "application/pdf",
};

/** URL `file://` untuk jalur absolut (Windows `C:\a\b` → `file:///C:/a/b`). */
export function fileUrl(absolute: string): string {
  const p = absolute.replace(/\\/g, "/");
  const withRoot = /^[A-Za-z]:\//.test(p) ? `/${p}` : p;
  return `file://${withRoot.split("/").map(encodeURIComponent).join("/").replace(/%3A/g, ":")}`;
}

/** Lampiran engine untuk berkas project yang di-mention (`rel` relatif terhadap `root`). */
export function mentionAttachment(root: string, rel: string): FileAttachment {
  const sep = root.includes("\\") ? "\\" : "/";
  const abs = `${root.replace(/[\\/]+$/, "")}${sep}${rel.replace(/[\\/]/g, sep)}`;
  const ext = rel.split(".").pop()?.toLowerCase() ?? "";
  return { mime: IMAGE_MIME[ext] ?? "text/plain", url: fileUrl(abs), filename: rel.replace(/\\/g, "/") };
}

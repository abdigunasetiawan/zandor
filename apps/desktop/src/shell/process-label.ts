// Logika murni untuk tampilan "proses" jawaban agent: label langkah terakhir (Berpikir, Mencari di internet, dst.), ringkasan
// tool, durasi, dan deteksi berkas dokumen yang dibuat agent. Tanpa React agar mudah diuji.
import type { MessagePart, ToolPart } from "@zandor/core";

const str = (v: unknown): string => (typeof v === "string" ? v : "");

/** Ringkasan satu baris per jenis tool (pencarian web → kueri/URL; shell → perintah; file → path; dst). */
export function toolSummary(tool: string, input: Record<string, unknown>, title?: string): string {
  const pick = (...keys: string[]) => keys.map((k) => str(input[k])).find(Boolean) ?? "";
  if (tool === "websearch" || tool === "codesearch") return pick("query") || title || "";
  if (tool === "webfetch") return pick("url") || title || "";
  if (tool === "bash") return pick("command", "description") || title || "";
  if (tool === "glob" || tool === "grep") return pick("pattern") || title || "";
  const fallback = pick("filePath", "path", "pattern", "command", "url", "query");
  return title || fallback || (Object.values(input).find((v) => typeof v === "string") as string | undefined) || "";
}

export type StepKey =
  | "step.thinking"
  | "step.skill"
  | "step.memoryRead"
  | "step.memoryWrite"
  | "step.web"
  | "step.webfetch"
  | "step.read"
  | "step.edit"
  | "step.write"
  | "step.shell"
  | "step.search"
  | "step.plan"
  | "step.ask"
  | "step.task"
  | "step.tool";

export type Step = { key: StepKey; detail: string; tool?: string };

/** Langkah yang sedang/terakhir dilakukan untuk satu part proses (reasoning atau tool). */
export function stepOf(part: MessagePart): Step | null {
  if (part.type === "reasoning") return { key: "step.thinking", detail: "" };
  if (part.type !== "tool") return null;
  const detail = toolSummary(part.tool, part.state.input, part.state.title);
  const t = part.tool.toLowerCase();
  const base = { detail, tool: part.tool };
  if (t === "skill") return { key: "step.skill", ...base };
  if (t.includes("memory")) {
    return { key: /search|read|get|recall|list|find/.test(t) ? "step.memoryRead" : "step.memoryWrite", ...base };
  }
  if (t === "websearch" || t === "codesearch") return { key: "step.web", ...base };
  if (t === "webfetch") return { key: "step.webfetch", ...base };
  if (t === "read") return { key: "step.read", ...base };
  if (t === "write") return { key: "step.write", ...base };
  if (t === "edit" || t === "multiedit" || t === "patch") return { key: "step.edit", ...base };
  if (t === "bash") return { key: "step.shell", ...base };
  if (t === "grep" || t === "glob" || t === "list") return { key: "step.search", ...base };
  if (t === "todowrite" || t === "todoread") return { key: "step.plan", ...base };
  if (t === "question") return { key: "step.ask", ...base };
  if (t === "task") return { key: "step.task", ...base };
  return { key: "step.tool", ...base };
}

/** Part proses = reasoning dan tool; sisanya (teks, berkas) bagian jawaban. */
export const isProcessPart = (p: MessagePart) => p.type === "reasoning" || p.type === "tool";

/**
 * Pisahkan part pesan asisten: semua yang sampai part proses TERAKHIR adalah proses (termasuk narasi di antara tool); sisanya
 * adalah jawaban yang disajikan. Tanpa part proses → semuanya jawaban.
 */
export function splitTurn(parts: MessagePart[]): { process: MessagePart[]; answer: MessagePart[] } {
  let last = -1;
  parts.forEach((p, i) => {
    if (isProcessPart(p)) last = i;
  });
  return { process: parts.slice(0, last + 1), answer: parts.slice(last + 1) };
}

/** Jumlah langkah yang dihitung di ringkasan: tool dan blok berpikir (bukan narasi teks). */
export const countSteps = (process: MessagePart[]) => process.filter(isProcessPart).length;

/** Detik utuh dari milidetik (minimum 0). */
export const elapsedSeconds = (ms: number) => Math.max(0, Math.floor(ms / 1000));

export type Elapsed = { minutes: number; seconds: number };
export function splitElapsed(totalSeconds: number): Elapsed {
  return { minutes: Math.floor(totalSeconds / 60), seconds: totalSeconds % 60 };
}

const DOC_EXTS = new Set([
  "pdf",
  "docx",
  "doc",
  "xlsx",
  "xls",
  "pptx",
  "ppt",
  "odt",
  "ods",
  "odp",
  "rtf",
  "md",
  "csv",
  "txt",
]);

/** Apakah path berupa dokumen (yang layak ditampilkan sebagai kartu berkas). */
export function isDocumentPath(path: string): boolean {
  const ext = path.split(/[\\/]/).pop()?.split(".").pop()?.toLowerCase();
  return !!ext && path.includes(".") && DOC_EXTS.has(ext);
}

/** Path dokumen yang BARU DIBUAT lewat tool `write` yang selesai (urut kemunculan, tanpa duplikat). */
export function createdDocuments(parts: MessagePart[]): string[] {
  const out: string[] = [];
  for (const p of parts) {
    if (p.type !== "tool" || p.tool !== "write" || p.state.status !== "completed") continue;
    const path = str(p.state.input.filePath) || str(p.state.input.path);
    if (path && isDocumentPath(path) && !out.includes(path)) out.push(path);
  }
  return out;
}

export type ToolPartLike = ToolPart;

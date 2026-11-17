// Logika murni untuk Panel "Aktivitas sesi" (garis waktu seluruh kejadian sesi) dan "Berkas sesi" (berkas yang disentuh agent).
// Dibangun dari pesan sesi yang sudah ada — tidak ada data tambahan dari engine dan tidak ada data karangan.
import type { ChatMessage, MessagePart, PermissionRequest, QuestionRequest, ToolPart } from "@zandor/core";
import { type Step, stepOf, toolSummary } from "./process-label";

export type TimelineKind = "user" | "reply" | "thinking" | "tool" | "error" | "permission" | "question";
export type TimelineStatus = "pending" | "running" | "completed" | "error";

export type TimelineEntry = {
  id: string;
  /** ms epoch; 0 bila tidak diketahui (entri menunggu). */
  at: number;
  kind: TimelineKind;
  /** Satu baris ringkas (isi pesan, ringkasan tool, galat). */
  title: string;
  /** Langkah (kunci i18n) untuk tool/berpikir, agar UI memberi label yang sama dengan tampilan proses. */
  step?: Step;
  status?: TimelineStatus;
  durationMs?: number;
  messageId?: string;
  /** Rincian yang bisa dibuka (input/keluaran tool, galat lengkap), dipotong. */
  detail?: string;
};

const DETAIL_MAX = 4000;
const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n)}…` : s);
const oneLine = (s: string) => s.replace(/\s+/g, " ").trim();

function textOf(m: ChatMessage): string {
  return m.parts
    .filter((p) => p.type === "text" && !p.synthetic)
    .map((p) => (p.type === "text" ? p.text : ""))
    .join("\n");
}

function toolDetail(p: ToolPart): string {
  const parts: string[] = [];
  const input = Object.entries(p.state.input)
    .filter(([, v]) => v !== undefined && v !== null && v !== "")
    .map(([k, v]) => `${k}: ${typeof v === "string" ? v : JSON.stringify(v)}`)
    .join("\n");
  if (input) parts.push(input);
  if (p.state.error) parts.push(p.state.error);
  else if (p.state.output) parts.push(p.state.output);
  return clip(parts.join("\n\n"), DETAIL_MAX);
}

/** Garis waktu kronologis: pesan pengguna, berpikir, tool, jawaban, galat; lalu izin/pertanyaan yang masih menunggu. */
export function buildTimeline(
  messages: ChatMessage[],
  pending: { permissions?: PermissionRequest[]; questions?: QuestionRequest[] } = {},
): TimelineEntry[] {
  const out: TimelineEntry[] = [];
  let last = 0;
  for (const m of messages) {
    const base = m.info.time.created;
    last = Math.max(last, m.info.time.completed ?? base);
    if (m.info.role === "user") {
      const text = oneLine(textOf(m));
      const files = m.parts.filter((p) => p.type === "file").length;
      out.push({
        id: m.info.id,
        at: base,
        kind: "user",
        title: clip(text || (files ? `${files} lampiran` : ""), 160),
        messageId: m.info.id,
        ...(text.length > 160 ? { detail: clip(text, DETAIL_MAX) } : {}),
      });
      continue;
    }
    let reply = "";
    for (const p of m.parts as MessagePart[]) {
      if (p.type === "reasoning") {
        const step = stepOf(p);
        if (step)
          out.push({
            id: p.id,
            at: base,
            kind: "thinking",
            title: clip(oneLine(p.text), 160),
            step,
            messageId: m.info.id,
            ...(p.text ? { detail: clip(p.text, DETAIL_MAX) } : {}),
          });
      } else if (p.type === "tool") {
        const step = stepOf(p);
        const start = p.state.time?.start ?? base;
        const end = p.state.time?.end;
        out.push({
          id: p.id,
          at: start,
          kind: "tool",
          title: clip(toolSummary(p.tool, p.state.input, p.state.title), 200),
          ...(step ? { step } : {}),
          status: p.state.status,
          ...(end !== undefined ? { durationMs: Math.max(0, end - start) } : {}),
          messageId: m.info.id,
          detail: toolDetail(p),
        });
        last = Math.max(last, end ?? start);
      } else if (p.type === "text" && !p.synthetic) {
        reply += (reply ? "\n" : "") + p.text;
      }
    }
    if (reply.trim()) {
      out.push({
        id: `${m.info.id}:reply`,
        at: m.info.time.completed ?? base,
        kind: "reply",
        title: clip(oneLine(reply), 160),
        messageId: m.info.id,
        ...(reply.length > 160 ? { detail: clip(reply, DETAIL_MAX) } : {}),
      });
    }
    if (m.info.error) {
      out.push({
        id: `${m.info.id}:error`,
        at: m.info.time.completed ?? base,
        kind: "error",
        title: clip(oneLine(m.info.error.message || m.info.error.name), 200),
        status: "error",
        messageId: m.info.id,
        detail: `${m.info.error.name}: ${m.info.error.message}`,
      });
    }
  }
  // urutan = urutan pesan, lalu urutan part di dalam pesan (sudah kronologis; jam tool dari engine tidak selalu sebanding)
  for (const p of pending.permissions ?? []) {
    out.push({
      id: `perm:${p.id}`,
      at: last,
      kind: "permission",
      title: `${p.permission}${p.patterns.length ? `: ${p.patterns.join(", ")}` : ""}`,
      status: "pending",
    });
  }
  for (const q of pending.questions ?? []) {
    out.push({
      id: `q:${q.id}`,
      at: last,
      kind: "question",
      title: q.questions.map((x) => x.question).join(" · "),
      status: "pending",
    });
  }
  return out;
}

export type FileOp = "read" | "edit" | "write";
export type TouchedFile = { path: string; ops: FileOp[]; count: number; lastAt: number; failed: boolean };

const OP_OF: Record<string, FileOp> = { read: "read", write: "write", edit: "edit", multiedit: "edit", patch: "edit" };

/** Berkas yang disentuh agent lewat tool read/edit/write sesi ini; terbaru dulu. */
export function touchedFiles(messages: ChatMessage[]): TouchedFile[] {
  const map = new Map<string, TouchedFile>();
  for (const m of messages) {
    for (const p of m.parts) {
      if (p.type !== "tool") continue;
      const op = OP_OF[p.tool.toLowerCase()];
      if (!op) continue;
      const raw = p.state.input.filePath ?? p.state.input.path ?? p.state.input.file;
      if (typeof raw !== "string" || !raw) continue;
      const at = p.state.time?.end ?? p.state.time?.start ?? m.info.time.created;
      const cur = map.get(raw) ?? { path: raw, ops: [], count: 0, lastAt: 0, failed: false };
      if (!cur.ops.includes(op)) cur.ops.push(op);
      cur.count++;
      cur.lastAt = Math.max(cur.lastAt, at);
      if (p.state.status === "error") cur.failed = true;
      map.set(raw, cur);
    }
  }
  return [...map.values()].sort((a, b) => b.lastAt - a.lastAt);
}

const normPath = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();

/**
 * Lengkapi daftar dari tool dengan perubahan menurut snapshot engine. Berkas yang dibuat/diubah lewat `bash`
 * (skrip, generator dokumen) tidak tercatat sebagai tool read/edit/write, tetapi tampil di snapshot.
 * Berkas yang dihapus dilewati karena tidak bisa dibuka. `at` = waktu untuk entri yang hanya ada di snapshot.
 */
export function mergeSnapshotFiles(
  touched: TouchedFile[],
  diffs: { file: string; status: "added" | "deleted" | "modified" }[],
  directory: string,
  at: number,
): TouchedFile[] {
  const seen = new Set(touched.map((f) => normPath(f.path)));
  const extra: TouchedFile[] = [];
  for (const d of diffs) {
    if (d.status === "deleted") continue;
    const abs = /^([a-zA-Z]:[\\/]|\/)/.test(d.file) ? d.file : `${directory.replace(/[\\/]+$/, "")}/${d.file}`;
    const key = normPath(abs);
    if (seen.has(key)) continue;
    seen.add(key);
    extra.push({ path: abs, ops: [d.status === "added" ? "write" : "edit"], count: 1, lastAt: at, failed: false });
  }
  return [...touched, ...extra].sort((a, b) => b.lastAt - a.lastAt);
}

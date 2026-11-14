// Info sesi (Panel 4): penggunaan token/biaya/konteks dihitung dari pesan sesi (fungsi murni, diuji).
import type { ChatMessage, MessagePart, ToolPart } from "../chat/types";

export type SessionUsage = {
  messages: { user: number; assistant: number };
  toolCalls: { total: number; completed: number; failed: number; running: number; byTool: Record<string, number> };
  tokens: { input: number; output: number; reasoning: number; total: number };
  /** Biaya yang dilaporkan engine (USD). null bila tidak ada pesan yang melaporkan biaya. */
  cost: number | null;
  /** Model pesan asisten terakhir. */
  model: { providerID: string | null; modelID: string | null };
  /** Token masukan pada giliran terakhir ≈ ukuran konteks yang dikirim. */
  contextTokens: number | null;
  /** Batas konteks model (bila diketahui) dan persentase terpakai. */
  contextLimit: number | null;
  contextPercent: number | null;
  /** Token tersisa di jendela konteks (null bila batas/pemakaian tidak diketahui). */
  contextRemaining: number | null;
  /** Batas token keluaran model (null bila tidak diketahui). */
  outputLimit: number | null;
  /** Total waktu asisten bekerja (ms), dari pesan yang sudah selesai. */
  workMs: number;
  firstAt: number | null;
  lastAt: number | null;
};

const tools = (m: ChatMessage): ToolPart[] => m.parts.filter((p: MessagePart): p is ToolPart => p.type === "tool");

export function sessionUsage(
  messages: ChatMessage[],
  contextLimit: number | null = null,
  outputLimit: number | null = null,
): SessionUsage {
  const u: SessionUsage = {
    messages: { user: 0, assistant: 0 },
    toolCalls: { total: 0, completed: 0, failed: 0, running: 0, byTool: {} },
    tokens: { input: 0, output: 0, reasoning: 0, total: 0 },
    cost: null,
    model: { providerID: null, modelID: null },
    contextTokens: null,
    contextLimit,
    outputLimit,
    contextRemaining: null,
    contextPercent: null,
    workMs: 0,
    firstAt: null,
    lastAt: null,
  };
  for (const m of messages) {
    const t = m.info.time;
    if (t.created > 0) {
      u.firstAt = u.firstAt === null ? t.created : Math.min(u.firstAt, t.created);
      u.lastAt = Math.max(u.lastAt ?? 0, t.completed ?? t.created);
    }
    if (m.info.role === "user") {
      u.messages.user++;
      continue;
    }
    u.messages.assistant++;
    if (m.info.providerID) u.model = { providerID: m.info.providerID, modelID: m.info.modelID ?? null };
    if (t.completed !== undefined && t.created > 0) u.workMs += Math.max(0, t.completed - t.created);
    const k = m.info.tokens;
    if (k) {
      u.tokens.input += k.input;
      u.tokens.output += k.output;
      u.tokens.reasoning += k.reasoning;
      u.tokens.total += k.total;
      if (k.input > 0) u.contextTokens = k.input;
    }
    if (typeof m.info.cost === "number") u.cost = (u.cost ?? 0) + m.info.cost;
    for (const p of tools(m)) {
      u.toolCalls.total++;
      u.toolCalls.byTool[p.tool] = (u.toolCalls.byTool[p.tool] ?? 0) + 1;
      if (p.state.status === "completed") u.toolCalls.completed++;
      else if (p.state.status === "error") u.toolCalls.failed++;
      else u.toolCalls.running++;
    }
  }
  if (contextLimit && u.contextTokens !== null) {
    u.contextPercent = Math.min(100, (u.contextTokens / contextLimit) * 100);
    u.contextRemaining = Math.max(0, contextLimit - u.contextTokens);
  }
  return u;
}

/** Singkat angka token: 950 → "950", 12400 → "12,4k", 2_300_000 → "2,3M" (locale mengatur pemisah desimal). */
export function formatTokens(n: number, locale = "id"): string {
  const f = (v: number) => new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(v);
  if (n >= 1_000_000) return `${f(n / 1_000_000)}M`;
  if (n >= 10_000) return `${f(n / 1000)}k`;
  if (n >= 1000) return `${f(Math.round(n / 100) / 10)}k`;
  return String(Math.round(n));
}

export type UsageLevel = "ok" | "warn" | "high";

/** Tingkat kepenuhan jendela konteks: ≥ 80% peringatan, ≥ 95% hampir penuh. null (tak diketahui) → "ok". */
export function usageLevel(percent: number | null): UsageLevel {
  if (percent === null) return "ok";
  if (percent >= 95) return "high";
  if (percent >= 80) return "warn";
  return "ok";
}

// Logika murni untuk panel "Trajectory" (ADR-073): buku besar per giliran (turn) yang memperlihatkan proses di balik layar
// — pesan pengguna, konteks sisipan, setiap panggilan model (model, agent, token, biaya, durasi), berpikir, jawaban, setiap
// tool call (input, keluaran, metadata, waktu), batas langkah, dan part lain yang dikirim engine apa adanya.
// Hanya dari data sesi yang benar-benar ada: rekaman tanpa waktu dibiarkan tanpa waktu (tidak ada durasi karangan).
import type { ChatMessage, MessagePart, TokenUsage } from "@zandor/core";
import { toolSummary } from "./process-label";

export type TrajKind = "user" | "context" | "model" | "thinking" | "assistant" | "tool" | "step" | "error" | "other";
/** Jalur pada ikhtisar waktu: masukan (pengguna/konteks), model, dan tool. */
export type TrajLane = "input" | "model" | "tools";
export type TrajStatus = "pending" | "running" | "completed" | "error";

export type TrajRecord = {
  id: string;
  /** Urutan global (0..n-1) untuk navigasi dan mode urutan di ikhtisar. */
  index: number;
  turn: number;
  kind: TrajKind;
  lane: TrajLane;
  /** Nama singkat: nama tool, jenis part mentah, atau model. */
  name?: string;
  /** Satu baris ringkas untuk kolom isi. */
  summary: string;
  /** Pratinjau satu baris keluaran (tool/jawaban) untuk kolom hasil. */
  preview?: string;
  /** ms epoch; undefined bila engine tidak mencatat waktu rekaman ini. */
  start?: number;
  end?: number;
  status?: TrajStatus;
  messageId: string;
  tokens?: TokenUsage;
  cost?: number;
  /** Teks masukan lengkap (pesan pengguna, argumen tool sebagai JSON). */
  input?: string;
  /** Teks keluaran lengkap (keluaran tool, galat, isi jawaban/berpikir). */
  output?: string;
  /** Objek mentah (part atau info pesan) untuk tab Raw. */
  raw: unknown;
};

export type TrajTurn = {
  turn: number;
  start?: number;
  end?: number;
  records: TrajRecord[];
  tokens: TokenUsage;
  cost: number;
  calls: number;
  modelCalls: number;
  errors: number;
};

const ZERO: TokenUsage = { input: 0, output: 0, reasoning: 0, total: 0 };
const PREVIEW_MAX = 200;
const oneLine = (s: string) => s.replace(/\s+/g, " ").trim();
const clip = (s: string, n = PREVIEW_MAX) => (s.length > n ? `${s.slice(0, n)}…` : s);
const json = (v: unknown) => {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
};
const addTokens = (a: TokenUsage, b?: TokenUsage): TokenUsage =>
  b
    ? {
        input: a.input + (b.input || 0),
        output: a.output + (b.output || 0),
        reasoning: a.reasoning + (b.reasoning || 0),
        total: a.total + (b.total || 0),
      }
    : a;

/** Durasi rekaman dalam ms, atau undefined bila awal/akhir tak tercatat. */
export const durationOf = (r: Pick<TrajRecord, "start" | "end">): number | undefined =>
  r.start !== undefined && r.end !== undefined && r.end >= r.start ? r.end - r.start : undefined;

export function formatMs(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(ms < 10_000 ? 2 : 1)} s`;
  const m = Math.floor(ms / 60_000);
  return `${m}m ${Math.round((ms % 60_000) / 1000)}s`;
}

export function formatTokens(n: number): string {
  return n >= 10_000 ? `${(n / 1000).toFixed(1)}k` : String(n);
}

function laneOf(kind: TrajKind): TrajLane {
  if (kind === "tool") return "tools";
  if (kind === "user" || kind === "context") return "input";
  return "model";
}

/** Bangun buku besar trajectory dari pesan sesi. Giliran baru dimulai di setiap pesan pengguna. */
export function buildTrajectory(messages: ChatMessage[]): TrajTurn[] {
  const turns: TrajTurn[] = [];
  let cur: TrajTurn | null = null;
  let index = 0;
  const open = (start?: number) => {
    cur = {
      turn: turns.length + 1,
      ...(start !== undefined ? { start } : {}),
      records: [],
      tokens: ZERO,
      cost: 0,
      calls: 0,
      modelCalls: 0,
      errors: 0,
    };
    turns.push(cur);
    return cur;
  };
  const push = (t: TrajTurn, r: Omit<TrajRecord, "index" | "turn" | "lane">) => {
    const rec: TrajRecord = { ...r, index: index++, turn: t.turn, lane: laneOf(r.kind) };
    t.records.push(rec);
    const stamps = [rec.start, rec.end].filter((x): x is number => x !== undefined);
    for (const s of stamps) {
      if (t.start === undefined || s < t.start) t.start = s;
      if (t.end === undefined || s > t.end) t.end = s;
    }
    if (rec.status === "error" || rec.kind === "error") t.errors++;
  };

  for (const m of messages) {
    const info = m.info;
    if (info.role === "user") {
      const t = open(info.time.created);
      const texts = m.parts.filter((p) => p.type === "text" && !p.synthetic);
      const text = texts.map((p) => (p.type === "text" ? p.text : "")).join("\n");
      const files = m.parts.filter((p) => p.type === "file");
      push(t, {
        id: info.id,
        kind: "user",
        summary: clip(oneLine(text)) || (files.length ? `[${files.length}]` : ""),
        start: info.time.created,
        messageId: info.id,
        input: text,
        raw: { info, parts: texts },
      });
      for (const p of m.parts) {
        if (p.type === "text" && p.synthetic) {
          push(t, {
            id: p.id,
            kind: "context",
            summary: clip(oneLine(p.text)),
            start: info.time.created,
            messageId: info.id,
            input: p.text,
            raw: p,
          });
        } else if (p.type === "file") {
          push(t, {
            id: p.id,
            kind: "context",
            name: "file",
            summary: `${p.filename ?? p.url.slice(0, 80)} · ${p.mime}`,
            start: info.time.created,
            messageId: info.id,
            raw: { ...p, url: p.url.startsWith("data:") ? `${p.url.slice(0, 60)}…` : p.url },
          });
        } else if (p.type === "other") {
          pushOther(t, p, info.time.created);
        }
      }
      continue;
    }

    // Pesan assistant = satu panggilan model (bisa beberapa langkah). Sesi bisa diawali assistant tanpa pesan pengguna.
    const t: TrajTurn = cur ?? open(info.time.created);
    t.modelCalls++;
    t.tokens = addTokens(t.tokens, info.tokens);
    t.cost += info.cost ?? 0;
    const status: TrajStatus = info.error ? "error" : info.time.completed ? "completed" : "running";
    push(t, {
      id: `${info.id}:model`,
      kind: "model",
      name: [info.providerID, info.modelID].filter(Boolean).join("/") || undefined,
      summary: [info.agent, info.finish].filter(Boolean).join(" · "),
      start: info.time.created,
      ...(info.time.completed ? { end: info.time.completed } : {}),
      status,
      messageId: info.id,
      ...(info.tokens ? { tokens: info.tokens } : {}),
      ...(info.cost !== undefined ? { cost: info.cost } : {}),
      raw: info,
    });

    let step = 1;
    for (const p of m.parts as MessagePart[]) {
      switch (p.type) {
        case "reasoning":
          if (!p.text.trim()) break;
          push(t, {
            id: p.id,
            kind: "thinking",
            summary: clip(oneLine(p.text)),
            messageId: info.id,
            output: p.text,
            raw: p,
          });
          break;
        case "text":
          if (!p.text.trim()) break;
          push(t, {
            id: p.id,
            kind: p.synthetic ? "context" : "assistant",
            summary: clip(oneLine(p.text)),
            messageId: info.id,
            output: p.text,
            raw: p,
          });
          break;
        case "tool": {
          t.calls++;
          const s = p.state;
          const out = s.error ?? s.output ?? "";
          push(t, {
            id: p.id,
            kind: "tool",
            name: p.tool,
            summary: clip(oneLine(toolSummary(p.tool, s.input, s.title))),
            ...(out ? { preview: clip(oneLine(out)) } : {}),
            ...(s.time?.start !== undefined ? { start: s.time.start } : {}),
            ...(s.time?.end !== undefined ? { end: s.time.end } : {}),
            status: s.status,
            messageId: info.id,
            input: json(s.input),
            ...(out ? { output: out } : {}),
            raw: p,
          });
          break;
        }
        case "step-finish":
          push(t, {
            id: p.id,
            kind: "step",
            name: `#${step++}`,
            summary: p.reason,
            messageId: info.id,
            tokens: p.tokens,
            cost: p.cost,
            raw: p,
          });
          break;
        case "file":
          push(t, {
            id: p.id,
            kind: "assistant",
            name: "file",
            summary: `${p.filename ?? ""} · ${p.mime}`,
            messageId: info.id,
            raw: { ...p, url: p.url.startsWith("data:") ? `${p.url.slice(0, 60)}…` : p.url },
          });
          break;
        case "other":
          pushOther(t, p);
          break;
      }
    }
    if (info.error) {
      push(t, {
        id: `${info.id}:error`,
        kind: "error",
        name: info.error.name,
        summary: clip(oneLine(info.error.message)),
        ...(info.time.completed ? { start: info.time.completed } : {}),
        status: "error",
        messageId: info.id,
        output: info.error.message,
        raw: info.error,
      });
    }
  }
  return turns;

  function pushOther(t: TrajTurn, p: Extract<MessagePart, { type: "other" }>, at?: number) {
    push(t, {
      id: p.id,
      kind: "other",
      name: p.rawType,
      summary: p.rawType,
      ...(at !== undefined ? { start: at } : {}),
      messageId: p.messageID,
      raw: p,
    });
  }
}

/** Ringkasan seluruh sesi (untuk bilah status panel). */
export function totals(turns: TrajTurn[]) {
  let tokens = ZERO;
  let cost = 0;
  let calls = 0;
  let modelCalls = 0;
  let errors = 0;
  for (const t of turns) {
    tokens = addTokens(tokens, t.tokens);
    cost += t.cost;
    calls += t.calls;
    modelCalls += t.modelCalls;
    errors += t.errors;
  }
  return { tokens, cost, calls, modelCalls, errors, records: turns.reduce((n, t) => n + t.records.length, 0) };
}

/** Pencarian teks di ringkasan, nama, masukan, dan keluaran (tanpa membedakan huruf besar/kecil). */
export function matchesQuery(r: TrajRecord, q: string): boolean {
  if (!q) return true;
  const n = q.toLowerCase();
  return [r.summary, r.name, r.input, r.output, r.preview].some((s) => s?.toLowerCase().includes(n));
}

export type TimelineSpan = {
  index: number;
  lane: TrajLane;
  kind: TrajKind;
  left: number;
  width: number;
  error: boolean;
};
export type TimelineModel = {
  spans: TimelineSpan[];
  boundaries: { turn: number; left: number }[];
  start: number;
  end: number;
};

/**
 * Proyeksikan rekaman ke ikhtisar tiga jalur dalam persen (0–100).
 * Mode "time": posisi dari waktu tercatat (rekaman tanpa waktu dilewati). Mode "sequence": satu slot per rekaman.
 */
export function buildTimeline(turns: TrajTurn[], mode: "time" | "sequence", now = Date.now()): TimelineModel | null {
  const recs = turns.flatMap((t) => t.records);
  if (recs.length === 0) return null;
  if (mode === "sequence") {
    const n = recs.length;
    const boundaries = turns
      .filter((t) => t.records[0])
      .map((t) => ({ turn: t.turn, left: ((t.records[0]?.index ?? 0) / n) * 100 }));
    return {
      start: 0,
      end: n,
      boundaries,
      spans: recs.map((r) => ({
        index: r.index,
        lane: r.lane,
        kind: r.kind,
        left: (r.index / n) * 100,
        width: 100 / n,
        error: r.status === "error" || r.kind === "error",
      })),
    };
  }
  const timed = recs.filter((r) => r.start !== undefined);
  if (timed.length === 0) return null;
  const start = Math.min(...timed.map((r) => r.start as number));
  const ends = timed.map((r) => r.end ?? (r.status === "running" ? now : (r.start as number)));
  const end = Math.max(...ends, start + 1);
  const span = end - start;
  const pct = (x: number) => ((x - start) / span) * 100;
  return {
    start,
    end,
    boundaries: turns.filter((t) => t.start !== undefined).map((t) => ({ turn: t.turn, left: pct(t.start as number) })),
    spans: timed.map((r, i) => ({
      index: r.index,
      lane: r.lane,
      kind: r.kind,
      left: pct(r.start as number),
      width: Math.max(0, pct(ends[i] as number) - pct(r.start as number)),
      error: r.status === "error" || r.kind === "error",
    })),
  };
}

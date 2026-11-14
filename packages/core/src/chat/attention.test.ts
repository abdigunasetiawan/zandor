import { describe, expect, test } from "vitest";
import { AttentionTracker, failureOf, isLimitError, SETTLE_MS } from "./attention";
import { initialChatState } from "./reducer";
import type { SessionChat } from "./store";
import type { ChatMessage } from "./types";

const session = (over: Partial<SessionChat> = {}): SessionChat => ({
  ...initialChatState("s1"),
  loaded: true,
  directory: "C:\\w",
  sending: false,
  loading: false,
  reconnecting: false,
  ...over,
});
const busy = (): Partial<SessionChat> => ({ status: { type: "busy" } });
const asst = (error?: { name: string; message: string }): ChatMessage => ({
  info: {
    id: "a1",
    sessionID: "s1",
    role: "assistant",
    time: { created: 1, completed: 2 },
    ...(error ? { error } : {}),
  },
  parts: [],
});
const away = { viewing: null, focused: true, now: 0 };
/** Amati lalu amati lagi setelah mengendap (akhir giliran dinilai setelah SETTLE_MS). */
const settle = (
  t: AttentionTracker,
  sessions: Record<string, SessionChat>,
  ctx: { viewing: string | null; focused: boolean; now?: number },
) => [...t.observe(sessions, { ...ctx, now: 0 }), ...t.observe(sessions, { ...ctx, now: SETTLE_MS + 1 })];
const perm = { id: "p1", sessionID: "s1", permission: "edit", patterns: ["a.ts"], metadata: {}, always: [] };

describe("isLimitError / failureOf", () => {
  test("mengenali batas penyedia dan membedakannya dari galat biasa; abort pengguna bukan kegagalan", () => {
    for (const m of [
      "Rate limit exceeded",
      "429 Too Many Requests",
      "You exceeded your quota",
      "insufficient credits",
      "context length exceeded",
    ])
      expect(isLimitError(m), m).toBe(true);
    expect(isLimitError("koneksi terputus")).toBe(false);
    expect(failureOf(session({ messages: [asst({ name: "APIError", message: "rate limit reached" })] }))).toEqual({
      kind: "limit",
      message: "rate limit reached",
    });
    expect(failureOf(session({ messages: [asst({ name: "X", message: "boom" })] }))?.kind).toBe("failed");
    expect(failureOf(session({ messages: [asst({ name: "MessageAbortedError", message: "dibatalkan" })] }))).toBeNull();
    expect(failureOf(session({ error: "gagal mengirim: jaringan" }))?.message).toBe("gagal mengirim: jaringan");
    expect(failureOf(session())).toBeNull();
  });
});

describe("AttentionTracker", () => {
  test("sesi yang baru terlihat tidak memicu apa pun; selesai saat tidak dilihat → peristiwa done + penanda belum dilihat", () => {
    const t = new AttentionTracker();
    expect(t.observe({ s1: session(busy()) }, away)).toEqual([]); // pengamatan pertama
    expect(t.indicator("s1", session(busy()))).toBe("busy");
    const events = settle(t, { s1: session() }, away);
    expect(events).toEqual([{ type: "done", sessionId: "s1" }]);
    expect(t.state().unseen).toEqual({ s1: "done" });
    expect(t.indicator("s1", session())).toBe("done");
    t.markSeen("s1");
    expect(t.indicator("s1", session())).toBe("idle");
  });

  test("selesai saat sesi dilihat dan jendela fokus → tetap diberitahukan (pemanggil memutuskan) tetapi tidak jadi 'belum dilihat'", () => {
    const t = new AttentionTracker();
    t.observe({ s1: session(busy()) }, { viewing: "s1", focused: true });
    const events = settle(t, { s1: session() }, { viewing: "s1", focused: true });
    expect(events).toHaveLength(1);
    expect(t.state().unseen).toEqual({});
  });

  test("gagal dan batas penyedia: peristiwa + indikator, abort pengguna tidak", () => {
    const t = new AttentionTracker();
    t.observe({ s1: session(busy()) }, away);
    const failed = session({ messages: [asst({ name: "APIError", message: "quota exceeded" })] });
    expect(settle(t, { s1: failed }, away)).toEqual([{ type: "limit", sessionId: "s1", message: "quota exceeded" }]);
    expect(t.indicator("s1", failed)).toBe("limit");
    t.observe({ s1: session(busy()) }, away);
    const aborted = session({ messages: [asst({ name: "MessageAbortedError", message: "x" })] });
    expect(settle(t, { s1: aborted }, away)).toEqual([{ type: "done", sessionId: "s1" }]);
  });

  test("butuh aksi: izin atau pertanyaan baru memicu peristiwa; indikator 'action' menimpa 'busy'", () => {
    const t = new AttentionTracker();
    t.observe({ s1: session(busy()) }, away);
    const waiting = session({ ...busy(), permissions: [perm] });
    expect(t.observe({ s1: waiting }, away)).toEqual([
      { type: "action", sessionId: "s1", kind: "permission", detail: "edit: a.ts" },
    ]);
    expect(t.indicator("s1", waiting)).toBe("action");
    expect(t.observe({ s1: waiting }, away)).toEqual([]); // tidak berulang
    const q = session({
      ...busy(),
      questions: [
        {
          id: "q1",
          sessionID: "s1",
          questions: [{ question: "Pilih?", header: "", options: [], multiple: false, custom: true }],
        },
      ],
    });
    const t2 = new AttentionTracker();
    t2.observe({ s1: session(busy()) }, away);
    expect(t2.observe({ s1: q }, away)).toEqual([
      { type: "action", sessionId: "s1", kind: "question", detail: "Pilih?" },
    ]);
  });

  test("giliran baru menghapus penanda 'selesai'; melihat sesi (fokus) juga menghapusnya", () => {
    const t = new AttentionTracker();
    t.observe({ s1: session(busy()) }, away);
    settle(t, { s1: session() }, away);
    expect(t.state().unseen.s1).toBe("done");
    t.observe({ s1: session(busy()) }, away);
    expect(t.state().unseen).toEqual({});
    settle(t, { s1: session() }, away);
    expect(t.state().unseen.s1).toBe("done");
    t.observe({ s1: session() }, { viewing: "s1", focused: true });
    expect(t.state().unseen).toEqual({});
  });

  test("subscribe dipanggil saat penanda berubah", () => {
    const t = new AttentionTracker();
    let n = 0;
    t.subscribe(() => n++);
    t.observe({ s1: session(busy()) }, away);
    settle(t, { s1: session() }, away);
    expect(n).toBeGreaterThan(0);
  });
});

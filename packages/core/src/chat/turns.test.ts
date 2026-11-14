import { describe, expect, test } from "vitest";
import { mergeAssistantTurns } from "./turns";
import type { ChatMessage, MessagePart } from "./types";

const text = (id: string, mid: string, t: string): MessagePart => ({
  id,
  messageID: mid,
  sessionID: "s",
  type: "text",
  text: t,
  synthetic: false,
});
const tool = (id: string, mid: string): MessagePart => ({
  id,
  messageID: mid,
  sessionID: "s",
  type: "tool",
  tool: "write",
  callID: id,
  state: { status: "completed", input: { filePath: "hello.md" } },
});
const msg = (
  id: string,
  role: "user" | "assistant",
  created: number,
  completed: number | undefined,
  parts: MessagePart[],
): ChatMessage => ({
  info: {
    id,
    sessionID: "s",
    role,
    time: { created, ...(completed !== undefined ? { completed } : {}) },
    modelID: "m",
  },
  parts,
});

describe("penggabungan giliran asisten", () => {
  test("pesan asisten berurutan jadi satu: waktu mulai pertama, selesai terakhir, part berurut, id sumber tercatat", () => {
    const merged = mergeAssistantTurns([
      msg("u1", "user", 0, 0, [text("p0", "u1", "buat hello.md")]),
      msg("a1", "assistant", 1000, 3000, [tool("p1", "a1")]),
      msg("a2", "assistant", 3000, 4000, [text("p2", "a2", "Dibuat: hello.md")]),
    ]);
    expect(merged.map((m) => m.info.id)).toEqual(["u1", "a2"]);
    const a = merged[1];
    expect(a?.info.time).toEqual({ created: 1000, completed: 4000 });
    expect(a?.parts.map((p) => p.id)).toEqual(["p1", "p2"]);
    expect(a?.sourceIds).toEqual(["a1", "a2"]);
  });
  test("pesan pengguna memisahkan giliran; tidak ada penggabungan lintas giliran", () => {
    const merged = mergeAssistantTurns([
      msg("a1", "assistant", 1, 2, [text("p1", "a1", "satu")]),
      msg("u2", "user", 3, 3, [text("p2", "u2", "lagi")]),
      msg("a2", "assistant", 4, 5, [text("p3", "a2", "dua")]),
    ]);
    expect(merged.map((m) => m.info.id)).toEqual(["a1", "u2", "a2"]);
  });
  test("selama berjalan: belum selesai → completed tidak ada; galat pesan mana pun terbawa", () => {
    const withError = msg("a2", "assistant", 3, undefined, []);
    const first = msg("a1", "assistant", 1, 2, []);
    first.info.error = { name: "X", message: "gagal" };
    const [m] = mergeAssistantTurns([first, withError]);
    expect(m?.info.time).toEqual({ created: 1 });
    expect(m?.info.error?.message).toBe("gagal");
  });
});

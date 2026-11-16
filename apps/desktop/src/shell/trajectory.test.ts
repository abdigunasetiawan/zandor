import type { ChatMessage } from "@zandor/core";
import { describe, expect, it } from "vitest";
import { buildTimeline, buildTrajectory, durationOf, matchesQuery, totals } from "./trajectory";

const base = { sessionID: "s" };
const msgs: ChatMessage[] = [
  {
    info: { id: "u1", ...base, role: "user", time: { created: 1000 } },
    parts: [
      { id: "p1", messageID: "u1", ...base, type: "text", text: "halo", synthetic: false },
      {
        id: "p2",
        messageID: "u1",
        ...base,
        type: "text",
        text: "<system-reminder>ctx</system-reminder>",
        synthetic: true,
      },
    ],
  },
  {
    info: {
      id: "a1",
      ...base,
      role: "assistant",
      time: { created: 1100, completed: 3000 },
      providerID: "deepseek",
      modelID: "v4",
      tokens: { input: 100, output: 20, reasoning: 5, total: 125 },
      cost: 0.01,
    },
    parts: [
      { id: "r1", messageID: "a1", ...base, type: "reasoning", text: "berpikir" },
      {
        id: "t1",
        messageID: "a1",
        ...base,
        type: "tool",
        tool: "bash",
        callID: "c1",
        state: {
          status: "error",
          input: { command: "pnpm dev" },
          error: "pnpm not found",
          time: { start: 1200, end: 1500 },
        },
      },
      { id: "x1", messageID: "a1", ...base, type: "text", text: "Selesai", synthetic: false },
      {
        id: "f1",
        messageID: "a1",
        ...base,
        type: "step-finish",
        reason: "stop",
        cost: 0.01,
        tokens: { input: 100, output: 20, reasoning: 5, total: 125 },
      },
    ],
  },
];

describe("trajectory", () => {
  it("memetakan pesan ke giliran dan rekaman berurutan", () => {
    const turns = buildTrajectory(msgs);
    expect(turns).toHaveLength(1);
    const kinds = turns[0]?.records.map((r) => r.kind);
    expect(kinds).toEqual(["user", "context", "model", "thinking", "tool", "assistant", "step"]);
    const tool = turns[0]?.records.find((r) => r.kind === "tool");
    expect(tool?.name).toBe("bash");
    expect(tool && durationOf(tool)).toBe(300);
    expect(tool?.output).toBe("pnpm not found");
    expect(turns[0]?.records.find((r) => r.kind === "model")?.name).toBe("deepseek/v4");
  });

  it("tidak mengarang waktu untuk rekaman tanpa stempel", () => {
    const thinking = buildTrajectory(msgs)[0]?.records.find((r) => r.kind === "thinking");
    expect(thinking?.start).toBeUndefined();
    expect(thinking && durationOf(thinking)).toBeUndefined();
  });

  it("menghitung total dan galat", () => {
    const s = totals(buildTrajectory(msgs));
    expect(s).toMatchObject({ calls: 1, modelCalls: 1, errors: 1, cost: 0.01 });
    expect(s.tokens.input).toBe(100);
  });

  it("mencari di input dan output", () => {
    const tool = buildTrajectory(msgs)[0]?.records.find((r) => r.kind === "tool");
    expect(tool && matchesQuery(tool, "NOT FOUND")).toBe(true);
    expect(tool && matchesQuery(tool, "zzz")).toBe(false);
  });

  it("ikhtisar waktu hanya memuat rekaman bertanda waktu", () => {
    const turns = buildTrajectory(msgs);
    const tl = buildTimeline(turns, "time");
    expect(tl?.spans.every((s) => s.left >= 0 && s.left <= 100)).toBe(true);
    expect(tl?.spans.some((s) => s.kind === "thinking")).toBe(false);
    expect(buildTimeline(turns, "sequence")?.spans).toHaveLength(7);
  });
});

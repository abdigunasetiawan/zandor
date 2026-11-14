import { describe, expect, test } from "vitest";
import type { ChatMessage } from "../chat";
import { formatTokens, sessionUsage, usageLevel } from "./usage";

describe("formatTokens", () => {
  test("singkat dengan satuan k/M dan pemisah sesuai locale", () => {
    expect(formatTokens(0, "en")).toBe("0");
    expect(formatTokens(950, "en")).toBe("950");
    expect(formatTokens(1234, "en")).toBe("1.2k");
    expect(formatTokens(12_400, "en")).toBe("12.4k");
    expect(formatTokens(200_000, "en")).toBe("200k");
    expect(formatTokens(2_300_000, "en")).toBe("2.3M");
    expect(formatTokens(12_400, "id")).toBe("12,4k");
  });
});

describe("usageLevel", () => {
  test("ambang 80% dan 95%; tidak diketahui → ok", () => {
    expect(usageLevel(null)).toBe("ok");
    expect(usageLevel(0)).toBe("ok");
    expect(usageLevel(79.9)).toBe("ok");
    expect(usageLevel(80)).toBe("warn");
    expect(usageLevel(94.9)).toBe("warn");
    expect(usageLevel(95)).toBe("high");
    expect(usageLevel(100)).toBe("high");
  });
});

describe("sessionUsage: konteks tersisa dan batas keluaran", () => {
  const msg = (input: number): ChatMessage => ({
    info: {
      id: "m",
      sessionID: "s",
      role: "assistant",
      time: { created: 1, completed: 2 },
      providerID: "p",
      modelID: "m",
      tokens: { input, output: 10, reasoning: 0, total: input + 10 },
    },
    parts: [],
  });

  test("dihitung dari pesan terakhir; tanpa batas → null", () => {
    const a = sessionUsage([msg(50_000)], 200_000, 8000);
    expect(a.contextPercent).toBe(25);
    expect(a.contextRemaining).toBe(150_000);
    expect(a.outputLimit).toBe(8000);
    const b = sessionUsage([msg(50_000)]);
    expect(b.contextPercent).toBeNull();
    expect(b.contextRemaining).toBeNull();
    expect(b.outputLimit).toBeNull();
  });

  test("melebihi batas: persen dibatasi 100 dan sisa 0", () => {
    const u = sessionUsage([msg(250_000)], 200_000);
    expect(u.contextPercent).toBe(100);
    expect(u.contextRemaining).toBe(0);
  });
});

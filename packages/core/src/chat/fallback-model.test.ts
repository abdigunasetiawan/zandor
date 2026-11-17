import { describe, expect, it } from "vitest";
import type { ModelOption } from "../engine/api";
import { fallbackModel } from "./store";

const m = (providerID: string, modelID: string, name = modelID): ModelOption => ({
  providerID,
  providerName: providerID,
  modelID,
  name,
  variants: [],
});

describe("model cadangan", () => {
  it("hanya OpenCode bawaan → DeepSeek dari OpenCode", () => {
    const models = [m("opencode", "grok-code"), m("opencode", "deepseek-v3.2"), m("opencode", "big-pickle")];
    expect(fallbackModel(models)).toEqual({ providerID: "opencode", modelID: "deepseek-v3.2" });
  });

  it("DeepSeek dikenali juga dari nama tampilan", () => {
    expect(fallbackModel([m("opencode", "a"), m("opencode", "x1", "DeepSeek V3")])?.modelID).toBe("x1");
  });

  it("ada koneksi lain → tetap model pertama (bukan dipaksa DeepSeek)", () => {
    const models = [m("anthropic", "claude"), m("opencode", "deepseek-v3.2")];
    expect(fallbackModel(models)).toEqual({ providerID: "anthropic", modelID: "claude" });
  });

  it("OpenCode tanpa DeepSeek → model pertama; daftar kosong → null", () => {
    expect(fallbackModel([m("opencode", "big-pickle")])?.modelID).toBe("big-pickle");
    expect(fallbackModel([])).toBeNull();
  });
});

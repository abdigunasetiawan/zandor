import { describe, expect, test } from "vitest";
import { compactTokens, parseModelInfo } from "./model-info";

describe("parseModelInfo", () => {
  test("OpenAI polos (hanya id) → tidak ada info tambahan", () => {
    expect(parseModelInfo({ data: [{ id: "gpt-x", object: "model", owned_by: "me" }] })).toEqual({});
  });
  test("OpenRouter: konteks, keluaran, parameter, modalitas, nama", () => {
    const r = parseModelInfo({
      data: [
        {
          id: "vendor/m1",
          name: "Vendor: M1",
          context_length: 200000,
          top_provider: { max_completion_tokens: 8192 },
          supported_parameters: ["tools", "reasoning", "temperature"],
          architecture: { input_modalities: ["text", "image"] },
        },
      ],
    });
    expect(r["vendor/m1"]).toEqual({
      name: "Vendor: M1",
      context: 200000,
      output: 8192,
      reasoning: true,
      toolCall: true,
      attachment: true,
    });
  });
  test("LM Studio / vLLM: max_context_length, max_model_len, capabilities, type vlm", () => {
    const r = parseModelInfo({
      data: [
        { id: "a", max_context_length: 32768, capabilities: ["tool_use"], type: "vlm" },
        { id: "b", max_model_len: 4096 },
      ],
    });
    expect(r.a).toEqual({ context: 32768, toolCall: true, attachment: true });
    expect(r.b).toEqual({ context: 4096 });
  });
  test("array polos, nilai buruk, dan sampah diabaikan; nama sama dengan id tidak dicatat", () => {
    expect(parseModelInfo("bukan")).toEqual({});
    expect(parseModelInfo([{ id: "x", name: "x", context_length: -5 }, 7, { nope: 1 }])).toEqual({});
  });
});

test("compactTokens", () => {
  expect(compactTokens(128000)).toBe("128K");
  expect(compactTokens(1_000_000)).toBe("1M");
  expect(compactTokens(512)).toBe("512");
});

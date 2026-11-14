// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import { MODEL_LOGOS } from "../model-logos.generated";
import { ModelLogo, modelLogoKey } from "./ModelLogo";

afterEach(cleanup);

describe("modelLogoKey", () => {
  test("keluarga model didahulukan dari provider", () => {
    expect(modelLogoKey("antigravity", "gemini-3.8-flash")).toBe("gemini");
    expect(modelLogoKey("openrouter", "anthropic/claude-sonnet-4")).toBe("claude");
    expect(modelLogoKey("opencode", "gpt-5-codex")).toBe("codex");
    expect(modelLogoKey("x", "o3-mini")).toBe("openai");
    expect(modelLogoKey("x", "deepseek-chat")).toBe("deepseek");
    expect(modelLogoKey("x", "Qwen3-Coder")).toBe("qwen");
    expect(modelLogoKey("x", "glm-4.6")).toBe("zhipu");
  });
  test("provider dipakai bila model tak dikenal; alias dinormalisasi", () => {
    expect(modelLogoKey("openrouter", "model-aneh")).toBe("openrouter");
    expect(modelLogoKey("fireworks-ai", "model-aneh")).toBe("fireworks");
    expect(modelLogoKey("amazon-bedrock", "x")).toBe("bedrock");
    expect(modelLogoKey("github-copilot", "x")).toBe("githubcopilot");
    expect(modelLogoKey("anthropic")).toBe("anthropic");
  });
  test("tidak cocok → null", () => {
    expect(modelLogoKey("zzz-tidak-ada", "model-aneh")).toBeNull();
    expect(modelLogoKey(null, null)).toBeNull();
  });
  test("semua kunci yang dituju resolver ada di data logo", () => {
    for (const k of [
      "claude",
      "openai",
      "codex",
      "gemini",
      "gemma",
      "deepseek",
      "qwen",
      "meta",
      "mistral",
      "grok",
      "zhipu",
    ])
      expect(MODEL_LOGOS[k], k).toBeDefined();
  });
});

describe("ModelLogo", () => {
  test("logo ada → svg dekoratif currentColor", () => {
    const { container } = render(<ModelLogo providerID="antigravity" modelID="gemini-3" />);
    const logo = container.querySelector("[data-logo]");
    expect(logo).toHaveAttribute("data-logo", "gemini");
    expect(logo).toHaveAttribute("aria-hidden", "true");
    // gemini punya varian berwarna merek → gambar data:, bukan svg currentColor
    expect(logo?.tagName).toBe("IMG");
    expect(logo?.getAttribute("src")).toMatch(/^data:image\/svg\+xml,/);
  });
  test("tanpa varian berwarna → svg monokrom currentColor", () => {
    const { container } = render(<ModelLogo providerID="x" modelID="gpt-5" />);
    const logo = container.querySelector("[data-logo]");
    expect(logo?.tagName.toLowerCase()).toBe("svg");
    expect(logo).toHaveAttribute("fill", "currentColor");
  });
  test("logo tidak ada → fallback tile inisial berlabel", () => {
    render(<ModelLogo providerID="zzz-tidak-ada" modelID="model-aneh" name="Zeta Lab" />);
    expect(screen.getByRole("img", { name: "Zeta Lab" })).toHaveTextContent("ZL");
  });
});

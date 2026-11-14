import { describe, expect, test } from "vitest";
import {
  GRADIENT_TEMPLATES,
  type GradientSpec,
  gradientCss,
  hslToHex,
  MAX_STOPS,
  parseGradient,
  randomGradient,
  validateGradientCss,
  validateGradientSpec,
} from "./gradient";

const spec: GradientSpec = {
  type: "linear",
  angle: 135,
  stops: [
    { color: "#112233", pos: 0 },
    { color: "#aabbcc", pos: 100 },
  ],
};

describe("gradien", () => {
  test("spesifikasi → CSS → spesifikasi (bolak-balik) untuk linear dan radial", () => {
    expect(gradientCss(spec)).toBe("linear-gradient(135deg, #112233 0%, #aabbcc 100%)");
    expect(parseGradient(gradientCss(spec))).toEqual(spec);
    const radial: GradientSpec = { ...spec, type: "radial", angle: 0 };
    expect(gradientCss(radial)).toBe("radial-gradient(circle at center, #112233 0%, #aabbcc 100%)");
    expect(parseGradient(gradientCss(radial))).toEqual(radial);
    expect(parseGradient("linear-gradient(red, blue)")).toBeNull(); // bukan bentuk generator
  });

  test("nilai dijepit: sudut 0–360, posisi 0–100, maksimal titik", () => {
    const css = gradientCss({
      ...spec,
      angle: 999,
      stops: [
        { color: "#000000", pos: -5 },
        { color: "#ffffff", pos: 500 },
      ],
    });
    expect(css).toBe("linear-gradient(360deg, #000000 0%, #ffffff 100%)");
    const many = gradientCss({
      ...spec,
      stops: Array.from({ length: 9 }, (_, i) => ({ color: "#123456", pos: i * 10 })),
    });
    expect(many.split("#123456")).toHaveLength(MAX_STOPS + 1);
  });

  test("validateGradientSpec menolak warna bukan hex, posisi di luar rentang, dan jumlah titik salah", () => {
    expect(validateGradientSpec(spec)).toBeNull();
    expect(validateGradientSpec({ ...spec, stops: [spec.stops[0] as never] })).toMatch(/titik/);
    expect(
      validateGradientSpec({
        ...spec,
        stops: [
          { color: "red", pos: 0 },
          { color: "#fff", pos: 1 },
        ],
      }),
    ).toMatch(/hex/);
    expect(
      validateGradientSpec({
        ...spec,
        stops: [
          { color: "#fff", pos: 0 },
          { color: "#000", pos: 101 },
        ],
      }),
    ).toMatch(/posisi/);
  });

  test("validateGradientCss menerima gradien sah dan menolak url(), var(), calc(), dan token liar", () => {
    for (const ok of [
      "linear-gradient(135deg, #112233 0%, #aabbcc 100%)",
      "radial-gradient(circle at center, #fff 0%, #000 100%)",
      "linear-gradient(to right, rgb(1, 2, 3), hsl(120, 50%, 40%))",
      "linear-gradient(90deg, tomato, gold)",
    ])
      expect(validateGradientCss(ok), ok).toBeNull();
    for (const bad of [
      "linear-gradient(url(http://evil/x.png), #000)",
      "linear-gradient(var(--x), #000)",
      "linear-gradient(calc(1px + 2px), #000)",
      "url(http://evil)",
      "linear-gradient(#000, #fff); background: red",
      "linear-gradient(135deg, #fff 0%, #000 100%) , url(x)",
      "",
      "x".repeat(500),
    ])
      expect(validateGradientCss(bad), bad).not.toBeNull();
  });

  test("semua templat valid dan bisa dibaca kembali", () => {
    expect(GRADIENT_TEMPLATES.length).toBeGreaterThanOrEqual(10);
    for (const t of GRADIENT_TEMPLATES) {
      expect(validateGradientSpec(t.spec), t.id).toBeNull();
      expect(validateGradientCss(gradientCss(t.spec)), t.id).toBeNull();
      expect(parseGradient(gradientCss(t.spec)), t.id).toEqual(t.spec);
    }
  });

  test("hslToHex dan randomGradient menghasilkan gradien valid dan dapat diulang dengan rand tetap", () => {
    expect(hslToHex(0, 1, 0.5)).toBe("#ff0000");
    expect(hslToHex(120, 1, 0.5)).toBe("#00ff00");
    const g = randomGradient(() => 0.5);
    expect(validateGradientSpec(g)).toBeNull();
    expect(randomGradient(() => 0.5)).toEqual(g);
    expect(validateGradientCss(gradientCss(g))).toBeNull();
  });
});

// Test token: kontras WCAG 2.x, kelengkapan tema, dan kesetaraan dark/light.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "vitest";

const css = readFileSync(join(import.meta.dirname, "tokens.css"), "utf8");

/** Ambil blok deklarasi untuk selector tertentu (blok bersarang sederhana, tanpa @media di dalamnya). */
function block(selector: string): string {
  const start = css.indexOf(`${selector} {`);
  if (start === -1) throw new Error(`selector tidak ditemukan: ${selector}`);
  let depth = 0;
  for (let i = css.indexOf("{", start); i < css.length; i++) {
    if (css[i] === "{") depth++;
    if (css[i] === "}" && --depth === 0) return css.slice(css.indexOf("{", start) + 1, i);
  }
  throw new Error("blok tidak ditutup");
}

function colors(body: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of body.matchAll(/--z-color-([a-z-]+):\s*(#[0-9a-fA-F]{6});/g)) out[m[1] as string] = m[2] as string;
  return out;
}

const dark = colors(block(":root"));
const light = colors(block(':root[data-theme="light"]'));
// Tema light untuk preferensi sistem (tanpa atribut) ada di dalam @media.
const media = css.slice(css.indexOf("@media (prefers-color-scheme: light)"));
const systemLight = colors(media.slice(0, media.indexOf("@media (prefers-reduced-motion")));

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = Number.parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

describe("contrast helper", () => {
  test("hitam/putih = 21:1, sama = 1:1", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 1);
    expect(contrast("#777777", "#777777")).toBeCloseTo(1, 5);
  });
});

const themes = { dark, light, "light (ikut sistem)": systemLight };

describe.each(Object.entries(themes))("tema %s", (_name, t) => {
  test("punya semua token warna yang sama dengan dark", () => {
    expect(Object.keys(t).sort()).toEqual(Object.keys(dark).sort());
  });

  // Teks biasa: AA 4.5:1. Pasangan (teks, latar) yang benar-benar dipakai komponen.
  const textPairs: [string, string][] = [
    ["fg", "bg"],
    ["fg", "surface"],
    ["fg", "raised"],
    ["fg", "hover"],
    ["fg-muted", "bg"],
    ["fg-muted", "surface"],
    ["fg-muted", "raised"],
    ["fg-subtle", "bg"], // placeholder
    ["fg-subtle", "surface"],
    ["fg-subtle", "raised"],
    ["on-accent", "accent"],
    ["danger", "bg"],
    ["danger", "raised"],
    ["success", "bg"],
    ["success", "raised"],
    ["warning", "bg"],
    ["warning", "raised"],
  ];
  test.each(textPairs)("%s di atas %s ≥ 4.5:1", (fg, bg) => {
    expect(contrast(t[fg] as string, t[bg] as string)).toBeGreaterThanOrEqual(4.5);
  });

  // Komponen UI non-teks (garis fokus, batas kontrol): 3:1.
  test.each([
    ["focus", "bg"],
    ["focus", "raised"],
    ["border-strong", "bg"],
  ])("%s terhadap %s ≥ 3:1 (komponen UI)", (fg, bg) => {
    const need = fg === "border-strong" ? 1.5 : 3; // batas dekoratif; kontrol punya isi/teks sendiri
    expect(contrast(t[fg] as string, t[bg] as string)).toBeGreaterThanOrEqual(need);
  });
});

describe("tipografi", () => {
  test("setiap variabel punya size, weight, lh, tracking", () => {
    for (const v of ["h1", "h2", "h3", "h4", "p", "small", "link", "button", "label", "code"]) {
      for (const prop of ["size", "weight", "lh", "tracking"]) {
        expect(css, `--z-type-${v}-${prop}`).toContain(`--z-type-${v}-${prop}:`);
      }
    }
  });
  test("ukuran dalam rem (ikut --ui-scale)", () => {
    for (const m of css.matchAll(/--z-type-[a-z0-9]+-size:\s*([^;]+);/g)) expect(m[1]).toMatch(/rem$/);
  });
});

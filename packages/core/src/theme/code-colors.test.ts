import { describe, expect, it } from "vitest";
import { computeVars, DEFAULT_APPEARANCE } from "./appearance";
import { codeColorsFromTheme, codeVarsFor, scopeColor } from "./code-colors";
import type { EditorTheme } from "./vscode-theme";

const THEME: EditorTheme = {
  id: "uji",
  name: "Uji",
  base: "vs-dark",
  colors: { "editor.background": "#282a36", "editor.foreground": "#f8f8f2" },
  rules: [],
  tokenColors: [
    { settings: { foreground: "#f8f8f2" } },
    { scope: ["keyword", "storage"], settings: { foreground: "#ff79c6" } },
    { scope: ["string"], settings: { foreground: "#f1fa8c" } },
    { scope: ["string.quoted.double"], settings: { foreground: "#aaaaaa" } },
    { scope: ["meta.tag string"], settings: { foreground: "#000000" } },
    { scope: ["comment"], settings: { foreground: "#6272a4", fontStyle: "italic" } },
    { scope: ["entity.name.function"], settings: { foreground: "#50fa7b" } },
  ],
  source: "berkas",
};

describe("code-colors", () => {
  it("memilih aturan scope paling spesifik, mengabaikan selektor turunan", () => {
    expect(scopeColor(THEME.tokenColors, "string.quoted")).toBe("#f1fa8c");
    expect(scopeColor(THEME.tokenColors, "string.quoted.double.js")).toBe("#aaaaaa");
    expect(scopeColor(THEME.tokenColors, "constant.numeric")).toBeNull();
  });

  it("menurunkan warna kode dari tema VS Code", () => {
    const c = codeColorsFromTheme(THEME);
    expect(c).toMatchObject({
      keyword: "#ff79c6",
      string: "#f1fa8c",
      comment: "#6272a4",
      function: "#50fa7b",
      bg: "#282a36",
      fg: "#f8f8f2",
    });
    expect(c.number).toBeUndefined(); // tidak didefinisikan → token bawaan
  });

  it("tema bawaan memakai palet Monaco; auto tidak menimpa apa pun", () => {
    expect(codeVarsFor("vs", [])["--z-code-bg"]).toBe("#fffffe");
    expect(codeVarsFor("auto", [])).toEqual({});
    expect(codeVarsFor("hilang", [THEME])).toEqual({});
  });

  it("computeVars ikut menyertakan warna kode tema editor", () => {
    const vars = computeVars({ ...DEFAULT_APPEARANCE, editorTheme: "uji", editorThemes: [THEME] }, "dark");
    expect(vars["--z-code-keyword"]).toBe("#ff79c6");
    expect(vars["--z-code-bg"]).toBe("#282a36");
  });
});

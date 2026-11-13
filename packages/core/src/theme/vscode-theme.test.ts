import { strToU8, zipSync } from "fflate";
import { describe, expect, test } from "vitest";
import { AppearanceStore, sanitizeAppearance } from "./appearance";
import { convertVsCodeTheme, sanitizeEditorTheme, stripJsonc, themesFromVsix } from "./vscode-theme";

const THEME = `{
  // komentar baris
  "name": "Senja Kopi",
  "type": "dark",
  "colors": {
    "editor.background": "#1e1e1e", /* komentar blok */
    "editor.foreground": "#d4d4d4",
    "bukan.warna": "url(javascript:alert(1))",
    "editor.selectionBackground": "#264f78aa",
  },
  "tokenColors": [
    { "scope": "comment", "settings": { "foreground": "#6a9955", "fontStyle": "italic" } },
    { "scope": ["string", "string.quoted"], "settings": { "foreground": "#ce9178" } },
    { "scope": "meta.function entity.name.function", "settings": { "foreground": "#dcdcaa" } },
    { "settings": { "foreground": "#cccccc" } },
    { "scope": "kosong", "settings": {} },
    { "scope": "jahat;}", "settings": { "foreground": "#ffffff" } },
    { "scope": "x", "settings": { "foreground": "red" } },
  ],
}`;

describe("stripJsonc", () => {
  test("komentar dan koma penutup dibuang; isi string aman", () => {
    expect(JSON.parse(stripJsonc('{ "a": "http://x // bukan komentar", /* c */ "b": [1, 2,], }'))).toEqual({
      a: "http://x // bukan komentar",
      b: [1, 2],
    });
  });
});

describe("convertVsCodeTheme", () => {
  test("warna valid disimpan, nilai berbahaya dibuang, aturan token dikonversi", () => {
    const { theme, errors } = convertVsCodeTheme(THEME);
    expect(errors).toEqual([]);
    expect(theme?.id).toBe("senja-kopi");
    expect(theme?.base).toBe("vs-dark");
    expect(theme?.colors).toEqual({
      "editor.background": "#1e1e1e",
      "editor.foreground": "#d4d4d4",
      "editor.selectionBackground": "#264f78aa",
    });
    expect(theme?.rules).toEqual([
      { token: "comment", foreground: "6a9955", fontStyle: "italic" },
      { token: "string", foreground: "ce9178" },
      { token: "string.quoted", foreground: "ce9178" },
      { token: "entity.name.function", foreground: "dcdcaa" },
      { token: "", foreground: "cccccc" },
    ]);
  });

  test("tipe terang → base vs; nama dari opsi; hc", () => {
    expect(
      convertVsCodeTheme('{"type":"light","colors":{"editor.background":"#fff"}}', { name: "Terang" }).theme?.base,
    ).toBe("vs");
    expect(
      convertVsCodeTheme('{"colors":{"editor.background":"#000"}}', { name: "X", uiTheme: "hc-black" }).theme?.base,
    ).toBe("hc-black");
  });

  test("galat: bukan JSON, tanpa nama, kosong, terlalu besar", () => {
    expect(convertVsCodeTheme("{bukan").errors[0]).toMatch(/bukan JSON/);
    expect(convertVsCodeTheme('{"colors":{"editor.background":"#000"}}').errors[0]).toMatch(/nama/);
    expect(convertVsCodeTheme('{"name":"A","colors":{}}').errors[0]).toMatch(/tidak memuat/);
    expect(convertVsCodeTheme("[]").errors[0]).toMatch(/objek/);
    expect(convertVsCodeTheme(" ".repeat(400_001)).errors[0]).toMatch(/terlalu besar/);
  });
});

describe("themesFromVsix", () => {
  const vsix = (files: Record<string, string>) =>
    zipSync(Object.fromEntries(Object.entries(files).map(([k, v]) => [k, strToU8(v)])));

  test("membaca tema dari contributes.themes; yang rusak dilaporkan, bukan dilempar", () => {
    const pkg = {
      contributes: {
        themes: [
          { label: "Dracula Gelap", uiTheme: "vs-dark", path: "./themes/dark.json" },
          { label: "Dracula Terang", uiTheme: "vs", path: "./themes/light.json" },
          { label: "Hilang", path: "./themes/none.json" },
          { label: "Traversal", path: "../../etc/passwd" },
          { label: "Rusak", path: "./themes/bad.json" },
        ],
      },
    };
    const bytes = vsix({
      "extension/package.json": JSON.stringify(pkg),
      "extension/themes/dark.json":
        '{"colors":{"editor.background":"#282a36"},"tokenColors":[{"scope":"comment","settings":{"foreground":"#6272a4"}}]}',
      "extension/themes/light.json": '{"type":"light","colors":{"editor.background":"#ffffff"}}',
      "extension/themes/bad.json": "{rusak",
      "extension/readme.md": "abaikan",
    });
    const r = themesFromVsix(bytes, "openvsx:dr.dracula@1.0.0");
    expect(r.errors).toEqual([]);
    expect(r.themes.map((t) => [t.id, t.base, t.source])).toEqual([
      ["dracula-gelap", "vs-dark", "openvsx:dr.dracula@1.0.0"],
      ["dracula-terang", "vs", "openvsx:dr.dracula@1.0.0"],
    ]);
    expect(r.skipped.join("|")).toMatch(/Hilang.*tidak ada/);
    expect(r.skipped.join("|")).toMatch(/Traversal.*path tidak valid/);
    expect(r.skipped.join("|")).toMatch(/Rusak/);
  });

  test("bukan zip, tanpa package.json, tanpa tema", () => {
    expect(themesFromVsix(new Uint8Array([1, 2, 3]), "x").errors[0]).toMatch(/tidak bisa dibuka/);
    expect(themesFromVsix(vsix({ "extension/lain.json": "{}" }), "x").errors[0]).toMatch(
      /package.json tidak ditemukan/,
    );
    expect(themesFromVsix(vsix({ "extension/package.json": '{"contributes":{}}' }), "x").errors[0]).toMatch(
      /tidak memuat tema/,
    );
    expect(themesFromVsix(vsix({ "extension/package.json": "{rusak" }), "x").errors[0]).toMatch(/rusak/);
  });
});

describe("tema editor di AppearanceStore", () => {
  const theme = convertVsCodeTheme(THEME).theme;
  const mem = () => {
    let v: string | null = null;
    return {
      get: () => v,
      set: (x: string) => {
        v = x;
      },
    };
  };

  test("tambah, pilih, hapus mengembalikan ke auto; bawaan Monaco boleh dipilih", () => {
    const st = new AppearanceStore(mem());
    expect(st.addEditorTheme(theme)).toBeNull();
    st.setEditorTheme("senja-kopi");
    expect(st.state().editorTheme).toBe("senja-kopi");
    st.setEditorTheme("hc-black");
    expect(st.state().editorTheme).toBe("hc-black");
    expect(() => st.setEditorTheme("tidak-ada")).toThrow(/tidak dikenal/);
    st.setEditorTheme("senja-kopi");
    st.removeEditorTheme("senja-kopi");
    expect(st.state().editorTheme).toBe("auto");
    expect(st.state().editorThemes).toEqual([]);
  });

  test("menolak tema tidak valid, id bawaan, dan melebihi batas", () => {
    const st = new AppearanceStore(mem());
    expect(st.addEditorTheme({ id: "x" })).toMatch(/tidak valid/);
    expect(st.addEditorTheme({ ...theme, id: "vs-dark" })).toMatch(/dipakai tema bawaan/);
    for (let i = 0; i < 20; i++) expect(st.addEditorTheme({ ...theme, id: `t-${i}` })).toBeNull();
    expect(st.addEditorTheme({ ...theme, id: "t-21" })).toMatch(/maksimal/);
    expect(st.addEditorTheme({ ...theme, id: "t-3" })).toBeNull(); // mengganti yang ada
  });

  test("data tersimpan dibersihkan: aturan/warna jahat dibuang, pilihan tak dikenal → auto", () => {
    const a = sanitizeAppearance({
      editorTheme: "hilang",
      editorThemes: [
        {
          id: "ok",
          name: "Ok",
          base: "vs",
          colors: { "editor.background": "#fff", x: "url(x)" },
          rules: [
            { token: "a;b", foreground: "ffffff" },
            { token: "comment", foreground: "nothex" },
          ],
        },
        { id: "Bad Id", name: "x", base: "vs", colors: {}, rules: [] },
        { id: "base-salah", name: "x", base: "neon", colors: {}, rules: [] },
      ],
    });
    expect(a.editorTheme).toBe("auto");
    expect(a.editorThemes).toHaveLength(1);
    expect(a.editorThemes[0]?.colors).toEqual({ "editor.background": "#fff" });
    expect(a.editorThemes[0]?.rules).toEqual([{ token: "comment" }]);
    expect(sanitizeEditorTheme(null)).toBeNull();
  });
});

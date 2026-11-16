// Warna sintaks blok kode di chat diturunkan dari TEMA EDITOR yang aktif, sehingga blok kode chat dan penampil kode
// (Monaco) tampil sama. Tema terpasang (VS Code) dibaca dari `tokenColors` + `colors`; tema bawaan Monaco memakai palet
// bawaannya; "auto" = token desain Zandor (tidak ada variabel yang ditimpa, dan Monaco juga memakai token itu).
import { isColor } from "./tokens";
import type { BUILTIN_EDITOR_THEMES, EditorTheme, TextMateRule } from "./vscode-theme";

export const CODE_COLOR_KEYS = [
  "keyword",
  "string",
  "number",
  "comment",
  "function",
  "type",
  "attr",
  "add",
  "del",
  "bg",
  "fg",
] as const;
export type CodeColorKey = (typeof CODE_COLOR_KEYS)[number];
export type CodeColors = Partial<Record<CodeColorKey, string>>;

/** Scope TextMate kandidat per jenis token hljs, urut dari yang paling diutamakan. */
const SCOPES: Record<Exclude<CodeColorKey, "bg" | "fg">, string[]> = {
  keyword: ["keyword.control", "keyword", "storage.type", "storage"],
  string: ["string.quoted", "string"],
  number: ["constant.numeric", "constant.language", "constant"],
  comment: ["comment.line", "comment"],
  function: ["entity.name.function", "support.function"],
  type: ["entity.name.type", "entity.name.class", "support.type", "support.class"],
  attr: ["entity.other.attribute-name", "support.type.property-name", "variable.other.property", "variable"],
  add: ["markup.inserted"],
  del: ["markup.deleted"],
};

/** Warna depan dari aturan paling spesifik yang cocok dengan `scope` (selektor sederhana; aturan belakangan menang). */
export function scopeColor(rules: readonly TextMateRule[], scope: string): string | null {
  let best: { len: number; color: string } | null = null;
  for (const r of rules) {
    const fg = r.settings.foreground;
    if (!fg || !isColor(fg) || !r.scope) continue;
    for (const s of r.scope) {
      if (s.includes(" ")) continue; // selektor turunan (mis. "meta.tag string") tidak berlaku umum
      if (scope === s || scope.startsWith(`${s}.`)) {
        if (!best || s.length >= best.len) best = { len: s.length, color: fg };
      }
    }
  }
  return best?.color ?? null;
}

/** Palet bawaan Monaco (sama dengan warna token tema vs / vs-dark / hc-black / hc-light). */
const BUILTIN: Record<(typeof BUILTIN_EDITOR_THEMES)[number], Required<CodeColors>> = {
  "vs-dark": {
    keyword: "#569cd6",
    string: "#ce9178",
    number: "#b5cea8",
    comment: "#6a9955",
    function: "#dcdcaa",
    type: "#4ec9b0",
    attr: "#9cdcfe",
    add: "#b5cea8",
    del: "#ce9178",
    bg: "#1e1e1e",
    fg: "#d4d4d4",
  },
  vs: {
    keyword: "#0000ff",
    string: "#a31515",
    number: "#098658",
    comment: "#008000",
    function: "#795e26",
    type: "#267f99",
    attr: "#e50000",
    add: "#098658",
    del: "#a31515",
    bg: "#fffffe",
    fg: "#000000",
  },
  "hc-black": {
    keyword: "#569cd6",
    string: "#ce9178",
    number: "#b5cea8",
    comment: "#7ca668",
    function: "#dcdcaa",
    type: "#4ec9b0",
    attr: "#9cdcfe",
    add: "#b5cea8",
    del: "#ce9178",
    bg: "#000000",
    fg: "#ffffff",
  },
  "hc-light": {
    keyword: "#0f4a85",
    string: "#264f78",
    number: "#096d48",
    comment: "#515151",
    function: "#5e2cbc",
    type: "#185e73",
    attr: "#264f78",
    add: "#096d48",
    del: "#b5200d",
    bg: "#ffffff",
    fg: "#292929",
  },
};

/** Warna kode dari tema VS Code terpasang. Jenis yang tidak didefinisikan tema dilewati (memakai token bawaan). */
export function codeColorsFromTheme(t: EditorTheme): CodeColors {
  const out: CodeColors = {};
  for (const [key, scopes] of Object.entries(SCOPES) as Array<[keyof typeof SCOPES, string[]]>) {
    for (const s of scopes) {
      const c = scopeColor(t.tokenColors, s);
      if (c) {
        out[key] = c;
        break;
      }
    }
  }
  out.add ??= t.colors["gitDecoration.addedResourceForeground"];
  out.del ??= t.colors["gitDecoration.deletedResourceForeground"];
  const bg = t.colors["editor.background"];
  const fg = t.colors["editor.foreground"] ?? t.tokenColors.find((r) => !r.scope)?.settings.foreground;
  if (bg) out.bg = bg;
  if (fg) out.fg = fg;
  for (const k of Object.keys(out) as CodeColorKey[]) if (!out[k] || !isColor(out[k])) delete out[k];
  return out;
}

/** Warna kode untuk pilihan tema editor (`"auto"`/tak dikenal → kosong = token desain). */
export function codeColorsFor(selected: string, installed: readonly EditorTheme[]): CodeColors {
  if (selected in BUILTIN) return { ...BUILTIN[selected as keyof typeof BUILTIN] };
  const t = installed.find((x) => x.id === selected);
  return t ? codeColorsFromTheme(t) : {};
}

/** Variabel CSS `--z-code-*` untuk pilihan tema editor. */
export function codeVarsFor(selected: string, installed: readonly EditorTheme[]): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const [k, v] of Object.entries(codeColorsFor(selected, installed))) if (v) vars[`--z-code-${k}`] = v;
  return vars;
}

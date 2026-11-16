// Pewarnaan TextMate (grammar + tema VS Code asli, lewat Shiki) untuk Monaco — ADR-071.
// Dipakai hanya saat tema editor terpasang (impor/Open VSX) aktif, agar penempatan warna sama dengan VS Code.
// Bahasa didaftarkan dengan id terpisah (`tm-<bahasa>`) sehingga tokenizer Monarch bawaan tetap utuh untuk tema lain.
import type { EditorTheme } from "@zandor/core";
import type { HighlighterCore, LanguageRegistration } from "shiki/core";
import { monaco } from "./monaco-setup";

type LangModule = { default: LanguageRegistration[] };

/** id bahasa Shiki → modul grammar (dimuat lazy). */
const GRAMMARS: Record<string, () => Promise<LangModule>> = {
  typescript: () => import("shiki/langs/typescript.mjs"),
  tsx: () => import("shiki/langs/tsx.mjs"),
  javascript: () => import("shiki/langs/javascript.mjs"),
  jsx: () => import("shiki/langs/jsx.mjs"),
  json: () => import("shiki/langs/json.mjs"),
  markdown: () => import("shiki/langs/markdown.mjs"),
  html: () => import("shiki/langs/html.mjs"),
  css: () => import("shiki/langs/css.mjs"),
  scss: () => import("shiki/langs/scss.mjs"),
  less: () => import("shiki/langs/less.mjs"),
  xml: () => import("shiki/langs/xml.mjs"),
  yaml: () => import("shiki/langs/yaml.mjs"),
  ini: () => import("shiki/langs/ini.mjs"),
  python: () => import("shiki/langs/python.mjs"),
  rust: () => import("shiki/langs/rust.mjs"),
  go: () => import("shiki/langs/go.mjs"),
  java: () => import("shiki/langs/java.mjs"),
  kotlin: () => import("shiki/langs/kotlin.mjs"),
  c: () => import("shiki/langs/c.mjs"),
  cpp: () => import("shiki/langs/cpp.mjs"),
  csharp: () => import("shiki/langs/csharp.mjs"),
  ruby: () => import("shiki/langs/ruby.mjs"),
  php: () => import("shiki/langs/php.mjs"),
  shellscript: () => import("shiki/langs/shellscript.mjs"),
  powershell: () => import("shiki/langs/powershell.mjs"),
  bat: () => import("shiki/langs/bat.mjs"),
  sql: () => import("shiki/langs/sql.mjs"),
  swift: () => import("shiki/langs/swift.mjs"),
  lua: () => import("shiki/langs/lua.mjs"),
  r: () => import("shiki/langs/r.mjs"),
  docker: () => import("shiki/langs/docker.mjs"),
  graphql: () => import("shiki/langs/graphql.mjs"),
};

const ALIAS: Record<string, string> = { shell: "shellscript", dockerfile: "docker" };

/** Bahasa Zandor (`languageFor`) + path → id grammar TextMate, atau null bila tidak ada grammar-nya. */
export function textMateLanguage(lang: string, path: string): string | null {
  const ext = path.toLowerCase().split(".").pop() ?? "";
  if (ext === "tsx") return "tsx";
  if (ext === "jsx") return "jsx";
  const id = ALIAS[lang] ?? lang;
  return Object.hasOwn(GRAMMARS, id) ? id : null;
}

let highlighter: Promise<HighlighterCore> | null = null;
function getHighlighter(): Promise<HighlighterCore> {
  highlighter ??= (async () => {
    const [{ createHighlighterCore }, { createOnigurumaEngine }] = await Promise.all([
      import("shiki/core"),
      import("shiki/engine/oniguruma"),
    ]);
    return createHighlighterCore({ themes: [], langs: [], engine: createOnigurumaEngine(import("shiki/wasm")) });
  })();
  return highlighter;
}

/** Peta warna tema TextMate yang sedang aktif (indeks = id warna di metadata token). */
let colorMap: string[] = [];
let active: string | null = null;
const registered = new Set<string>();

const FONT_STYLE = ["italic", "bold", "underline", "strikethrough"] as const;
const fontStyleOf = (bits: number) => FONT_STYLE.filter((_, i) => bits & (1 << i)).join(" ");

class State implements monaco.languages.IState {
  constructor(readonly stack: unknown) {}
  clone() {
    return this;
  }
  equals(o: monaco.languages.IState) {
    return o instanceof State && o.stack === this.stack;
  }
}

function setProvider(lang: string, h: HighlighterCore) {
  monaco.languages.setTokensProvider(`tm-${lang}`, {
    getInitialState: () => new State(null),
    tokenize(line, state) {
      const s = state as State;
      if (line.length > 20_000) return { endState: s, tokens: [{ startIndex: 0, scopes: "" }] };
      // biome-ignore lint/suspicious/noExplicitAny: StateStack internal vscode-textmate (INITIAL = null diterima)
      const r = h.getLanguage(lang).tokenizeLine2(line, (s.stack ?? undefined) as any, 500);
      const tokens: monaco.languages.IToken[] = [];
      for (let i = 0; i < r.tokens.length; i += 2) {
        const meta = r.tokens[i + 1] as number;
        // tata letak metadata vscode-textmate: fontStyle bit 11..14, foreground bit 15..23
        const fg = (meta >>> 15) & 0x1ff;
        const fs = (meta >>> 11) & 0xf;
        tokens.push({ startIndex: r.tokens[i] as number, scopes: fs ? `t${fg}.${fs}` : `t${fg}` });
      }
      return { endState: new State(r.ruleStack), tokens };
    },
  });
}

/** Muat grammar dan daftarkan bahasa Monaco `tm-<lang>`. */
export async function ensureTextMateLanguage(lang: string): Promise<string> {
  const id = `tm-${lang}`;
  if (registered.has(lang)) return id;
  const h = await getHighlighter();
  const load = GRAMMARS[lang];
  if (!load) throw new Error(`grammar ${lang} tidak tersedia`);
  await h.loadLanguage((await load()).default);
  if (!registered.has(lang)) {
    monaco.languages.register({ id });
    setProvider(lang, h);
    registered.add(lang);
  }
  return id;
}

const hash = (s: string) => {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (Math.imul(31, h) + s.charCodeAt(i)) | 0;
  return (h >>> 0).toString(36);
};

/** Aktifkan tema terpasang sebagai tema TextMate; mengembalikan nama tema Monaco yang harus dipakai. */
export async function activateTextMateTheme(t: EditorTheme): Promise<string> {
  const h = await getHighlighter();
  const dark = t.base === "vs-dark" || t.base === "hc-black";
  const name = `zt-${t.id}-${hash(JSON.stringify([t.colors, t.tokenColors]))}`;
  if (!h.getLoadedThemes().includes(name))
    await h.loadTheme({ name, type: dark ? "dark" : "light", colors: t.colors, tokenColors: t.tokenColors });
  if (active === name) return name;
  colorMap = h.setTheme(name).colorMap;
  const rules: monaco.editor.ITokenThemeRule[] = [];
  colorMap.forEach((c, i) => {
    if (!c) return;
    const foreground = c.replace(/^#/, "");
    rules.push({ token: `t${i}`, foreground });
    for (let fs = 1; fs < 16; fs++) rules.push({ token: `t${i}.${fs}`, foreground, fontStyle: fontStyleOf(fs) });
  });
  monaco.editor.defineTheme(name, { base: t.base, inherit: true, colors: t.colors, rules });
  active = name;
  // indeks warna berubah → paksa semua model `tm-*` ditokenisasi ulang
  for (const lang of registered) setProvider(lang, h);
  return name;
}

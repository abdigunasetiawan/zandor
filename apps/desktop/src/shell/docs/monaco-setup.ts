// Monaco (MIT) untuk penampil dokumen, hanya fitur yang dibutuhkan viewer read-only: editor inti, cari (Ctrl+F), lipat,
// salin, menu konteks, dan pewarnaan sintaks per bahasa (tokenizer dimuat malas per bahasa). Tanpa layanan bahasa
// (TS/JSON/CSS worker) karena itu fitur editor penuh, bukan penampil. Worker editor sekelas (CSP: worker-src 'self').
import * as monaco from "monaco-editor/editor/editor.api";
import "monaco-editor/editor/contrib/find/browser/findController";
import "monaco-editor/editor/contrib/folding/browser/folding";
import "monaco-editor/editor/contrib/clipboard/browser/clipboard";
import "monaco-editor/editor/contrib/contextmenu/browser/contextmenu";
import "monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching";
import "monaco-editor/editor/contrib/wordHighlighter/browser/wordHighlighter";
import "monaco-editor/languages/definitions/typescript/register";
import "monaco-editor/languages/definitions/javascript/register";
import "monaco-editor/languages/definitions/markdown/register";
import "monaco-editor/languages/definitions/html/register";
import "monaco-editor/languages/definitions/css/register";
import "monaco-editor/languages/definitions/scss/register";
import "monaco-editor/languages/definitions/less/register";
import "monaco-editor/languages/definitions/xml/register";
import "monaco-editor/languages/definitions/yaml/register";
import "monaco-editor/languages/definitions/ini/register";
import "monaco-editor/languages/definitions/python/register";
import "monaco-editor/languages/definitions/rust/register";
import "monaco-editor/languages/definitions/go/register";
import "monaco-editor/languages/definitions/java/register";
import "monaco-editor/languages/definitions/kotlin/register";
import "monaco-editor/languages/definitions/cpp/register";
import "monaco-editor/languages/definitions/csharp/register";
import "monaco-editor/languages/definitions/ruby/register";
import "monaco-editor/languages/definitions/php/register";
import "monaco-editor/languages/definitions/shell/register";
import "monaco-editor/languages/definitions/powershell/register";
import "monaco-editor/languages/definitions/bat/register";
import "monaco-editor/languages/definitions/sql/register";
import "monaco-editor/languages/definitions/swift/register";
import "monaco-editor/languages/definitions/lua/register";
import "monaco-editor/languages/definitions/r/register";
import "monaco-editor/languages/definitions/dockerfile/register";
import "monaco-editor/languages/definitions/graphql/register";
import EditorWorker from "monaco-editor/editor/editor.worker?worker";

(self as unknown as { MonacoEnvironment: unknown }).MonacoEnvironment = {
  getWorker: () => new EditorWorker(),
};

/** Bahasa Zandor → id bahasa Monaco yang tokenizer-nya kita muat (JSON memakai tokenizer JavaScript; C memakai C++). */
export function monacoLanguage(lang: string): string {
  if (lang === "json") return "javascript";
  if (lang === "c") return "cpp";
  return lang;
}

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Tema editor yang mengikuti token desain (bukan warna tetap). Dipanggil ulang saat tema aplikasi berganti. */
export function applyZandorTheme(dark: boolean): string {
  const name = dark ? "zandor-dark" : "zandor-light";
  monaco.editor.defineTheme(name, {
    base: dark ? "vs-dark" : "vs",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": cssVar("--z-color-bg"),
      "editor.foreground": cssVar("--z-color-fg"),
      "editorLineNumber.foreground": cssVar("--z-color-fg-subtle"),
      "editor.lineHighlightBackground": cssVar("--z-color-hover"),
      "editorWidget.background": cssVar("--z-color-raised"),
    },
  });
  return name;
}

export { monaco };

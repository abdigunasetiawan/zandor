// Saran prompt dinamis (Fase 11): murni, dihitung dari konteks nyata (project, perubahan git, berkas yang sedang dibuka,
// berkas di akar project). Tidak ada model yang dipanggil dan tidak ada saran "pengisi": tiap saran punya syarat.
import type { DocKind } from "./docs";

export type SuggestionId =
  | "review-changes"
  | "commit-message"
  | "explain-file"
  | "write-tests"
  | "run-tests"
  | "write-readme"
  | "explain-project"
  | "find-todos"
  | "plan-week"
  | "summarize-text"
  | "explain-concept";

export type SuggestionContext = {
  hasProject: boolean;
  /** Nama berkas/folder di akar project (hasil listing), bila sudah dimuat. */
  rootFiles: readonly string[] | null;
  /** Path berkas yang berubah menurut git (kosong bila bukan repo / bersih). */
  changed: readonly string[];
  staged: number;
  /** Berkas yang sedang dibuka di Panel 3. */
  activeDoc: { path: string; kind: DocKind } | null;
};

export type Suggestion = { id: SuggestionId; params: Record<string, string | number> };

const CODE_EXT = /\.(?:[cm]?[jt]sx?|py|rs|go|java|kt|cs|cpp|c|h|rb|php|swift|dart|vue|svelte)$/i;
const has = (files: readonly string[], re: RegExp) => files.some((f) => re.test(f));

/** Maks 4 saran, berurutan menurut relevansi (yang paling spesifik dulu). */
export function suggestPrompts(ctx: SuggestionContext, max = 4): Suggestion[] {
  const out: Suggestion[] = [];
  const add = (id: SuggestionId, params: Suggestion["params"] = {}) => {
    if (!out.some((s) => s.id === id)) out.push({ id, params });
  };

  if (!ctx.hasProject) {
    add("plan-week");
    add("summarize-text");
    add("explain-concept");
    return out.slice(0, max);
  }

  if (ctx.changed.length > 0) {
    add("review-changes", { n: ctx.changed.length });
    if (ctx.staged > 0 || ctx.changed.length <= 20) add("commit-message");
  }
  const doc = ctx.activeDoc;
  if (doc) {
    const name = doc.path.split("/").pop() ?? doc.path;
    if (doc.kind === "code" || doc.kind === "markdown") add("explain-file", { name, path: doc.path });
    if (doc.kind === "code" && CODE_EXT.test(doc.path) && !/\.(?:test|spec)\./i.test(doc.path))
      add("write-tests", { name, path: doc.path });
  }
  const root = ctx.rootFiles;
  if (root) {
    if (!has(root, /^readme(?:\.[a-z]+)?$/i)) add("write-readme");
    if (has(root, /^(?:package\.json|cargo\.toml|pyproject\.toml|go\.mod|pom\.xml)$/i)) add("run-tests");
  }
  add("explain-project");
  add("find-todos");
  return out.slice(0, max);
}

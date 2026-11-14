import { describe, expect, test } from "vitest";
import { type SuggestionContext, suggestPrompts } from "./suggestions";

const base: SuggestionContext = { hasProject: true, rootFiles: null, changed: [], staged: 0, activeDoc: null };
const ids = (c: Partial<SuggestionContext>) => suggestPrompts({ ...base, ...c }).map((s) => s.id);

describe("saran prompt dinamis", () => {
  test("tanpa project: saran umum saja, tanpa saran yang butuh project", () => {
    expect(ids({ hasProject: false })).toEqual(["plan-week", "summarize-text", "explain-concept"]);
  });

  test("project polos (listing belum dimuat): hanya saran yang tidak bergantung data", () => {
    expect(ids({})).toEqual(["explain-project", "find-todos"]);
  });

  test("perubahan git → tinjau + pesan commit di urutan teratas, dengan jumlah berkas", () => {
    const s = suggestPrompts({ ...base, changed: ["a.ts", "b.ts"], staged: 1 });
    expect(s[0]).toEqual({ id: "review-changes", params: { n: 2 } });
    expect(s[1]?.id).toBe("commit-message");
  });

  test("berkas kode terbuka → jelaskan + tulis tes; berkas tes/markdown tidak ditawari tes", () => {
    expect(suggestPrompts({ ...base, activeDoc: { path: "src/app.ts", kind: "code" } }).slice(0, 2)).toEqual([
      { id: "explain-file", params: { name: "app.ts", path: "src/app.ts" } },
      { id: "write-tests", params: { name: "app.ts", path: "src/app.ts" } },
    ]);
    expect(ids({ activeDoc: { path: "src/app.test.ts", kind: "code" } })).not.toContain("write-tests");
    expect(ids({ activeDoc: { path: "README.md", kind: "markdown" } })).toContain("explain-file");
    expect(ids({ activeDoc: { path: "README.md", kind: "markdown" } })).not.toContain("write-tests");
    expect(ids({ activeDoc: { path: "data.csv", kind: "csv" } })).not.toContain("explain-file");
    expect(ids({ activeDoc: { path: "notes.txt", kind: "code" } })).not.toContain("write-tests");
  });

  test("akar project: README hilang → buat README; manifes ada → jalankan tes", () => {
    expect(ids({ rootFiles: ["src", "package.json"] })).toEqual([
      "write-readme",
      "run-tests",
      "explain-project",
      "find-todos",
    ]);
    expect(ids({ rootFiles: ["README.md", "Cargo.toml"] })).toEqual(["run-tests", "explain-project", "find-todos"]);
    expect(ids({ rootFiles: ["readme.rst"] })).not.toContain("write-readme");
    expect(ids({ rootFiles: ["notes"] })).toEqual(["write-readme", "explain-project", "find-todos"]);
  });

  test("batas jumlah dan tanpa duplikat", () => {
    const all = suggestPrompts({
      hasProject: true,
      rootFiles: ["package.json"],
      changed: ["a"],
      staged: 1,
      activeDoc: { path: "a.ts", kind: "code" },
    });
    expect(all).toHaveLength(4);
    expect(new Set(all.map((s) => s.id)).size).toBe(4);
    expect(suggestPrompts({ ...base, changed: ["a"] }, 2)).toHaveLength(2);
  });
});

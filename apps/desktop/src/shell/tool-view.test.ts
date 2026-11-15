import type { ToolPart } from "@zandor/core";
import { describe, expect, it } from "vitest";
import {
  contentRows,
  describeTool,
  diffStats,
  grepGroups,
  parseDiff,
  readLines,
  splitPath,
  tallyProcess,
  taskResult,
  toolKind,
} from "./tool-view";

const part = (tool: string, input: Record<string, unknown>, extra: Partial<ToolPart["state"]> = {}): ToolPart => ({
  id: `p_${tool}`,
  messageID: "m1",
  sessionID: "s1",
  type: "tool",
  tool,
  callID: "c1",
  state: { status: "completed", input, ...extra },
});

// Diff nyata dari tool edit opencode 1.19.5 (header Index/====/---/+++ lalu hunk).
const EDIT_DIFF = [
  "Index: C:\\w\\a.txt",
  "===================================================================",
  "--- C:\\w\\a.txt",
  "+++ C:\\w\\a.txt",
  "@@ -1,5 +1,5 @@",
  " satu",
  "-dua",
  "+DUA-BARU",
  " tiga",
  " empat",
  " lima",
  "",
].join("\n");

describe("toolKind", () => {
  it("memetakan tool engine ke jenis tampilan", () => {
    expect(toolKind("read")).toBe("read");
    expect(toolKind("edit")).toBe("edit");
    expect(toolKind("bash")).toBe("shell");
    expect(toolKind("grep")).toBe("search");
    expect(toolKind("glob")).toBe("search");
    expect(toolKind("webfetch")).toBe("web");
    expect(toolKind("task")).toBe("task");
    expect(toolKind("memory_search")).toBe("memory");
    expect(toolKind("todowrite")).toBe("plan");
    expect(toolKind("apa-saja")).toBe("generic");
  });
});

describe("splitPath", () => {
  it("menyisakan dua folder terakhir dan memisahkan nama berkas", () => {
    const p = splitPath("C:\\Users\\a\\Code\\Zandor\\apps\\desktop\\src\\shell\\ChatPanel.tsx");
    expect(p.name).toBe("ChatPanel.tsx");
    expect(p.dir).toBe("…/src/shell/");
    expect(p.full).toContain("Zandor");
  });
  it("path pendek tanpa awalan elipsis; nama saja tanpa folder", () => {
    expect(splitPath("src/a.ts").dir).toBe("src/");
    expect(splitPath("README.md").dir).toBe("");
  });
});

describe("parseDiff", () => {
  it("membuang header, menomori baris dari hunk, dan menghitung perubahan", () => {
    const rows = parseDiff(EDIT_DIFF);
    expect(rows[0]).toEqual({ kind: "hunk", text: "@@ -1,5 +1,5 @@" });
    const del = rows.find((r) => r.kind === "del");
    const add = rows.find((r) => r.kind === "add");
    expect(del).toMatchObject({ text: "-dua", oldNo: 2 });
    expect(add).toMatchObject({ text: "+DUA-BARU", newNo: 2 });
    expect(rows.find((r) => r.text === " tiga")).toMatchObject({ oldNo: 3, newNo: 3 });
    expect(rows.some((r) => r.text.startsWith("Index:") || r.text.startsWith("==="))).toBe(false);
    expect(diffStats(rows)).toEqual({ adds: 1, dels: 1 });
  });
  it("teks bukan unified diurai per awalan tanpa nomor", () => {
    const rows = parseDiff("-lama\n+baru\nsama");
    expect(rows.map((r) => r.kind)).toEqual(["del", "add", "ctx"]);
    expect(rows[0]?.oldNo).toBeUndefined();
  });
  it("beberapa hunk: nomor mengikuti header masing-masing", () => {
    const rows = parseDiff("@@ -1,1 +1,1 @@\n-a\n+b\n@@ -10,1 +10,2 @@\n x\n+y");
    expect(rows.filter((r) => r.kind === "hunk")).toHaveLength(2);
    expect(rows.find((r) => r.text === "+y")?.newNo).toBe(11);
  });
});

describe("contentRows", () => {
  it("isi berkas baru menjadi baris tambah bernomor tanpa baris kosong penutup", () => {
    expect(contentRows("# Judul\nisi\n")).toEqual([
      { kind: "add", text: "+# Judul", newNo: 1 },
      { kind: "add", text: "+isi", newNo: 2 },
    ]);
  });
});

describe("readLines", () => {
  it("mengambil baris bernomor dari pembungkus engine dan memisahkan catatan", () => {
    const out =
      "<path>C:\\w\\a.txt</path>\n<type>file</type>\n<content>\n2: dua\n3: tiga\n4: empat\n\n(Showing lines 2-4 of 5. Use offset=5 to continue.)\n</content>";
    const r = readLines(out);
    expect(r.lines).toEqual([
      { no: 2, text: "dua" },
      { no: 3, text: "tiga" },
      { no: 4, text: "empat" },
    ]);
    expect(r.note).toContain("Showing lines 2-4 of 5");
  });
  it("keluaran tanpa pembungkus tetap dibaca", () => {
    expect(readLines("1: a\n2: b").lines).toHaveLength(2);
  });
});

describe("grepGroups", () => {
  it("mengelompokkan hasil per berkas dengan nomor baris", () => {
    const out = "Found 2 matches\nC:\\w\\a.txt:\n  Line 3: tiga\nC:\\w\\b.ts:\n  Line 1: tiga lagi\n";
    expect(grepGroups(out)).toEqual([
      { file: "C:\\w\\a.txt", lines: [{ no: 3, text: "tiga" }] },
      { file: "C:\\w\\b.ts", lines: [{ no: 1, text: "tiga lagi" }] },
    ]);
  });
});

describe("taskResult", () => {
  it("mengambil isi <task_result>", () => {
    expect(taskResult('<task id="x" state="completed">\n<task_result>\nok selesai\n</task_result>\n</task>')).toBe(
      "ok selesai",
    );
    expect(taskResult("teks biasa")).toBe("teks biasa");
  });
});

describe("describeTool", () => {
  it("read: berkas + rentang baris dari metadata engine", () => {
    const v = describeTool(
      part(
        "read",
        { filePath: "C:\\w\\src\\a.txt", offset: 2, limit: 3 },
        { metadata: { display: { lineStart: 2, lineEnd: 4, totalLines: 5 } } },
      ),
    );
    expect(v.kind).toBe("read");
    expect(v.path?.name).toBe("a.txt");
    expect(v.range).toEqual({ from: 2, to: 4, total: 5 });
  });
  it("read tanpa offset/limit: tanpa rentang (seluruh berkas)", () => {
    expect(describeTool(part("read", { filePath: "a.ts" })).range).toBeUndefined();
  });
  it("read dengan offset+limit tanpa metadata: rentang dari input", () => {
    expect(describeTool(part("read", { filePath: "a.ts", offset: 10, limit: 5 })).range).toEqual({ from: 10, to: 14 });
  });
  it("edit: +/− dari filediff dan teks diff", () => {
    const v = describeTool(
      part(
        "edit",
        { filePath: "a.txt" },
        { metadata: { diff: EDIT_DIFF, filediff: { file: "a.txt", patch: EDIT_DIFF, additions: 1, deletions: 1 } } },
      ),
    );
    expect(v).toMatchObject({ kind: "edit", adds: 1, dels: 1 });
    expect(v.diff).toContain("@@");
  });
  it("edit: +/− dihitung dari diff bila filediff tidak ada", () => {
    const v = describeTool(part("edit", { filePath: "a.txt" }, { metadata: { diff: EDIT_DIFF } }));
    expect(v).toMatchObject({ adds: 1, dels: 1 });
  });
  it("write: jumlah baris isi sebagai tambahan", () => {
    const v = describeTool(part("write", { filePath: "baru.md", content: "# Judul\nisi\n" }));
    expect(v).toMatchObject({ kind: "write", adds: 2 });
  });
  it("bash: perintah, deskripsi, dan kode keluar", () => {
    const v = describeTool(
      part("bash", { command: "echo halo", description: "Cetak halo" }, { metadata: { exit: 0, output: "halo\n" } }),
    );
    expect(v).toMatchObject({ kind: "shell", command: "echo halo", description: "Cetak halo", exit: 0 });
  });
  it("grep/glob: pola dan jumlah hasil", () => {
    expect(describeTool(part("grep", { pattern: "tiga" }, { metadata: { matches: 4 } }))).toMatchObject({
      target: "tiga",
      count: 4,
    });
    expect(describeTool(part("glob", { pattern: "**/*.ts" }, { metadata: { count: 7 } })).count).toBe(7);
  });
  it("task: deskripsi, jenis sub-agen, dan sesi anak", () => {
    const v = describeTool(
      part(
        "task",
        { description: "Cari berkas", prompt: "x", subagent_type: "general" },
        { metadata: { sessionId: "ses_anak" } },
      ),
    );
    expect(v).toMatchObject({ kind: "task", target: "Cari berkas", agent: "general", childSession: "ses_anak" });
  });
  it("todowrite: daftar rencana", () => {
    const v = describeTool(
      part("todowrite", {
        todos: [
          { content: "A", status: "completed" },
          { content: "B", status: "pending" },
        ],
      }),
    );
    expect(v.count).toBe(2);
    expect(v.todos?.[1]).toEqual({ content: "B", status: "pending" });
  });
  it("tool tak dikenal: cadangan dari judul atau string pertama", () => {
    expect(describeTool(part("aneh", { x: "isi" })).target).toBe("isi");
    expect(describeTool(part("aneh", {}, { title: "Judul" })).target).toBe("Judul");
  });
});

describe("tallyProcess", () => {
  it("menghitung per jenis; berkas yang sama diedit berulang dihitung sekali, +/− dijumlahkan", () => {
    const edit = (n: number) =>
      part(
        "edit",
        { filePath: "a.ts" },
        { metadata: { filediff: { file: "a.ts", patch: "", additions: n, deletions: 1 } } },
      );
    const t = tallyProcess([
      part("read", { filePath: "a.ts" }),
      part("read", { filePath: "b.ts" }),
      edit(3),
      edit(2),
      part("bash", { command: "ls" }),
      part("grep", { pattern: "x" }),
      part("webfetch", { url: "http://x" }),
      part("task", { description: "d" }),
      part("skill", { name: "s" }),
      { id: "r1", messageID: "m1", sessionID: "s1", type: "reasoning", text: "hm" },
    ]);
    expect(t).toMatchObject({ read: 2, edit: 1, shell: 1, search: 1, web: 1, task: 1, other: 1, adds: 5, dels: 2 });
  });
});

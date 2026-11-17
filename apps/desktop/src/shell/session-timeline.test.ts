import type { ChatMessage } from "@zandor/core";
import { describe, expect, test } from "vitest";
import { buildTimeline, mergeSnapshotFiles, touchedFiles } from "./session-timeline";

const user = (id: string, text: string, at: number): ChatMessage => ({
  info: { id, sessionID: "s", role: "user", time: { created: at } },
  parts: [{ id: `${id}t`, messageID: id, sessionID: "s", type: "text", text, synthetic: false }],
});
const asst = (
  id: string,
  at: number,
  parts: ChatMessage["parts"],
  error?: { name: string; message: string },
): ChatMessage => ({
  info: {
    id,
    sessionID: "s",
    role: "assistant",
    time: { created: at, completed: at + 500 },
    ...(error ? { error } : {}),
  },
  parts,
});
const tool = (
  id: string,
  name: string,
  input: Record<string, unknown>,
  status: "completed" | "error" | "running",
  start: number,
  end?: number,
) =>
  ({
    id,
    messageID: "a1",
    sessionID: "s",
    type: "tool" as const,
    tool: name,
    callID: id,
    state: {
      status,
      input,
      ...(status === "error" ? { error: "gagal" } : { output: "ok" }),
      time: { start, ...(end ? { end } : {}) },
    },
  }) satisfies ChatMessage["parts"][number];

describe("buildTimeline", () => {
  const msgs: ChatMessage[] = [
    user("u1", "Buat file hello", 1000),
    asst("a1", 1100, [
      { id: "r1", messageID: "a1", sessionID: "s", type: "reasoning", text: "Saya akan menulis berkas" },
      tool("t1", "write", { filePath: "hello.md" }, "completed", 1200, 1500),
      tool("t2", "bash", { command: "ls" }, "error", 1600, 1650),
      { id: "x1", messageID: "a1", sessionID: "s", type: "text", text: "Selesai.", synthetic: false },
    ]),
  ];

  test("urut waktu: pengguna, berpikir, tool (status + durasi), jawaban", () => {
    const tl = buildTimeline(msgs);
    expect(tl.map((e) => e.kind)).toEqual(["user", "thinking", "tool", "tool", "reply"]);
    const t1 = tl.find((e) => e.id === "t1");
    expect(t1).toMatchObject({ status: "completed", durationMs: 300, title: "hello.md", step: { key: "step.write" } });
    expect(tl.find((e) => e.id === "t2")).toMatchObject({ status: "error", title: "ls" });
    expect(tl.at(-1)?.title).toBe("Selesai.");
  });

  test("galat pesan, serta izin dan pertanyaan yang menunggu di akhir", () => {
    const withErr = [...msgs, asst("a2", 3000, [], { name: "ProviderError", message: "kuota habis" })];
    const tl = buildTimeline(withErr, {
      permissions: [{ id: "p1", sessionID: "s", permission: "edit", patterns: ["a.ts"], metadata: {}, always: [] }],
      questions: [
        {
          id: "q1",
          sessionID: "s",
          questions: [{ question: "Pilih?", header: "", options: [], multiple: false, custom: true }],
        },
      ],
    });
    expect(tl.find((e) => e.kind === "error")).toMatchObject({ title: "kuota habis", status: "error" });
    expect(tl.slice(-2).map((e) => e.kind)).toEqual(["permission", "question"]);
    expect(tl.at(-2)).toMatchObject({ status: "pending", title: "edit: a.ts" });
  });

  test("sesi kosong → daftar kosong; rincian tool dipotong", () => {
    expect(buildTimeline([])).toEqual([]);
    const big = asst("a9", 5000, [tool("t9", "bash", { command: "x".repeat(10_000) }, "completed", 5000, 5100)]);
    expect((buildTimeline([big]).find((e) => e.id === "t9")?.detail ?? "").length).toBeLessThan(4100);
  });
});

describe("touchedFiles", () => {
  test("mengelompokkan per berkas: operasi, jumlah, terbaru dulu, dan penanda gagal", () => {
    const m = asst("a1", 100, [
      tool("1", "read", { filePath: "a.ts" }, "completed", 100, 110),
      tool("2", "edit", { filePath: "a.ts" }, "completed", 200, 210),
      tool("3", "write", { filePath: "b.md" }, "error", 300, 310),
      tool("4", "bash", { command: "ls" }, "completed", 400, 410), // bukan tool berkas
    ]);
    const files = touchedFiles([m]);
    expect(files.map((f) => f.path)).toEqual(["b.md", "a.ts"]);
    expect(files[1]).toMatchObject({ ops: ["read", "edit"], count: 2, failed: false });
    expect(files[0]).toMatchObject({ ops: ["write"], failed: true });
    expect(touchedFiles([])).toEqual([]);
  });
});

describe("mergeSnapshotFiles", () => {
  test("menambah berkas buatan bash dari snapshot; tanpa duplikat; yang dihapus dilewati", () => {
    const touched = touchedFiles([
      asst("a1", 100, [tool("1", "read", { filePath: "C:\\chat\\a.md" }, "completed", 100, 110)]),
    ]);
    const out = mergeSnapshotFiles(
      touched,
      [
        { file: "a.md", status: "modified" }, // sama dengan C:\chat\a.md
        { file: "halo.docx", status: "added" },
        { file: "lama.txt", status: "deleted" },
      ],
      "C:\\chat",
      500,
    );
    expect(out.map((f) => f.path)).toEqual(["C:\\chat/halo.docx", "C:\\chat\\a.md"]);
    expect(out[0]).toMatchObject({ ops: ["write"] });
  });
});

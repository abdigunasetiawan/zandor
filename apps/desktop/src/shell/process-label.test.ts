import type { MessagePart } from "@zandor/core";
import { describe, expect, test } from "vitest";
import {
  countSteps,
  createdDocuments,
  elapsedSeconds,
  isDocumentPath,
  splitElapsed,
  splitTurn,
  stepOf,
  toolSummary,
} from "./process-label";

const base = { messageID: "m", sessionID: "s" };
const tool = (
  id: string,
  name: string,
  input: Record<string, unknown> = {},
  status: "completed" | "running" | "error" = "completed",
): MessagePart => ({ ...base, id, type: "tool", tool: name, callID: id, state: { status, input } });
const text = (id: string, t: string): MessagePart => ({ ...base, id, type: "text", text: t, synthetic: false });
const reasoning = (id: string): MessagePart => ({ ...base, id, type: "reasoning", text: "…" });

describe("label langkah", () => {
  test("berpikir, skill, memori, internet, berkas, shell, rencana", () => {
    expect(stepOf(reasoning("1"))?.key).toBe("step.thinking");
    expect(stepOf(tool("2", "skill", { name: "debugging" }))?.key).toBe("step.skill");
    expect(stepOf(tool("3", "memory_search", { query: "x" }))?.key).toBe("step.memoryRead");
    expect(stepOf(tool("4", "memory_write", { text: "x" }))?.key).toBe("step.memoryWrite");
    expect(stepOf(tool("5", "websearch", { query: "bun" }))).toMatchObject({ key: "step.web", detail: "bun" });
    expect(stepOf(tool("6", "webfetch", { url: "https://x" }))?.key).toBe("step.webfetch");
    expect(stepOf(tool("7", "read", { filePath: "a.ts" }))?.key).toBe("step.read");
    expect(stepOf(tool("8", "edit", { filePath: "a.ts" }))?.key).toBe("step.edit");
    expect(stepOf(tool("9", "write", { filePath: "a.md" }))?.key).toBe("step.write");
    expect(stepOf(tool("10", "bash", { command: "bun test" }))).toMatchObject({
      key: "step.shell",
      detail: "bun test",
    });
    expect(stepOf(tool("11", "grep", { pattern: "x" }))?.key).toBe("step.search");
    expect(stepOf(tool("12", "todowrite"))?.key).toBe("step.plan");
    expect(stepOf(tool("13", "tool_aneh"))).toMatchObject({ key: "step.tool", tool: "tool_aneh" });
    expect(stepOf(text("14", "halo"))).toBeNull();
  });
  test("ringkasan tool memakai kueri/url/perintah/path", () => {
    expect(toolSummary("websearch", { query: "q" })).toBe("q");
    expect(toolSummary("read", { filePath: "a.ts" })).toBe("a.ts");
    expect(toolSummary("bash", {}, "judul")).toBe("judul");
  });
});

describe("pemisahan proses dan jawaban", () => {
  test("semua sampai part proses terakhir = proses; sisanya jawaban", () => {
    const parts = [reasoning("1"), text("2", "narasi"), tool("3", "bash"), text("4", "jawaban akhir")];
    const { process, answer } = splitTurn(parts);
    expect(process.map((p) => p.id)).toEqual(["1", "2", "3"]);
    expect(answer.map((p) => p.id)).toEqual(["4"]);
    expect(countSteps(process)).toBe(2);
  });
  test("tanpa tool: semuanya jawaban; hanya tool: jawaban kosong", () => {
    expect(splitTurn([text("1", "a")])).toEqual({ process: [], answer: [text("1", "a")] });
    expect(splitTurn([tool("1", "bash")]).answer).toEqual([]);
  });
});

describe("durasi", () => {
  test("detik utuh dan menit:detik", () => {
    expect(elapsedSeconds(3999)).toBe(3);
    expect(elapsedSeconds(-5)).toBe(0);
    expect(splitElapsed(75)).toEqual({ minutes: 1, seconds: 15 });
  });
});

describe("dokumen yang dibuat agent", () => {
  test("hanya tool write yang selesai dengan ekstensi dokumen, tanpa duplikat", () => {
    expect(isDocumentPath("docs/laporan.PDF")).toBe(true);
    expect(isDocumentPath("src/app.ts")).toBe(false);
    expect(isDocumentPath("README")).toBe(false);
    const parts = [
      tool("1", "write", { filePath: "out/laporan.docx" }),
      tool("2", "write", { filePath: "out/laporan.docx" }),
      tool("3", "write", { filePath: "src/app.ts" }),
      tool("4", "write", { filePath: "out/data.csv" }, "error"),
      tool("5", "edit", { filePath: "out/lama.md" }),
      tool("6", "write", { filePath: "notes/catatan.md" }),
    ];
    expect(createdDocuments(parts)).toEqual(["out/laporan.docx", "notes/catatan.md"]);
  });
});

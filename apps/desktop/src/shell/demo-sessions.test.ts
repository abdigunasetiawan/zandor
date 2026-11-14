import { visibleMessages } from "@zandor/core";
import { describe, expect, test } from "vitest";
import { DEMO_KEY, DEMO_SESSIONS, demoById } from "./demo-sessions";
import { fileUrlToPath } from "./FileCard";
import { SAFE_IMAGE_URI } from "./image-uri";

describe("sesi demo bawaan", () => {
  test("ada 3 sesi, id unik, dan terpisah dari kunci project/Chat", () => {
    expect(DEMO_SESSIONS).toHaveLength(3);
    expect(new Set(DEMO_SESSIONS.map((d) => d.id)).size).toBe(3);
    expect(DEMO_KEY).not.toBe("chat");
    expect(demoById("tidak-ada")).toBeUndefined();
  });

  test("tiap sesi terbentuk wajar: id pesan/part berurut, sesi hanya-baca, dimuat penuh", () => {
    for (const d of DEMO_SESSIONS) {
      const s = d.build();
      expect(s.loaded).toBe(true);
      expect(s.sending).toBe(false);
      expect(s.status.type).toBe("idle");
      expect(s.messages.length).toBeGreaterThan(1);
      const ids = s.messages.map((m) => m.info.id);
      expect([...ids].sort()).toEqual(ids);
      for (const m of s.messages) {
        const pids = m.parts.map((p) => p.id);
        expect([...pids].sort()).toEqual(pids);
        expect(new Set(pids).size).toBe(pids.length);
      }
      expect(visibleMessages(s).length).toBe(s.messages.length);
    }
  });

  test("trajectory mencakup berpikir, tool call, pencarian web, memori, skill, edit berdiff, dan tool galat", () => {
    const s = demoById("demo-trajectory")?.build();
    const parts = s?.messages.flatMap((m) => m.parts) ?? [];
    const tools = parts.flatMap((p) => (p.type === "tool" ? [p] : []));
    const names = new Set(tools.map((t) => t.tool));
    for (const n of [
      "skill",
      "memory_search",
      "memory_write",
      "websearch",
      "webfetch",
      "bash",
      "read",
      "edit",
      "grep",
      "todowrite",
    ])
      expect(names.has(n), n).toBe(true);
    expect(parts.some((p) => p.type === "reasoning")).toBe(true);
    expect(tools.some((t) => t.state.status === "error")).toBe(true);
    expect(tools.some((t) => typeof t.state.metadata?.diff === "string")).toBe(true);
    expect(s?.todos.length).toBeGreaterThan(0);
  });

  test("render kaya mencakup tabel, kode, rumus, diagram, chart, gambar, dan kartu berkas", () => {
    const parts =
      demoById("demo-render")
        ?.build()
        .messages.flatMap((m) => m.parts) ?? [];
    const text = parts.map((p) => (p.type === "text" ? p.text : "")).join("\n");
    expect(text).toMatch(/\| Provider \|/); // tabel
    expect(text).toMatch(/```ts/); // kode
    expect(text).toMatch(/\$\$/); // rumus blok
    expect(text).toMatch(/```mermaid\nflowchart/); // diagram
    expect(text).toMatch(/pie showData/); // chart
    expect(text).toMatch(/xychart-beta/); // chart xy
    expect(text).toMatch(/!\[[^\]]+\]\(data:image\/svg\+xml;base64,/); // gambar markdown (data URI)
    const files = parts.flatMap((p) => (p.type === "file" ? [p] : []));
    expect(files.length).toBeGreaterThanOrEqual(5);
    expect(files.some((f) => SAFE_IMAGE_URI.test(f.url))).toBe(true);
  });

  test("izin & pertanyaan: tiga izin (edit, bash, direktori luar) dan satu formulir 3 pertanyaan", () => {
    const d = demoById("demo-ask");
    expect(d?.initialPermissions.map((p) => p.permission)).toEqual(["edit", "bash", "external_directory"]);
    expect(d?.initialQuestions).toHaveLength(1);
    const q = d?.initialQuestions[0]?.questions ?? [];
    expect(q).toHaveLength(3);
    expect(q.some((x) => x.multiple)).toBe(true);
    expect(q.some((x) => x.custom && x.options.length === 0)).toBe(true);
    // sesi dibangun tanpa izin/pertanyaan (tampilan menambahkannya dari state lokal agar bisa di-reset)
    expect(d?.build().permissions).toEqual([]);
  });
});

describe("gambar dan berkas aman", () => {
  test("hanya data: URI bertipe gambar yang dimuat", () => {
    expect(SAFE_IMAGE_URI.test("data:image/png;base64,AAAA")).toBe(true);
    expect(SAFE_IMAGE_URI.test("data:image/svg+xml;base64,AAAA")).toBe(true);
    expect(SAFE_IMAGE_URI.test("https://evil.example/x.png?leak=1")).toBe(false);
    expect(SAFE_IMAGE_URI.test("data:text/html;base64,AAAA")).toBe(false);
    expect(SAFE_IMAGE_URI.test("file:///C:/rahasia.png")).toBe(false);
  });
  test("URL file:// → path lokal; selain itu null", () => {
    expect(fileUrlToPath("file:///C:/Work/a%20b.txt")).toBe("C:/Work/a b.txt");
    expect(fileUrlToPath("file:///home/x/a.md")).toBe("/home/x/a.md");
    expect(fileUrlToPath("demo://laporan.pdf")).toBeNull();
    expect(fileUrlToPath("https://x/y")).toBeNull();
  });
});

import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import { BROWSER_ACTIONS, CHAT_SCOPE_HINT, createControlTools, isInside, requestControl } from "./control";

describe("kontrol aplikasi (ADR-077)", () => {
  test("permintaan ditulis atomik, jawaban dibaca lalu dihapus", async () => {
    const dir = mkdtempSync(join(tmpdir(), "zctl-"));
    // "Aplikasi" tiruan: ambil permintaan dan tulis jawaban.
    const app = setInterval(() => {
      const reqDir = join(dir, "requests");
      if (!existsSync(reqDir)) return;
      for (const f of readdirSync(reqDir).filter((n) => n.endsWith(".json"))) {
        const req = JSON.parse(readFileSync(join(reqDir, f), "utf8"));
        mkdirSync(join(dir, "responses"), { recursive: true });
        writeFileSync(
          join(dir, "responses", f),
          JSON.stringify({ ok: true, result: { echo: req.action, d: req.directory } }),
        );
      }
    }, 20);
    try {
      const r = await requestControl(
        dir,
        { action: "status", args: {}, sessionID: "s", directory: "D" },
        { timeoutMs: 2000, pollMs: 20 },
      );
      expect(r).toEqual({ ok: true, result: { echo: "status", d: "D" } });
      expect(readdirSync(join(dir, "responses"))).toEqual([]);
    } finally {
      clearInterval(app);
    }
  });

  test("tanpa jawaban → pesan jujur dan permintaan dibersihkan", async () => {
    const dir = mkdtempSync(join(tmpdir(), "zctl-"));
    const r = await requestControl(
      dir,
      { action: "status", args: {}, sessionID: "s", directory: "D" },
      { timeoutMs: 100, pollMs: 20 },
    );
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/tidak merespons/);
    expect(readdirSync(join(dir, "requests"))).toEqual([]);
  });

  test("zandor_browser mengirim aksi sebagai browser_<aksi> dan hanya aksi baca-saja F1", async () => {
    const dir = mkdtempSync(join(tmpdir(), "zctl-"));
    expect([...BROWSER_ACTIONS]).toEqual(["navigate", "tabs", "snapshot", "screenshot", "back", "forward", "reload"]);
    const seen: { action: string; args: Record<string, unknown> }[] = [];
    const app = setInterval(() => {
      const reqDir = join(dir, "requests");
      if (!existsSync(reqDir)) return;
      for (const f of readdirSync(reqDir).filter((n) => n.endsWith(".json"))) {
        const req = JSON.parse(readFileSync(join(reqDir, f), "utf8"));
        rmSync(join(reqDir, f), { force: true }); // seperti `take` di aplikasi: permintaan diambil sekali
        seen.push({ action: req.action, args: req.args });
        mkdirSync(join(dir, "responses"), { recursive: true });
        writeFileSync(join(dir, "responses", f), JSON.stringify({ ok: true, result: "ok" }));
      }
    }, 20);
    try {
      const tools = createControlTools(dir);
      const ctx = { sessionID: "s", directory: "D", worktree: "D", abort: new AbortController().signal } as never;
      const out = await tools.zandor_browser.execute({ action: "navigate", url: "https://x.test/" }, ctx);
      expect(out).toBe("ok");
      expect(seen).toEqual([{ action: "browser_navigate", args: { url: "https://x.test/" } }]);
      expect(tools.zandor_browser.description).toMatch(/halaman_tidak_tepercaya/);
      expect(tools.zandor_browser.description).toMatch(/baca-saja/);
    } finally {
      clearInterval(app);
    }
  });
});

describe("saran project dari agent", () => {
  test("isInside: folder sesi Chat dikenali lintas pemisah dan huruf besar-kecil", () => {
    expect(isInside(String.raw`C:\Data\chats`, "c:/data/CHATS/chat-1")).toBe(true);
    expect(isInside(String.raw`C:\Data\chats`, String.raw`C:\Data\chats`)).toBe(false);
    expect(isInside(String.raw`C:\Data\chats`, String.raw`C:\Data\chats-lain\x`)).toBe(false);
    expect(isInside("", String.raw`C:\Data\x`)).toBe(false);
    expect(CHAT_SCOPE_HINT).toMatch(/zandor_suggest_project/);
  });

  test("zandor_suggest_project mengirim aksi suggest_project beserta ringkasan", async () => {
    const dir = mkdtempSync(join(tmpdir(), "zctl-"));
    let seen: { action: string; args: Record<string, unknown>; sessionID: string } | null = null;
    const app = setInterval(() => {
      const reqDir = join(dir, "requests");
      if (!existsSync(reqDir)) return;
      for (const f of readdirSync(reqDir).filter((n) => n.endsWith(".json"))) {
        const req = JSON.parse(readFileSync(join(reqDir, f), "utf8"));
        rmSync(join(reqDir, f), { force: true });
        seen = req;
        mkdirSync(join(dir, "responses"), { recursive: true });
        writeFileSync(join(dir, "responses", f), JSON.stringify({ ok: true, result: "Kartu saran ditampilkan." }));
      }
    }, 20);
    try {
      const t = createControlTools(dir).zandor_suggest_project;
      const out = await t.execute(
        { reason: "Perlu menginstal dependensi.", summary: "Pengguna ingin aplikasi todo React lengkap dengan tes." },
        { sessionID: "ses_1", directory: "D", worktree: "D", abort: new AbortController().signal } as never,
      );
      expect(out).toBe("Kartu saran ditampilkan.");
      expect(seen).toMatchObject({ action: "suggest_project", sessionID: "ses_1" });
      expect((seen as unknown as { args: { summary: string } }).args.summary).toMatch(/todo React/);
    } finally {
      clearInterval(app);
    }
  });
});

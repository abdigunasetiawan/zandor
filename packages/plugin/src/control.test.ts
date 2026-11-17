import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import { requestControl } from "./control";

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
});

import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "vitest";
import { ZandorPlugin } from "./index";
import { redactSecrets } from "./secrets";
import { createSystemTrace } from "./system-trace";

let dir = "";
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "zandor-sys-"));
});
afterEach(() => rmSync(dir, { recursive: true, force: true }));

const lines = (sid: string) =>
  readFileSync(join(dir, `${sid}.jsonl`), "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l));

describe("rekaman system prompt", () => {
  test("hanya menulis saat isi berubah", () => {
    let t = 1;
    const tr = createSystemTrace(dir, () => t++);
    tr.record("ses_a", ["kamu asisten", "env"]);
    tr.record("ses_a", ["kamu asisten", "env"]);
    tr.record("ses_a", ["kamu asisten", "env", "memori"]);
    expect(lines("ses_a")).toEqual([
      { t: 1, system: ["kamu asisten", "env"] },
      { t: 2, system: ["kamu asisten", "env", "memori"] },
    ]);
  });

  test("proses baru melanjutkan dari rekaman terakhir di disk (tanpa duplikat)", () => {
    createSystemTrace(dir).record("ses_b", ["x"]);
    createSystemTrace(dir).record("ses_b", ["x"]);
    expect(lines("ses_b")).toHaveLength(1);
  });

  test("secret disamarkan sebelum ditulis", () => {
    createSystemTrace(dir).record("ses_c", ["kunci: sk-abcdefghijklmnopqrstuvwx"]);
    const raw = readFileSync(join(dir, "ses_c.jsonl"), "utf8");
    expect(raw).not.toContain("sk-abcdefghijklmnopqrstuvwx");
    expect(raw).toContain("[disamarkan: kunci API bergaya sk-]");
  });

  test("id sesi yang tidak aman diabaikan", () => {
    const tr = createSystemTrace(dir);
    tr.record("../keluar", ["x"]);
    tr.record(undefined, ["x"]);
    expect(existsSync(join(dir, "..", "keluar.jsonl"))).toBe(false);
  });

  test("redactSecrets mengganti semua kemunculan", () => {
    expect(redactSecrets("a sk-aaaaaaaaaaaaaaaaaaaa b sk-bbbbbbbbbbbbbbbbbbbb")).toBe(
      "a [disamarkan: kunci API bergaya sk-] b [disamarkan: kunci API bergaya sk-]",
    );
  });

  test("hook plugin merekam isi akhir sesudah suntikan memori", async () => {
    const mem = join(dir, "memory");
    const sys = join(dir, "system");
    const hooks = await ZandorPlugin({ directory: dir } as never, { memoryDir: mem, systemDir: sys });
    const output = { system: ["dasar"] };
    await hooks["experimental.chat.system.transform"]?.({ sessionID: "ses_d" } as never, output);
    const rec = JSON.parse(readFileSync(join(sys, "ses_d.jsonl"), "utf8").trim());
    expect(rec.system).toEqual(output.system);
    expect(rec.system[0]).toBe("dasar");
  });
});

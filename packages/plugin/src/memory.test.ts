import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "vitest";
import { ZandorPlugin } from "./index";
import { normalizeProjectPath, projectKey } from "./keys";
import { createMemoryStore, memoryEnabled, readSettings, scanIndexer } from "./memory";
import { scanSecrets } from "./secrets";
import {
  addCandidate,
  readQueue,
  skillMarkdown,
  submitDraft,
  TOOL_COUNT_THRESHOLD,
  validateDraft,
  writeQueue,
} from "./skills";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "zandor-mem-"));
});
afterEach(() => rmSync(dir, { recursive: true, force: true }));

describe("kunci project", () => {
  test("normalisasi: huruf kecil, slash, tanpa slash akhir, tanpa awalan \\\\?\\", () => {
    expect(normalizeProjectPath("C:\\Work\\Alpha\\")).toBe("c:/work/alpha");
    expect(normalizeProjectPath("\\\\?\\C:\\Work\\Alpha")).toBe("c:/work/alpha");
    expect(projectKey("C:\\Work\\Alpha")).toBe(projectKey("c:/work/alpha/"));
    expect(projectKey("C:/a")).not.toBe(projectKey("C:/b"));
    expect(projectKey("C:/a")).toMatch(/^[0-9a-f]{16}$/);
  });
  test("nilai tetap (dipakai juga oleh UI; jangan diubah tanpa mengubah keduanya)", () => {
    expect(projectKey("C:/Work/alpha")).toBe("3081fc0461f842d4");
  });
});

describe("pemindai secret", () => {
  test("mendeteksi kunci/token umum", () => {
    for (const t of [
      "pakai sk-CONTOHPALSU0123456789abcdefghij untuk API",
      `token ghp_${"a".repeat(36)}`,
      "AKIAABCDEFGHIJKLMNOP",
      "-----BEGIN RSA PRIVATE KEY-----",
      "Authorization: Bearer abcdefghijklmnop12345",
      "postgres://admin:hunter22@db.local/x",
      "MY_API_KEY=abcdef123456789",
    ])
      expect(scanSecrets(t), t).not.toEqual([]);
  });
  test("teks biasa lolos", () => {
    for (const t of ["Gunakan Bun untuk menjalankan tes.", "Port dev adalah 1420; token desain ada di styles.css"])
      expect(scanSecrets(t), t).toEqual([]);
  });
  test("tidak membocorkan nilai secret dalam hasil", () => {
    expect(JSON.stringify(scanSecrets("sk-CONTOHPALSU0123456789abcdefghij"))).not.toContain("CONTOHPALSU");
  });
});

describe("pengaturan opt-in", () => {
  test("bawaan mati; urutan sesi → project → bawaan", () => {
    const s = readSettings(dir);
    expect(s.defaultEnabled).toBe(false);
    expect(memoryEnabled(s, "k1", "ses1")).toBe(false);
    expect(memoryEnabled({ ...s, projects: { k1: true } }, "k1", "ses1")).toBe(true);
    expect(memoryEnabled({ ...s, projects: { k1: true }, sessions: { ses1: false } }, "k1", "ses1")).toBe(false);
    expect(memoryEnabled({ ...s, defaultEnabled: true }, "k2")).toBe(true);
  });
  test("berkas rusak → bawaan aman (mati)", () => {
    writeFileSync(join(dir, "settings.json"), "{bukan json");
    expect(readSettings(dir).defaultEnabled).toBe(false);
  });
});

describe("MemoryStore", () => {
  const make = () => createMemoryStore(dir, "C:/Work/alpha", scanIndexer(dir, projectKey("C:/Work/alpha")));

  test("tulis → blok memuat fakta; edit berkas langsung terbaca", () => {
    const m = make();
    expect(m.block()).toBe("");
    expect(m.write("project", "Proyek memakai Tauri dan Bun.")).toEqual({ ok: true });
    expect(m.write("global", "Jawab dalam Bahasa Indonesia.")).toEqual({ ok: true });
    expect(m.block()).toContain("Proyek memakai Tauri dan Bun.");
    expect(m.block()).toContain("Jawab dalam Bahasa Indonesia.");
    writeFileSync(m.paths.global, "- Edit dari UI\n");
    expect(m.block()).toContain("Edit dari UI");
    expect(m.block()).not.toContain("Bahasa Indonesia");
  });

  test("menolak secret dan isi kosong", () => {
    const m = make();
    const r = m.write("global", "kunci sk-CONTOHPALSU0123456789abcdefghij");
    expect(r.ok).toBe(false);
    expect(existsSync(m.paths.global)).toBe(false);
    expect(m.write("global", "   ").ok).toBe(false);
  });

  test("blok dipotong pada anggaran", () => {
    const m = make();
    writeFileSync(m.paths.global, `- ${"x".repeat(9000)}\n`);
    const b = m.block();
    expect(b.length).toBeLessThan(6200);
    expect(b.endsWith("</zandor-memory>")).toBe(true);
  });

  test("pencarian cadangan menemukan baris dan memisahkan project", () => {
    const m = make();
    m.write("project", "Database memakai SQLite");
    m.write("global", "Selalu jawab singkat");
    expect(m.search("sqlite").map((h) => h.scope)).toEqual(["project"]);
    expect(m.search("singkat")[0]?.text).toBe("Selalu jawab singkat");
    expect(m.search("tidakada")).toEqual([]);
  });

  test("meta.json memetakan key → path untuk UI", () => {
    const m = make();
    const meta = JSON.parse(readFileSync(m.paths.meta, "utf8"));
    expect(meta.projects[m.key].path).toBe("C:/Work/alpha");
  });
});

describe("antrean skill", () => {
  const q = () => join(dir, "q.jsonl");

  test("validateDraft: nama, panjang, secret", () => {
    const ok = {
      name: "tulis-changelog",
      description: "Dipakai saat membuat catatan rilis.",
      body: "1. Baca git log.\n2. Kelompokkan per tipe.",
    };
    expect(validateDraft(ok)).toBeNull();
    expect(validateDraft({ ...ok, name: "Tulis Changelog" })).toMatch(/Nama skill/);
    expect(validateDraft({ ...ok, description: "pendek" })).toMatch(/Deskripsi/);
    expect(validateDraft({ ...ok, body: "x" })).toMatch(/Isi skill/);
    expect(validateDraft({ ...ok, body: `${ok.body}\nsk-CONTOHPALSU0123456789abcdefghij` })).toMatch(/secret/);
  });

  test("kandidat → draf mengisi kandidat terbuka milik sesi", () => {
    addCandidate(q(), { trigger: "correction", sessionID: "s1", excerpt: "salah" });
    addCandidate(q(), { trigger: "tool-count", sessionID: "s2", count: TOOL_COUNT_THRESHOLD });
    const e = submitDraft(
      q(),
      { name: "a-b", description: "deskripsi cukup panjang", body: "x".repeat(30) },
      "s1",
      "k",
    );
    expect(e.status).toBe("pending");
    const all = readQueue(q());
    expect(all).toHaveLength(2);
    expect(all.find((x) => x.sessionID === "s1")?.name).toBe("a-b");
    expect(all.find((x) => x.sessionID === "s2")?.status).toBe("candidate");
  });

  test("draf tanpa kandidat membuat entri manual pending", () => {
    const e = submitDraft(
      q(),
      { name: "a-b", description: "deskripsi cukup panjang", body: "x".repeat(30) },
      "s9",
      "k",
    );
    expect(e.trigger).toBe("manual");
    expect(readQueue(q())).toHaveLength(1);
  });

  test("berkas spike lama (status draft tanpa id) tetap terbaca", () => {
    writeFileSync(q(), `${JSON.stringify({ t: 5, status: "draft", trigger: "correction", sessionID: "s" })}\nrusak\n`);
    const [e] = readQueue(q());
    expect(e?.status).toBe("candidate");
    expect(e?.id).toBe("legacy-5");
    writeQueue(q(), [e as NonNullable<typeof e>]);
    expect(readQueue(q())).toHaveLength(1);
  });

  test("SKILL.md memakai frontmatter yang aman", () => {
    const md = skillMarkdown({ name: "a-b", description: 'pakai "kutip"\nbaris', body: "isi" });
    expect(md.startsWith('---\nname: a-b\ndescription: "pakai \\"kutip\\" baris"\n---\n')).toBe(true);
  });
});

describe("plugin (hook)", () => {
  const input = { directory: "C:/Work/alpha", worktree: "C:/Work/alpha" } as never;
  // biome-ignore lint/suspicious/noExplicitAny: hook dipanggil langsung tanpa engine
  const load = async (opts: Record<string, unknown> = {}): Promise<any> =>
    ZandorPlugin(input, { memoryDir: dir, ...opts });
  const sys = async (
    h: { "experimental.chat.system.transform": (...a: never[]) => Promise<void> },
    sessionID: string,
  ) => {
    const out = { system: [] as string[] };
    await (h["experimental.chat.system.transform"] as (i: unknown, o: unknown) => Promise<void>)({ sessionID }, out);
    return out.system.join("\n");
  };

  test("tanpa memoryDir hanya guard (tidak ada hook memori/tool)", async () => {
    const h = await ZandorPlugin(input, {});
    expect(Object.keys(h)).toEqual(["shell.env"]);
  });

  test("opt-in: mati → tidak ada suntikan dan tool menolak; hidup → blok disuntik", async () => {
    const h = await load();
    const store = createMemoryStore(dir, "C:/Work/alpha", scanIndexer(dir, projectKey("C:/Work/alpha")));
    writeFileSync(store.paths.global, "- fakta global\n");
    expect(await sys(h, "s1")).toBe("");
    expect(await h.tool.memory_write.execute({ scope: "global", content: "x" }, { sessionID: "s1" })).toMatch(
      /dinonaktifkan/,
    );
    writeFileSync(join(dir, "settings.json"), JSON.stringify({ projects: { [projectKey("C:/Work/alpha")]: true } }));
    expect(await sys(h, "s1")).toContain("fakta global");
    expect(await h.tool.memory_write.execute({ scope: "global", content: "baru" }, { sessionID: "s1" })).toBe(
      "Tersimpan.",
    );
    writeFileSync(
      join(dir, "settings.json"),
      JSON.stringify({ projects: { [projectKey("C:/Work/alpha")]: true }, sessions: { s1: false } }),
    );
    expect(await sys(h, "s1")).toBe("");
    expect(await sys(h, "s2")).toContain("baru");
  });

  test("sematan disuntik per sesi dan masuk konteks kompaksi", async () => {
    const h = await load();
    writeFileSync(join(dir, "settings.json"), JSON.stringify({ defaultEnabled: true }));
    writeFileSync(join(dir, "pins", "s1.md"), "JANGAN lupa: port 1420");
    expect(await sys(h, "s1")).toContain("<zandor-pinned>");
    expect(await sys(h, "s2")).not.toContain("port 1420");
    const out = { context: [] as string[] };
    await h["experimental.session.compacting"]({ sessionID: "s1" }, out);
    expect(out.context.join("")).toContain("port 1420");
  });

  test("pemicu skill hanya bila skill creator aktif; koreksi dan /reflect memunculkan petunjuk sekali", async () => {
    const h = await load();
    const msg = (text: string) => h["chat.message"]({ sessionID: "s1" }, { parts: [{ type: "text", text }] });
    await msg("salah, bukan begitu");
    expect(readQueue(join(dir, "skill-queue.jsonl"))).toHaveLength(0);

    writeFileSync(join(dir, "settings.json"), JSON.stringify({ skillCreator: true }));
    await msg("terima kasih");
    expect(readQueue(join(dir, "skill-queue.jsonl"))).toHaveLength(0);
    await msg("salah, bukan begitu");
    await msg("/reflect");
    const q = readQueue(join(dir, "skill-queue.jsonl"));
    expect(q.map((e) => e.trigger)).toEqual(["correction", "reflect"]);
    expect(q.every((e) => e.status === "candidate" && e.project === projectKey("C:/Work/alpha"))).toBe(true);

    expect(await sys(h, "s1")).toContain("skill_propose");
    expect(await sys(h, "s1")).not.toContain("skill_propose");
  });

  test("skill creator per sesi dan per project (sesi → project → bawaan)", async () => {
    const h = await load();
    const msg = (sid: string) =>
      h["chat.message"]({ sessionID: sid }, { parts: [{ type: "text", text: "salah, bukan begitu" }] });
    const key = projectKey("C:/Work/alpha");
    writeFileSync(join(dir, "settings.json"), JSON.stringify({ skillCreatorSessions: { on1: true } }));
    await msg("off1");
    await msg("on1");
    expect(readQueue(join(dir, "skill-queue.jsonl")).map((e) => e.sessionID)).toEqual(["on1"]);
    writeFileSync(
      join(dir, "settings.json"),
      JSON.stringify({ skillCreatorProjects: { [key]: true }, skillCreatorSessions: { off2: false } }),
    );
    await msg("any");
    await msg("off2");
    expect(readQueue(join(dir, "skill-queue.jsonl")).map((e) => e.sessionID)).toEqual(["on1", "any"]);
  });

  test("tool-count memicu tepat sekali pada ambang", async () => {
    const h = await load();
    writeFileSync(join(dir, "settings.json"), JSON.stringify({ skillCreator: true }));
    for (let i = 0; i < TOOL_COUNT_THRESHOLD + 3; i++) await h["tool.execute.after"]({ sessionID: "s1", tool: "bash" });
    const q = readQueue(join(dir, "skill-queue.jsonl"));
    expect(q).toHaveLength(1);
    expect(q[0]?.trigger).toBe("tool-count");
  });

  test("skill_propose: menolak secret, menyimpan draf valid sebagai pending", async () => {
    const h = await load();
    const ctx = { sessionID: "s1" };
    const draft = {
      name: "tulis-changelog",
      description: "Dipakai saat membuat catatan rilis.",
      body: "1. Baca git log.\n2. Kelompokkan per tipe.",
    };
    expect(await h.tool.skill_propose.execute(draft, ctx)).toMatch(/dinonaktifkan/);
    writeFileSync(join(dir, "settings.json"), JSON.stringify({ skillCreator: true }));
    expect(
      await h.tool.skill_propose.execute({ ...draft, body: `${draft.body}\nsk-CONTOHPALSU0123456789abcdefghij` }, ctx),
    ).toMatch(/secret/);
    expect(readQueue(join(dir, "skill-queue.jsonl"))).toHaveLength(0);
    expect(await h.tool.skill_propose.execute(draft, ctx)).toMatch(/menunggu persetujuan/);
    expect(readQueue(join(dir, "skill-queue.jsonl"))[0]?.status).toBe("pending");
  });
});

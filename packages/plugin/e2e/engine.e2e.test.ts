// Uji nyata: plugin Zandor (bundel) dimuat `opencode serve` 1.19.5 asli; model = mock OpenAI-compatible lokal.
// Jalankan: `bun run e2e:engine` di packages/plugin (butuh binary sidecar: `bun run prepare:engine`).
// Bukan bagian vitest (memakai bun:test dan proses engine sungguhan).
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { type ChildProcess, spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { projectKey } from "../src/keys";
import { readQueue, skillMarkdown } from "../src/skills";

const pkg = resolve(import.meta.dir, "..");
const bin = resolve(pkg, "..", "..", "apps", "desktop", "src-tauri", "binaries", "opencode-x86_64-pc-windows-msvc.exe");
const tmp = (tag: string) => realpathSync.native(mkdtempSync(join(tmpdir(), `zandor-p9-${tag}-`)));

type Req = { system: string; tools: string[]; text: string };
const requests: Req[] = [];
const contentText = (c: unknown): string =>
  typeof c === "string" ? c : Array.isArray(c) ? c.map((p: { text?: string }) => p.text ?? "").join(" ") : "";
const toolCall = (name: string, args: unknown) => ({
  role: "assistant",
  tool_calls: [
    { index: 0, id: `call_${Date.now()}`, type: "function", function: { name, arguments: JSON.stringify(args) } },
  ],
});

function decide(body: { messages?: Array<{ role: string; content: unknown }>; tools?: unknown[] }) {
  const msgs = body.messages ?? [];
  const last = msgs[msgs.length - 1];
  const say = (t: string) => ({ delta: { role: "assistant", content: t }, finish: "stop" });
  if (!body.tools || last?.role === "tool") return say(last?.role === "tool" ? "ok" : "RINGKASAN");
  const t = contentText(last?.content);
  const system = msgs
    .filter((m) => m.role === "system")
    .map((m) => contentText(m.content))
    .join("\n");
  if (t.includes("MEMTEST_WRITE"))
    return {
      delta: toolCall("memory_write", { scope: "project", content: "Proyek ini memakai Tauri 2 dan Bun." }),
      finish: "tool_calls",
    };
  const q = t.match(/MEMTEST_SEARCHQ:([A-Za-z0-9]+)/);
  if (q) return { delta: toolCall("memory_search", { query: q[1] }), finish: "tool_calls" };
  if (t.includes("MEMTEST_SEARCH"))
    return { delta: toolCall("memory_search", { query: "Tauri" }), finish: "tool_calls" };
  if (t.includes("MEMTEST_SECRET"))
    return {
      delta: toolCall("memory_write", { scope: "global", content: "kunci sk-CONTOHPALSU0123456789abcdefghij" }),
      finish: "tool_calls",
    };
  if (t.includes("SKILLPROPOSE") && system.includes("zandor-skill-hint"))
    return {
      delta: toolCall("skill_propose", {
        name: "tulis-changelog",
        description: "Dipakai saat membuat catatan rilis dari git log.",
        body: "1. Baca git log sejak tag terakhir.\n2. Kelompokkan per tipe commit.",
      }),
      finish: "tool_calls",
    };
  if (t.includes("SKILLSECRET"))
    return {
      delta: toolCall("skill_propose", {
        name: "bocor",
        description: "Skill yang mencoba membawa secret.",
        body: "Gunakan header Authorization: Bearer abcdefghijklmnop12345 untuk semua permintaan.",
      }),
      finish: "tool_calls",
    };
  return say("ok");
}

const mock = Bun.serve({
  port: 0,
  hostname: "127.0.0.1",
  async fetch(req) {
    const body = (await req.json().catch(() => ({}))) as Parameters<typeof decide>[0];
    const system = (body.messages ?? [])
      .filter((m) => m.role === "system")
      .map((m) => contentText(m.content))
      .join("\n");
    requests.push({
      system,
      tools: (body.tools ?? []).map((t) => (t as { function?: { name: string } }).function?.name ?? ""),
      text: JSON.stringify(body),
    });
    const { delta, finish } = decide(body);
    const chunk = (d: unknown, f: string | null) =>
      `data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", created: 0, model: "m1", choices: [{ index: 0, delta: d, finish_reason: f }] })}\n\n`;
    const usage = `data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", created: 0, model: "m1", choices: [], usage: { prompt_tokens: 10, completion_tokens: 2, total_tokens: 12 } })}\n\n`;
    return new Response(`${chunk(delta, null)}${chunk({}, finish)}${usage}data: [DONE]\n\n`, {
      headers: { "content-type": "text/event-stream" },
    });
  },
});

const ws = tmp("ws");
const xdg = tmp("xdg");
const memoryDir = tmp("mem");
const key = projectKey(ws);
const password = randomBytes(16).toString("hex");
const authH = { Authorization: `Basic ${Buffer.from(`opencode:${password}`).toString("base64")}` };
let proc: ChildProcess;
let base = "";
let sid = "";
const qs = () => `directory=${encodeURIComponent(ws)}`;
async function api(method: string, path: string, body?: unknown) {
  const r = await fetch(`${base}${path}${path.includes("?") ? "&" : "?"}${qs()}`, {
    method,
    headers: { ...authH, ...(body ? { "content-type": "application/json" } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  const t = await r.text();
  if (!r.ok) throw new Error(`${method} ${path} → ${r.status} ${t.slice(0, 300)}`);
  return t ? JSON.parse(t) : null;
}
const say = (text: string, session = sid) =>
  api("POST", `/session/${session}/message`, {
    model: { providerID: "zmock", modelID: "m1" },
    parts: [{ type: "text", text }],
  });
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");
const setSettings = (s: object) => writeFileSync(join(memoryDir, "settings.json"), JSON.stringify(s));
const lastMain = () => [...requests].reverse().find((r) => r.tools.length > 0) as Req;
const toolPart = async (name: string) => {
  const msgs = await api("GET", `/session/${sid}/message`);
  return msgs
    .flatMap((m: { parts: unknown[] }) => m.parts)
    .reverse()
    .find((p: { type: string; tool?: string }) => p.type === "tool" && p.tool === name);
};

beforeAll(async () => {
  const built = Bun.spawnSync(
    ["bun", "build", "src/index.ts", "--target", "bun", "--outfile", "dist/zandor-plugin.js"],
    { cwd: pkg },
  );
  if (built.exitCode !== 0) throw new Error(`build plugin gagal: ${built.stderr.toString()}`);
  if (!existsSync(bin)) throw new Error(`sidecar tidak ada: ${bin} (jalankan bun run prepare:engine)`);
  writeFileSync(join(memoryDir, "global.md"), "- Nama panggilan user: Budi. Suka jawaban singkat.\n");
  // Skill terpasang: satu di project (.opencode/skills), satu global (XDG config).
  const projSkill = join(ws, ".opencode", "skills", "skill-proyek");
  const globSkill = join(xdg, "config", "opencode", "skills", "skill-global");
  mkdirSync(projSkill, { recursive: true });
  mkdirSync(globSkill, { recursive: true });
  writeFileSync(
    join(projSkill, "SKILL.md"),
    skillMarkdown({ name: "skill-proyek", description: "Skill uji tingkat project.", body: "Langkah uji project." }),
  );
  writeFileSync(
    join(globSkill, "SKILL.md"),
    skillMarkdown({ name: "skill-global", description: "Skill uji tingkat global.", body: "Langkah uji global." }),
  );

  const config = {
    $schema: "https://opencode.ai/config.json",
    provider: {
      zmock: {
        npm: "@ai-sdk/openai-compatible",
        name: "Mock",
        options: { baseURL: `http://127.0.0.1:${mock.port}/v1`, apiKey: "x" },
        models: { m1: { name: "m1" } },
      },
    },
    model: "zmock/m1",
    small_model: "zmock/m1",
    permission: { "*": "allow" },
    plugin: [[pathToFileURL(join(pkg, "dist", "zandor-plugin.js")).href, { memoryDir }]],
  };
  const env: Record<string, string> = {
    ...(process.env as Record<string, string>),
    OPENCODE_SERVER_PASSWORD: password,
    OPENCODE_DISABLE_AUTOUPDATE: "1",
    OPENCODE_CONFIG_CONTENT: JSON.stringify(config),
  };
  for (const k of ["CONFIG", "DATA", "CACHE", "STATE"]) env[`XDG_${k}_HOME`] = join(xdg, k.toLowerCase());
  proc = spawn(bin, ["serve", "--hostname", "127.0.0.1", "--port", "0"], {
    cwd: ws,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  base = await new Promise<string>((res, rej) => {
    const t = setTimeout(() => rej(new Error("engine tidak siap")), 30000);
    proc.stdout?.on("data", (d) => {
      const m = String(d).match(/listening on (http:\/\/\S+)/);
      if (m) {
        clearTimeout(t);
        res(m[1] as string);
      }
    });
  });
  sid = (await api("POST", "/session", {})).id;
}, 90000);

afterAll(() => {
  proc?.kill();
  mock.stop(true);
  for (const d of [ws, xdg, memoryDir]) {
    try {
      rmSync(d, { recursive: true, force: true });
    } catch {}
  }
});

describe("Fase 9 di opencode 1.19.5 asli", () => {
  test("opt-in: bawaan mati → tidak ada blok memori dan tool menolak menulis", async () => {
    await say("halo");
    expect(lastMain().system).not.toContain("<zandor-memory>");
    expect(lastMain().tools).toContain("memory_write");
    await say("MEMTEST_WRITE coba");
    expect((await toolPart("memory_write")).state.output).toContain("dinonaktifkan");
    expect(existsSync(join(memoryDir, "projects", `${key}.md`))).toBe(false);
  }, 90000);

  test("diaktifkan per project (tanpa restart): blok disuntik, tulis → langsung terbaca, meta.json terisi", async () => {
    setSettings({ projects: { [key]: true } });
    await say("halo lagi");
    expect(lastMain().system).toContain("Suka jawaban singkat");
    await say("MEMTEST_WRITE simpan");
    expect(read(join(memoryDir, "projects", `${key}.md`))).toContain("Tauri 2 dan Bun");
    expect(lastMain().system).toContain("Tauri 2 dan Bun");
    expect(JSON.parse(read(join(memoryDir, "meta.json"))).projects[key].path.toLowerCase()).toBe(ws.toLowerCase());
    expect(existsSync(join(memoryDir, "index.sqlite"))).toBe(true); // FTS5 dari bun:sqlite aktif
  }, 90000);

  test("memory_write menolak secret", async () => {
    await say("MEMTEST_SECRET simpan");
    const out = (await toolPart("memory_write")).state.output as string;
    expect(out).toContain("secret");
    expect(read(join(memoryDir, "global.md"))).not.toContain("sk-CONTOH");
  }, 90000);

  test("override per sesi mematikan memori untuk sesi itu saja", async () => {
    setSettings({ projects: { [key]: true }, sessions: { [sid]: false } });
    await say("halo");
    expect(lastMain().system).not.toContain("<zandor-memory>");
    const s2 = (await api("POST", "/session", {})).id;
    await say("halo", s2);
    expect(lastMain().system).toContain("<zandor-memory>");
  }, 90000);

  test("skill creator mati secara bawaan: koreksi tidak mencatat kandidat", async () => {
    setSettings({ projects: { [key]: true } });
    await say("salah, bukan begitu");
    expect(readQueue(join(memoryDir, "skill-queue.jsonl"))).toHaveLength(0);
  }, 90000);

  test("koreksi → kandidat → petunjuk di giliran yang sama → model memanggil skill_propose → pending", async () => {
    setSettings({ projects: { [key]: true }, skillCreator: true });
    await say("salah, bukan begitu. SKILLPROPOSE");
    const q = readQueue(join(memoryDir, "skill-queue.jsonl"));
    expect(q).toHaveLength(1);
    expect(q[0]).toMatchObject({ trigger: "correction", status: "pending", name: "tulis-changelog", project: key });
    expect((await toolPart("skill_propose")).state.output).toContain("menunggu persetujuan");
  }, 90000);

  test("draf berisi secret ditolak dan tidak masuk antrean", async () => {
    await say("SKILLSECRET coba");
    expect((await toolPart("skill_propose")).state.output).toContain("secret");
    expect(readQueue(join(memoryDir, "skill-queue.jsonl")).some((e) => e.name === "bocor")).toBe(false);
  }, 90000);

  test("skill terpasang (project dan global) ditemukan engine", async () => {
    const skills = (await api("GET", "/skill")) as Array<{ name: string }>;
    const names = skills.map((s) => s.name);
    expect(names).toContain("skill-proyek");
    expect(names).toContain("skill-global");
  }, 30000);

  test("sematan masuk prompt kompaksi dan tetap ada sesudahnya", async () => {
    writeFileSync(join(memoryDir, "pins", `${sid}.md`), "- Folder docs/ tidak boleh dihapus (ZANDOR-PIN-42)\n");
    setSettings({ projects: { [key]: true }, skillCreator: true });
    const before = requests.length;
    await api("POST", `/session/${sid}/summarize`, { providerID: "zmock", modelID: "m1" });
    const compaction = requests.slice(before).find((r) => r.text.includes("WAJIB dipertahankan"));
    expect(compaction).toBeDefined();
    expect(compaction?.text).toContain("ZANDOR-PIN-42");
    await say("lanjut");
    expect(lastMain().system).toContain("ZANDOR-PIN-42");
  }, 90000);
});

// ---------- Cakupan tambahan: isolasi project, lintas sesi, skill creator per project/sesi, pemicu ----------
describe("Cakupan memori dan skill creator (engine asli, model tiruan)", () => {
  const ws2 = tmp("ws2");
  const key2 = projectKey(ws2);
  const queueFile = join(memoryDir, "skill-queue.jsonl");
  const model = { providerID: "zmock", modelID: "m1" };
  async function apiAt(dir: string, method: string, path: string, body?: unknown) {
    const r = await fetch(`${base}${path}${path.includes("?") ? "&" : "?"}directory=${encodeURIComponent(dir)}`, {
      method,
      headers: { ...authH, ...(body ? { "content-type": "application/json" } : {}) },
      body: body ? JSON.stringify(body) : undefined,
    });
    const t = await r.text();
    if (!r.ok) throw new Error(`${method} ${path} → ${r.status} ${t.slice(0, 300)}`);
    return t ? JSON.parse(t) : null;
  }
  const newSession = async (dir: string): Promise<string> => (await apiAt(dir, "POST", "/session", {})).id;
  const sayAt = (dir: string, session: string, text: string) =>
    apiAt(dir, "POST", `/session/${session}/message`, { model, parts: [{ type: "text", text }] });
  const toolOutput = async (dir: string, session: string, name: string): Promise<string> => {
    const msgs = (await apiAt(dir, "GET", `/session/${session}/message`)) as Array<{ parts: unknown[] }>;
    const part = msgs
      .flatMap((m) => m.parts)
      .reverse()
      .find((p) => (p as { type: string; tool?: string }).type === "tool" && (p as { tool?: string }).tool === name);
    return String((part as { state?: { output?: string } } | undefined)?.state?.output ?? "");
  };
  const resetQueue = () => writeFileSync(queueFile, "");
  const queue = () => readQueue(queueFile);

  afterAll(() => {
    try {
      rmSync(ws2, { recursive: true, force: true });
    } catch {}
  });

  test("isolasi: memori project A tidak bocor ke project B; global tetap berlaku di keduanya", async () => {
    mkdirSync(join(memoryDir, "projects"), { recursive: true });
    writeFileSync(join(memoryDir, "projects", `${key}.md`), "- Project A memakai RAHASIA-A-111\n");
    setSettings({ defaultEnabled: true });
    const sB = await newSession(ws2);
    await sayAt(ws2, sB, "halo dari B");
    expect(lastMain().system).toContain("Suka jawaban singkat"); // global
    expect(lastMain().system).not.toContain("RAHASIA-A-111");
    const sA = await newSession(ws);
    await say("halo dari A", sA);
    expect(lastMain().system).toContain("RAHASIA-A-111");
  }, 90000);

  test("aktif hanya di project A: project B tidak disuntik dan tool menolak menulis", async () => {
    setSettings({ projects: { [key]: true } });
    const sB = await newSession(ws2);
    await sayAt(ws2, sB, "halo");
    expect(lastMain().system).not.toContain("<zandor-memory>");
    await sayAt(ws2, sB, "MEMTEST_WRITE coba di B");
    expect(await toolOutput(ws2, sB, "memory_write")).toContain("dinonaktifkan");
    expect(existsSync(join(memoryDir, "projects", `${key2}.md`))).toBe(false);
  }, 90000);

  test("semua sesi satu project berbagi pengaturan; yang ditulis di sesi A terbaca di sesi baru dan lewat memory_search", async () => {
    setSettings({ projects: { [key]: true } });
    const sA = await newSession(ws);
    await say("MEMTEST_WRITE simpan di A", sA);
    expect(await toolOutput(ws, sA, "memory_write")).toContain("Tersimpan");
    const sNew = await newSession(ws);
    await say("halo sesi baru", sNew);
    expect(lastMain().system).toContain("Tauri 2 dan Bun");
    await say("MEMTEST_SEARCH cari", sNew);
    const found = await toolOutput(ws, sNew, "memory_search");
    expect(found).toContain("2 dan Bun"); // kata yang cocok disorot [..] oleh FTS
    expect(found.split(/\r?\n/).filter((l) => l.includes("2 dan Bun"))).toHaveLength(1); // tanpa duplikat
  }, 90000);

  test("indeks pencarian mengikuti berkas: memori yang diketik/diedit lewat UI bisa dicari, yang dihapus tidak muncul", async () => {
    setSettings({ projects: { [key]: true } });
    const file = join(memoryDir, "projects", `${key}.md`);
    const s = await newSession(ws);
    writeFileSync(file, "- Kode rahasia proyek uji: BIRUXYZ77\n"); // seperti diketik pengguna di halaman Memori
    await say("MEMTEST_SEARCHQ:BIRUXYZ77", s);
    expect(await toolOutput(ws, s, "memory_search")).toContain("[BIRUXYZ77]");
    writeFileSync(file, "- Fakta pengganti tanpa kode\n"); // pengguna menghapus baris itu
    await say("MEMTEST_SEARCHQ:BIRUXYZ77", s);
    expect(await toolOutput(ws, s, "memory_search")).toContain("Tidak ditemukan");
  }, 90000);

  test("memory_write tidak menggandakan fakta yang sama persis", async () => {
    setSettings({ projects: { [key]: true } });
    const file = join(memoryDir, "projects", `${key}.md`);
    writeFileSync(file, "");
    const s = await newSession(ws);
    await say("MEMTEST_WRITE satu", s);
    await say("MEMTEST_WRITE dua", s);
    const lines = readFileSync(file, "utf8")
      .split(String.fromCharCode(10))
      .filter((l) => l.includes("Tauri 2 dan Bun"));
    expect(lines).toHaveLength(1);
  }, 90000);

  test("project B punya memori sendiri yang tidak terlihat di A", async () => {
    setSettings({ projects: { [key]: true, [key2]: true } });
    const sB = await newSession(ws2);
    await sayAt(ws2, sB, "MEMTEST_WRITE simpan di B");
    expect(await toolOutput(ws2, sB, "memory_write")).toContain("Tersimpan");
    expect(readFileSync(join(memoryDir, "projects", `${key2}.md`), "utf8")).toContain("Tauri 2 dan Bun");
    expect(readFileSync(join(memoryDir, "projects", `${key}.md`), "utf8")).not.toContain("RAHASIA-B");
  }, 90000);

  test("skill creator per project: mati global, hanya project A → koreksi tercatat di A, tidak di B", async () => {
    resetQueue();
    setSettings({
      projects: { [key]: true, [key2]: true },
      skillCreator: false,
      skillCreatorProjects: { [key]: true },
    });
    await say("salah, bukan begitu", await newSession(ws));
    expect(queue()).toHaveLength(1);
    expect(queue()[0]).toMatchObject({ trigger: "correction", project: key });
    await sayAt(ws2, await newSession(ws2), "salah, bukan begitu");
    expect(queue()).toHaveLength(1);
  }, 90000);

  test("skill creator per sesi menimpa project", async () => {
    resetQueue();
    const off = await newSession(ws);
    const on = await newSession(ws);
    setSettings({
      projects: { [key]: true },
      skillCreatorProjects: { [key]: true },
      skillCreatorSessions: { [off]: false },
    });
    await say("salah, bukan begitu", off);
    expect(queue()).toHaveLength(0);
    await say("salah, bukan begitu", on);
    expect(queue()).toHaveLength(1);
    expect(queue()[0]?.sessionID).toBe(on);
  }, 90000);

  test("/reflect mencatat kandidat bertrigger reflect; kalimat biasa tidak", async () => {
    resetQueue();
    setSettings({ projects: { [key]: true }, skillCreator: true });
    const s = await newSession(ws);
    await say("tolong rangkum ini", s);
    expect(queue()).toHaveLength(0);
    await say("/reflect", s);
    expect(queue()).toHaveLength(1);
    expect(queue()[0]).toMatchObject({ trigger: "reflect", sessionID: s });
  }, 90000);

  test("pemicu ≥ 5 tool call dalam satu sesi mencatat kandidat tool-count", async () => {
    resetQueue();
    setSettings({ projects: { [key]: true }, skillCreator: true });
    const s = await newSession(ws);
    for (let i = 0; i < 5; i++) await say(`MEMTEST_SEARCH ke-${i + 1}`, s);
    const q = queue().filter((e) => e.sessionID === s);
    expect(q).toHaveLength(1);
    expect(q[0]).toMatchObject({ trigger: "tool-count" });
  }, 180000);

  test("skill yang baru dipasang baru terlihat engine setelah instance dimuat ulang (dispose)", async () => {
    const dir = join(xdg, "config", "opencode", "skills", "skill-setujui");
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      join(dir, "SKILL.md"),
      skillMarkdown({ name: "skill-setujui", description: "Skill uji hasil persetujuan.", body: "Langkah uji." }),
    );
    const list = async () => ((await api("GET", "/skill")) as Array<{ name: string }>).map((s) => s.name);
    // Engine menyimpan cache skill per instance: tanpa dispose, skill baru tidak terlihat (dasar ADR-063).
    expect(await list()).not.toContain("skill-setujui");
    await api("POST", "/instance/dispose");
    expect(await list()).toContain("skill-setujui");
  }, 60000);
});

// Test S6: plugin Zandor dimuat oleh `opencode serve` 1.19.5 asli; model = mock OpenAI-compatible
// lokal yang mencatat setiap request (system prompt, daftar tool) dan menjalankan skenario tool call.
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { randomBytes } from "node:crypto";

const root = resolve(import.meta.dir, "..");
const bin = resolve(root, "..", "target", "debug", process.platform === "win32" ? "opencode.exe" : "opencode");
const tmp = (tag: string) => realpathSync.native(mkdtempSync(join(tmpdir(), `zandor-s6-${tag}-`)));

type Req = { body: any; system: string; tools: string[]; text: string };
const requests: Req[] = [];

// ---------- mock model ----------
const toolCall = (calls: { name: string; args: unknown }[]) => ({
  role: "assistant",
  tool_calls: calls.map((c, i) => ({ index: i, id: `call_${Date.now()}_${i}`, type: "function", function: { name: c.name, arguments: JSON.stringify(c.args) } })),
});
const contentText = (c: unknown): string =>
  typeof c === "string" ? c : Array.isArray(c) ? c.map((p: any) => p.text ?? "").join(" ") : "";

function decide(body: any): { delta: any; finish: string } {
  const msgs: any[] = body.messages ?? [];
  const last = msgs[msgs.length - 1];
  const say = (t: string) => ({ delta: { role: "assistant", content: t }, finish: "stop" });
  if (!body.tools || last?.role === "tool") return say(last?.role === "tool" ? "ok" : "RINGKASAN");
  const t = contentText(last?.content);
  if (t.includes("MEMTEST_WRITE")) return { delta: toolCall([{ name: "memory_write", args: { scope: "project", content: "Proyek ini memakai Tauri 2 dan Bun." } }]), finish: "tool_calls" };
  if (t.includes("MEMTEST_SEARCH")) return { delta: toolCall([{ name: "memory_search", args: { query: "Tauri" } }]), finish: "tool_calls" };
  if (t.includes("MEMTEST_MANY"))
    return { delta: toolCall([0, 1, 2].map(() => ({ name: "memory_search", args: { query: "Bun" } }))), finish: "tool_calls" };
  return say("ok");
}

const mock = Bun.serve({
  port: 0,
  hostname: "127.0.0.1",
  async fetch(req) {
    const body = (await req.json().catch(() => ({}))) as any;
    const system = (body.messages ?? []).filter((m: any) => m.role === "system").map((m: any) => contentText(m.content)).join("\n");
    requests.push({ body, system, tools: (body.tools ?? []).map((t: any) => t.function?.name), text: JSON.stringify(body) });
    const { delta, finish } = decide(body);
    const chunk = (d: any, f: string | null) =>
      `data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", created: 0, model: "m1", choices: [{ index: 0, delta: d, finish_reason: f }] })}\n\n`;
    const usage = `data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", created: 0, model: "m1", choices: [], usage: { prompt_tokens: 10, completion_tokens: 2, total_tokens: 12 } })}\n\n`;
    return new Response(chunk(delta, null) + chunk({}, finish) + usage + "data: [DONE]\n\n", { headers: { "content-type": "text/event-stream" } });
  },
});

// ---------- engine ----------
const ws = tmp("ws");
const xdg = tmp("xdg");
const memoryDir = tmp("mem");
const password = randomBytes(16).toString("hex");
const authH = { Authorization: "Basic " + Buffer.from("opencode:" + password).toString("base64") };
let proc: ChildProcess;
let base = "";
let sid = "";
const q = () => `directory=${encodeURIComponent(ws)}`;
async function api(method: string, path: string, body?: unknown) {
  const r = await fetch(`${base}${path}${path.includes("?") ? "&" : "?"}${q()}`, {
    method,
    headers: { ...authH, ...(body ? { "content-type": "application/json" } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  const t = await r.text();
  if (!r.ok) throw new Error(`${method} ${path} → ${r.status} ${t.slice(0, 300)}`);
  return t ? JSON.parse(t) : null;
}
const say = (text: string) => api("POST", `/session/${sid}/message`, { model: { providerID: "zmock", modelID: "m1" }, parts: [{ type: "text", text }] });
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");
const hookLog = () => read(join(memoryDir, "hook-log.jsonl")).trim().split("\n").filter(Boolean).map((l) => JSON.parse(l));
const lastMainRequest = () => [...requests].reverse().find((r) => r.tools.length > 0)!;

beforeAll(async () => {
  writeFileSync(join(memoryDir, "global.md"), "- Nama panggilan user: Budi. Suka jawaban singkat.\n");
  const config = {
    $schema: "https://opencode.ai/config.json",
    provider: { zmock: { npm: "@ai-sdk/openai-compatible", name: "Mock", options: { baseURL: `http://127.0.0.1:${mock.port}/v1`, apiKey: "x" }, models: { m1: { name: "m1" } } } },
    model: "zmock/m1",
    small_model: "zmock/m1",
    permission: { "*": "allow" },
    plugin: [[pathToFileURL(process.env.ZANDOR_PLUGIN_PATH ?? join(root, "src", "plugin.ts")).href, { memoryDir }]],
  };
  const env: Record<string, string> = { ...(process.env as Record<string, string>), OPENCODE_SERVER_PASSWORD: password, OPENCODE_DISABLE_AUTOUPDATE: "1", OPENCODE_CONFIG_CONTENT: JSON.stringify(config) };
  for (const k of ["CONFIG", "DATA", "CACHE", "STATE"]) env[`XDG_${k}_HOME`] = join(xdg, k.toLowerCase());
  proc = spawn(bin, ["serve", "--hostname", "127.0.0.1", "--port", "0"], { cwd: ws, env, stdio: ["ignore", "pipe", "pipe"] });
  base = await new Promise<string>((res, rej) => {
    const t = setTimeout(() => rej(new Error("engine tidak siap")), 30000);
    proc.stdout!.on("data", (d) => {
      const m = String(d).match(/listening on (http:\/\/\S+)/);
      if (m) { clearTimeout(t); res(m[1]); }
    });
  });
  sid = (await api("POST", "/session", {})).id;
}, 60000);

afterAll(() => {
  proc?.kill();
  mock.stop(true);
  for (const d of [ws, xdg, memoryDir]) try { rmSync(d, { recursive: true, force: true }); } catch {}
});

// ---------- skenario (berurutan) ----------
describe("plugin memori di opencode 1.19.5", () => {
  test("plugin termuat dengan opsi dari config", async () => {
    await say("halo");
    const init = hookLog().find((h) => h.hook === "init");
    expect(init).toBeDefined();
    expect(existsSync(join(memoryDir, "index.sqlite"))).toBe(true); // bun:sqlite tersedia di runtime plugin
  }, 60000);

  test("blok memori disuntik ke system prompt", () => {
    const r = lastMainRequest();
    expect(r.system).toContain("<zandor-memory>");
    expect(r.system).toContain("Suka jawaban singkat");
  });

  test("tool kustom memory_write & memory_search terdaftar untuk model", () => {
    const r = lastMainRequest();
    expect(r.tools).toContain("memory_write");
    expect(r.tools).toContain("memory_search");
  });

  test("agent menulis memori → tersimpan & langsung muncul di giliran berikutnya", async () => {
    await say("MEMTEST_WRITE simpan fakta project");
    const files = [...new Bun.Glob("projects/*.md").scanSync(memoryDir)];
    expect(files.length).toBe(1);
    expect(read(join(memoryDir, files[0]))).toContain("Tauri 2 dan Bun");
    expect(lastMainRequest().system).toContain("Tauri 2 dan Bun"); // request lanjutan setelah tool
  }, 60000);

  test("memory_search memakai FTS5 dan hasilnya kembali ke model", async () => {
    await say("MEMTEST_SEARCH cari");
    const msgs = await api("GET", `/session/${sid}/message`);
    const part = msgs.flatMap((m: any) => m.parts).reverse().find((p: any) => p.type === "tool" && p.tool === "memory_search");
    expect(part.state.status).toBe("completed");
    expect(part.state.output).toContain("[Tauri]");
  }, 60000);

  test("pemicu skill creator: koreksi user masuk antrean", async () => {
    await say("salah, bukan begitu caranya");
    const queue = read(join(memoryDir, "skill-queue.jsonl"));
    expect(queue).toContain('"trigger":"correction"');
    expect(queue).toContain('"status":"draft"');
  }, 60000);

  test("pemicu skill creator: jumlah tool call per sesi", async () => {
    await say("MEMTEST_MANY tiga pencarian");
    const n = hookLog().filter((h) => h.hook === "tool.execute.after" && h.sessionID === sid).length;
    expect(n).toBeGreaterThanOrEqual(5);
    expect(read(join(memoryDir, "skill-queue.jsonl"))).toContain('"trigger":"tool-count"');
  }, 60000);

  test("konteks yang disematkan masuk prompt kompaksi dan tetap ada setelahnya", async () => {
    writeFileSync(join(memoryDir, "pins", `${sid}.md`), "- Folder docs/ tidak boleh dihapus (ZANDOR-PIN-42)\n");
    const before = requests.length;
    await api("POST", `/session/${sid}/summarize`, { providerID: "zmock", modelID: "m1" });
    const during = requests.slice(before);
    expect(hookLog().some((h) => h.hook === "session.compacting" && h.sessionID === sid)).toBe(true);
    const compactionReq = during.find((r) => r.text.includes("WAJIB dipertahankan"));
    expect(compactionReq).toBeDefined();
    expect(compactionReq!.text).toContain("ZANDOR-PIN-42");
    await say("lanjut");
    expect(lastMainRequest().system).toContain("ZANDOR-PIN-42");
    expect(lastMainRequest().system).toContain("<zandor-memory>");
  }, 90000);

  test("opt-out per sesi lalu global: tidak ada suntikan, tool menolak menulis", async () => {
    writeFileSync(join(memoryDir, "settings.json"), JSON.stringify({ enabled: true, disabledSessions: [sid] }));
    await say("halo lagi");
    expect(lastMainRequest().system).not.toContain("<zandor-memory>");
    await say("MEMTEST_WRITE coba simpan");
    const msgs = await api("GET", `/session/${sid}/message`);
    const part = msgs.flatMap((m: any) => m.parts).reverse().find((p: any) => p.type === "tool" && p.tool === "memory_write");
    expect(part.state.output).toContain("dinonaktifkan");
    writeFileSync(join(memoryDir, "settings.json"), JSON.stringify({ enabled: false, disabledSessions: [] }));
    const s2 = (await api("POST", "/session", {})).id;
    await api("POST", `/session/${s2}/message`, { model: { providerID: "zmock", modelID: "m1" }, parts: [{ type: "text", text: "halo" }] });
    expect(lastMainRequest().system).not.toContain("<zandor-memory>");
  }, 90000);

  test("ringkasan hook yang benar-benar dipanggil", () => {
    const counts: Record<string, number> = {};
    for (const h of hookLog()) counts[h.hook] = (counts[h.hook] ?? 0) + 1;
    console.log("hook terpanggil:", counts, "· request ke model:", requests.length);
    writeFileSync(join(root, "..", "target", "s6-hooks.json"), JSON.stringify({ counts, requests: requests.length }, null, 2));
    for (const k of ["init", "system.transform", "chat.message", "tool.execute.after", "session.compacting"]) expect(counts[k]).toBeGreaterThan(0);
  });
});

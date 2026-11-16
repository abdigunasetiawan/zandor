// Uji nyata background job (ADR-072): plugin Zandor (bundel) dimuat `opencode serve` 1.19.5 asli; model = mock
// OpenAI-compatible lokal yang memanggil tool job_*. Server yang dijalankan job adalah server HTTP sungguhan.
// Jalankan: `bun test e2e/jobs.e2e.test.ts` di packages/plugin (butuh sidecar: `bun run prepare:engine`).
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { type ChildProcess, spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const pkg = resolve(import.meta.dir, "..");
const bin = resolve(pkg, "..", "..", "apps", "desktop", "src-tauri", "binaries", "opencode-x86_64-pc-windows-msvc.exe");
const tmp = (tag: string) => realpathSync.native(mkdtempSync(join(tmpdir(), `zandor-jobs-${tag}-`)));

const ws = tmp("ws");
const xdg = tmp("xdg");
const jobsDir = tmp("jobs");
const serverPort = 20000 + Math.floor(Math.random() * 20000);
// "Dev server" sungguhan: mendengarkan port lalu mencetak URL-nya seperti vite.
writeFileSync(
  join(ws, "server.cjs"),
  `require("http").createServer((q,r)=>r.end("halo-dari-job")).listen(${serverPort},"127.0.0.1",()=>console.log("  Local: http://localhost:${serverPort}/"));`,
);
const command = `node server.cjs`;

type Req = { system: string; tools: string[] };
const requests: Req[] = [];
const contentText = (c: unknown): string =>
  typeof c === "string" ? c : Array.isArray(c) ? c.map((p: { text?: string }) => p.text ?? "").join(" ") : "";
let callN = 0;
const toolCall = (name: string, args: unknown) => ({
  role: "assistant",
  tool_calls: [
    { index: 0, id: `call_${++callN}`, type: "function", function: { name, arguments: JSON.stringify(args) } },
  ],
});

function decide(body: { messages?: Array<{ role: string; content: unknown }>; tools?: unknown[] }) {
  const msgs = body.messages ?? [];
  const last = msgs[msgs.length - 1];
  const say = (t: string) => ({ delta: { role: "assistant", content: t }, finish: "stop" });
  if (!body.tools || last?.role === "tool") return say("ok");
  const t = contentText(last?.content);
  const id = t.match(/(job-[a-z0-9]+)/)?.[1];
  if (t.includes("JOBSTART"))
    return { delta: toolCall("job_start", { command, name: "server uji", wait_seconds: 15 }), finish: "tool_calls" };
  if (t.includes("JOBESCAPE")) return { delta: toolCall("job_start", { command, cwd: ".." }), finish: "tool_calls" };
  if (t.includes("JOBOUTPUT") && id)
    return { delta: toolCall("job_output", { id, wait_for: "Local", timeout_seconds: 5 }), finish: "tool_calls" };
  if (t.includes("JOBLIST")) return { delta: toolCall("job_list", {}), finish: "tool_calls" };
  return say("ok");
}

const mock = Bun.serve({
  port: 0,
  hostname: "127.0.0.1",
  async fetch(req) {
    const body = (await req.json().catch(() => ({}))) as Parameters<typeof decide>[0];
    requests.push({
      system: (body.messages ?? [])
        .filter((m) => m.role === "system")
        .map((m) => contentText(m.content))
        .join("\n"),
      tools: (body.tools ?? []).map((x) => (x as { function?: { name: string } }).function?.name ?? ""),
    });
    const { delta, finish } = decide(body);
    const chunk = (d: unknown, f: string | null) =>
      `data: ${JSON.stringify({ id: "c", object: "chat.completion.chunk", created: 0, model: "m1", choices: [{ index: 0, delta: d, finish_reason: f }] })}\n\n`;
    return new Response(`${chunk(delta, null)}${chunk({}, finish)}data: [DONE]\n\n`, {
      headers: { "content-type": "text/event-stream" },
    });
  },
});

const password = randomBytes(16).toString("hex");
const authH = { Authorization: `Basic ${Buffer.from(`opencode:${password}`).toString("base64")}` };
let proc: ChildProcess;
let base = "";
let sid = "";
async function api(method: string, path: string, body?: unknown) {
  const r = await fetch(`${base}${path}${path.includes("?") ? "&" : "?"}directory=${encodeURIComponent(ws)}`, {
    method,
    headers: { ...authH, ...(body ? { "content-type": "application/json" } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  const t = await r.text();
  if (!r.ok) throw new Error(`${method} ${path} → ${r.status} ${t.slice(0, 300)}`);
  return t ? JSON.parse(t) : null;
}
const say = (text: string) =>
  api("POST", `/session/${sid}/message`, {
    model: { providerID: "zmock", modelID: "m1" },
    parts: [{ type: "text", text }],
  });
const toolPart = async (name: string) => {
  const msgs = await api("GET", `/session/${sid}/message`);
  return msgs
    .flatMap((m: { parts: unknown[] }) => m.parts)
    .reverse()
    .find((p: { type: string; tool?: string }) => p.type === "tool" && p.tool === name) as {
    state: { status: string; output?: string; error?: string };
  };
};
const lastMain = () => [...requests].reverse().find((r) => r.tools.length > 0) as Req;
const metas = () =>
  readdirSync(jobsDir)
    .filter((f) => f.endsWith(".json"))
    .map((f) => JSON.parse(readFileSync(join(jobsDir, f), "utf8")));
const until = async (fn: () => boolean | Promise<boolean>, ms = 15000) => {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await fn()) return true;
    await new Promise((r) => setTimeout(r, 200));
  }
  return false;
};
const ping = () =>
  fetch(`http://127.0.0.1:${serverPort}/`)
    .then((r) => r.text())
    .catch(() => "");

beforeAll(async () => {
  const built = Bun.spawnSync(
    ["bun", "build", "src/index.ts", "--target", "bun", "--outfile", "dist/zandor-plugin.js"],
    { cwd: pkg },
  );
  if (built.exitCode !== 0) throw new Error(`build plugin gagal: ${built.stderr.toString()}`);
  if (!existsSync(bin)) throw new Error(`sidecar tidak ada: ${bin} (jalankan bun run prepare:engine)`);
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
    // Sama dengan bawaan Zandor: bash wajib ditanya → job_start juga harus ditanya.
    permission: { "*": "allow", bash: "ask" },
    plugin: [[pathToFileURL(join(pkg, "dist", "zandor-plugin.js")).href, { jobsDir }]],
  };
  const env: Record<string, string> = {
    ...(process.env as Record<string, string>),
    OPENCODE_SERVER_PASSWORD: password,
    ZANDOR_SECRET_UJI: "sk-jangan-bocor",
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
    const t = setTimeout(() => rej(new Error("engine tidak siap")), 60000);
    proc.stdout?.on("data", (d) => {
      const m = String(d).match(/listening on (http:\/\/\S+)/);
      if (m) {
        clearTimeout(t);
        res(m[1] as string);
      }
    });
  });
  sid = (await api("POST", "/session", {})).id;
}, 120000);

afterAll(() => {
  proc?.kill();
  mock.stop(true);
  for (const d of [ws, xdg, jobsDir]) {
    try {
      rmSync(d, { recursive: true, force: true });
    } catch {}
  }
});

describe("background job di opencode 1.19.5 asli", () => {
  test("tool job_* terdaftar dan petunjuk 'jangan pakai bash' disuntik", async () => {
    await say("halo");
    for (const n of ["job_start", "job_output", "job_list", "job_stop", "job_restart"])
      expect(lastMain().tools).toContain(n);
    expect(lastMain().system).toContain("JANGAN pakai bash");
    expect(lastMain().system).not.toContain("<zandor-jobs>");
  }, 90000);

  test("job_start meminta izin bash; disetujui → server hidup, port terdeteksi", async () => {
    const pending = say("JOBSTART jalankan server");
    let perm: { id: string; permission: string; patterns: string[] } | undefined;
    expect(
      await until(async () => {
        perm = ((await api("GET", "/permission")) as (typeof perm)[]).find((p) => p?.permission === "bash");
        return !!perm;
      }),
    ).toBe(true);
    expect(perm?.patterns).toContain(command);
    expect(metas()).toHaveLength(0); // belum ada proses sebelum disetujui
    await api("POST", `/permission/${perm?.id}/reply`, { reply: "once" });
    await pending;
    const part = await toolPart("job_start");
    expect(part.state.status).toBe("completed");
    expect(part.state.output).toContain(`http://localhost:${serverPort}`);
    expect(part.state.output).toContain("status: running");
    expect(await ping()).toBe("halo-dari-job");
    const [m] = metas();
    expect(m).toMatchObject({ status: "running", ports: [serverPort], sessionID: sid });
    expect(readFileSync(join(jobsDir, `${m.id}.log`), "utf8")).toContain("Local:");
  }, 90000);

  test("giliran berikutnya: blok <zandor-jobs> + job_output/job_list membaca job", async () => {
    const id = metas()[0].id as string;
    await say(`JOBOUTPUT ${id}`);
    expect(lastMain().system).toContain("<zandor-jobs>");
    expect(lastMain().system).toContain(`${id} [running] port ${serverPort}`);
    expect((await toolPart("job_output")).state.output).toContain("Local:");
    await say("JOBLIST");
    expect((await toolPart("job_list")).state.output).toContain(id);
  }, 90000);

  test("cwd di luar project ditolak tanpa menjalankan apa pun", async () => {
    await say("JOBESCAPE");
    expect((await toolPart("job_start")).state.output).toContain("di luar folder project");
    expect(metas()).toHaveLength(1);
  }, 90000);

  test("berkas kontrol dari UI menghentikan server (pohon proses ikut mati)", async () => {
    const id = metas()[0].id as string;
    writeFileSync(join(jobsDir, "control", `${id}.stop`), "");
    expect(await until(() => metas()[0].status === "stopped")).toBe(true);
    expect(await until(async () => (await ping()) === "")).toBe(true);
  }, 30000);
});

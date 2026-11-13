// Spike S6 — plugin opencode Zandor: memori (Markdown + SQLite FTS5) & pemicu skill creator.
//
// Dimuat lewat config: "plugin": [["file:///…/plugin.ts", { "memoryDir": "<dir>" }]]
// Berjalan di runtime Bun milik binary opencode (bun:sqlite tersedia).
//
// Tata letak memoryDir (semua bisa diedit UI Zandor; dibaca ulang setiap giliran):
//   settings.json            { enabled: boolean, disabledSessions: string[] }
//   global.md                memori jangka panjang lintas project
//   projects/<id>.md         memori per project (id = hash path worktree)
//   pins/<sessionID>.md      konteks yang disematkan (tahan kompaksi)
//   index.sqlite             indeks FTS5 untuk memory_search
//   skill-queue.jsonl        kandidat skill (antrean persetujuan)
import type { Plugin } from "@opencode-ai/plugin";
import { tool } from "@opencode-ai/plugin";
import { Database } from "bun:sqlite";
import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const MAX_BLOCK_CHARS = 6000; // anggaran blok memori di system prompt
const SKILL_TOOL_THRESHOLD = 5; // jumlah tool call per sesi yang memicu kandidat skill
const CORRECTION = /\b(salah|bukan begitu|jangan (lagi|begitu)|keliru|that'?s wrong|not like that)\b/i;
const SENSITIVE_ENV = (k: string) =>
  k.startsWith("ZANDOR_SECRET_") || ["ZANDOR_PROXY_TOKEN", "OPENCODE_SERVER_PASSWORD", "OPENCODE_CONFIG_CONTENT", "OPENCODE_AUTH_CONTENT"].includes(k);

type Settings = { enabled: boolean; disabledSessions: string[] };

export const ZandorPlugin: Plugin = async (input, options) => {
  const dir = String(options?.memoryDir ?? "");
  if (!dir) throw new Error("zandor: opsi memoryDir wajib");
  for (const d of ["projects", "pins"]) mkdirSync(join(dir, d), { recursive: true });
  const projectId = createHash("sha256").update(input.worktree || input.directory).digest("hex").slice(0, 16);
  const paths = {
    settings: join(dir, "settings.json"),
    global: join(dir, "global.md"),
    project: join(dir, "projects", `${projectId}.md`),
    pins: (sid: string) => join(dir, "pins", `${sid.replace(/[^\w-]/g, "_")}.md`),
    queue: join(dir, "skill-queue.jsonl"),
    log: join(dir, "hook-log.jsonl"),
  };
  const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8").trim() : "");
  const settings = (): Settings => {
    try {
      return { enabled: true, disabledSessions: [], ...JSON.parse(read(paths.settings) || "{}") };
    } catch {
      return { enabled: true, disabledSessions: [] };
    }
  };
  const enabledFor = (sid?: string) => {
    const s = settings();
    return s.enabled && !(sid && s.disabledSessions.includes(sid));
  };
  const hookLog = (hook: string, data: Record<string, unknown> = {}) =>
    appendFileSync(paths.log, JSON.stringify({ t: Date.now(), hook, ...data }) + "\n");

  // --- indeks FTS5 ---
  const db = new Database(join(dir, "index.sqlite"));
  db.run("CREATE VIRTUAL TABLE IF NOT EXISTS mem USING fts5(scope, project, content, created UNINDEXED)");
  const index = (scope: string, content: string) =>
    db.query("INSERT INTO mem(scope, project, content, created) VALUES (?, ?, ?, ?)").run(scope, projectId, content, Date.now());

  const block = (sid?: string) => {
    const g = read(paths.global);
    const p = read(paths.project);
    if (!g && !p) return "";
    let text = `<zandor-memory>\n${g ? `## Memori global\n${g}\n` : ""}${p ? `## Memori project\n${p}\n` : ""}</zandor-memory>`;
    if (text.length > MAX_BLOCK_CHARS) text = text.slice(0, MAX_BLOCK_CHARS) + "\n…(dipotong)</zandor-memory>";
    return text;
  };

  const toolCalls = new Map<string, number>();
  const queue = (entry: Record<string, unknown>) =>
    appendFileSync(paths.queue, JSON.stringify({ t: Date.now(), status: "draft", ...entry }) + "\n");

  hookLog("init", { projectId, directory: input.directory });

  return {
    // S4: secret & token server tidak boleh diwarisi proses anak.
    "shell.env": async (_i, output) => {
      for (const k of Object.keys(process.env)) if (SENSITIVE_ENV(k)) output.env[k] = "";
    },

    // Suntik blok memori ke system prompt setiap giliran (dibaca ulang → edit UI langsung berlaku).
    "experimental.chat.system.transform": async (i, output) => {
      const on = enabledFor(i.sessionID);
      hookLog("system.transform", { sessionID: i.sessionID, enabled: on });
      if (!on) return;
      const b = block(i.sessionID);
      if (b) output.system.push(b);
      // Sematan juga disuntik tiap giliran: tetap ada walau ringkasan kompaksi menghilangkannya.
      const pins = i.sessionID ? read(paths.pins(i.sessionID)) : "";
      if (pins) output.system.push(`<zandor-pinned>\n${pins}\n</zandor-pinned>`);
    },

    // Konteks yang disematkan ikut ke prompt kompaksi → tidak hilang setelah dipadatkan.
    "experimental.session.compacting": async (i, output) => {
      const pins = read(paths.pins(i.sessionID));
      hookLog("session.compacting", { sessionID: i.sessionID, pins: pins.length });
      if (pins) output.context.push(`<zandor-pinned>\nWAJIB dipertahankan apa adanya dalam ringkasan:\n${pins}\n</zandor-pinned>`);
    },

    // Pemicu skill creator: koreksi dari user.
    "chat.message": async (i, output) => {
      const text = output.parts.map((p) => (p.type === "text" ? p.text : "")).join(" ");
      hookLog("chat.message", { sessionID: i.sessionID });
      if (CORRECTION.test(text)) queue({ trigger: "correction", sessionID: i.sessionID, excerpt: text.slice(0, 200) });
    },

    // Pemicu skill creator: banyak tool call dalam satu sesi.
    "tool.execute.after": async (i) => {
      const n = (toolCalls.get(i.sessionID) ?? 0) + 1;
      toolCalls.set(i.sessionID, n);
      hookLog("tool.execute.after", { sessionID: i.sessionID, tool: i.tool, n });
      if (n === SKILL_TOOL_THRESHOLD) queue({ trigger: "tool-count", sessionID: i.sessionID, count: n });
    },

    tool: {
      memory_write: tool({
        description: "Simpan fakta penting ke memori Zandor agar diingat di sesi berikutnya. Gunakan untuk preferensi user, keputusan project, dan fakta yang stabil.",
        args: {
          scope: tool.schema.enum(["project", "global"]).describe("project = khusus project ini; global = semua project"),
          content: tool.schema.string().min(1).max(2000).describe("Fakta singkat dalam satu kalimat atau poin"),
        },
        async execute(args, ctx) {
          if (!enabledFor(ctx.sessionID)) return "Memori dinonaktifkan untuk sesi ini; tidak ada yang disimpan.";
          const file = args.scope === "global" ? paths.global : paths.project;
          appendFileSync(file, `- ${args.content.replace(/\n+/g, " ")}\n`);
          index(args.scope, args.content);
          return { title: `memori ${args.scope} disimpan`, output: "Tersimpan.", metadata: { scope: args.scope } };
        },
      }),
      memory_search: tool({
        description: "Cari memori Zandor (FTS5) untuk fakta yang pernah disimpan.",
        args: { query: tool.schema.string().min(1).describe("Kata kunci") },
        async execute(args, ctx) {
          if (!enabledFor(ctx.sessionID)) return "Memori dinonaktifkan untuk sesi ini.";
          const q = args.query.replace(/["*^:()]/g, " ").trim().split(/\s+/).map((w) => `"${w}"`).join(" OR ");
          const rows = db
            .query("SELECT scope, snippet(mem, 2, '[', ']', '…', 12) AS s FROM mem WHERE mem MATCH ? AND (scope = 'global' OR project = ?) ORDER BY rank LIMIT 5")
            .all(q, projectId) as { scope: string; s: string }[];
          return rows.length ? rows.map((r) => `(${r.scope}) ${r.s}`).join("\n") : "Tidak ditemukan.";
        },
      }),
    },
  };
};

// Sengaja tanpa `export default`: opencode bisa memuat setiap export fungsi sebagai plugin.

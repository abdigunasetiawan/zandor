// Plugin opencode Zandor: guard env (wajib, ADR-022) + memori dan skill creator (ADR-023, Fase 9).
// Memori/skill hanya aktif bila opsi `memoryDir` diberikan; tanpa itu plugin hanya menjadi guard.
import { mkdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type { Plugin } from "@opencode-ai/plugin";
import { tool } from "@opencode-ai/plugin";
import { scrubEnv } from "./env-guard";
import { projectKey } from "./keys";
import { createMemoryStore, type Indexer, memoryFacts, scanIndexer } from "./memory";
import {
  addCandidate,
  CORRECTION,
  REFLECT,
  readQueue,
  skillHint,
  submitDraft,
  TOOL_COUNT_THRESHOLD,
  validateDraft,
} from "./skills";

/** Bagian `bun:sqlite` yang dipakai (tipe Bun tidak dimuat di paket ini). */
type SqliteDb = {
  run(sql: string): unknown;
  query(sql: string): { run(...a: unknown[]): unknown; all(...a: unknown[]): unknown[] };
};

/** FTS5 lewat `bun:sqlite` (ada di runtime opencode); bila tidak tersedia, pakai pemindaian teks. */
async function openIndexer(dir: string, key: string): Promise<Indexer> {
  try {
    const modName = "bun:sqlite";
    const { Database } = (await import(/* @vite-ignore */ modName)) as { Database: new (file: string) => SqliteDb };
    const db = new Database(join(dir, "index.sqlite"));
    db.run("CREATE VIRTUAL TABLE IF NOT EXISTS mem USING fts5(scope, project, content, created UNINDEXED)");
    // Indeks hanyalah turunan dari berkas Markdown (sumber kebenaran): disinkronkan ulang bila berkas berubah, sehingga
    // memori yang diketik/diedit/dihapus lewat UI atau tangan ikut tercermin (bukan hanya yang ditulis lewat memory_write).
    const files = { global: join(dir, "global.md"), project: join(dir, "projects", `${key}.md`) } as const;
    const stamp = (f: string) => {
      try {
        const st = statSync(f);
        return `${st.mtimeMs}:${st.size}`;
      } catch {
        return "-";
      }
    };
    let signature = "";
    const sync = () => {
      const now = `${stamp(files.global)}|${stamp(files.project)}`;
      if (now === signature) return;
      db.query("DELETE FROM mem WHERE scope = 'global' OR project = ?").run(key);
      const ins = db.query("INSERT INTO mem(scope, project, content, created) VALUES (?, ?, ?, ?)");
      const at = Date.now();
      for (const fact of memoryFacts(files.global)) ins.run("global", key, fact, at);
      for (const fact of memoryFacts(files.project)) ins.run("project", key, fact, at);
      signature = now;
    };
    return {
      add() {
        // diindeks lewat sync() saat pencarian (berkas = sumber kebenaran)
      },
      search(query, project, limit) {
        sync();
        const q = query
          .replace(/["*^:()]/g, " ")
          .trim()
          .split(/\s+/)
          .filter(Boolean)
          .map((w) => `"${w}"`)
          .join(" OR ");
        if (!q) return [];
        const rows = db
          .query(
            "SELECT scope, snippet(mem, 2, '[', ']', '…', 12) AS s FROM mem WHERE mem MATCH ? AND (scope = 'global' OR project = ?) ORDER BY rank LIMIT ?",
          )
          .all(q, project, limit) as { scope: "global" | "project"; s: string }[];
        return rows.map((r) => ({ scope: r.scope, text: r.s }));
      },
    };
  } catch {
    return scanIndexer(dir, key);
  }
}

export const ZandorPlugin: Plugin = async (input, options) => {
  // Supervisor menghapus penanda sebelum start lalu memeriksanya sesudah engine siap. Tanpa penanda, perlindungan
  // env tidak aktif dan engine TIDAK boleh dipakai.
  const marker = typeof options?.markerFile === "string" ? options.markerFile : null;
  if (marker) {
    mkdirSync(dirname(marker), { recursive: true });
    writeFileSync(marker, JSON.stringify({ guard: "shell.env", pid: process.pid, at: Date.now() }));
  }
  const guard = {
    "shell.env": async (_i: unknown, output: { env: Record<string, string> }) => {
      Object.assign(output.env, scrubEnv(process.env));
    },
  };

  const dir = typeof options?.memoryDir === "string" ? options.memoryDir : "";
  if (!dir) return guard;

  mkdirSync(dir, { recursive: true });
  const worktree = input.directory || input.worktree;
  const key = projectKey(worktree);
  const store = createMemoryStore(dir, worktree, await openIndexer(dir, key));
  const toolCalls = new Map<string, number>();
  /** Sesi yang kandidat skill-nya baru dicatat → petunjuk disuntik pada giliran berikutnya. */
  const hinted = new Map<string, string>();

  return {
    ...guard,

    // Blok memori + sematan + petunjuk skill disuntik tiap giliran (dibaca ulang → edit UI langsung berlaku).
    "experimental.chat.system.transform": async (i, output) => {
      if (store.enabled(i.sessionID)) {
        const b = store.block();
        if (b) output.system.push(b);
        const pins = i.sessionID ? store.pins(i.sessionID) : "";
        if (pins) output.system.push(`<zandor-pinned>\n${pins}\n</zandor-pinned>`);
      }
      const sid = i.sessionID;
      const trigger = sid ? hinted.get(sid) : undefined;
      if (sid && trigger && store.skillCreatorOn(sid)) {
        output.system.push(skillHint(trigger));
        hinted.delete(sid);
      }
    },

    // Sematan ikut prompt kompaksi → tidak hilang setelah dipadatkan.
    "experimental.session.compacting": async (i, output) => {
      if (!store.enabled(i.sessionID)) return;
      const pins = store.pins(i.sessionID);
      if (pins)
        output.context.push(
          `<zandor-pinned>\nWAJIB dipertahankan apa adanya dalam ringkasan:\n${pins}\n</zandor-pinned>`,
        );
    },

    // Pemicu skill creator: koreksi dari user dan /reflect.
    "chat.message": async (i, output) => {
      if (!store.skillCreatorOn(i.sessionID)) return;
      const text = output.parts.map((p) => (p.type === "text" ? p.text : "")).join(" ");
      const trigger = REFLECT.test(text) ? "reflect" : CORRECTION.test(text) ? "correction" : null;
      if (!trigger) return;
      addCandidate(store.paths.queue, {
        trigger,
        sessionID: i.sessionID,
        project: key,
        excerpt: text.slice(0, 200),
      });
      hinted.set(i.sessionID, trigger);
    },

    // Pemicu skill creator: banyak tool call dalam satu sesi.
    "tool.execute.after": async (i) => {
      const n = (toolCalls.get(i.sessionID) ?? 0) + 1;
      toolCalls.set(i.sessionID, n);
      if (n === TOOL_COUNT_THRESHOLD && store.skillCreatorOn(i.sessionID)) {
        addCandidate(store.paths.queue, { trigger: "tool-count", sessionID: i.sessionID, project: key, count: n });
        hinted.set(i.sessionID, "tool-count");
      }
    },

    tool: {
      memory_write: tool({
        description:
          "Simpan fakta penting ke memori Zandor agar diingat di sesi berikutnya. Gunakan untuk preferensi user, keputusan project, dan fakta yang stabil. Jangan menyimpan secret.",
        args: {
          scope: tool.schema
            .enum(["project", "global"])
            .describe("project = khusus project ini; global = semua project"),
          content: tool.schema.string().min(1).max(2000).describe("Fakta singkat dalam satu kalimat atau poin"),
        },
        async execute(args, ctx) {
          if (!store.enabled(ctx.sessionID)) return "Memori dinonaktifkan untuk sesi ini; tidak ada yang disimpan.";
          const r = store.write(args.scope, args.content);
          return r.ok ? "Tersimpan." : r.reason;
        },
      }),
      memory_search: tool({
        description: "Cari memori Zandor untuk fakta yang pernah disimpan.",
        args: { query: tool.schema.string().min(1).describe("Kata kunci") },
        async execute(args, ctx) {
          if (!store.enabled(ctx.sessionID)) return "Memori dinonaktifkan untuk sesi ini.";
          const hits = store.search(args.query);
          return hits.length ? hits.map((h) => `(${h.scope}) ${h.text}`).join("\n") : "Tidak ditemukan.";
        },
      }),
      skill_propose: tool({
        description:
          "Usulkan skill baru (prosedur yang bisa dipakai ulang) ke antrean persetujuan pengguna. Skill tidak aktif sebelum disetujui. Jangan menyertakan secret.",
        args: {
          name: tool.schema.string().describe("Nama kebab-case, mis. tulis-changelog"),
          description: tool.schema.string().describe("Kapan skill ini dipakai (10–300 karakter)"),
          body: tool.schema.string().describe("Langkah-langkah dalam Markdown singkat"),
        },
        async execute(args, ctx) {
          if (!store.skillCreatorOn(ctx.sessionID)) return "Skill creator dinonaktifkan; tidak ada yang diajukan.";
          const err = validateDraft(args);
          if (err) return err;
          const e = submitDraft(store.paths.queue, args, ctx.sessionID, key);
          const dup = readQueue(store.paths.queue).filter((x) => x.name === e.name && x.status === "pending").length;
          return `Draf skill "${e.name}" menunggu persetujuan pengguna${dup > 1 ? " (nama sama sudah ada di antrean)" : ""}.`;
        },
      }),
    },
  };
};
// Sengaja tanpa `export default`: opencode bisa memuat setiap export fungsi sebagai plugin.

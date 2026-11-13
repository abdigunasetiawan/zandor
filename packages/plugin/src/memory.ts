// Penyimpanan memori Zandor (ADR-011/023): Markdown (global, project, sematan) + pencarian.
// Semua berkas dibaca ulang setiap giliran, jadi edit dari UI langsung berlaku tanpa restart engine.
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { projectKey, safeId } from "./keys";
import { scanSecrets } from "./secrets";

export const MAX_BLOCK_CHARS = 6000;

export type MemorySettings = {
  /** Bawaan untuk project/sesi yang belum menentukan sendiri. Opt-in: bawaan `false`. */
  defaultEnabled: boolean;
  projects: Record<string, boolean>;
  sessions: Record<string, boolean>;
  /** Skill creator (pemicu + draf) aktif; terpisah dari memori. */
  skillCreator: boolean;
};

export const DEFAULT_SETTINGS: MemorySettings = {
  defaultEnabled: false,
  projects: {},
  sessions: {},
  skillCreator: false,
};

export type SearchHit = { scope: "global" | "project"; text: string };

/** Pencarian: implementasi FTS5 (bun:sqlite) disuntik plugin; cadangan = pemindaian teks. */
export interface Indexer {
  add(scope: "global" | "project", project: string, content: string): void;
  search(query: string, project: string, limit: number): SearchHit[];
}

export function memoryPaths(dir: string, key: string) {
  return {
    settings: join(dir, "settings.json"),
    meta: join(dir, "meta.json"),
    global: join(dir, "global.md"),
    project: join(dir, "projects", `${key}.md`),
    pins: (sid: string) => join(dir, "pins", `${safeId(sid)}.md`),
    queue: join(dir, "skill-queue.jsonl"),
  };
}

const readText = (p: string) => (existsSync(p) ? readFileSync(p, "utf8").trim() : "");

export function readSettings(dir: string): MemorySettings {
  try {
    const raw = JSON.parse(readText(join(dir, "settings.json")) || "{}");
    return {
      defaultEnabled: raw.defaultEnabled === true,
      projects: typeof raw.projects === "object" && raw.projects ? raw.projects : {},
      sessions: typeof raw.sessions === "object" && raw.sessions ? raw.sessions : {},
      skillCreator: raw.skillCreator === true,
    };
  } catch {
    return { ...DEFAULT_SETTINGS, projects: {}, sessions: {} };
  }
}

/** Sesi → project → bawaan. */
export function memoryEnabled(s: MemorySettings, key: string, sessionID?: string): boolean {
  if (sessionID && typeof s.sessions[sessionID] === "boolean") return s.sessions[sessionID] as boolean;
  if (typeof s.projects[key] === "boolean") return s.projects[key] as boolean;
  return s.defaultEnabled;
}

export function createMemoryStore(dir: string, worktree: string, indexer: Indexer) {
  const key = projectKey(worktree);
  const paths = memoryPaths(dir, key);
  for (const d of ["projects", "pins"]) mkdirSync(join(dir, d), { recursive: true });

  // Peta key → path agar UI bisa menampilkan nama project.
  try {
    const meta = JSON.parse(readText(paths.meta) || "{}") as {
      projects?: Record<string, { path: string; seen: number }>;
    };
    meta.projects = { ...meta.projects, [key]: { path: worktree, seen: Date.now() } };
    writeFileSync(paths.meta, JSON.stringify(meta, null, 2));
  } catch {
    // meta hanya kemudahan UI
  }

  const enabled = (sessionID?: string) => memoryEnabled(readSettings(dir), key, sessionID);

  return {
    key,
    paths,
    enabled,
    skillCreatorOn: () => readSettings(dir).skillCreator,

    /** Blok `<zandor-memory>` untuk system prompt, atau "" bila tidak ada isi. */
    block(): string {
      const g = readText(paths.global);
      const p = readText(paths.project);
      if (!g && !p) return "";
      const text = `<zandor-memory>\n${g ? `## Memori global\n${g}\n` : ""}${p ? `## Memori project\n${p}\n` : ""}</zandor-memory>`;
      return text.length > MAX_BLOCK_CHARS ? `${text.slice(0, MAX_BLOCK_CHARS)}\n…(dipotong)</zandor-memory>` : text;
    },

    pins: (sessionID: string) => readText(paths.pins(sessionID)),

    /** Tulis fakta; menolak teks yang tampak memuat secret. */
    write(scope: "global" | "project", content: string): { ok: true } | { ok: false; reason: string } {
      const found = scanSecrets(content);
      if (found.length) return { ok: false, reason: `Ditolak: teks tampak memuat secret (${found.join(", ")}).` };
      const clean = content.replace(/\s*\n+\s*/g, " ").trim();
      if (!clean) return { ok: false, reason: "Ditolak: isi kosong." };
      appendFileSync(scope === "global" ? paths.global : paths.project, `- ${clean}\n`);
      indexer.add(scope, key, clean);
      return { ok: true };
    },

    search: (query: string) => indexer.search(query, key, 5),
  };
}

/** Cadangan tanpa SQLite: cari kata di berkas Markdown. */
export function scanIndexer(dir: string, key: string): Indexer {
  return {
    add() {},
    search(query, _project, limit) {
      const words = query.toLowerCase().split(/\s+/).filter(Boolean);
      const hits: SearchHit[] = [];
      for (const [scope, file] of [
        ["global", join(dir, "global.md")],
        ["project", join(dir, "projects", `${key}.md`)],
      ] as const) {
        for (const line of readText(file).split("\n")) {
          const l = line.toLowerCase();
          if (words.some((w) => l.includes(w))) hits.push({ scope, text: line.replace(/^-\s*/, "") });
        }
      }
      return hits.slice(0, limit);
    },
  };
}

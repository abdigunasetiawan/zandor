// Store memori & skill (Fase 9, ADR-037). Host (Tauri) membaca/menulis `<app_data>/memory` dan folder skill opencode;
// plugin Zandor di engine membaca berkas yang sama setiap giliran, jadi perubahan dari UI langsung berlaku.
// Format berkas = kontrak dengan `packages/plugin` (diuji paritasnya di memory.test.ts).

export type MemoryScope = "global" | "project";
export type MemoryFile = { rel: string; size: number; modified: number };
export type SkillInfo = { name: string; scope: MemoryScope; path: string };

export interface MemoryHost {
  /** false = pratinjau browser: data TIDAK dibaca engine. */
  connected: boolean;
  list(): Promise<MemoryFile[]>;
  read(rel: string): Promise<string>;
  write(rel: string, text: string): Promise<void>;
  remove(rel: string): Promise<void>;
  skillList(scope: MemoryScope, project: string | null): Promise<SkillInfo[]>;
  skillRead(scope: MemoryScope, project: string | null, name: string): Promise<string>;
  skillWrite(
    scope: MemoryScope,
    project: string | null,
    name: string,
    markdown: string,
    overwrite: boolean,
  ): Promise<string>;
  skillDelete(scope: MemoryScope, project: string | null, name: string): Promise<void>;
}

export type MemorySettings = {
  defaultEnabled: boolean;
  projects: Record<string, boolean>;
  sessions: Record<string, boolean>;
  skillCreator: boolean;
};

export type SkillStatus = "candidate" | "pending" | "approved" | "rejected";
export type SkillTrigger = "correction" | "tool-count" | "reflect" | "manual";
export type SkillEntry = {
  id: string;
  t: number;
  status: SkillStatus;
  trigger: SkillTrigger;
  sessionID?: string;
  project?: string;
  excerpt?: string;
  count?: number;
  name?: string;
  description?: string;
  body?: string;
};

export type InstalledSkills = { global: SkillInfo[]; projects: Record<string, SkillInfo[]> };

export type MemoryState = {
  ready: boolean;
  loading: boolean;
  error: string | null;
  settings: MemorySettings;
  /** key project → path (ditulis plugin saat project dibuka). */
  known: Record<string, string>;
  files: MemoryFile[];
  queue: SkillEntry[];
  installed: InstalledSkills;
};

export const DEFAULT_MEMORY_SETTINGS: MemorySettings = {
  defaultEnabled: false,
  projects: {},
  sessions: {},
  skillCreator: false,
};

const INITIAL: MemoryState = {
  ready: false,
  loading: false,
  error: null,
  settings: DEFAULT_MEMORY_SETTINGS,
  known: {},
  files: [],
  queue: [],
  installed: { global: [], projects: {} },
};

// ---- fungsi murni (paritas dengan plugin) ----

export function normalizeProjectPath(path: string): string {
  const p = path.replace(/\\/g, "/").replace(/^\/\/\?\//, "");
  return (p.length > 1 ? p.replace(/\/+$/, "") : p).toLowerCase();
}

/** sha256 16 heks dari path ternormalisasi; harus sama dengan `projectKey` di plugin. */
export async function projectKey(path: string): Promise<string> {
  const data = new TextEncoder().encode(normalizeProjectPath(path));
  const hash = await crypto.subtle.digest("SHA-256", data);
  return [...new Uint8Array(hash)]
    .slice(0, 8)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export function parseSettings(text: string): MemorySettings {
  try {
    const raw = JSON.parse(text || "{}") as Partial<MemorySettings>;
    return {
      defaultEnabled: raw.defaultEnabled === true,
      projects: typeof raw.projects === "object" && raw.projects ? raw.projects : {},
      sessions: typeof raw.sessions === "object" && raw.sessions ? raw.sessions : {},
      skillCreator: raw.skillCreator === true,
    };
  } catch {
    return { ...DEFAULT_MEMORY_SETTINGS, projects: {}, sessions: {} };
  }
}

export function parseQueue(text: string): SkillEntry[] {
  const out: SkillEntry[] = [];
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      const e = JSON.parse(line) as SkillEntry;
      if (!e.id) e.id = `legacy-${e.t}`;
      if ((e.status as string) === "draft") e.status = "candidate";
      out.push(e);
    } catch {
      // baris rusak dilewati
    }
  }
  return out;
}

export const serializeQueue = (q: SkillEntry[]) => q.map((e) => JSON.stringify(e)).join("\n") + (q.length ? "\n" : "");

/** Sesi → project → bawaan (sama dengan plugin). */
export function memoryEnabled(s: MemorySettings, key: string | null, sessionID?: string): boolean {
  if (sessionID && typeof s.sessions[sessionID] === "boolean") return s.sessions[sessionID] as boolean;
  if (key && typeof s.projects[key] === "boolean") return s.projects[key] as boolean;
  return s.defaultEnabled;
}

const SKILL_NAME = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const RULES: Array<[string, RegExp]> = [
  ["kunci API bergaya sk-", /\bsk-[A-Za-z0-9_-]{16,}/],
  ["token GitHub", /\bgh[pousr]_[A-Za-z0-9]{30,}/],
  ["token Slack", /\bxox[abprs]-[A-Za-z0-9-]{10,}/],
  ["kunci akses AWS", /\bAKIA[0-9A-Z]{16}\b/],
  ["kunci Google API", /\bAIza[0-9A-Za-z_-]{30,}/],
  ["token JWT", /\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/],
  ["kunci privat", /-----BEGIN [A-Z ]*PRIVATE KEY-----/],
  ["header Authorization", /\bauthorization\s*[:=]\s*(bearer|basic)\s+[A-Za-z0-9._~+/=-]{12,}/i],
  ["kredensial di URL", /\b[a-z][a-z0-9+.-]*:\/\/[^\s/:@]+:[^\s/@]{3,}@/i],
  ["variabel rahasia", /\b[A-Z0-9_]*(?:SECRET|TOKEN|PASSWORD|API_?KEY)[A-Z0-9_]*\s*[=:]\s*["']?[^\s"']{8,}/],
];

export function scanSecrets(text: string): string[] {
  return RULES.filter(([, re]) => re.test(text)).map(([name]) => name);
}

export type SkillDraft = { name: string; description: string; body: string };

/** Pesan galat (Bahasa Indonesia) atau null bila draf valid. Sama dengan `validateDraft` di plugin. */
export function validateSkillDraft(d: SkillDraft): string | null {
  if (!SKILL_NAME.test(d.name) || d.name.length > 64)
    return "Nama skill harus huruf kecil/angka dengan tanda hubung (maks 64), mis. `tulis-changelog`.";
  if (d.description.trim().length < 10 || d.description.length > 300) return "Deskripsi harus 10–300 karakter.";
  if (d.body.trim().length < 20 || d.body.length > 8000) return "Isi skill harus 20–8000 karakter.";
  const found = scanSecrets(`${d.name}\n${d.description}\n${d.body}`);
  if (found.length) return `Ditolak: draf tampak memuat secret (${found.join(", ")}).`;
  return null;
}

export function skillMarkdown(e: SkillDraft): string {
  const desc = e.description.replace(/\s+/g, " ").replace(/"/g, '\\"');
  return `---\nname: ${e.name}\ndescription: "${desc}"\n---\n\n${e.body.trim()}\n`;
}

/** Ambil `name`, `description`, dan isi dari SKILL.md buatan Zandor/pengguna; null bila bukan format itu. */
export function parseSkillMarkdown(md: string): SkillDraft | null {
  const m = /^---\r?\nname:\s*(.+?)\r?\ndescription:\s*(.+?)\r?\n---\r?\n\r?\n?([\s\S]*)$/.exec(md);
  if (!m) return null;
  const unquote = (s: string) => (/^".*"$/.test(s) ? s.slice(1, -1).replace(/\\"/g, '"') : s);
  return { name: unquote(m[1] as string), description: unquote(m[2] as string), body: (m[3] as string).trim() };
}

export const FILE_GLOBAL = "global.md";
export const FILE_SETTINGS = "settings.json";
export const FILE_QUEUE = "skill-queue.jsonl";
export const FILE_META = "meta.json";
export const projectFile = (key: string) => `projects/${key}.md`;
export const pinFile = (sessionId: string) => `pins/${sessionId.replace(/[^\w-]/g, "_")}.md`;

export class MemoryStore {
  private snap: MemoryState = INITIAL;
  private listeners = new Set<() => void>();
  private projectPaths: string[] = [];

  constructor(private readonly host: MemoryHost) {}

  get connected(): boolean {
    return this.host.connected;
  }

  state(): MemoryState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private update(patch: Partial<MemoryState>) {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l();
  }

  /** Muat semuanya. `projectPaths` = path project terdaftar (untuk skill project). */
  async refresh(projectPaths: string[] = this.projectPaths): Promise<void> {
    this.projectPaths = projectPaths;
    this.update({ loading: true, error: null });
    try {
      const [settingsText, metaText, queueText, files, globalSkills] = await Promise.all([
        this.host.read(FILE_SETTINGS),
        this.host.read(FILE_META),
        this.host.read(FILE_QUEUE),
        this.host.list(),
        this.host.skillList("global", null),
      ]);
      let known: Record<string, string> = {};
      try {
        const meta = JSON.parse(metaText || "{}") as { projects?: Record<string, { path: string }> };
        known = Object.fromEntries(Object.entries(meta.projects ?? {}).map(([k, v]) => [k, v.path]));
      } catch {
        // meta rusak: abaikan
      }
      const projects: Record<string, SkillInfo[]> = {};
      for (const p of projectPaths) projects[p] = await this.host.skillList("project", p).catch(() => []);
      this.update({
        ready: true,
        loading: false,
        settings: parseSettings(settingsText),
        known,
        files,
        queue: parseQueue(queueText),
        installed: { global: globalSkills, projects },
      });
    } catch (e) {
      this.update({ loading: false, ready: true, error: e instanceof Error ? e.message : String(e) });
    }
  }

  /** Muat hanya pengaturan opt-in (ringan; dipakai panel Info sesi). */
  async loadSettings(): Promise<void> {
    try {
      this.update({ settings: parseSettings(await this.host.read(FILE_SETTINGS)) });
    } catch (e) {
      this.update({ error: e instanceof Error ? e.message : String(e) });
    }
  }

  // ---- pengaturan (baca ulang dulu agar perubahan lain tidak tertimpa) ----

  async patchSettings(fn: (s: MemorySettings) => MemorySettings): Promise<void> {
    const cur = parseSettings(await this.host.read(FILE_SETTINGS));
    const next = fn(cur);
    await this.host.write(FILE_SETTINGS, JSON.stringify(next, null, 2));
    this.update({ settings: next });
  }

  setDefaultEnabled = (on: boolean) => this.patchSettings((s) => ({ ...s, defaultEnabled: on }));
  setSkillCreator = (on: boolean) => this.patchSettings((s) => ({ ...s, skillCreator: on }));
  /** `null` = ikuti bawaan (hapus override). */
  setProjectEnabled = (key: string, on: boolean | null) =>
    this.patchSettings((s) => {
      const projects = { ...s.projects };
      if (on === null) delete projects[key];
      else projects[key] = on;
      return { ...s, projects };
    });
  setSessionEnabled = (sessionId: string, on: boolean | null) =>
    this.patchSettings((s) => {
      const sessions = { ...s.sessions };
      if (on === null) delete sessions[sessionId];
      else sessions[sessionId] = on;
      return { ...s, sessions };
    });

  // ---- berkas memori ----

  readFile = (rel: string) => this.host.read(rel);

  async saveFile(rel: string, text: string): Promise<void> {
    await this.host.write(rel, text);
    this.update({ files: await this.host.list() });
  }

  async deleteFile(rel: string): Promise<void> {
    await this.host.remove(rel);
    this.update({ files: await this.host.list() });
  }

  // ---- antrean skill ----

  private async mutateQueue(fn: (q: SkillEntry[]) => SkillEntry[]): Promise<void> {
    // Baca ulang tepat sebelum menulis: plugin bisa menambah baris kapan saja.
    const q = fn(parseQueue(await this.host.read(FILE_QUEUE)));
    await this.host.write(FILE_QUEUE, serializeQueue(q));
    this.update({ queue: q });
  }

  /** Ubah isi draf sebelum disetujui; nama/deskripsi/isi divalidasi dan dipindai secret. */
  async editDraft(id: string, d: SkillDraft): Promise<string | null> {
    const err = validateSkillDraft(d);
    if (err) return err;
    await this.mutateQueue((q) => q.map((e) => (e.id === id ? { ...e, ...d, status: "pending" } : e)));
    return null;
  }

  async reject(id: string): Promise<void> {
    await this.mutateQueue((q) => q.map((e) => (e.id === id ? { ...e, status: "rejected" } : e)));
  }

  async removeEntry(id: string): Promise<void> {
    await this.mutateQueue((q) => q.filter((e) => e.id !== id));
  }

  /** Setujui draf: validasi ulang → tulis SKILL.md → tandai approved. Mengembalikan galat atau null. */
  async approve(id: string, scope: MemoryScope, projectPath: string | null): Promise<string | null> {
    const entry = parseQueue(await this.host.read(FILE_QUEUE)).find((e) => e.id === id);
    if (entry?.status !== "pending") return "Draf tidak ditemukan atau belum siap disetujui.";
    const draft = { name: entry.name ?? "", description: entry.description ?? "", body: entry.body ?? "" };
    const err = validateSkillDraft(draft);
    if (err) return err;
    if (scope === "project" && !projectPath) return "Pilih project untuk skill ini.";
    try {
      await this.host.skillWrite(
        scope,
        scope === "project" ? projectPath : null,
        draft.name,
        skillMarkdown(draft),
        false,
      );
    } catch (e) {
      return e instanceof Error ? e.message : String(e);
    }
    await this.mutateQueue((q) => q.map((x) => (x.id === id ? { ...x, status: "approved" } : x)));
    await this.refresh();
    return null;
  }

  // ---- skill terpasang ----

  skillRead = (scope: MemoryScope, project: string | null, name: string) => this.host.skillRead(scope, project, name);

  async skillSave(scope: MemoryScope, project: string | null, name: string, md: string): Promise<string | null> {
    const parsed = parseSkillMarkdown(md);
    if (parsed) {
      const err = validateSkillDraft(parsed);
      if (err) return err;
      if (parsed.name !== name) return "Nama di frontmatter harus sama dengan nama folder skill.";
    } else if (scanSecrets(md).length) {
      return `Ditolak: teks tampak memuat secret (${scanSecrets(md).join(", ")}).`;
    }
    try {
      await this.host.skillWrite(scope, project, name, md, true);
    } catch (e) {
      return e instanceof Error ? e.message : String(e);
    }
    await this.refresh();
    return null;
  }

  async skillRemove(scope: MemoryScope, project: string | null, name: string): Promise<void> {
    await this.host.skillDelete(scope, project, name);
    await this.refresh();
  }
}

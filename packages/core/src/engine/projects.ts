// Project & sesi (Fase 4).
// - PROJECT = folder. Daftar project, nama, dan warna dimiliki Zandor (registry JSON): opencode hanya mengenal
//   repositori git dan menggabungkan semua folder non-git ke satu project "global" (ADR-032).
// - SESI dimiliki engine dan terikat ke direktori (`GET /session?directory=`). Sesi tanpa project ("Chat")
//   memakai direktori khusus milik Zandor.
// Hapus project = lepas dari daftar; sesi di engine TIDAK dihapus.
import { type EngineApi, isDefaultSessionTitle, type SessionInfo } from "./api";

export const CHAT_KEY = "chat" as const;
export type ProjectKey = string; // id project atau CHAT_KEY

export type Project = { id: string; path: string; name: string; color: string | null; addedAt: number };
export type SessionsState = {
  status: "idle" | "loading" | "ready" | "error";
  items: SessionInfo[];
  error: string | null;
};
export type Selection = { key: ProjectKey | null; sessionId: string | null };

export type ProjectsState = {
  loaded: boolean;
  projects: Project[];
  chatDir: string | null;
  engineReady: boolean;
  sessions: Record<ProjectKey, SessionsState>;
  selection: Selection;
};

export type RegistryStorage = { load(): Promise<string | null>; save(text: string): Promise<void> };

const REGISTRY_VERSION = 1;
const PROJECT_COLORS = ["slate", "red", "orange", "amber", "green", "teal", "blue", "violet", "pink"] as const;
export const isProjectColor = (c: string): boolean => (PROJECT_COLORS as readonly string[]).includes(c);
export { PROJECT_COLORS };

const WINDOWS_PATH = /^[A-Za-z]:[\\/]/;

/** Rapikan path: pemisah konsisten, tanpa pemisah di ujung (kecuali akar drive). */
export function normalizePath(path: string): string {
  const p = path.trim();
  if (WINDOWS_PATH.test(p)) {
    const s = p.replace(/\//g, "\\").replace(/\\+$/g, "");
    return /^[A-Za-z]:$/.test(s) ? `${s}\\` : s;
  }
  return p.length > 1 ? p.replace(/\/+$/g, "") : p;
}

/** Kunci identitas: Windows tidak peka huruf besar/kecil. */
export const pathKey = (path: string): string => {
  const n = normalizePath(path);
  return WINDOWS_PATH.test(n) ? n.toLowerCase() : n;
};

export function baseName(path: string): string {
  const n = normalizePath(path);
  const parts = n.split(/[\\/]/).filter(Boolean);
  const last = parts.at(-1) ?? n;
  return /^[A-Za-z]:$/.test(last) ? `${last}\\` : last;
}

const emptySessions = (): SessionsState => ({ status: "idle", items: [], error: null });
const byRecent = (a: SessionInfo, b: SessionInfo) => b.time.updated - a.time.updated || b.time.created - a.time.created;

type Persisted = { version: number; projects: Project[]; selection: Selection };

function parseRegistry(text: string | null): { value: Persisted | null; error: string | null } {
  if (!text) return { value: null, error: null };
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { value: null, error: "registry project rusak (bukan JSON valid)" };
  }
  const o = raw as { version?: unknown; projects?: unknown; selection?: unknown };
  if (!o || typeof o !== "object" || o.version !== REGISTRY_VERSION || !Array.isArray(o.projects)) {
    return { value: null, error: "registry project berformat tidak dikenal" };
  }
  const projects: Project[] = [];
  const seen = new Set<string>();
  for (const p of o.projects as Array<Partial<Project>>) {
    if (typeof p?.id !== "string" || typeof p.path !== "string" || typeof p.name !== "string") continue;
    const k = pathKey(p.path);
    if (seen.has(k) || seen.has(`id:${p.id}`)) continue;
    seen.add(k);
    seen.add(`id:${p.id}`);
    projects.push({
      id: p.id,
      path: normalizePath(p.path),
      name: p.name,
      color: typeof p.color === "string" && isProjectColor(p.color) ? p.color : null,
      addedAt: typeof p.addedAt === "number" ? p.addedAt : 0,
    });
  }
  const sel = o.selection as Partial<Selection> | undefined;
  const selection: Selection = {
    key: typeof sel?.key === "string" ? sel.key : null,
    sessionId: typeof sel?.sessionId === "string" ? sel.sessionId : null,
  };
  return { value: { version: REGISTRY_VERSION, projects, selection }, error: null };
}

export type ProjectStoreOptions = {
  storage: RegistryStorage;
  newId?: () => string;
  now?: () => number;
  onWarning?: (message: string) => void;
};

export class ProjectStore {
  private snap: ProjectsState = {
    loaded: false,
    projects: [],
    chatDir: null,
    engineReady: false,
    sessions: {},
    selection: { key: null, sessionId: null },
  };
  private listeners = new Set<() => void>();
  private api: EngineApi | null = null;
  private requestSeq = new Map<ProjectKey, number>();
  private readonly newId: () => string;
  private readonly now: () => number;

  constructor(private readonly options: ProjectStoreOptions) {
    this.newId = options.newId ?? (() => crypto.randomUUID());
    this.now = options.now ?? Date.now;
  }

  state(): ProjectsState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private update(patch: Partial<ProjectsState>) {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l();
  }

  private updateSessions(key: ProjectKey, patch: Partial<SessionsState>) {
    const cur = this.snap.sessions[key] ?? emptySessions();
    this.update({ sessions: { ...this.snap.sessions, [key]: { ...cur, ...patch } } });
  }

  private async persist() {
    const data: Persisted = { version: REGISTRY_VERSION, projects: this.snap.projects, selection: this.snap.selection };
    await this.options.storage.save(JSON.stringify(data));
  }

  /** Muat registry. Rusak/tidak dikenal → mulai kosong dan lapor (data lama tidak ditimpa sampai ada perubahan). */
  async init(): Promise<void> {
    const text = await this.options.storage.load();
    const { value, error } = parseRegistry(text);
    if (error) this.options.onWarning?.(error);
    this.update({
      loaded: true,
      projects: value?.projects ?? [],
      selection: value?.selection ?? { key: null, sessionId: null },
    });
  }

  setChatDir(dir: string | null) {
    this.update({ chatDir: dir ? normalizePath(dir) : null });
    if (this.api && dir) void this.refresh(CHAT_KEY);
  }

  /** Pasang/lepas koneksi engine. Saat terpasang, semua daftar sesi dimuat ulang. */
  /** Klien engine aktif (null bila engine belum siap); dipakai UI untuk pohon file. */
  engineApi(): EngineApi | null {
    return this.api;
  }

  setApi(api: EngineApi | null) {
    this.api = api;
    this.update({ engineReady: api !== null });
    if (api) void this.refreshAll();
  }

  private directoryOf(key: ProjectKey): string | null {
    if (key === CHAT_KEY) return this.snap.chatDir;
    return this.snap.projects.find((p) => p.id === key)?.path ?? null;
  }

  async refreshAll(): Promise<void> {
    await Promise.all([CHAT_KEY, ...this.snap.projects.map((p) => p.id)].map((k) => this.refresh(k)));
  }

  /** Muat sesi satu project. Respons basi (permintaan lebih baru sudah dikirim) dibuang. */
  async refresh(key: ProjectKey): Promise<void> {
    const dir = this.directoryOf(key);
    const api = this.api;
    if (!dir || !api) return;
    const seq = (this.requestSeq.get(key) ?? 0) + 1;
    this.requestSeq.set(key, seq);
    this.updateSessions(key, { status: "loading", error: null });
    try {
      const items = (await api.listSessions(dir)).slice().sort(byRecent);
      if (this.requestSeq.get(key) !== seq) return;
      this.updateSessions(key, { status: "ready", items, error: null });
      this.dropStaleSelection();
    } catch (e) {
      if (this.requestSeq.get(key) !== seq) return;
      this.updateSessions(key, { status: "error", error: e instanceof Error ? e.message : String(e) });
    }
  }

  /** Pilihan yang menunjuk sesi yang tidak ada lagi (dihapus di tempat lain) dikosongkan. */
  private dropStaleSelection() {
    const { key, sessionId } = this.snap.selection;
    if (!key || !sessionId) return;
    const s = this.snap.sessions[key];
    if (s?.status === "ready" && !s.items.some((i) => i.id === sessionId)) {
      this.update({ selection: { key, sessionId: null } });
      void this.persist();
    }
  }

  // ---------- Project ----------

  /** Tambah folder sebagai project. Folder yang sudah ada tidak digandakan; project yang ada dikembalikan. */
  async addProject(path: string): Promise<Project> {
    const norm = normalizePath(path);
    if (!norm) throw new Error("path project kosong");
    if (this.snap.chatDir && pathKey(this.snap.chatDir) === pathKey(norm)) {
      throw new Error("folder ini dipakai untuk Chat dan tidak bisa menjadi project");
    }
    const existing = this.snap.projects.find((p) => pathKey(p.path) === pathKey(norm));
    if (existing) {
      this.select(existing.id, this.snap.selection.key === existing.id ? this.snap.selection.sessionId : null);
      return existing;
    }
    const project: Project = { id: this.newId(), path: norm, name: baseName(norm), color: null, addedAt: this.now() };
    this.update({ projects: [...this.snap.projects, project], selection: { key: project.id, sessionId: null } });
    await this.persist();
    void this.refresh(project.id);
    return project;
  }

  async removeProject(id: string): Promise<void> {
    if (!this.snap.projects.some((p) => p.id === id)) return;
    const { [id]: _removed, ...rest } = this.snap.sessions;
    const sel = this.snap.selection;
    this.update({
      projects: this.snap.projects.filter((p) => p.id !== id),
      sessions: rest,
      selection: sel.key === id ? { key: null, sessionId: null } : sel,
    });
    await this.persist();
  }

  async renameProject(id: string, name: string): Promise<void> {
    const n = name.trim();
    if (!n) throw new Error("nama project tidak boleh kosong");
    if (!this.snap.projects.some((p) => p.id === id)) throw new Error("project tidak ditemukan");
    this.update({ projects: this.snap.projects.map((p) => (p.id === id ? { ...p, name: n } : p)) });
    await this.persist();
  }

  async setProjectColor(id: string, color: string | null): Promise<void> {
    if (color !== null && !isProjectColor(color)) throw new Error(`warna tidak dikenal: ${color}`);
    this.update({ projects: this.snap.projects.map((p) => (p.id === id ? { ...p, color } : p)) });
    await this.persist();
  }

  // ---------- Sesi ----------

  select(key: ProjectKey | null, sessionId: string | null) {
    this.update({ selection: { key, sessionId } });
    void this.persist();
  }

  private requireApi(): EngineApi {
    if (!this.api) throw new Error("engine belum siap");
    return this.api;
  }

  async createSession(key: ProjectKey, title?: string): Promise<SessionInfo> {
    const dir = this.directoryOf(key);
    if (!dir) throw new Error(key === CHAT_KEY ? "direktori Chat belum tersedia" : "project tidak ditemukan");
    const session = await this.requireApi().createSession(dir, title);
    const cur = this.snap.sessions[key] ?? emptySessions();
    this.updateSessions(key, { status: "ready", items: [session, ...cur.items.filter((s) => s.id !== session.id)] });
    this.select(key, session.id);
    return session;
  }

  async renameSession(key: ProjectKey, sessionId: string, title: string): Promise<void> {
    const t = title.trim();
    if (!t) throw new Error("judul sesi tidak boleh kosong");
    const dir = this.directoryOf(key);
    if (!dir) throw new Error("project tidak ditemukan");
    const updated = await this.requireApi().renameSession(dir, sessionId, t);
    const cur = this.snap.sessions[key] ?? emptySessions();
    this.updateSessions(key, { items: cur.items.map((s) => (s.id === sessionId ? updated : s)).sort(byRecent) });
  }

  async deleteSession(key: ProjectKey, sessionId: string): Promise<void> {
    const dir = this.directoryOf(key);
    if (!dir) throw new Error("project tidak ditemukan");
    await this.requireApi().deleteSession(dir, sessionId);
    const cur = this.snap.sessions[key] ?? emptySessions();
    this.updateSessions(key, { items: cur.items.filter((s) => s.id !== sessionId) });
    const sel = this.snap.selection;
    if (sel.key === key && sel.sessionId === sessionId) this.select(key, null);
  }

  /** Sesi terpilih beserta project-nya (untuk panel Chat). */
  selected(): { key: ProjectKey; session: SessionInfo; project: Project | null } | null {
    const { key, sessionId } = this.snap.selection;
    if (!key || !sessionId) return null;
    const session = this.snap.sessions[key]?.items.find((s) => s.id === sessionId);
    if (!session) return null;
    return { key, session, project: this.snap.projects.find((p) => p.id === key) ?? null };
  }
}

/** Judul tampil: judul bawaan engine ("New session - …") dianggap belum bernama → null. */
export function sessionTitle(session: Pick<SessionInfo, "title">): string | null {
  return isDefaultSessionTitle(session.title) ? null : session.title;
}

// Project & sesi (Fase 4).
// - PROJECT = folder. Daftar project, nama, dan warna dimiliki Zandor (registry JSON): opencode hanya mengenal
//   repositori git dan menggabungkan semua folder non-git ke satu project "global" (ADR-032).
// - SESI dimiliki engine dan terikat ke direktori (`GET /session?directory=`). Sesi tanpa project ("Chat")
//   memakai direktori khusus milik Zandor.
// Hapus project = lepas dari daftar; sesi di engine TIDAK dihapus.
import { type EngineApi, type EngineConnectionInfo, isDefaultSessionTitle, type SessionInfo } from "./api";
import {
  createRoutedApi,
  normalizeRemoteUrl,
  type RemoteEngine,
  type RemoteStatus,
  type RouteTarget,
  remoteUrlProblem,
} from "./remote";

export const CHAT_KEY = "chat" as const;
export type ProjectKey = string; // id project atau CHAT_KEY

export type Project = {
  id: string;
  path: string;
  name: string;
  color: string | null;
  addedAt: number;
  /** Id engine remote; tidak ada = engine lokal. Path adalah path di mesin engine itu. */
  engine?: string;
};
export type SessionsState = {
  status: "idle" | "loading" | "ready" | "error";
  items: SessionInfo[];
  error: string | null;
};
export type Selection = { key: ProjectKey | null; sessionId: string | null };
export type SessionTab = { key: ProjectKey; sessionId: string };
const MAX_TABS = 12;

export type ProjectsState = {
  loaded: boolean;
  projects: Project[];
  chatDir: string | null;
  engineReady: boolean;
  sessions: Record<ProjectKey, SessionsState>;
  selection: Selection;
  /** Sesi yang dibuka sebagai tab di atas panel Chat (urutan dibuka). */
  tabs: SessionTab[];
  /** Engine remote terdaftar (URL + Basic Auth). */
  remotes: RemoteEngine[];
  /** Status koneksi tiap engine remote (hasil cek health terakhir). */
  remoteStatus: Record<string, RemoteStatus>;
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

type Persisted = {
  version: number;
  projects: Project[];
  selection: Selection;
  tabs: SessionTab[];
  remotes?: RemoteEngine[];
};

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
      ...(typeof p.engine === "string" && p.engine ? { engine: p.engine } : {}),
    });
  }
  const remotes: RemoteEngine[] = [];
  const rawRemotes = (o as { remotes?: unknown }).remotes;
  if (Array.isArray(rawRemotes)) {
    for (const r of rawRemotes as Array<Partial<RemoteEngine>>) {
      if (typeof r?.id !== "string" || typeof r.url !== "string" || remotes.some((x) => x.id === r.id)) continue;
      remotes.push({
        id: r.id,
        name: typeof r.name === "string" && r.name.trim() ? r.name : r.url,
        url: r.url,
        username: typeof r.username === "string" && r.username ? r.username : "opencode",
        password: typeof r.password === "string" ? r.password : "",
      });
    }
  }
  const sel = o.selection as Partial<Selection> | undefined;
  const selection: Selection = {
    key: typeof sel?.key === "string" ? sel.key : null,
    sessionId: typeof sel?.sessionId === "string" ? sel.sessionId : null,
  };
  const tabs: SessionTab[] = [];
  if (Array.isArray((o as { tabs?: unknown }).tabs)) {
    for (const t of (o as { tabs: Array<Partial<SessionTab>> }).tabs) {
      if (
        typeof t?.key === "string" &&
        typeof t.sessionId === "string" &&
        !tabs.some((x) => x.sessionId === t.sessionId)
      ) {
        tabs.push({ key: t.key, sessionId: t.sessionId });
      }
    }
  }
  return {
    value: { version: REGISTRY_VERSION, projects, selection, tabs: tabs.slice(-MAX_TABS), remotes },
    error: null,
  };
}

/**
 * Folder khusus tiap sesi tanpa project ("Chat"): `chats/<nama>/`. Tanpa adapter ini (tes/legacy) semua sesi Chat berbagi
 * folder Chat tunggal.
 */
export interface ChatFolderHost {
  /** Buat folder `nama` di dalam folder Chat; mengembalikan path lengkapnya. */
  create(name: string): Promise<string>;
  /** Path lengkap semua folder sesi yang ada. */
  list(): Promise<string[]>;
  /** Hapus folder bila (dan hanya bila) kosong; galat diabaikan. */
  removeIfEmpty(path: string): Promise<void>;
}

/** Nama folder sesi Chat: `chat-YYYYMMDD-HHMMSS-xxxx` (urut waktu, unik, aman untuk semua sistem berkas). */
export function chatFolderName(at: Date, suffix: string): string {
  const p = (n: number) => String(n).padStart(2, "0");
  const stamp = `${at.getFullYear()}${p(at.getMonth() + 1)}${p(at.getDate())}-${p(at.getHours())}${p(at.getMinutes())}${p(at.getSeconds())}`;
  return `chat-${stamp}-${
    suffix
      .replace(/[^a-z0-9]/gi, "")
      .toLowerCase()
      .slice(0, 6) || "x"
  }`;
}

export type ProjectStoreOptions = {
  storage: RegistryStorage;
  chatFolders?: ChatFolderHost;
  newId?: () => string;
  now?: () => number;
  onWarning?: (message: string) => void;
  /** Pembuat klien engine untuk engine remote (App memakai `createEngineApi`). Tanpa ini engine remote tidak dipakai. */
  createApi?: (info: EngineConnectionInfo) => EngineApi;
  /** Dipanggil setiap klien gabungan berganti (engine lokal/remote berubah) agar store lain ikut memakainya. */
  onApi?: (api: EngineApi | null) => void;
};

export class ProjectStore {
  private snap: ProjectsState = {
    loaded: false,
    projects: [],
    chatDir: null,
    engineReady: false,
    sessions: {},
    selection: { key: null, sessionId: null },
    tabs: [],
    remotes: [],
    remoteStatus: {},
  };
  private listeners = new Set<() => void>();
  private api: EngineApi | null = null;
  private local: EngineApi | null = null;
  private remoteApis = new Map<string, EngineApi>();
  private checkSeq = new Map<string, number>();
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
    const data: Persisted = {
      version: REGISTRY_VERSION,
      projects: this.snap.projects,
      selection: this.snap.selection,
      tabs: this.snap.tabs,
      ...(this.snap.remotes.length ? { remotes: this.snap.remotes } : {}),
    };
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
      tabs: value?.tabs ?? [],
      remotes: value?.remotes ?? [],
    });
    for (const r of this.snap.remotes) this.connectRemote(r);
    if (this.snap.remotes.length) this.rebuildApi();
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

  /** Pasang engine lokal (null = belum siap). Klien gabungan dibangun ulang; semua daftar sesi dimuat ulang. */
  setApi(api: EngineApi | null) {
    this.local = api;
    this.rebuildApi();
  }

  /**
   * Klien gabungan: tanpa engine remote = klien lokal apa adanya (perilaku lama); dengan engine remote = router per
   * direktori project. `engineReady` tetap menggambarkan engine lokal.
   */
  private rebuildApi() {
    this.api =
      this.remoteApis.size > 0
        ? createRoutedApi({
            local: this.local,
            remote: (id) => this.remoteApis.get(id) ?? null,
            route: (dir) => this.routeOf(dir),
            remoteName: (id) => this.snap.remotes.find((r) => r.id === id)?.name ?? id,
          })
        : this.local;
    this.update({ engineReady: this.local !== null });
    this.options.onApi?.(this.api);
    if (this.api) void this.refreshAll();
  }

  /** Engine untuk direktori: project yang memuatnya (atau sama persis) menentukan; lainnya (Chat) lokal. */
  routeOf(directory: string): RouteTarget {
    const k = pathKey(directory);
    let best: Project | null = null;
    for (const p of this.snap.projects) {
      const pk = pathKey(p.path);
      const base = pk.replace(/[\\/]+$/, "");
      const inside = k === pk || k.startsWith(`${base}/`) || k.startsWith(`${base}\\`);
      if (inside && (!best || pk.length > pathKey(best.path).length)) best = p;
    }
    return best?.engine ? { kind: "remote", id: best.engine } : { kind: "local" };
  }

  // ---------- engine remote ----------

  private connectRemote(r: RemoteEngine) {
    const make = this.options.createApi;
    if (!make) return;
    this.remoteApis.set(r.id, make({ url: r.url, username: r.username, password: r.password }));
    void this.checkRemote(r.id);
  }

  /** Cek health engine remote; status jujur (galat berisi sebab nyata). */
  async checkRemote(id: string): Promise<RemoteStatus | null> {
    const api = this.remoteApis.get(id);
    if (!api) return null;
    const seq = (this.checkSeq.get(id) ?? 0) + 1;
    this.checkSeq.set(id, seq);
    const set = (s: RemoteStatus) => {
      if (this.checkSeq.get(id) !== seq || !this.snap.remotes.some((r) => r.id === id)) return;
      this.update({ remoteStatus: { ...this.snap.remoteStatus, [id]: s } });
    };
    set({ status: "checking", version: null, error: null });
    let result: RemoteStatus;
    try {
      const h = await api.health();
      result = { status: "ready", version: h.version, error: null };
    } catch (e) {
      result = { status: "error", version: null, error: e instanceof Error ? e.message : String(e) };
    }
    set(result);
    return result;
  }

  private validateRemote(input: Omit<RemoteEngine, "id">): Omit<RemoteEngine, "id"> {
    const url = normalizeRemoteUrl(input.url);
    const problem = remoteUrlProblem(url);
    if (problem === "insecure")
      throw new Error("URL engine di mesin lain harus HTTPS (HTTP hanya untuk localhost/tunnel)");
    if (problem) throw new Error("URL engine tidak valid");
    return {
      name: input.name.trim() || url,
      url,
      username: input.username.trim() || "opencode",
      password: input.password,
    };
  }

  async addRemote(input: Omit<RemoteEngine, "id">): Promise<RemoteEngine> {
    const remote: RemoteEngine = { id: this.newId(), ...this.validateRemote(input) };
    this.update({ remotes: [...this.snap.remotes, remote] });
    await this.persist();
    this.connectRemote(remote);
    this.rebuildApi();
    return remote;
  }

  async updateRemote(id: string, input: Omit<RemoteEngine, "id">): Promise<void> {
    if (!this.snap.remotes.some((r) => r.id === id)) throw new Error("engine remote tidak ditemukan");
    const next: RemoteEngine = { id, ...this.validateRemote(input) };
    this.update({ remotes: this.snap.remotes.map((r) => (r.id === id ? next : r)) });
    await this.persist();
    this.connectRemote(next);
    this.rebuildApi();
  }

  /** Hapus engine remote. Ditolak selama masih dipakai project (lepas project-nya dulu). */
  async removeRemote(id: string): Promise<void> {
    const users = this.snap.projects.filter((p) => p.engine === id);
    if (users.length) throw new Error(`engine masih dipakai project: ${users.map((p) => p.name).join(", ")}`);
    const { [id]: _gone, ...status } = this.snap.remoteStatus;
    this.update({ remotes: this.snap.remotes.filter((r) => r.id !== id), remoteStatus: status });
    this.remoteApis.delete(id);
    await this.persist();
    this.rebuildApi();
  }

  private directoryOf(key: ProjectKey): string | null {
    if (key === CHAT_KEY) return this.snap.chatDir;
    return this.snap.projects.find((p) => p.id === key)?.path ?? null;
  }

  async refreshAll(): Promise<void> {
    await Promise.all([CHAT_KEY, ...this.snap.projects.map((p) => p.id)].map((k) => this.refresh(k)));
  }

  /**
   * Sesi satu kelompok. Project = satu folder. Chat = folder Chat itu sendiri (sesi lama yang berbagi folder) ditambah SEMUA
   * folder sesi di bawahnya; folder yang gagal dibaca diabaikan selama minimal satu berhasil.
   */
  private async listAll(api: EngineApi, key: ProjectKey, dir: string): Promise<SessionInfo[]> {
    const host = this.options.chatFolders;
    if (key !== CHAT_KEY || !host) return api.listSessions(dir);
    const dirs = [...new Set([dir, ...(await host.list().catch(() => []))])];
    const results: SessionInfo[][] = [];
    let firstError: unknown = null;
    for (let i = 0; i < dirs.length; i += 6) {
      const batch = await Promise.allSettled(dirs.slice(i, i + 6).map((d) => api.listSessions(d)));
      for (const r of batch) {
        if (r.status === "fulfilled") results.push(r.value);
        else firstError ??= r.reason;
      }
    }
    if (results.length === 0 && firstError) throw firstError;
    const seen = new Set<string>();
    return results.flat().filter((s) => {
      if (seen.has(s.id)) return false;
      seen.add(s.id);
      return true;
    });
  }

  /** Direktori kerja satu sesi: project → folder project; Chat → folder milik sesi itu (atau folder Chat untuk sesi lama). */
  private dirForSession(key: ProjectKey, sessionId: string): string | null {
    if (key === CHAT_KEY) {
      const s = this.snap.sessions[key]?.items.find((x) => x.id === sessionId);
      return s?.directory || this.snap.chatDir;
    }
    return this.directoryOf(key);
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
      const items = (await this.listAll(api, key, dir)).slice().sort(byRecent);
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
    // Tab untuk sesi yang sudah tidak ada (daftar sudah berstatus siap) ikut dibuang.
    const alive = this.snap.tabs.filter((t) => {
      const g = this.snap.sessions[t.key];
      return !(g?.status === "ready" && !g.items.some((i) => i.id === t.sessionId));
    });
    if (alive.length !== this.snap.tabs.length) {
      this.update({ tabs: alive });
      void this.persist();
    }
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
  async addProject(path: string, engine: string | null = null): Promise<Project> {
    const norm = normalizePath(path);
    if (!norm) throw new Error("path project kosong");
    if (engine && !this.snap.remotes.some((r) => r.id === engine)) throw new Error("engine remote tidak ditemukan");
    if (!engine && this.snap.chatDir && pathKey(this.snap.chatDir) === pathKey(norm)) {
      throw new Error("folder ini dipakai untuk Chat dan tidak bisa menjadi project");
    }
    const existing = this.snap.projects.find((p) => pathKey(p.path) === pathKey(norm));
    if (existing) {
      this.select(existing.id, this.snap.selection.key === existing.id ? this.snap.selection.sessionId : null);
      return existing;
    }
    const project: Project = {
      id: this.newId(),
      path: norm,
      name: baseName(norm),
      color: null,
      addedAt: this.now(),
      ...(engine ? { engine } : {}),
    };
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
      tabs: this.snap.tabs.filter((t) => t.key !== id),
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
    let tabs = this.snap.tabs;
    if (key && sessionId && !tabs.some((t) => t.sessionId === sessionId)) {
      tabs = [...tabs, { key, sessionId }];
      if (tabs.length > MAX_TABS) tabs = tabs.slice(tabs.length - MAX_TABS);
    }
    this.update({ selection: { key, sessionId }, tabs });
    void this.persist();
  }

  /** Tutup tab. Bila tab aktif, pindah ke tetangganya (atau tanpa sesi bila tab terakhir). */
  closeTab(sessionId: string) {
    const i = this.snap.tabs.findIndex((t) => t.sessionId === sessionId);
    if (i === -1) return;
    const tabs = this.snap.tabs.filter((t) => t.sessionId !== sessionId);
    const sel = this.snap.selection;
    if (sel.sessionId !== sessionId) {
      this.update({ tabs });
      void this.persist();
      return;
    }
    const next = tabs[Math.min(i, tabs.length - 1)];
    this.update({
      tabs,
      selection: next ? { key: next.key, sessionId: next.sessionId } : { key: sel.key, sessionId: null },
    });
    void this.persist();
  }

  /**
   * Tutup banyak tab sekaligus: `others` (kecuali tab jangkar), `right`/`left` (di sebelah kanan/kiri jangkar, jangkar tetap),
   * atau `all`. Bila tab terpilih ikut tertutup, pilihan pindah ke tab jangkar (atau kosong untuk `all`).
   */
  closeTabsBy(mode: "others" | "right" | "left" | "all", anchorId?: string) {
    const tabs = this.snap.tabs;
    const i = anchorId ? tabs.findIndex((t) => t.sessionId === anchorId) : -1;
    if (mode !== "all" && i < 0) return;
    const keep = tabs.filter((_, idx) =>
      mode === "all" ? false : mode === "others" ? idx === i : mode === "right" ? idx <= i : idx >= i,
    );
    if (keep.length === tabs.length) return;
    const sel = this.snap.selection;
    let selection = sel;
    if (sel.sessionId && !keep.some((t) => t.sessionId === sel.sessionId)) {
      const anchor = i >= 0 ? tabs[i] : undefined;
      selection = anchor ? { key: anchor.key, sessionId: anchor.sessionId } : { key: sel.key, sessionId: null };
    }
    this.update({ tabs: keep, selection });
    void this.persist();
  }

  /** Sesi hasil operasi engine di luar createSession (mis. fork): masukkan ke daftar dan pilih. */
  adoptSession(key: ProjectKey, session: SessionInfo) {
    const cur = this.snap.sessions[key] ?? emptySessions();
    this.updateSessions(key, { status: "ready", items: [session, ...cur.items.filter((s) => s.id !== session.id)] });
    this.select(key, session.id);
  }

  private requireApi(): EngineApi {
    if (!this.api) throw new Error("engine belum siap");
    return this.api;
  }

  /** `select: false` membuat sesi tanpa memindahkan fokus pengguna (dipakai automasi). */
  async createSession(key: ProjectKey, title?: string, options: { select?: boolean } = {}): Promise<SessionInfo> {
    let dir = this.directoryOf(key);
    if (!dir) throw new Error(key === CHAT_KEY ? "direktori Chat belum tersedia" : "project tidak ditemukan");
    if (key === CHAT_KEY && this.options.chatFolders) {
      // sesi tanpa project: folder kerja SENDIRI per sesi
      dir = await this.options.chatFolders.create(chatFolderName(new Date(this.now()), this.newId()));
    }
    const session = await this.requireApi().createSession(dir, title);
    const cur = this.snap.sessions[key] ?? emptySessions();
    this.updateSessions(key, { status: "ready", items: [session, ...cur.items.filter((s) => s.id !== session.id)] });
    if (options.select !== false) this.select(key, session.id);
    return session;
  }

  async renameSession(key: ProjectKey, sessionId: string, title: string): Promise<void> {
    const t = title.trim();
    if (!t) throw new Error("judul sesi tidak boleh kosong");
    const dir = this.dirForSession(key, sessionId);
    if (!dir) throw new Error("project tidak ditemukan");
    const updated = await this.requireApi().renameSession(dir, sessionId, t);
    const cur = this.snap.sessions[key] ?? emptySessions();
    this.updateSessions(key, { items: cur.items.map((s) => (s.id === sessionId ? updated : s)).sort(byRecent) });
  }

  async deleteSession(key: ProjectKey, sessionId: string): Promise<void> {
    const dir = this.dirForSession(key, sessionId);
    if (!dir) throw new Error("project tidak ditemukan");
    await this.requireApi().deleteSession(dir, sessionId);
    // folder milik sesi Chat dibersihkan hanya bila kosong (berkas hasil kerja tidak pernah dihapus)
    if (key === CHAT_KEY && this.snap.chatDir && dir !== this.snap.chatDir) {
      void this.options.chatFolders?.removeIfEmpty(dir).catch(() => {});
    }
    const cur = this.snap.sessions[key] ?? emptySessions();
    this.updateSessions(key, { items: cur.items.filter((s) => s.id !== sessionId) });
    this.closeTab(sessionId);
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

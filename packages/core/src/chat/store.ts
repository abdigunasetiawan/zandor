// Store chat: memuat riwayat, berlangganan SSE per direktori, mengirim prompt, abort, membalas izin, dan memilih
// model. Semua logika di sini murni terhadap `EngineApi` (diuji dengan engine palsu).
import type { EngineApi, ModelOption, ModelRef } from "../engine/api";
import { applyEvent, type ChatState, clearError, initialChatState, loadMessages } from "./reducer";
import type { EngineEvent, PermissionReply } from "./types";

export type SessionChat = ChatState & {
  directory: string;
  /** Prompt sedang dikirim ke engine (sebelum event pertama tiba). */
  sending: boolean;
  loading: boolean;
  /** Koneksi SSE direktori terputus dan sedang menyambung ulang. */
  reconnecting: boolean;
};

export type ChatStoreState = {
  models: ModelOption[];
  modelsStatus: "idle" | "loading" | "ready" | "error";
  modelsError: string | null;
  /** Model terpilih untuk prompt berikutnya (null bila belum ada model). */
  model: ModelRef | null;
  sessions: Record<string, SessionChat>;
};

export type ChatPrefs = { get(key: string): string | null; set(key: string, value: string): void };
export type ChatStoreOptions = { prefs: ChatPrefs; onWarning?: (message: string) => void };

const MODEL_PREF = "zandor.model";
const MAX_RELOADS = 3;
const MESSAGE_EVENTS = new Set<EngineEvent["type"]>([
  "message.updated",
  "message.removed",
  "message.part.updated",
  "message.part.removed",
  "message.part.delta",
]);

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));
const sameModel = (a: ModelRef | null, b: ModelRef | null) =>
  a?.providerID === b?.providerID && a?.modelID === b?.modelID;

export class ChatStore {
  private snap: ChatStoreState = { models: [], modelsStatus: "idle", modelsError: null, model: null, sessions: {} };
  private listeners = new Set<() => void>();
  private api: EngineApi | null = null;
  private subs = new Map<string, { unsub: () => void; sessions: Set<string> }>();
  private dirty = new Set<string>();
  private reloading = new Set<string>();
  private preferred: ModelRef | null;

  constructor(private readonly options: ChatStoreOptions) {
    this.preferred = parseModel(options.prefs.get(MODEL_PREF));
  }

  state(): ChatStoreState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private update(patch: Partial<ChatStoreState>) {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l();
  }

  private patchSession(id: string, fn: (s: SessionChat) => SessionChat) {
    const cur = this.snap.sessions[id];
    if (!cur) return;
    const next = fn(cur);
    if (next === cur) return;
    this.update({ sessions: { ...this.snap.sessions, [id]: next } });
  }

  // ---------- koneksi engine ----------

  /** Dipanggil saat engine siap/tidak siap. Menyambung ulang langganan dan memuat ulang sesi yang terbuka. */
  setApi(api: EngineApi | null) {
    if (api === this.api) return;
    for (const s of this.subs.values()) s.unsub();
    this.subs.clear();
    this.api = api;
    if (!api) {
      this.update({ modelsStatus: "idle" });
      return;
    }
    void this.refreshModels();
    const dirs = new Set(Object.values(this.snap.sessions).map((s) => s.directory));
    for (const dir of dirs) this.attach(dir);
  }

  async refreshModels(): Promise<void> {
    const api = this.api;
    if (!api) return;
    this.update({ modelsStatus: "loading", modelsError: null });
    try {
      const models = await api.listModels();
      if (api !== this.api) return;
      this.update({ models, modelsStatus: "ready", model: this.pickModel(models) });
    } catch (e) {
      if (api !== this.api) return;
      this.update({ modelsStatus: "error", modelsError: errMsg(e) });
    }
  }

  private pickModel(models: ModelOption[]): ModelRef | null {
    const has = (r: ModelRef | null) =>
      !!r && models.some((m) => m.providerID === r.providerID && m.modelID === r.modelID);
    if (has(this.snap.model)) return this.snap.model;
    if (has(this.preferred)) return this.preferred;
    const first = models[0];
    return first ? { providerID: first.providerID, modelID: first.modelID } : null;
  }

  selectModel(ref: ModelRef) {
    if (sameModel(ref, this.snap.model)) return;
    this.preferred = ref;
    try {
      this.options.prefs.set(MODEL_PREF, JSON.stringify(ref));
    } catch (e) {
      this.options.onWarning?.(`gagal menyimpan pilihan model: ${errMsg(e)}`);
    }
    this.update({ model: ref });
  }

  // ---------- sesi ----------

  /** Buka sesi: muat riwayat dan mulai berlangganan event direktorinya. Aman dipanggil berulang. */
  open(directory: string, sessionId: string) {
    if (!this.snap.sessions[sessionId]) {
      const fresh: SessionChat = {
        ...initialChatState(sessionId),
        directory,
        sending: false,
        loading: false,
        reconnecting: false,
      };
      this.update({ sessions: { ...this.snap.sessions, [sessionId]: fresh } });
    }
    this.attach(directory);
    void this.reload(sessionId);
  }

  /** Lepas sesi dari memori (dan hentikan langganan bila direktori tak dipakai lagi). */
  close(sessionId: string) {
    const s = this.snap.sessions[sessionId];
    if (!s) return;
    const { [sessionId]: _drop, ...rest } = this.snap.sessions;
    this.update({ sessions: rest });
    const sub = this.subs.get(s.directory);
    if (sub) {
      sub.sessions.delete(sessionId);
      if (sub.sessions.size === 0) {
        sub.unsub();
        this.subs.delete(s.directory);
      }
    }
  }

  private attach(directory: string) {
    const api = this.api;
    if (!api) return;
    let sub = this.subs.get(directory);
    if (!sub) {
      const entry = { unsub: () => {}, sessions: new Set<string>() };
      sub = entry;
      this.subs.set(directory, entry);
      entry.unsub = api.subscribe(directory, {
        onEvent: (ev) => this.onEvent(directory, ev),
        onState: (st) => this.onConnection(directory, st),
      });
    }
    for (const [id, s] of Object.entries(this.snap.sessions)) if (s.directory === directory) sub.sessions.add(id);
  }

  private sessionsOf(directory: string): string[] {
    return Object.values(this.snap.sessions)
      .filter((s) => s.directory === directory)
      .map((s) => s.sessionID);
  }

  private onConnection(directory: string, st: "connected" | "reconnecting") {
    for (const id of this.sessionsOf(directory)) {
      this.patchSession(id, (s) =>
        s.reconnecting === (st === "reconnecting") ? s : { ...s, reconnecting: st === "reconnecting" },
      );
      // Setelah (re)koneksi, event yang terlewat harus diambil lewat riwayat.
      if (st === "connected") void this.reload(id);
    }
  }

  private onEvent(directory: string, ev: EngineEvent) {
    const isMessageEvent = MESSAGE_EVENTS.has(ev.type);
    for (const id of this.sessionsOf(directory)) {
      if (isMessageEvent && this.reloading.has(id)) {
        // Snapshot riwayat sedang diambil; event ini mungkin sudah/atau belum tercakup → muat ulang setelahnya.
        this.dirty.add(id);
        continue;
      }
      this.patchSession(id, (s) => ({ ...s, ...applyEvent(s, ev) }));
    }
  }

  private async reload(sessionId: string) {
    const api = this.api;
    const start = this.snap.sessions[sessionId];
    if (!api || !start || this.reloading.has(sessionId)) return;
    this.reloading.add(sessionId);
    this.patchSession(sessionId, (s) => ({ ...s, loading: true }));
    try {
      for (let attempt = 0; attempt < MAX_RELOADS; attempt++) {
        this.dirty.delete(sessionId);
        const [messages, permissions] = await Promise.all([
          api.listMessages(start.directory, sessionId),
          api.listPermissions(start.directory).catch(() => []),
        ]);
        if (api !== this.api || !this.snap.sessions[sessionId]) return;
        const mine = permissions.filter((p) => p.sessionID === sessionId);
        this.patchSession(sessionId, (s) => ({ ...s, ...loadMessages(s, messages, mine), error: s.error }));
        if (!this.dirty.has(sessionId)) break;
      }
    } catch (e) {
      this.patchSession(sessionId, (s) => ({ ...s, error: `gagal memuat riwayat: ${errMsg(e)}` }));
    } finally {
      this.reloading.delete(sessionId);
      this.dirty.delete(sessionId);
      this.patchSession(sessionId, (s) => (s.loading ? { ...s, loading: false } : s));
    }
  }

  // ---------- aksi ----------

  async send(sessionId: string, text: string): Promise<void> {
    const api = this.api;
    const s = this.snap.sessions[sessionId];
    const trimmed = text.trim();
    if (!api || !s) throw new Error("engine belum siap");
    if (!trimmed) throw new Error("pesan kosong");
    if (!this.snap.model) throw new Error("belum ada model; tambahkan provider terlebih dahulu");
    if (s.sending || s.status.type !== "idle") throw new Error("sesi sedang berjalan");
    this.patchSession(sessionId, (x) => ({ ...clearError(x), ...{ sending: true } }) as SessionChat);
    try {
      await api.sendPrompt(s.directory, sessionId, { text: trimmed, model: this.snap.model });
    } catch (e) {
      this.patchSession(sessionId, (x) => ({ ...x, error: `gagal mengirim: ${errMsg(e)}` }));
      throw e;
    } finally {
      this.patchSession(sessionId, (x) => ({ ...x, sending: false }));
    }
  }

  async abort(sessionId: string): Promise<void> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) return;
    await this.api.abort(s.directory, sessionId);
  }

  async reply(sessionId: string, requestId: string, reply: PermissionReply): Promise<void> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) throw new Error("engine belum siap");
    await this.api.replyPermission(s.directory, requestId, reply);
    // Event `permission.replied` menghapusnya; hapus lokal juga agar tombol langsung hilang.
    this.patchSession(sessionId, (x) => ({ ...x, permissions: x.permissions.filter((p) => p.id !== requestId) }));
  }
}

function parseModel(raw: string | null): ModelRef | null {
  if (!raw) return null;
  try {
    const v = JSON.parse(raw) as Partial<ModelRef>;
    return typeof v.providerID === "string" && typeof v.modelID === "string"
      ? { providerID: v.providerID, modelID: v.modelID }
      : null;
  } catch {
    return null;
  }
}

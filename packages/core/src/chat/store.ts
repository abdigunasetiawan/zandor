// Store chat: memuat riwayat, berlangganan SSE per direktori, mengirim prompt, abort, membalas izin, dan memilih
// model. Semua logika di sini murni terhadap `EngineApi` (diuji dengan engine palsu).
import type { EngineApi, FileAttachment, ModelOption, ModelRef, SessionInfo } from "../engine/api";
import { applyEvent, type ChatState, clearError, initialChatState, loadMessages } from "./reducer";
import type { EngineEvent, PermissionReply, PermissionRequest, QuestionRequest, Todo } from "./types";

export type SessionChat = ChatState & {
  directory: string;
  /** Prompt sedang dikirim ke engine (sebelum event pertama tiba). */
  sending: boolean;
  loading: boolean;
  /** Koneksi SSE direktori terputus dan sedang menyambung ulang. */
  reconnecting: boolean;
};

/** Pesan yang diantre selagi agent bekerja; dikirim berurutan begitu sesi idle. Hanya di memori (tidak disimpan ke disk). */
export type QueuedMessage = { id: string; text: string; files: FileAttachment[] };
export const MAX_QUEUE = 50;

export type ChatStoreState = {
  models: ModelOption[];
  modelsStatus: "idle" | "loading" | "ready" | "error";
  modelsError: string | null;
  /** Model terpilih untuk prompt berikutnya (null bila belum ada model). */
  model: ModelRef | null;
  /** Varian reasoning terpilih (null = bawaan model). */
  variant: string | null;
  /** Cara menjawab permintaan izin: tanya pengguna, setujui otomatis edit file, atau setujui semuanya. */
  permissionMode: PermissionMode;
  /** Model yang disematkan pengguna, sebagai kunci `providerID/modelID` (urutan disematkan). */
  pinned: string[];
  /** Model yang terakhir dipakai (dipilih atau dikirimi prompt), terbaru dulu, maksimal MAX_RECENT, kunci `providerID/modelID`. */
  recent: string[];
  /** Pesan yang disematkan per sesi (id pesan, urutan disematkan). */
  pinnedMessages: Record<string, string[]>;
  /** Antrean pesan per sesi (urutan kirim). */
  queues: Record<string, QueuedMessage[]>;
  /** Sesi yang antreannya dijeda (Stop ditekan, atau pengiriman otomatis gagal): tidak dikirim sampai dilanjutkan. */
  queuePaused: Record<string, boolean>;
  sessions: Record<string, SessionChat>;
};

export type PermissionMode = "ask" | "auto-edit" | "auto-all";
export const PERMISSION_MODES: readonly PermissionMode[] = ["ask", "auto-edit", "auto-all"];

export type ChatPrefs = { get(key: string): string | null; set(key: string, value: string): void };
export type ChatStoreOptions = {
  prefs: ChatPrefs;
  onWarning?: (message: string) => void;
  /** Model bawaan pengguna (Pengaturan → Provider & model); dipakai bila belum ada pilihan di sesi ini. */
  defaultModel?: () => ModelRef | null;
  /** Dipanggil untuk setiap event engine yang diterima (mis. untuk Log aktivitas). */
  onEvent?: (event: EngineEvent) => void;
};

const MODEL_PREF = "zandor.model";
const MODE_PREF = "zandor.permissionMode";
const VARIANT_PREF = "zandor.variant";
const PINNED_PREF = "zandor.pinnedModels";
const RECENT_PREF = "zandor.recentModels";
const PINNED_MSG_PREF = "zandor.pinnedMessages";
export const MAX_RECENT = 3;
export const modelKey = (r: ModelRef) => `${r.providerID}/${r.modelID}`;
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
  private snap: ChatStoreState = {
    models: [],
    modelsStatus: "idle",
    modelsError: null,
    model: null,
    variant: null,
    permissionMode: "ask",
    pinned: [],
    recent: [],
    pinnedMessages: {},
    queues: {},
    queuePaused: {},
    sessions: {},
  };
  private qSeq = 0;
  /** Sesi yang sedang dihentikan oleh "kirim langsung": pesan terdepan boleh dikirim walau giliran berakhir dengan galat/abort. */
  private forceNext = new Set<string>();
  private listeners = new Set<() => void>();
  private api: EngineApi | null = null;
  private subs = new Map<string, { unsub: () => void; sessions: Set<string> }>();
  private dirty = new Set<string>();
  private reloading = new Set<string>();
  private preferred: ModelRef | null;

  constructor(private readonly options: ChatStoreOptions) {
    this.preferred = parseModel(options.prefs.get(MODEL_PREF));
    const mode = options.prefs.get(MODE_PREF);
    if (mode && (PERMISSION_MODES as readonly string[]).includes(mode)) {
      this.snap.permissionMode = mode as PermissionMode;
    }
    this.snap.variant = options.prefs.get(VARIANT_PREF) || null;
    this.snap.pinned = parsePinned(options.prefs.get(PINNED_PREF));
    this.snap.recent = parsePinned(options.prefs.get(RECENT_PREF)).slice(0, MAX_RECENT);
    this.snap.pinnedMessages = parsePinnedMessages(options.prefs.get(PINNED_MSG_PREF));
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
      const model = this.pickModel(models);
      this.update({
        models,
        modelsStatus: "ready",
        model,
        variant: this.validVariant(models, model, this.snap.variant),
      });
    } catch (e) {
      if (api !== this.api) return;
      this.update({ modelsStatus: "error", modelsError: errMsg(e) });
    }
  }

  private pickModel(models: ModelOption[]): ModelRef | null {
    const has = (r: ModelRef | null) =>
      !!r && models.some((m) => m.providerID === r.providerID && m.modelID === r.modelID);
    if (has(this.snap.model)) return this.snap.model;
    const def = this.options.defaultModel?.() ?? null;
    if (has(def)) return def;
    if (has(this.preferred)) return this.preferred;
    const first = models[0];
    return first ? { providerID: first.providerID, modelID: first.modelID } : null;
  }

  private validVariant(models: ModelOption[], model: ModelRef | null, variant: string | null): string | null {
    if (!variant || !model) return null;
    const m = models.find((o) => o.providerID === model.providerID && o.modelID === model.modelID);
    return m?.variants.includes(variant) ? variant : null;
  }

  setVariant(variant: string | null) {
    const v = this.validVariant(this.snap.models, this.snap.model, variant);
    if (v === this.snap.variant) return;
    this.safePref(VARIANT_PREF, v ?? "");
    this.update({ variant: v });
  }

  setPermissionMode(mode: PermissionMode) {
    if (mode === this.snap.permissionMode) return;
    this.safePref(MODE_PREF, mode);
    this.update({ permissionMode: mode });
    // Permintaan yang sudah menunggu ikut dijawab sesuai mode baru.
    for (const s of Object.values(this.snap.sessions)) {
      for (const p of s.permissions) this.maybeAutoReply(s.directory, p);
    }
  }

  /** Sematkan/lepas sematan model. Kunci tetap disimpan walau model sementara hilang dari daftar. */
  togglePin(ref: ModelRef) {
    const key = modelKey(ref);
    const pinned = this.snap.pinned.includes(key)
      ? this.snap.pinned.filter((k) => k !== key)
      : [...this.snap.pinned, key];
    this.safePref(PINNED_PREF, JSON.stringify(pinned));
    this.update({ pinned });
  }

  private safePref(key: string, value: string) {
    try {
      this.options.prefs.set(key, value);
    } catch (e) {
      this.options.onWarning?.(`gagal menyimpan preferensi: ${errMsg(e)}`);
    }
  }

  private maybeAutoReply(directory: string, req: PermissionRequest) {
    const mode = this.snap.permissionMode;
    const allowed = mode === "auto-all" || (mode === "auto-edit" && req.permission === "edit");
    if (!allowed || !this.api) return;
    void this.api.replyPermission(directory, req.id, "once").catch((e) => {
      this.options.onWarning?.(`gagal menjawab izin otomatis: ${errMsg(e)}`);
    });
  }

  /** Sematkan/lepas sematan satu pesan di sesi. Pesan yang hilang dari riwayat tetap tercatat sampai dilepas. */
  toggleMessagePin(sessionId: string, messageId: string) {
    const cur = this.snap.pinnedMessages[sessionId] ?? [];
    const next = cur.includes(messageId) ? cur.filter((m) => m !== messageId) : [...cur, messageId];
    const { [sessionId]: _old, ...rest } = this.snap.pinnedMessages;
    const pinnedMessages = next.length > 0 ? { ...rest, [sessionId]: next } : rest;
    this.safePref(PINNED_MSG_PREF, JSON.stringify(pinnedMessages));
    this.update({ pinnedMessages });
  }

  /** Catat model sebagai "terakhir digunakan" (dipindah ke depan, maksimal MAX_RECENT). */
  private touchRecent(ref: ModelRef) {
    const key = modelKey(ref);
    if (this.snap.recent[0] === key) return;
    const recent = [key, ...this.snap.recent.filter((k) => k !== key)].slice(0, MAX_RECENT);
    this.safePref(RECENT_PREF, JSON.stringify(recent));
    this.update({ recent });
  }

  selectModel(ref: ModelRef) {
    this.touchRecent(ref);
    if (sameModel(ref, this.snap.model)) return;
    this.preferred = ref;
    try {
      this.options.prefs.set(MODEL_PREF, JSON.stringify(ref));
    } catch (e) {
      this.options.onWarning?.(`gagal menyimpan pilihan model: ${errMsg(e)}`);
    }
    this.update({ model: ref, variant: this.validVariant(this.snap.models, ref, this.snap.variant) });
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
    try {
      this.options.onEvent?.(ev);
    } catch {
      // pengamat tidak boleh mengganggu aliran event
    }
    if (ev.type === "permission.asked") this.maybeAutoReply(directory, ev.request);
    const isMessageEvent = MESSAGE_EVENTS.has(ev.type);
    for (const id of this.sessionsOf(directory)) {
      if (isMessageEvent && this.reloading.has(id)) {
        // Snapshot riwayat sedang diambil; event ini mungkin sudah/atau belum tercakup → muat ulang setelahnya.
        this.dirty.add(id);
        continue;
      }
      this.patchSession(id, (s) => ({ ...s, ...applyEvent(s, ev) }));
      this.pump(id);
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
        const [messages, permissions, revert, questions, todos] = await Promise.all([
          api.listMessages(start.directory, sessionId),
          api.listPermissions(start.directory).catch(() => []),
          api.getRevert(start.directory, sessionId).catch(() => null),
          api.listQuestions(start.directory).catch((): QuestionRequest[] => []),
          api.listTodos(start.directory, sessionId).catch((): Todo[] => []),
        ]);
        if (api !== this.api || !this.snap.sessions[sessionId]) return;
        const mine = permissions.filter((p) => p.sessionID === sessionId);
        const myQuestions = questions.filter((q) => q.sessionID === sessionId);
        this.patchSession(sessionId, (s) => ({
          ...s,
          ...loadMessages(s, messages, mine, revert, { questions: myQuestions, todos }),
          error: s.error,
        }));
        if (!this.dirty.has(sessionId)) break;
      }
    } catch (e) {
      this.patchSession(sessionId, (s) => ({ ...s, error: `gagal memuat riwayat: ${errMsg(e)}` }));
    } finally {
      this.reloading.delete(sessionId);
      this.dirty.delete(sessionId);
      this.patchSession(sessionId, (s) => (s.loading ? { ...s, loading: false } : s));
      this.pump(sessionId); // antrean yang tertahan selama memuat ulang riwayat
    }
  }

  // ---------- aksi ----------

  async send(sessionId: string, text: string, files: FileAttachment[] = []): Promise<void> {
    const api = this.api;
    const s = this.snap.sessions[sessionId];
    const trimmed = text.trim();
    if (!api || !s) throw new Error("engine belum siap");
    if (!trimmed && files.length === 0) throw new Error("pesan kosong");
    if (!this.snap.model) throw new Error("belum ada model; tambahkan provider terlebih dahulu");
    if (s.sending || s.status.type !== "idle") throw new Error("sesi sedang berjalan");
    this.touchRecent(this.snap.model);
    this.patchSession(sessionId, (x) => ({ ...clearError(x), ...{ sending: true } }) as SessionChat);
    try {
      await api.sendPrompt(s.directory, sessionId, {
        text: trimmed || files.map((f) => f.filename ?? f.url).join(", "),
        model: this.snap.model,
        ...(this.snap.variant ? { variant: this.snap.variant } : {}),
        ...(files.length ? { files } : {}),
      });
    } catch (e) {
      this.patchSession(sessionId, (x) => ({ ...x, error: `gagal mengirim: ${errMsg(e)}` }));
      throw e;
    } finally {
      this.patchSession(sessionId, (x) => ({ ...x, sending: false }));
      // "kirim langsung": giliran yang dihentikan baru berakhir setelah permintaannya kembali → lanjutkan antrean
      if (this.forceNext.has(sessionId)) this.pump(sessionId);
    }
  }

  // ---------- antrean pesan ----------

  queueOf(sessionId: string): QueuedMessage[] {
    return this.snap.queues[sessionId] ?? [];
  }

  private setQueue(sessionId: string, items: QueuedMessage[]) {
    const queues = { ...this.snap.queues };
    if (items.length === 0) delete queues[sessionId];
    else queues[sessionId] = items;
    const patch: Partial<ChatStoreState> = { queues };
    if (items.length === 0 && this.snap.queuePaused[sessionId]) {
      const { [sessionId]: _drop, ...rest } = this.snap.queuePaused;
      patch.queuePaused = rest;
    }
    this.update(patch);
  }

  /** Tambah pesan ke antrean sesi. Mengembalikan id antrean. */
  enqueue(sessionId: string, text: string, files: FileAttachment[] = []): string {
    const trimmed = text.trim();
    if (!this.snap.sessions[sessionId]) throw new Error("sesi tidak ditemukan");
    if (!trimmed && files.length === 0) throw new Error("pesan kosong");
    const cur = this.queueOf(sessionId);
    if (cur.length >= MAX_QUEUE) throw new Error(`antrean penuh (maks ${MAX_QUEUE})`);
    const id = `q${++this.qSeq}`;
    this.setQueue(sessionId, [...cur, { id, text: trimmed, files }]);
    return id;
  }

  editQueued(sessionId: string, id: string, text: string): void {
    const trimmed = text.trim();
    const cur = this.queueOf(sessionId);
    const item = cur.find((q) => q.id === id);
    if (!item) return;
    if (!trimmed && item.files.length === 0) {
      this.removeQueued(sessionId, id);
      return;
    }
    this.setQueue(
      sessionId,
      cur.map((q) => (q.id === id ? { ...q, text: trimmed } : q)),
    );
  }

  removeQueued(sessionId: string, id: string): void {
    this.setQueue(
      sessionId,
      this.queueOf(sessionId).filter((q) => q.id !== id),
    );
  }

  /** Pindahkan item ke indeks `toIndex` (0 = dikirim paling dulu). */
  moveQueued(sessionId: string, id: string, toIndex: number): void {
    const cur = this.queueOf(sessionId);
    const from = cur.findIndex((q) => q.id === id);
    if (from < 0) return;
    const to = Math.max(0, Math.min(cur.length - 1, toIndex));
    if (to === from) return;
    const next = [...cur];
    const [item] = next.splice(from, 1);
    if (!item) return;
    next.splice(to, 0, item);
    this.setQueue(sessionId, next);
  }

  /**
   * Kirim langsung: item dipindah ke depan antrean; bila agent sedang bekerja, giliran itu dihentikan dulu lalu item
   * dikirim begitu sesi idle (item lain tetap menunggu di belakangnya).
   */
  async sendQueuedNow(sessionId: string, id: string): Promise<void> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) throw new Error("engine belum siap");
    this.moveQueued(sessionId, id, 0);
    if (!this.queueOf(sessionId).some((q) => q.id === id)) return;
    const { [sessionId]: _p, ...rest } = this.snap.queuePaused;
    this.update({ queuePaused: rest });
    if (s.status.type !== "idle" || s.sending) {
      this.forceNext.add(sessionId);
      await this.api.abort(s.directory, sessionId);
    }
    this.pump(sessionId, true);
  }

  /** Lanjutkan antrean yang dijeda. */
  resumeQueue(sessionId: string): void {
    const { [sessionId]: _p, ...rest } = this.snap.queuePaused;
    this.update({ queuePaused: rest });
    this.pump(sessionId, true);
  }

  /** Kirim pesan terdepan bila sesi idle dan antrean tidak dijeda. `force` mengabaikan galat sesi sebelumnya. */
  private pump(sessionId: string, force = false) {
    const s = this.snap.sessions[sessionId];
    const queue = this.snap.queues[sessionId];
    if (!s || !queue || queue.length === 0 || !this.api) return;
    const forced = force || this.forceNext.has(sessionId);
    if (s.status.type !== "idle" || s.sending || s.loading) return;
    if (this.snap.queuePaused[sessionId]) return;
    if (s.error && !forced) return;
    if (s.permissions.length > 0 || s.questions.length > 0) return;
    const [head, ...rest] = queue;
    if (!head) return;
    this.forceNext.delete(sessionId);
    this.setQueue(sessionId, rest);
    this.send(sessionId, head.text, head.files).catch(() => {
      // gagal (mis. belum ada model): kembalikan ke depan dan jeda supaya tidak berulang
      this.setQueue(sessionId, [head, ...this.queueOf(sessionId)]);
      this.update({ queuePaused: { ...this.snap.queuePaused, [sessionId]: true } });
    });
  }

  /** Ulangi jawaban terakhir: revert ke pesan pengguna terakhir lalu kirim ulang isinya (teks + lampiran). */
  async regenerate(sessionId: string): Promise<void> {
    const api = this.api;
    const s = this.snap.sessions[sessionId];
    if (!api || !s) throw new Error("engine belum siap");
    if (s.status.type !== "idle" || s.sending) throw new Error("sesi sedang berjalan");
    const cut = s.revert;
    const visible = cut === null ? s.messages : s.messages.filter((m) => m.info.id < cut);
    const user = [...visible].reverse().find((m) => m.info.role === "user");
    if (!user) throw new Error("tidak ada pesan pengguna untuk diulang");
    const text = user.parts
      .filter((p) => p.type === "text" && !p.synthetic)
      .map((p) => (p.type === "text" ? p.text : ""))
      .join("");
    const files: FileAttachment[] = user.parts.flatMap((p) =>
      p.type === "file" && p.url ? [{ mime: p.mime, url: p.url, ...(p.filename ? { filename: p.filename } : {}) }] : [],
    );
    await api.revert(s.directory, sessionId, user.info.id);
    await this.send(sessionId, text, files);
  }

  /** Kembalikan sesi (dan perubahan file) ke sebelum pesan ini. */
  async revertTo(sessionId: string, messageId: string): Promise<void> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) throw new Error("engine belum siap");
    if (s.status.type !== "idle") throw new Error("hentikan sesi sebelum revert");
    await this.api.revert(s.directory, sessionId, messageId);
    this.patchSession(sessionId, (x) => ({ ...x, revert: messageId }));
  }

  async unrevert(sessionId: string): Promise<void> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) throw new Error("engine belum siap");
    await this.api.unrevert(s.directory, sessionId);
    this.patchSession(sessionId, (x) => ({ ...x, revert: null }));
  }

  /** Cabangkan sesi sampai pesan ini menjadi sesi baru (pemanggil memilihnya di daftar project). */
  async fork(sessionId: string, messageId: string): Promise<SessionInfo> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) throw new Error("engine belum siap");
    return this.api.fork(s.directory, sessionId, messageId);
  }

  async abort(sessionId: string): Promise<void> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) return;
    // Stop oleh pengguna menjeda antrean: pesan yang menunggu tidak dikirim diam-diam setelah dihentikan.
    if (this.queueOf(sessionId).length > 0)
      this.update({ queuePaused: { ...this.snap.queuePaused, [sessionId]: true } });
    await this.api.abort(s.directory, sessionId);
  }

  /** Jawab pertanyaan agent. `answers[i]` = pilihan/teks untuk pertanyaan ke-i. */
  async answerQuestion(sessionId: string, requestId: string, answers: string[][]): Promise<void> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) throw new Error("engine belum siap");
    await this.api.replyQuestion(s.directory, requestId, answers);
    this.patchSession(sessionId, (x) => ({ ...x, questions: x.questions.filter((q) => q.id !== requestId) }));
  }

  /** Lewati/tolak pertanyaan agent (agent melanjutkan tanpa jawaban). */
  async rejectQuestion(sessionId: string, requestId: string): Promise<void> {
    const s = this.snap.sessions[sessionId];
    if (!this.api || !s) throw new Error("engine belum siap");
    await this.api.rejectQuestion(s.directory, requestId);
    this.patchSession(sessionId, (x) => ({ ...x, questions: x.questions.filter((q) => q.id !== requestId) }));
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

function parsePinnedMessages(raw: string | null): Record<string, string[]> {
  if (!raw) return {};
  try {
    const v = JSON.parse(raw) as unknown;
    if (typeof v !== "object" || v === null || Array.isArray(v)) return {};
    const out: Record<string, string[]> = {};
    for (const [k, ids] of Object.entries(v)) {
      if (Array.isArray(ids)) {
        const list = [...new Set(ids.filter((x): x is string => typeof x === "string"))];
        if (list.length > 0) out[k] = list;
      }
    }
    return out;
  } catch {
    return {};
  }
}

function parsePinned(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const v = JSON.parse(raw) as unknown;
    return Array.isArray(v) ? [...new Set(v.filter((x): x is string => typeof x === "string"))] : [];
  } catch {
    return [];
  }
}

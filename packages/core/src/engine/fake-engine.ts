// Engine palsu di memori untuk test (core dan UI). Meniru perilaku yang terbukti pada engine asli:
// sesi terikat direktori, judul bawaan "New session - <ISO>", daftar berkas menurut path relatif.
import type { ChatMessage, EngineEvent, PermissionReply, PermissionRequest } from "../chat/types";
import type {
  AuthMethod,
  CatalogProvider,
  EngineApi,
  FileAttachment,
  FileNode,
  ModelOption,
  ModelRef,
  OAuthStart,
  SessionInfo,
  SubscriptionHandlers,
} from "./api";

export type FakeEngine = EngineApi & {
  sessions: Map<string, SessionInfo[]>;
  files: Map<string, Record<string, FileNode[]>>;
  calls: string[];
  /** Jika diisi, semua panggilan berikutnya menolak dengan pesan ini. */
  failWith: string | null;
  tick: number;
  models: ModelOption[];
  messages: Map<string, ChatMessage[]>;
  permissions: PermissionRequest[];
  prompts: Array<{
    directory: string;
    sessionId: string;
    text: string;
    model?: ModelRef;
    variant?: string;
    files?: FileAttachment[];
  }>;
  reverts: Map<string, string | null>;
  forks: Array<{ sessionId: string; messageId: string }>;
  replies: Array<{ requestId: string; reply: PermissionReply }>;
  aborted: string[];
  /** Dipanggil saat prompt diterima; test memakainya untuk menjalankan skenario respons lewat `emit`. */
  onPrompt: ((directory: string, sessionId: string, text: string) => void) | null;
  catalog: CatalogProvider[];
  authMethods: Record<string, AuthMethod[]>;
  oauth: {
    starts: Array<{ providerId: string; method: number; inputs?: Record<string, string> }>;
    callbacks: Array<{ providerId: string; method: number; code?: string }>;
    next: OAuthStart;
  };
  subscribers: Map<string, Set<SubscriptionHandlers>>;
  /** Kirim event ke semua pelanggan direktori. */
  emit(directory: string, event: EngineEvent): void;
  /** Simulasikan koneksi SSE putus lalu tersambung lagi. */
  reconnect(directory: string): void;
};

export function createFakeEngine(): FakeEngine {
  let n = 0;
  const e: FakeEngine = {
    sessions: new Map(),
    files: new Map(),
    calls: [],
    failWith: null,
    tick: 1_000,
    models: [],
    messages: new Map(),
    permissions: [],
    prompts: [],
    reverts: new Map(),
    forks: [],
    replies: [],
    aborted: [],
    onPrompt: null,
    catalog: [],
    authMethods: {},
    oauth: {
      starts: [],
      callbacks: [],
      next: { url: "https://auth.example.test/authorize", method: "auto", instructions: "Selesaikan di browser." },
    },
    async listCatalog() {
      if (e.failWith) throw new Error(e.failWith);
      return [...e.catalog];
    },
    async listAuthMethods() {
      if (e.failWith) throw new Error(e.failWith);
      return { ...e.authMethods };
    },
    async oauthAuthorize(providerId, method, inputs) {
      if (e.failWith) throw new Error(e.failWith);
      e.oauth.starts.push({ providerId, method, ...(inputs ? { inputs } : {}) });
      return e.oauth.next;
    },
    async oauthCallback(providerId, method, code) {
      if (e.failWith) throw new Error(e.failWith);
      e.oauth.callbacks.push({ providerId, method, ...(code ? { code } : {}) });
    },
    subscribers: new Map(),
    emit(directory, event) {
      for (const h of e.subscribers.get(directory) ?? []) h.onEvent(event);
    },
    reconnect(directory) {
      for (const h of e.subscribers.get(directory) ?? []) {
        h.onState("reconnecting");
        h.onState("connected");
      }
    },
    async listModels() {
      if (e.failWith) throw new Error(e.failWith);
      return [...e.models];
    },
    async listMessages(_dir, sessionId) {
      e.calls.push(`messages:${sessionId}`);
      if (e.failWith) throw new Error(e.failWith);
      return [...(e.messages.get(sessionId) ?? [])];
    },
    async listPermissions() {
      if (e.failWith) throw new Error(e.failWith);
      return [...e.permissions];
    },
    async sendPrompt(directory, sessionId, input) {
      if (e.failWith) throw new Error(e.failWith);
      e.prompts.push({
        directory,
        sessionId,
        text: input.text,
        ...(input.model ? { model: input.model } : {}),
        ...(input.variant ? { variant: input.variant } : {}),
        ...(input.files ? { files: input.files } : {}),
      });
      e.onPrompt?.(directory, sessionId, input.text);
    },
    async getRevert(_dir, sessionId) {
      return e.reverts.get(sessionId) ?? null;
    },
    async revert(_dir, sessionId, messageId) {
      if (e.failWith) throw new Error(e.failWith);
      e.reverts.set(sessionId, messageId);
    },
    async unrevert(_dir, sessionId) {
      if (e.failWith) throw new Error(e.failWith);
      e.reverts.set(sessionId, null);
    },
    async fork(dir, sessionId, messageId) {
      if (e.failWith) throw new Error(e.failWith);
      e.forks.push({ sessionId, messageId });
      return e.createSession(dir, `Cabang dari ${sessionId}`);
    },
    async abort(_dir, sessionId) {
      if (e.failWith) throw new Error(e.failWith);
      e.aborted.push(sessionId);
    },
    async replyPermission(_dir, requestId, reply) {
      if (e.failWith) throw new Error(e.failWith);
      e.replies.push({ requestId, reply });
    },
    subscribe(directory, handlers) {
      const set = e.subscribers.get(directory) ?? new Set();
      set.add(handlers);
      e.subscribers.set(directory, set);
      queueMicrotask(() => handlers.onState("connected"));
      return () => {
        set.delete(handlers);
      };
    },
    async health() {
      e.calls.push("health");
      if (e.failWith) throw new Error(e.failWith);
      return { version: "1.19.5" };
    },
    async listSessions(dir) {
      e.calls.push(`list:${dir}`);
      if (e.failWith) throw new Error(e.failWith);
      return [...(e.sessions.get(dir) ?? [])];
    },
    async createSession(dir, title) {
      e.calls.push(`create:${dir}`);
      if (e.failWith) throw new Error(e.failWith);
      const t = ++e.tick;
      const s: SessionInfo = {
        id: `ses_${++n}`,
        title: title ?? `New session - ${new Date(t).toISOString()}`,
        directory: dir,
        time: { created: t, updated: t },
      };
      e.sessions.set(dir, [...(e.sessions.get(dir) ?? []), s]);
      return s;
    },
    async renameSession(dir, id, title) {
      e.calls.push(`rename:${id}`);
      if (e.failWith) throw new Error(e.failWith);
      const list = e.sessions.get(dir) ?? [];
      const i = list.findIndex((s) => s.id === id);
      if (i === -1) throw new Error("sesi tidak ada");
      const prev = list[i] as SessionInfo;
      const updated = { ...prev, title, time: { ...prev.time, updated: ++e.tick } };
      list[i] = updated;
      return updated;
    },
    async deleteSession(dir, id) {
      e.calls.push(`delete:${id}`);
      if (e.failWith) throw new Error(e.failWith);
      e.sessions.set(
        dir,
        (e.sessions.get(dir) ?? []).filter((s) => s.id !== id),
      );
    },
    async listFiles(dir, path) {
      e.calls.push(`files:${dir}:${path}`);
      if (e.failWith) throw new Error(e.failWith);
      return e.files.get(dir)?.[path] ?? [];
    },
  };
  return e;
}

// Antarmuka engine yang dibutuhkan UI (Fase 4). Implementasi nyata membungkus SDK resmi @opencode-ai/sdk
// (tipe sama persis dengan engine yang di-pin, ADR-021); test memakai implementasi palsu.
import { createOpencodeClient } from "@opencode-ai/sdk/v2/client";
import { attachmentParts, type PromptFilePart } from "../attachments";
import {
  type ChatMessage,
  type EngineEvent,
  type PermissionReply,
  type PermissionRequest,
  type QuestionRequest,
  type Todo,
  toEngineEvent,
  toMessage,
  toPermission,
  toQuestion,
  toTodos,
} from "../chat/types";
import { createPtyClient, type PtyClient } from "../terminal/pty";

export type EngineConnectionInfo = { url: string; username: string; password: string };

export type SessionInfo = {
  id: string;
  title: string;
  directory: string;
  time: { created: number; updated: number };
};

/** Perubahan satu berkas oleh sesi, dari snapshot engine: patch unified (bisa kosong untuk berkas biner) + jumlah baris. */
export type SessionFileDiff = {
  file: string;
  patch: string;
  additions: number;
  deletions: number;
  status: "added" | "deleted" | "modified";
};

export type FileNode = {
  name: string;
  /** Relatif terhadap direktori project. */
  path: string;
  absolute: string;
  type: "file" | "directory";
  ignored: boolean;
};

export type ModelOption = {
  providerID: string;
  providerName: string;
  modelID: string;
  name: string;
  /** Varian reasoning yang diterima model (kosong bila tidak ada). */
  variants: string[];
  /** Batas jendela konteks model (token), bila diketahui. */
  contextLimit?: number;
  /** Batas token keluaran model, bila diketahui. */
  outputLimit?: number;
  /** Kemampuan (dari katalog engine), bila diketahui. */
  reasoning?: boolean;
  toolCall?: boolean;
  /** Menerima lampiran (gambar/PDF/dll). */
  attachment?: boolean;
  /** Modalitas masukan yang didukung (mis. text, image, pdf). */
  inputs?: string[];
};
export type FileAttachment = { mime: string; url: string; filename?: string };
export type EngineCommand = {
  name: string;
  description?: string;
  source?: "command" | "mcp" | "skill";
  /** Placeholder argumen ($1, $ARGUMENTS) yang dipakai templat. */
  hints: string[];
};
export type ModelRef = { providerID: string; modelID: string };
export type SubscriptionState = "connected" | "reconnecting";
export type SubscriptionHandlers = {
  onEvent(event: EngineEvent): void;
  /** `connected` dipanggil setiap (re)koneksi berhasil: pemanggil wajib menyinkronkan ulang state-nya. */
  onState(state: SubscriptionState): void;
};

/** Provider di katalog engine (models.dev + kustom). */
export type CatalogProvider = {
  id: string;
  name: string;
  /** Nama env var yang dikenali engine untuk kunci provider ini. */
  env: string[];
  /** Engine menganggap provider ini siap dipakai (ada kunci/token atau tanpa kunci). */
  connected: boolean;
  modelCount: number;
};
export type AuthPrompt = {
  type: "text" | "select";
  key: string;
  message: string;
  placeholder?: string;
  options?: Array<{ label: string; value: string; hint?: string }>;
};
export type AuthMethod = { type: "oauth" | "api"; label: string; prompts: AuthPrompt[] };
export type OAuthStart = { url: string; method: "auto" | "code"; instructions: string };

export interface EngineApi {
  /** Terminal (PTY) di mesin engine. */
  pty: PtyClient;
  health(): Promise<{ version: string }>;
  listSessions(directory: string): Promise<SessionInfo[]>;
  createSession(directory: string, title?: string): Promise<SessionInfo>;
  renameSession(directory: string, sessionId: string, title: string): Promise<SessionInfo>;
  deleteSession(directory: string, sessionId: string): Promise<void>;
  listFiles(directory: string, path: string): Promise<FileNode[]>;
  /** Isi berkas lewat engine (cadangan di luar Tauri; teks bisa terpotong spasi akhir oleh engine). */
  readFile(directory: string, path: string): Promise<Uint8Array>;
  listModels(): Promise<ModelOption[]>;
  listMessages(directory: string, sessionId: string): Promise<ChatMessage[]>;
  listPermissions(directory: string): Promise<PermissionRequest[]>;
  /** Pertanyaan agent yang menunggu jawaban (semua sesi di direktori). */
  listQuestions(directory: string): Promise<QuestionRequest[]>;
  listTodos(directory: string, sessionId: string): Promise<Todo[]>;
  /** `answers[i]` = label yang dipilih (atau teks bebas) untuk pertanyaan ke-i. */
  replyQuestion(directory: string, requestId: string, answers: string[][]): Promise<void>;
  rejectQuestion(directory: string, requestId: string): Promise<void>;
  /** Kirim prompt tanpa menunggu jawaban; hasilnya mengalir lewat `subscribe`. */
  sendPrompt(
    directory: string,
    sessionId: string,
    input: { text: string; model?: ModelRef; variant?: string; files?: FileAttachment[] },
  ): Promise<void>;
  /** Perubahan berkas yang dibuat sesi ini (berdasarkan snapshot engine; tidak bergantung pada git). */
  sessionDiff(directory: string, sessionId: string): Promise<SessionFileDiff[]>;
  /** Id pesan tujuan revert sesi, atau null. */
  getRevert(directory: string, sessionId: string): Promise<string | null>;
  revert(directory: string, sessionId: string, messageId: string): Promise<void>;
  unrevert(directory: string, sessionId: string): Promise<void>;
  /** Cabangkan sesi. Tanpa `messageId` seluruh riwayat ikut disalin. */
  fork(directory: string, sessionId: string, messageId?: string): Promise<SessionInfo>;
  /** Cari berkas project berdasarkan nama (fuzzy, oleh engine). Mengembalikan jalur relatif. */
  findFiles(directory: string, query: string, limit?: number): Promise<string[]>;
  /** Perintah slash milik engine (bawaan, config `command/`, MCP prompt, skill). */
  listCommands(directory: string): Promise<EngineCommand[]>;
  /** Jalankan perintah slash engine di sesi (hasil mengalir lewat `subscribe`). */
  runCommand(
    directory: string,
    sessionId: string,
    input: { command: string; arguments: string; model?: ModelRef; variant?: string; files?: FileAttachment[] },
  ): Promise<void>;
  /** Ringkas (compact) riwayat sesi dengan model terpilih agar konteks lega. */
  summarize(directory: string, sessionId: string, model: ModelRef): Promise<void>;
  abort(directory: string, sessionId: string): Promise<void>;
  /** Buang instance engine satu direktori agar cache (skill, config) dibaca ulang; dibuat ulang otomatis saat dipakai. */
  disposeInstance(directory: string): Promise<void>;
  replyPermission(directory: string, requestId: string, reply: PermissionReply): Promise<void>;
  listCatalog(): Promise<CatalogProvider[]>;
  /** Metode autentikasi per id provider (OAuth/API) menurut engine. */
  listAuthMethods(): Promise<Record<string, AuthMethod[]>>;
  oauthAuthorize(providerId: string, method: number, inputs?: Record<string, string>): Promise<OAuthStart>;
  /** Selesaikan OAuth. Metode "auto": menunggu sampai browser selesai; "code": `code` dari pengguna. */
  oauthCallback(providerId: string, method: number, code?: string): Promise<void>;
  /** Berlangganan event SSE satu direktori dengan reconnect otomatis. Mengembalikan fungsi berhenti. */
  subscribe(directory: string, handlers: SubscriptionHandlers): () => void;
}

/** Judul bawaan engine untuk sesi baru ("New session - 2026-…") berarti "belum diberi nama". */
export const isDefaultSessionTitle = (title: string) => /^New session - \d{4}-\d{2}-\d{2}T/.test(title);

export function basicAuthHeader(info: Pick<EngineConnectionInfo, "username" | "password">): string {
  const bytes = new TextEncoder().encode(`${info.username}:${info.password}`);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return `Basic ${btoa(bin)}`;
}

/** Respons sukses wajib berisi body; body kosong berarti engine/protokol tidak sesuai harapan. */
function body<T>(r: { data?: T }, what: string): T {
  if (r.data === undefined) throw new Error(`engine mengembalikan respons kosong untuk ${what}`);
  return r.data;
}

const toSession = (s: {
  id: string;
  title: string;
  directory: string;
  time: { created: number; updated: number };
}): SessionInfo => ({
  id: s.id,
  title: s.title,
  directory: s.directory,
  time: { created: s.time.created, updated: s.time.updated },
});

export function createEngineApi(info: EngineConnectionInfo, fetchImpl?: typeof fetch): EngineApi {
  const client = createOpencodeClient({
    baseUrl: info.url,
    headers: { Authorization: basicAuthHeader(info) },
    throwOnError: true,
    ...(fetchImpl ? { fetch: fetchImpl as never } : {}),
  });
  return {
    pty: createPtyClient(info, fetchImpl),
    async health() {
      const r = await client.global.health();
      return { version: body(r, "global.health").version };
    },
    async listSessions(directory) {
      const r = await client.session.list({ directory });
      return body(r, "session.list").map(toSession);
    },
    async createSession(directory, title) {
      const r = await client.session.create({ directory, ...(title ? { title } : {}) });
      return toSession(body(r, "session.create"));
    },
    async renameSession(directory, sessionId, title) {
      const r = await client.session.update({ sessionID: sessionId, directory, title });
      return toSession(body(r, "session.update"));
    },
    async deleteSession(directory, sessionId) {
      await client.session.delete({ sessionID: sessionId, directory });
    },
    async readFile(directory, path) {
      const r = await client.file.read({ directory, path });
      const d = body(r, "file.read") as { type: string; content: string; encoding?: string };
      if (d.encoding === "base64") {
        const bin = atob(d.content);
        const out = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
        return out;
      }
      return new TextEncoder().encode(d.content);
    },
    async listModels() {
      const r = await client.config.providers();
      const out: ModelOption[] = [];
      for (const p of body(r, "config.providers").providers) {
        for (const m of Object.values(p.models)) {
          out.push({
            providerID: p.id,
            providerName: p.name,
            modelID: m.id,
            name: m.name,
            variants: Object.keys(m.variants ?? {}),
            ...(m.limit?.context ? { contextLimit: m.limit.context } : {}),
            ...(m.limit?.output ? { outputLimit: m.limit.output } : {}),
            ...(m.capabilities
              ? {
                  reasoning: !!m.capabilities.reasoning,
                  toolCall: !!m.capabilities.toolcall,
                  attachment: !!m.capabilities.attachment,
                  inputs: Object.entries(m.capabilities.input ?? {})
                    .filter(([, on]) => on)
                    .map(([k]) => k),
                }
              : {}),
          });
        }
      }
      return out;
    },
    async listMessages(directory, sessionId) {
      const r = await client.session.messages({ sessionID: sessionId, directory });
      return body(r, "session.messages")
        .map(toMessage)
        .filter((m): m is ChatMessage => m !== null);
    },
    async listPermissions(directory) {
      const r = await client.permission.list({ directory });
      return body(r, "permission.list")
        .map(toPermission)
        .filter((p): p is PermissionRequest => p !== null);
    },
    async listQuestions(directory) {
      const r = await client.question.list({ directory });
      return body(r, "question.list")
        .map(toQuestion)
        .filter((q): q is QuestionRequest => q !== null);
    },
    async listTodos(directory, sessionId) {
      const r = await client.session.todo({ sessionID: sessionId, directory });
      return toTodos(body(r, "session.todo"));
    },
    async replyQuestion(directory, requestId, answers) {
      await client.question.reply({ requestID: requestId, directory, answers });
    },
    async rejectQuestion(directory, requestId) {
      await client.question.reject({ requestID: requestId, directory });
    },
    async sendPrompt(directory, sessionId, input) {
      await client.session.promptAsync({
        sessionID: sessionId,
        directory,
        ...(input.model ? { model: input.model } : {}),
        ...(input.variant ? { variant: input.variant } : {}),
        parts: [{ type: "text", text: input.text }, ...attachmentParts(input.files ?? [])],
      });
    },
    async getRevert(directory, sessionId) {
      const r = await client.session.get({ sessionID: sessionId, directory });
      return body(r, "session.get").revert?.messageID ?? null;
    },
    async sessionDiff(directory, sessionId) {
      const r = await client.session.diff({ sessionID: sessionId, directory });
      return body(r, "session.diff").map((d) => ({
        file: d.file ?? "",
        patch: d.patch ?? "",
        additions: d.additions ?? 0,
        deletions: d.deletions ?? 0,
        status: d.status ?? "modified",
      }));
    },
    async revert(directory, sessionId, messageId) {
      await client.session.revert({ sessionID: sessionId, directory, messageID: messageId });
    },
    async unrevert(directory, sessionId) {
      await client.session.unrevert({ sessionID: sessionId, directory });
    },
    async fork(directory, sessionId, messageId) {
      const r = await client.session.fork({
        sessionID: sessionId,
        directory,
        ...(messageId ? { messageID: messageId } : {}),
      });
      return toSession(body(r, "session.fork"));
    },
    async findFiles(directory, query, limit = 20) {
      const r = await client.find.files({ directory, query, dirs: "false", limit });
      return body(r, "find.files");
    },
    async listCommands(directory) {
      const r = await client.command.list({ directory });
      return body(r, "command.list").map((c) => ({
        name: c.name,
        hints: c.hints ?? [],
        ...(c.description ? { description: c.description } : {}),
        ...(c.source ? { source: c.source } : {}),
      }));
    },
    async runCommand(directory, sessionId, input) {
      const all = attachmentParts(input.files ?? []);
      const cmdParts = {
        files: all.filter((p): p is PromptFilePart => p.type === "file"),
        notes: all.flatMap((p) => (p.type === "text" ? [p.text] : [])),
      };
      await client.session.command({
        sessionID: sessionId,
        directory,
        command: input.command,
        // Perintah hanya menerima bagian `file`; catatan lampiran yang tak terbaca model ditambahkan ke argumen.
        arguments: [input.arguments, ...cmdParts.notes].filter(Boolean).join("\n\n"),
        ...(input.model ? { model: `${input.model.providerID}/${input.model.modelID}` } : {}),
        ...(input.variant ? { variant: input.variant } : {}),
        ...(cmdParts.files.length
          ? {
              parts: cmdParts.files,
            }
          : {}),
      });
    },
    async summarize(directory, sessionId, model) {
      await client.session.summarize({
        sessionID: sessionId,
        directory,
        providerID: model.providerID,
        modelID: model.modelID,
      });
    },
    async abort(directory, sessionId) {
      await client.session.abort({ sessionID: sessionId, directory });
    },
    async disposeInstance(directory) {
      await client.instance.dispose({ directory });
    },
    async replyPermission(directory, requestId, reply) {
      await client.permission.reply({ requestID: requestId, directory, reply });
    },
    subscribe(directory, handlers) {
      const ctl = new AbortController();
      void (async () => {
        let delay = 500;
        while (!ctl.signal.aborted) {
          try {
            const sub = await client.event.subscribe({ directory }, { signal: ctl.signal, sseMaxRetryAttempts: 1 });
            delay = 500;
            handlers.onState("connected");
            for await (const raw of sub.stream) {
              const ev = toEngineEvent(raw);
              if (ev) handlers.onEvent(ev);
            }
          } catch {
            // jatuh ke reconnect di bawah
          }
          if (ctl.signal.aborted) return;
          handlers.onState("reconnecting");
          await new Promise((r) => setTimeout(r, delay));
          delay = Math.min(delay * 2, 5000);
        }
      })();
      return () => ctl.abort();
    },
    async listCatalog() {
      const r = await client.provider.list();
      const data = body(r, "provider.list");
      const connected = new Set(data.connected);
      return data.all.map((p) => ({
        id: p.id,
        name: p.name,
        env: p.env,
        connected: connected.has(p.id),
        modelCount: Object.keys(p.models).length,
      }));
    },
    async listAuthMethods() {
      const r = await client.provider.auth();
      const out: Record<string, AuthMethod[]> = {};
      for (const [id, methods] of Object.entries(body(r, "provider.auth"))) {
        out[id] = (methods as Array<Record<string, unknown>>).map((m) => ({
          type: m.type === "oauth" ? "oauth" : "api",
          label: typeof m.label === "string" ? m.label : id,
          prompts: Array.isArray(m.prompts) ? (m.prompts as AuthPrompt[]) : [],
        }));
      }
      return out;
    },
    async oauthAuthorize(providerId, method, inputs) {
      const r = await client.provider.oauth.authorize({
        providerID: providerId,
        method,
        ...(inputs ? { inputs } : {}),
      });
      const d = body(r, "oauth.authorize") as { url: string; method: string; instructions: string };
      return { url: d.url, method: d.method === "code" ? "code" : "auto", instructions: d.instructions ?? "" };
    },
    async oauthCallback(providerId, method, code) {
      await client.provider.oauth.callback({ providerID: providerId, method, ...(code ? { code } : {}) });
    },
    async listFiles(directory, path) {
      const r = await client.file.list({ directory, path });
      return body(r, "file.list").map((n) => ({
        name: n.name,
        path: n.path,
        absolute: n.absolute,
        type: n.type,
        ignored: n.ignored,
      }));
    },
  };
}

// Reducer murni untuk state chat satu sesi: event engine → state baru (immutable, snapshot stabil bila tak berubah).
import {
  byId,
  type ChatMessage,
  type EngineEvent,
  type MessagePart,
  type PermissionRequest,
  type QuestionRequest,
  type SessionRunStatus,
  type Todo,
} from "./types";

export type ChatState = {
  sessionID: string;
  /** Riwayat sudah dimuat dari engine (sebelumnya state hanya dari event). */
  loaded: boolean;
  messages: ChatMessage[];
  status: SessionRunStatus;
  /** Pesan tujuan revert aktif; pesan sejak id ini disembunyikan. */
  revert: string | null;
  permissions: PermissionRequest[];
  /** Pertanyaan agent yang menunggu jawaban pengguna. */
  questions: QuestionRequest[];
  /** Daftar tugas agent terbaru. */
  todos: Todo[];
  /** Galat tingkat sesi terakhir (mis. provider gagal); dihapus saat prompt baru dikirim. */
  error: string | null;
  /** Delta yang tiba sebelum part-nya dikenal; diterapkan saat part muncul. */
  pending: Record<string, string>;
};

export const initialChatState = (sessionID: string): ChatState => ({
  sessionID,
  loaded: false,
  messages: [],
  status: { type: "idle" },
  revert: null,
  permissions: [],
  questions: [],
  todos: [],
  error: null,
  pending: {},
});

const upsertMessage = (messages: ChatMessage[], next: ChatMessage): ChatMessage[] => {
  const i = messages.findIndex((m) => m.info.id === next.info.id);
  if (i === -1) return [...messages, next].sort((a, b) => byId(a.info, b.info));
  const out = messages.slice();
  out[i] = next;
  return out;
};

const upsertPart = (parts: MessagePart[], next: MessagePart): MessagePart[] => {
  const i = parts.findIndex((p) => p.id === next.id);
  if (i === -1) return [...parts, next].sort(byId);
  const out = parts.slice();
  out[i] = next;
  return out;
};

/** Ganti seluruh riwayat (hasil muat awal/sinkron ulang). Event yang sudah tiba dipertahankan bila lebih baru. */
export function loadMessages(
  state: ChatState,
  messages: ChatMessage[],
  permissions: PermissionRequest[],
  revert: string | null = state.revert,
  extra: { questions?: QuestionRequest[]; todos?: Todo[] } = {},
): ChatState {
  const sorted = [...messages].sort((a, b) => byId(a.info, b.info));
  const last = sorted.at(-1);
  // Bila pesan asisten terakhir belum selesai, sesi masih berjalan walau belum ada event status.
  const running = last?.info.role === "assistant" && last.info.time.completed === undefined;
  return {
    ...state,
    loaded: true,
    revert,
    messages: sorted,
    permissions,
    questions: extra.questions ?? state.questions,
    todos: extra.todos ?? state.todos,
    pending: {},
    status: running && state.status.type === "idle" ? { type: "busy" } : state.status,
  };
}

export function applyEvent(state: ChatState, ev: EngineEvent): ChatState {
  switch (ev.type) {
    case "message.updated": {
      if (ev.info.sessionID !== state.sessionID) return state;
      const existing = state.messages.find((m) => m.info.id === ev.info.id);
      const next: ChatMessage = { info: ev.info, parts: existing?.parts ?? [] };
      return { ...state, messages: upsertMessage(state.messages, next) };
    }
    case "message.removed": {
      if (ev.sessionID !== state.sessionID) return state;
      return { ...state, messages: state.messages.filter((m) => m.info.id !== ev.messageID) };
    }
    case "message.part.updated": {
      const part = ev.part;
      if (part.sessionID !== state.sessionID) return state;
      let incoming = part;
      let pending = state.pending;
      const buffered = pending[part.id];
      if (buffered !== undefined) {
        // Delta yang menunggu part ini: ditambahkan bila teks part masih lebih pendek (belum memuatnya).
        if ((incoming.type === "text" || incoming.type === "reasoning") && !incoming.text.endsWith(buffered)) {
          incoming = { ...incoming, text: incoming.text + buffered };
        }
        const { [part.id]: _drop, ...rest } = pending;
        pending = rest;
      }
      const existing = state.messages.find((m) => m.info.id === part.messageID);
      // Part tiba sebelum message.updated: buat pesan sementara agar tidak hilang; info menyusul.
      const base: ChatMessage = existing ?? {
        info: {
          id: part.messageID,
          sessionID: part.sessionID,
          role: "assistant",
          time: { created: 0 },
        },
        parts: [],
      };
      return {
        ...state,
        pending,
        messages: upsertMessage(state.messages, { info: base.info, parts: upsertPart(base.parts, incoming) }),
      };
    }
    case "message.part.removed": {
      if (ev.sessionID !== state.sessionID) return state;
      return {
        ...state,
        messages: state.messages.map((m) =>
          m.info.id === ev.messageID ? { ...m, parts: m.parts.filter((p) => p.id !== ev.partID) } : m,
        ),
      };
    }
    case "message.part.delta": {
      if (ev.sessionID !== state.sessionID || ev.field !== "text") return state;
      const msg = state.messages.find((m) => m.info.id === ev.messageID);
      const part = msg?.parts.find((p) => p.id === ev.partID);
      if (!msg || !part) {
        return { ...state, pending: { ...state.pending, [ev.partID]: (state.pending[ev.partID] ?? "") + ev.delta } };
      }
      if (part.type !== "text" && part.type !== "reasoning") return state;
      const updated = { ...part, text: part.text + ev.delta };
      return {
        ...state,
        messages: state.messages.map((m) =>
          m.info.id === ev.messageID ? { ...m, parts: m.parts.map((p) => (p.id === ev.partID ? updated : p)) } : m,
        ),
      };
    }
    case "session.status": {
      if (ev.sessionID !== state.sessionID) return state;
      if (sameStatus(state.status, ev.status)) return state;
      return { ...state, status: ev.status };
    }
    case "session.revert": {
      if (ev.sessionID !== state.sessionID || ev.revert === state.revert) return state;
      return { ...state, revert: ev.revert };
    }
    case "session.error": {
      if (ev.sessionID !== null && ev.sessionID !== state.sessionID) return state;
      return { ...state, error: ev.error.message };
    }
    case "permission.asked": {
      if (ev.request.sessionID !== state.sessionID) return state;
      if (state.permissions.some((p) => p.id === ev.request.id)) return state;
      return { ...state, permissions: [...state.permissions, ev.request] };
    }
    case "question.asked": {
      if (ev.request.sessionID !== state.sessionID) return state;
      if (state.questions.some((q) => q.id === ev.request.id)) return state;
      return { ...state, questions: [...state.questions, ev.request] };
    }
    case "question.closed": {
      if (ev.sessionID !== state.sessionID) return state;
      if (!state.questions.some((q) => q.id === ev.requestID)) return state;
      return { ...state, questions: state.questions.filter((q) => q.id !== ev.requestID) };
    }
    case "todo.updated": {
      if (ev.sessionID !== state.sessionID) return state;
      return { ...state, todos: ev.todos };
    }
    case "permission.replied": {
      if (ev.sessionID !== state.sessionID) return state;
      if (!state.permissions.some((p) => p.id === ev.requestID)) return state;
      return { ...state, permissions: state.permissions.filter((p) => p.id !== ev.requestID) };
    }
  }
}

const sameStatus = (a: SessionRunStatus, b: SessionRunStatus) =>
  a.type === b.type &&
  (a.type !== "retry" || (b.type === "retry" && a.attempt === b.attempt && a.message === b.message));

export const clearError = (state: ChatState): ChatState => (state.error === null ? state : { ...state, error: null });

/** Teks gabungan pesan (untuk salin/pencarian): hanya part teks non-sintetis. */
export function messageText(m: ChatMessage): string {
  return m.parts
    .filter((p) => p.type === "text" && !p.synthetic)
    .map((p) => (p.type === "text" ? p.text : ""))
    .join("");
}

/** Pesan yang terlihat: yang sejak id revert (jika ada) disembunyikan. */
export function visibleMessages(state: Pick<ChatState, "messages" | "revert">): ChatMessage[] {
  const cut = state.revert;
  return cut === null ? state.messages : state.messages.filter((m) => m.info.id < cut);
}

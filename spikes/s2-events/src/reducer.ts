// Spike S2 — reducer event SSE opencode → state transcript per sesi.
// Tipe diambil dari SDK resmi (@opencode-ai/sdk@1.19.5, MIT), versinya sama dengan engine yang di-pin.
//
// Catatan desain:
// - Sumber kebenaran tiap part = event `message.part.updated` (selalu membawa part utuh).
//   `message.part.delta` hanya untuk streaming (menambah string ke `part[field]`).
// - Delta yang datang sebelum part-nya dikenal ditahan di buffer lalu diterapkan saat part tiba.
// - Urutan pesan & part = urutan ID (ID opencode monoton naik).
// - Spike ini memakai mutasi; produk (React) perlu versi immutable / store dengan selector.
import type {
  Event,
  Message,
  Part,
  PermissionRequest,
  QuestionRequest,
  SessionStatus,
  SnapshotFileDiff,
  Todo,
} from "@opencode-ai/sdk/v2/types";

export type PermissionView = PermissionRequest & { reply?: "once" | "always" | "reject" };
export type QuestionView = QuestionRequest & { answers?: string[][]; rejected?: boolean };

export type MessageView = { info: Message; parts: Map<string, Part> };

export type SessionView = {
  id: string;
  title?: string;
  status: SessionStatus;
  messages: Map<string, MessageView>;
  permissions: Map<string, PermissionView>;
  questions: Map<string, QuestionView>;
  todos: Todo[];
  diff: SnapshotFileDiff[];
  error?: unknown;
  compactedAt: number[];
};

/** Event yang sengaja tidak memengaruhi transcript (milik panel lain atau infrastruktur). */
export const IGNORED_FOR_TRANSCRIPT = new Set<string>([
  "server.connected",
  "server.heartbeat", // dipancarkan engine, tidak ada di skema OpenAPI 1.19.5
  "global.disposed",
  "server.instance.disposed",
  "plugin.added",
  "catalog.updated",
  "reference.updated",
  "integration.updated",
  "integration.connection.updated",
  "models-dev.refreshed",
  "installation.updated",
  "installation.update-available",
  "file.edited", // Panel 3/Files
  "file.watcher.updated",
  "pty.created", // Panel 4 Terminal
  "pty.updated",
  "pty.exited",
  "pty.deleted",
  "lsp.updated",
  "vcs.branch.updated", // Panel 4 Git
  "project.updated",
  "project.directories.updated",
  "mcp.tools.changed",
  "mcp.browser.open.failed",
  "command.executed",
  "workspace.ready",
  "workspace.failed",
  "workspace.status",
  "worktree.ready",
  "worktree.failed",
  "tui.prompt.append",
  "tui.command.execute",
  "tui.toast.show",
  "tui.session.select",
]);

/** Event yang ditangani `TranscriptStore.apply` (harus sinkron dengan `switch` di bawah). */
export const HANDLED_EVENTS = new Set<string>([
  "session.created",
  "session.updated",
  "session.deleted",
  "session.status",
  "session.idle",
  "session.error",
  "session.diff",
  "session.compacted",
  "message.updated",
  "message.removed",
  "message.part.updated",
  "message.part.delta",
  "message.part.removed",
  "permission.asked",
  "permission.replied",
  "question.asked",
  "question.replied",
  "question.rejected",
  "todo.updated",
]);

/**
 * Keluarga event "v2" (`session.next.*`, permission/question v2) ada di skema 1.19.5 tetapi
 * TIDAK dipancarkan oleh `/event` pada rekaman nyata. Dicatat terpisah agar terdeteksi bila
 * engine versi berikutnya mulai memakainya.
 */
export const isV2Family = (type: string) =>
  type.startsWith("session.next.") || type.startsWith("permission.v2.") || type.startsWith("question.v2.");

export class TranscriptStore {
  sessions = new Map<string, SessionView>();
  /** Event yang tidak dikenali / tidak ditangani — dipakai test kelengkapan. */
  unhandled = new Map<string, number>();
  private pendingDeltas = new Map<string, { field: string; delta: string }[]>();

  session(id: string): SessionView {
    let s = this.sessions.get(id);
    if (!s) {
      s = {
        id,
        status: { type: "idle" },
        messages: new Map(),
        permissions: new Map(),
        questions: new Map(),
        todos: [],
        diff: [],
        compactedAt: [],
      };
      this.sessions.set(id, s);
    }
    return s;
  }

  private message(sessionID: string, messageID: string): MessageView | undefined {
    return this.session(sessionID).messages.get(messageID);
  }

  apply(ev: Event | { type: string; properties?: unknown }): void {
    const e = ev as Event;
    switch (e.type) {
      case "session.created":
      case "session.updated": {
        const s = this.session(e.properties.info.id);
        s.title = e.properties.info.title;
        return;
      }
      case "session.deleted":
        this.sessions.delete(e.properties.info.id);
        return;
      case "session.status":
        this.session(e.properties.sessionID).status = e.properties.status;
        return;
      case "session.idle":
        this.session(e.properties.sessionID).status = { type: "idle" };
        return;
      case "session.error":
        if (e.properties.sessionID) this.session(e.properties.sessionID).error = e.properties.error;
        return;
      case "session.diff":
        this.session(e.properties.sessionID).diff = e.properties.diff;
        return;
      case "session.compacted":
        this.session(e.properties.sessionID).compactedAt.push(Date.now());
        return;

      case "message.updated": {
        const info = e.properties.info;
        const s = this.session(info.sessionID);
        const cur = s.messages.get(info.id);
        if (cur) cur.info = info;
        else s.messages.set(info.id, { info, parts: new Map() });
        return;
      }
      case "message.removed":
        this.session(e.properties.sessionID).messages.delete(e.properties.messageID);
        return;

      case "message.part.updated": {
        const part = e.properties.part;
        let m = this.message(part.sessionID, part.messageID);
        if (!m) {
          // Part bisa tiba sebelum message.updated: buat kerangka minimal.
          m = { info: { id: part.messageID, sessionID: part.sessionID } as Message, parts: new Map() };
          this.session(part.sessionID).messages.set(part.messageID, m);
        }
        m.parts.set(part.id, structuredClone(part));
        const buffered = this.pendingDeltas.get(part.id);
        if (buffered) {
          this.pendingDeltas.delete(part.id);
          for (const d of buffered) this.appendDelta(m.parts.get(part.id)!, d.field, d.delta);
        }
        return;
      }
      case "message.part.delta": {
        const p = e.properties;
        const part = this.message(p.sessionID, p.messageID)?.parts.get(p.partID);
        if (part) this.appendDelta(part, p.field, p.delta);
        else {
          const list = this.pendingDeltas.get(p.partID) ?? [];
          list.push({ field: p.field, delta: p.delta });
          this.pendingDeltas.set(p.partID, list);
        }
        return;
      }
      case "message.part.removed":
        this.message(e.properties.sessionID, e.properties.messageID)?.parts.delete(e.properties.partID);
        return;

      case "permission.asked":
        this.session(e.properties.sessionID).permissions.set(e.properties.id, { ...e.properties });
        return;
      case "permission.replied": {
        const p = this.session(e.properties.sessionID).permissions.get(e.properties.requestID);
        if (p) p.reply = e.properties.reply;
        return;
      }
      case "question.asked":
        this.session(e.properties.sessionID).questions.set(e.properties.id, { ...e.properties });
        return;
      case "question.replied": {
        const q = this.session(e.properties.sessionID).questions.get(e.properties.requestID);
        if (q) q.answers = e.properties.answers;
        return;
      }
      case "question.rejected": {
        const q = this.session(e.properties.sessionID).questions.get(e.properties.requestID);
        if (q) q.rejected = true;
        return;
      }
      case "todo.updated":
        this.session(e.properties.sessionID).todos = e.properties.todos;
        return;
    }
    if (!IGNORED_FOR_TRANSCRIPT.has(ev.type)) this.unhandled.set(ev.type, (this.unhandled.get(ev.type) ?? 0) + 1);
  }

  private appendDelta(part: Part, field: string, delta: string) {
    const rec = part as unknown as Record<string, unknown>;
    rec[field] = (typeof rec[field] === "string" ? (rec[field] as string) : "") + delta;
  }
}

/** Pesan terurut + part terurut (ID monoton). */
export function orderedMessages(s: SessionView): { info: Message; parts: Part[] }[] {
  return [...s.messages.values()]
    .sort((a, b) => (a.info.id < b.info.id ? -1 : 1))
    .map((m) => ({ info: m.info, parts: [...m.parts.values()].sort((a, b) => (a.id < b.id ? -1 : 1)) }));
}

/** Izin yang terkait dengan tool call tertentu. */
export function permissionsForCall(s: SessionView, callID: string): PermissionView[] {
  return [...s.permissions.values()].filter((p) => p.tool?.callID === callID);
}

/** Izin yang masih menunggu jawaban. */
export function pendingPermissions(s: SessionView): PermissionView[] {
  return [...s.permissions.values()].filter((p) => !p.reply);
}

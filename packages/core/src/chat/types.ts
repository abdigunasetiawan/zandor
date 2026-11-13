// Model data chat (Fase 5): pesan, part, event dari engine. Bentuk di sini adalah hasil normalisasi atas JSON
// engine (diverifikasi terhadap rekaman SSE asli, lihat fixtures/real-turn.ndjson) — bukan tipe SDK mentah,
// sehingga UI tidak bergantung pada skema internal engine dan data tak dikenal tidak merusak tampilan.

export type TokenUsage = { input: number; output: number; reasoning: number; total: number };

export type MessageError = { name: string; message: string };

export type MessageInfo = {
  id: string;
  sessionID: string;
  role: "user" | "assistant";
  time: { created: number; completed?: number };
  parentID?: string;
  agent?: string;
  providerID?: string;
  modelID?: string;
  finish?: string;
  error?: MessageError;
  tokens?: TokenUsage;
  cost?: number;
};

export type ToolState = {
  status: "pending" | "running" | "completed" | "error";
  input: Record<string, unknown>;
  output?: string;
  error?: string;
  title?: string;
  time?: { start: number; end?: number };
};

type PartBase = { id: string; messageID: string; sessionID: string };
export type TextPart = PartBase & { type: "text"; text: string; synthetic: boolean };
export type ReasoningPart = PartBase & { type: "reasoning"; text: string };
export type ToolPart = PartBase & { type: "tool"; tool: string; callID: string; state: ToolState };
export type StepFinishPart = PartBase & { type: "step-finish"; reason: string; cost: number; tokens: TokenUsage };
export type FilePart = PartBase & { type: "file"; mime: string; filename?: string; url: string };
export type OtherPart = PartBase & { type: "other"; rawType: string };
export type MessagePart = TextPart | ReasoningPart | ToolPart | StepFinishPart | FilePart | OtherPart;

export type ChatMessage = { info: MessageInfo; parts: MessagePart[] };

export type PermissionRequest = {
  id: string;
  sessionID: string;
  /** Jenis izin, mis. "edit", "bash", "read", "external_directory". */
  permission: string;
  patterns: string[];
  metadata: Record<string, unknown>;
  always: string[];
  tool?: { messageID: string; callID: string };
};

export type PermissionReply = "once" | "always" | "reject";

export type SessionRunStatus =
  | { type: "idle" }
  | { type: "busy" }
  | { type: "retry"; attempt: number; message: string };

export type EngineEvent =
  | { type: "message.updated"; info: MessageInfo }
  | { type: "message.removed"; sessionID: string; messageID: string }
  | { type: "message.part.updated"; part: MessagePart }
  | { type: "message.part.removed"; sessionID: string; messageID: string; partID: string }
  | { type: "message.part.delta"; sessionID: string; messageID: string; partID: string; field: string; delta: string }
  | { type: "session.status"; sessionID: string; status: SessionRunStatus }
  | { type: "session.error"; sessionID: string | null; error: MessageError }
  | { type: "permission.asked"; request: PermissionRequest }
  | { type: "permission.replied"; sessionID: string; requestID: string; reply: PermissionReply };

// ---------- normalisasi (defensif: input tak tepercaya, bentuk bisa berubah antar versi engine) ----------

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const str = (v: unknown): string | undefined => (typeof v === "string" ? v : undefined);
const num = (v: unknown, d = 0): number => (typeof v === "number" && Number.isFinite(v) ? v : d);

function usage(v: unknown): TokenUsage {
  const o = isObj(v) ? v : {};
  const input = num(o.input);
  const output = num(o.output);
  const reasoning = num(o.reasoning);
  return { input, output, reasoning, total: num(o.total, input + output + reasoning) };
}

export function toMessageError(v: unknown): MessageError | undefined {
  if (!isObj(v)) return undefined;
  const data = isObj(v.data) ? v.data : {};
  const name = str(v.name) ?? "UnknownError";
  const message = str(data.message) ?? str(v.message) ?? name;
  return { name, message };
}

export function toMessageInfo(raw: unknown): MessageInfo | null {
  if (!isObj(raw)) return null;
  const id = str(raw.id);
  const sessionID = str(raw.sessionID);
  const role = raw.role === "user" || raw.role === "assistant" ? raw.role : null;
  if (!id || !sessionID || !role) return null;
  const t = isObj(raw.time) ? raw.time : {};
  const model = isObj(raw.model) ? raw.model : {};
  const info: MessageInfo = {
    id,
    sessionID,
    role,
    time: { created: num(t.created), ...(typeof t.completed === "number" ? { completed: t.completed } : {}) },
  };
  const parentID = str(raw.parentID);
  if (parentID) info.parentID = parentID;
  const agent = str(raw.agent);
  if (agent) info.agent = agent;
  const providerID = str(raw.providerID) ?? str(model.providerID);
  if (providerID) info.providerID = providerID;
  const modelID = str(raw.modelID) ?? str(model.modelID);
  if (modelID) info.modelID = modelID;
  const finish = str(raw.finish);
  if (finish) info.finish = finish;
  const error = toMessageError(raw.error);
  if (error) info.error = error;
  if (isObj(raw.tokens)) info.tokens = usage(raw.tokens);
  if (typeof raw.cost === "number") info.cost = raw.cost;
  return info;
}

function toToolState(raw: unknown): ToolState {
  const o = isObj(raw) ? raw : {};
  const status =
    o.status === "running" || o.status === "completed" || o.status === "error" || o.status === "pending"
      ? o.status
      : "pending";
  const state: ToolState = { status, input: isObj(o.input) ? o.input : {} };
  const output = str(o.output);
  if (output !== undefined) state.output = output;
  const error = str(o.error);
  if (error !== undefined) state.error = error;
  const title = str(o.title);
  if (title !== undefined) state.title = title;
  if (isObj(o.time)) {
    state.time = { start: num(o.time.start), ...(typeof o.time.end === "number" ? { end: o.time.end } : {}) };
  }
  return state;
}

export function toPart(raw: unknown): MessagePart | null {
  if (!isObj(raw)) return null;
  const id = str(raw.id);
  const messageID = str(raw.messageID);
  const sessionID = str(raw.sessionID);
  const type = str(raw.type);
  if (!id || !messageID || !sessionID || !type) return null;
  const base = { id, messageID, sessionID };
  switch (type) {
    case "text":
      return { ...base, type, text: str(raw.text) ?? "", synthetic: raw.synthetic === true };
    case "reasoning":
      return { ...base, type, text: str(raw.text) ?? "" };
    case "tool":
      return {
        ...base,
        type,
        tool: str(raw.tool) ?? "?",
        callID: str(raw.callID) ?? id,
        state: toToolState(raw.state),
      };
    case "step-finish":
      return { ...base, type, reason: str(raw.reason) ?? "", cost: num(raw.cost), tokens: usage(raw.tokens) };
    case "file":
      return {
        ...base,
        type,
        mime: str(raw.mime) ?? "application/octet-stream",
        url: str(raw.url) ?? "",
        ...(str(raw.filename) ? { filename: str(raw.filename) as string } : {}),
      };
    default:
      return { ...base, type: "other", rawType: type };
  }
}

export function toMessage(raw: unknown): ChatMessage | null {
  if (!isObj(raw)) return null;
  const info = toMessageInfo(raw.info);
  if (!info) return null;
  const parts = (Array.isArray(raw.parts) ? raw.parts : []).map(toPart).filter((p): p is MessagePart => p !== null);
  return { info, parts: parts.sort(byId) };
}

export const byId = (a: { id: string }, b: { id: string }) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);

export function toPermission(raw: unknown): PermissionRequest | null {
  if (!isObj(raw)) return null;
  const id = str(raw.id);
  const sessionID = str(raw.sessionID);
  if (!id || !sessionID) return null;
  const strs = (v: unknown) => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);
  const req: PermissionRequest = {
    id,
    sessionID,
    permission: str(raw.permission) ?? "unknown",
    patterns: strs(raw.patterns),
    metadata: isObj(raw.metadata) ? raw.metadata : {},
    always: strs(raw.always),
  };
  if (isObj(raw.tool) && str(raw.tool.messageID) && str(raw.tool.callID)) {
    req.tool = { messageID: raw.tool.messageID as string, callID: raw.tool.callID as string };
  }
  return req;
}

/** Event SSE mentah → EngineEvent. Event yang tidak relevan/tidak dikenal → null (diabaikan). */
export function toEngineEvent(raw: unknown): EngineEvent | null {
  if (!isObj(raw)) return null;
  const type = str(raw.type);
  const p = isObj(raw.properties) ? raw.properties : {};
  switch (type) {
    case "message.updated": {
      const info = toMessageInfo(p.info);
      return info ? { type, info } : null;
    }
    case "message.removed": {
      const sessionID = str(p.sessionID);
      const messageID = str(p.messageID);
      return sessionID && messageID ? { type, sessionID, messageID } : null;
    }
    case "message.part.updated": {
      const part = toPart(p.part);
      return part ? { type, part } : null;
    }
    case "message.part.removed": {
      const sessionID = str(p.sessionID);
      const messageID = str(p.messageID);
      const partID = str(p.partID);
      return sessionID && messageID && partID ? { type, sessionID, messageID, partID } : null;
    }
    case "message.part.delta": {
      const sessionID = str(p.sessionID);
      const messageID = str(p.messageID);
      const partID = str(p.partID);
      const field = str(p.field);
      const delta = str(p.delta);
      return sessionID && messageID && partID && field && delta !== undefined
        ? { type, sessionID, messageID, partID, field, delta }
        : null;
    }
    case "session.status": {
      const sessionID = str(p.sessionID);
      const s = isObj(p.status) ? p.status : {};
      if (!sessionID) return null;
      if (s.type === "busy") return { type, sessionID, status: { type: "busy" } };
      if (s.type === "retry") {
        return { type, sessionID, status: { type: "retry", attempt: num(s.attempt), message: str(s.message) ?? "" } };
      }
      return { type, sessionID, status: { type: "idle" } };
    }
    case "session.idle": {
      const sessionID = str(p.sessionID);
      return sessionID ? { type: "session.status", sessionID, status: { type: "idle" } } : null;
    }
    case "session.error": {
      const error = toMessageError(p.error) ?? { name: "UnknownError", message: "galat tidak diketahui" };
      return { type, sessionID: str(p.sessionID) ?? null, error };
    }
    case "permission.asked": {
      const request = toPermission({ ...p, id: p.id });
      return request ? { type, request } : null;
    }
    case "permission.replied": {
      const sessionID = str(p.sessionID);
      const requestID = str(p.requestID);
      const reply = p.reply === "once" || p.reply === "always" || p.reply === "reject" ? p.reply : null;
      return sessionID && requestID && reply ? { type, sessionID, requestID, reply } : null;
    }
    default:
      return null;
  }
}

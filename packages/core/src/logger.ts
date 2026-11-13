// Logger terstruktur Zandor.
// - Level: debug < info < warn < error.
// - Sink bisa diganti (konsol, ring buffer untuk Log aktivitas Panel 4, file lewat Rust nanti).
// - Data sensitif disamarkan sebelum masuk sink mana pun (pelajaran ADR-022: secret jangan bocor ke log).

export const LEVELS = ["debug", "info", "warn", "error"] as const;
export type Level = (typeof LEVELS)[number];

export type LogEntry = {
  t: number;
  level: Level;
  scope: string;
  msg: string;
  data?: Record<string, unknown>;
};

export type Sink = (entry: LogEntry) => void;

export type Logger = {
  debug(msg: string, data?: Record<string, unknown>): void;
  info(msg: string, data?: Record<string, unknown>): void;
  warn(msg: string, data?: Record<string, unknown>): void;
  error(msg: string, data?: Record<string, unknown>): void;
  child(scope: string): Logger;
};

const SENSITIVE_KEY = /pass(word)?|secret|token|api[-_]?key|authorization|cookie|credential/i;
const SENSITIVE_VALUE = /\b(sk-[A-Za-z0-9_-]{8,}|Bearer\s+[A-Za-z0-9._~+/=-]{8,}|Basic\s+[A-Za-z0-9+/=]{8,})/g;
export const REDACTED = "[disamarkan]";

/** Samarkan kunci/nilai yang tampak seperti secret, rekursif, aman terhadap siklus. */
export function redact(value: unknown, seen = new WeakSet<object>()): unknown {
  if (typeof value === "string") return value.replace(SENSITIVE_VALUE, REDACTED);
  if (value === null || typeof value !== "object") return value;
  if (seen.has(value)) return "[siklus]";
  seen.add(value);
  if (value instanceof Error)
    return { name: value.name, message: redact(value.message, seen), stack: redact(value.stack, seen) };
  if (Array.isArray(value)) return value.map((v) => redact(v, seen));
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(value)) out[k] = SENSITIVE_KEY.test(k) ? REDACTED : redact(v, seen);
  return out;
}

export type LoggerOptions = { level?: Level; sinks: Sink[]; now?: () => number };

export function createLogger(scope: string, options: LoggerOptions): Logger {
  const min = LEVELS.indexOf(options.level ?? "info");
  const now = options.now ?? Date.now;
  const emit = (level: Level, msg: string, data?: Record<string, unknown>) => {
    if (LEVELS.indexOf(level) < min) return;
    const entry: LogEntry = { t: now(), level, scope, msg: redact(msg) as string };
    if (data) entry.data = redact(data) as Record<string, unknown>;
    for (const sink of options.sinks) {
      try {
        sink(entry);
      } catch {
        // Sink yang rusak tidak boleh menjatuhkan pemanggil.
      }
    }
  };
  return {
    debug: (m, d) => emit("debug", m, d),
    info: (m, d) => emit("info", m, d),
    warn: (m, d) => emit("warn", m, d),
    error: (m, d) => emit("error", m, d),
    child: (sub) => createLogger(`${scope}.${sub}`, options),
  };
}

/** Ring buffer untuk Log aktivitas (Panel 4). */
export function createMemorySink(capacity = 1000) {
  const entries: LogEntry[] = [];
  const sink: Sink = (e) => {
    entries.push(e);
    if (entries.length > capacity) entries.splice(0, entries.length - capacity);
  };
  return { sink, entries: () => entries.slice() };
}

export const consoleSink: Sink = (e) => {
  const line = `[${new Date(e.t).toISOString()}] ${e.level.toUpperCase()} ${e.scope}: ${e.msg}`;
  if (e.level === "error") console.error(line, e.data ?? "");
  else console.warn(line, e.data ?? "");
};

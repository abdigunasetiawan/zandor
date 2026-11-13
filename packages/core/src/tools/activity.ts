// Log aktivitas Panel 4: event agent (tool call, izin, galat) dan event aplikasi (engine, git, terminal, logger).
// Ring buffer terbatas; data sensitif sudah disamarkan di logger (redact) sebelum masuk sini.
import type { EngineEvent } from "../chat/types";
import type { LogEntry } from "../logger";

export type ActivitySource = "app" | "engine" | "chat" | "tool" | "git" | "terminal";
export type ActivityLevel = "info" | "warn" | "error";
export type ActivityEntry = { id: number; at: number; source: ActivitySource; level: ActivityLevel; message: string };

export class ActivityLog {
  private entries: ActivityEntry[] = [];
  private listeners = new Set<() => void>();
  private nextId = 1;
  private snapshot: readonly ActivityEntry[] = [];
  private lastTool = new Map<string, string>();

  constructor(
    private readonly capacity = 500,
    private readonly now: () => number = Date.now,
  ) {}

  list(): readonly ActivityEntry[] {
    return this.snapshot;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  add(source: ActivitySource, level: ActivityLevel, message: string) {
    const entry: ActivityEntry = { id: this.nextId++, at: this.now(), source, level, message: message.slice(0, 500) };
    this.entries.push(entry);
    if (this.entries.length > this.capacity) this.entries.splice(0, this.entries.length - this.capacity);
    this.snapshot = [...this.entries];
    for (const l of this.listeners) l();
  }

  clear() {
    if (this.entries.length === 0) return;
    this.entries = [];
    this.snapshot = [];
    this.lastTool.clear();
    for (const l of this.listeners) l();
  }

  /** Sink untuk Logger: warn/error aplikasi masuk log aktivitas. */
  sink = (e: LogEntry) => {
    if (e.level === "debug" || e.level === "info") return;
    this.add("app", e.level, `${e.scope}: ${e.msg}`);
  };

  /** Catat event engine yang relevan (tool call, izin, galat sesi). Event streaming (delta) diabaikan. */
  fromEngineEvent(ev: EngineEvent) {
    switch (ev.type) {
      case "message.part.updated": {
        const p = ev.part;
        if (p.type !== "tool") return;
        const key = `${p.id}:${p.state.status}`;
        if (this.lastTool.get(p.id) === key) return;
        this.lastTool.set(p.id, key);
        if (p.state.status === "running") this.add("tool", "info", `${p.tool} dimulai`);
        else if (p.state.status === "completed")
          this.add("tool", "info", `${p.tool} selesai${p.state.title ? `: ${p.state.title}` : ""}`);
        else if (p.state.status === "error") this.add("tool", "error", `${p.tool} gagal: ${p.state.error ?? "?"}`);
        return;
      }
      case "permission.asked":
        this.add("chat", "warn", `izin diminta: ${ev.request.permission} ${ev.request.patterns.join(", ")}`.trim());
        return;
      case "permission.replied":
        this.add("chat", "info", `izin dijawab: ${ev.reply}`);
        return;
      case "session.error":
        this.add("chat", "error", `galat sesi: ${ev.error.message}`);
        return;
      case "session.status":
        if (ev.status.type === "retry")
          this.add("chat", "warn", `mencoba ulang (percobaan ${ev.status.attempt}): ${ev.status.message}`);
        return;
      default:
        return;
    }
  }
}

// Adapter background job (ADR-072). Di Tauri: perintah Rust yang membaca `<app_data>/jobs` (ditulis plugin Zandor di
// engine) dan menulis berkas kontrol. Di luar Tauri tidak ada engine nyata → `null` (UI menjelaskan, tanpa data palsu).
export type JobStatus = "starting" | "running" | "exited" | "failed" | "stopped" | "lost";

export type JobInfo = {
  id: string;
  name: string;
  command: string;
  cwd: string;
  sessionID: string;
  directory: string;
  pid: number | null;
  status: JobStatus;
  exitCode: number | null;
  ports: number[];
  startedAt: number;
  endedAt: number | null;
  restarts: number;
};

export type JobAction = "stop" | "restart" | "remove";

export interface JobsHost {
  list(): Promise<JobInfo[]>;
  log(id: string): Promise<string>;
  control(id: string, action: JobAction): Promise<void>;
}

export const isLive = (s: JobStatus) => s === "running" || s === "starting";

const STATUSES: JobStatus[] = ["starting", "running", "exited", "failed", "stopped", "lost"];

/** Validasi meta mentah dari berkas (ditulis proses lain → tidak dipercaya). */
export function toJob(raw: unknown): JobInfo | null {
  if (typeof raw !== "object" || raw === null) return null;
  const r = raw as Record<string, unknown>;
  if (typeof r.id !== "string" || typeof r.command !== "string") return null;
  const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : null);
  return {
    id: r.id,
    name: typeof r.name === "string" && r.name ? r.name : r.command,
    command: r.command,
    cwd: typeof r.cwd === "string" ? r.cwd : "",
    sessionID: typeof r.sessionID === "string" ? r.sessionID : "",
    directory: typeof r.directory === "string" ? r.directory : "",
    pid: num(r.pid),
    status: STATUSES.includes(r.status as JobStatus) ? (r.status as JobStatus) : "lost",
    exitCode: num(r.exitCode),
    ports: Array.isArray(r.ports) ? r.ports.filter((p): p is number => typeof p === "number").slice(0, 8) : [],
    startedAt: num(r.startedAt) ?? 0,
    endedAt: num(r.endedAt),
    restarts: num(r.restarts) ?? 0,
  };
}

export async function loadJobsHost(): Promise<JobsHost | null> {
  const { isTauri, invoke } = await import("@tauri-apps/api/core");
  if (!isTauri()) return null;
  return {
    async list() {
      const raw = await invoke<unknown[]>("jobs_list");
      return raw.map(toJob).filter((j): j is JobInfo => j !== null);
    },
    log: (id) => invoke<string>("job_log", { id }),
    async control(id, action) {
      await invoke("job_control", { id, action });
      // Job yang tidak dikelola engine saat ini (mis. `lost`) tidak akan diproses plugin: hapus berkasnya langsung.
      if (action === "remove") await invoke("job_forget", { id });
    },
  };
}

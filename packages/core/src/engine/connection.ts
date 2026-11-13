// Status koneksi ke engine. Sumbernya snapshot dari supervisor (Tauri) atau konfigurasi remote; di sini hanya
// penyimpan status yang bisa diamati, dengan snapshot stabil untuk useSyncExternalStore.
import type { EngineConnectionInfo } from "./api";

export type EngineStatus = "stopped" | "starting" | "ready" | "restarting" | "error";

export type EngineState = {
  status: EngineStatus;
  info: EngineConnectionInfo | null;
  version: string | null;
  error: string | null;
  restarts: number;
  /** Asal engine: proses lokal yang disupervisi, atau URL remote/override. */
  source: "local" | "remote" | null;
};

export const INITIAL_ENGINE_STATE: EngineState = {
  status: "stopped",
  info: null,
  version: null,
  error: null,
  restarts: 0,
  source: null,
};

/** Bentuk snapshot dari Rust (`EngineSnapshot`, field snake_case dari serde). */
export type RawEngineSnapshot = {
  status: EngineStatus;
  url: string;
  username: string;
  password: string;
  version: string;
  restarts: number;
  error: string | null;
};

export function fromSnapshot(raw: RawEngineSnapshot): EngineState {
  const ready = raw.status === "ready" && raw.url !== "";
  return {
    status: raw.status,
    info: ready ? { url: raw.url, username: raw.username, password: raw.password } : null,
    version: raw.version || null,
    error: raw.error,
    restarts: raw.restarts,
    source: raw.status === "stopped" ? null : "local",
  };
}

export class ConnectionStore {
  private snap: EngineState = INITIAL_ENGINE_STATE;
  private listeners = new Set<() => void>();

  state(): EngineState {
    return this.snap;
  }

  /** Ganti status. Tidak memicu notifikasi bila isinya sama (mencegah render ulang sia-sia). */
  set(next: EngineState): void {
    const a = this.snap;
    const same =
      a.status === next.status &&
      a.version === next.version &&
      a.error === next.error &&
      a.restarts === next.restarts &&
      a.source === next.source &&
      a.info?.url === next.info?.url &&
      a.info?.password === next.info?.password;
    if (same) return;
    this.snap = next;
    for (const l of this.listeners) l();
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
}

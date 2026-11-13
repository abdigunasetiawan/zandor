// Adapter ke host (Tauri). Di luar Tauri (browser/e2e) tidak ada supervisor: engine bisa diarahkan ke URL lewat
// localStorage `zandor.engine.override` ({url,username,password}) dan registry disimpan di localStorage.
import type { RawEngineSnapshot } from "@zandor/core";

export type EngineHost = {
  /** true bila berjalan di Tauri (ada supervisor, folder picker native, registry di disk). */
  native: boolean;
  snapshot(): Promise<RawEngineSnapshot>;
  onChange(cb: (s: RawEngineSnapshot) => void): Promise<() => void>;
  restart(): Promise<void>;
  chatsDir(): Promise<string | null>;
  registryLoad(): Promise<string | null>;
  registrySave(text: string): Promise<void>;
  /** Dialog pilih folder; null bila dibatalkan atau tidak tersedia. */
  pickFolder(): Promise<string | null>;
};

const OVERRIDE_KEY = "zandor.engine.override";
const REGISTRY_KEY = "zandor.projects";
const CHATS_KEY = "zandor.chatsDir";

const stopped = (error: string | null = null): RawEngineSnapshot => ({
  status: error ? "error" : "stopped",
  url: "",
  username: "opencode",
  password: "",
  version: "",
  restarts: 0,
  error,
});

export function createBrowserHost(): EngineHost {
  const read = (k: string) => {
    try {
      return localStorage.getItem(k);
    } catch {
      return null;
    }
  };
  return {
    native: false,
    async snapshot() {
      const raw = read(OVERRIDE_KEY);
      if (!raw) return stopped();
      try {
        const o = JSON.parse(raw) as { url?: string; username?: string; password?: string };
        if (!o.url) return stopped("override engine tanpa url");
        return {
          status: "ready",
          url: o.url,
          username: o.username ?? "opencode",
          password: o.password ?? "",
          version: "override",
          restarts: 0,
          error: null,
        };
      } catch {
        return stopped("override engine bukan JSON valid");
      }
    },
    async onChange() {
      return () => {};
    },
    async restart() {},
    async chatsDir() {
      return read(CHATS_KEY);
    },
    async registryLoad() {
      return read(REGISTRY_KEY);
    },
    async registrySave(text) {
      localStorage.setItem(REGISTRY_KEY, text);
    },
    async pickFolder() {
      return null;
    },
  };
}

export async function loadEngineHost(): Promise<EngineHost> {
  const { isTauri, invoke } = await import("@tauri-apps/api/core");
  if (!isTauri()) return createBrowserHost();
  const { listen } = await import("@tauri-apps/api/event");
  return {
    native: true,
    snapshot: () => invoke<RawEngineSnapshot>("engine_state"),
    onChange: (cb) => listen<RawEngineSnapshot>("engine-state", (e) => cb(e.payload)),
    restart: () => invoke("engine_restart"),
    chatsDir: () => invoke<string>("chats_dir"),
    registryLoad: () => invoke<string | null>("registry_load"),
    registrySave: (text) => invoke("registry_save", { text }),
    async pickFolder() {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({ directory: true, multiple: false });
      return typeof r === "string" ? r : null;
    },
  };
}

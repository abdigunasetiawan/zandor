// Adapter ke host (Tauri). Di luar Tauri (browser/e2e) tidak ada supervisor: engine bisa diarahkan ke URL lewat
// localStorage `zandor.engine.override` ({url,username,password}) dan registry disimpan di localStorage.
import { attachmentFromPath, type FileAttachment, MAX_INLINE_BYTES, type RawEngineSnapshot } from "@zandor/core";

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
  /** Pilih lampiran: berkas (beberapa) atau satu folder. Berkas oleh dialog native; di browser lewat <input>. */
  pickAttachments(kind: "files" | "folder"): Promise<{ items: FileAttachment[]; skipped: string[] } | null>;
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
    pickAttachments(kind) {
      if (kind === "folder") return Promise.resolve(null); // folder hanya lewat dialog native
      return new Promise((resolve) => {
        const input = document.createElement("input");
        input.type = "file";
        input.multiple = true;
        input.onchange = async () => {
          const items: FileAttachment[] = [];
          const skipped: string[] = [];
          for (const f of Array.from(input.files ?? [])) {
            if (f.size > MAX_INLINE_BYTES) {
              skipped.push(f.name);
              continue;
            }
            const url = await new Promise<string>((res, rej) => {
              const r = new FileReader();
              r.onload = () => res(String(r.result));
              r.onerror = () => rej(r.error);
              r.readAsDataURL(f);
            });
            items.push({ mime: f.type || "application/octet-stream", url, filename: f.name });
          }
          resolve({ items, skipped });
        };
        input.oncancel = () => resolve(null);
        input.click();
      });
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
    async pickAttachments(kind) {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open(
        kind === "folder" ? { directory: true, multiple: false } : { directory: false, multiple: true },
      );
      if (!r) return null;
      const paths = Array.isArray(r) ? r : [r];
      return { items: paths.map((p) => attachmentFromPath(p, kind === "folder")), skipped: [] };
    },
  };
}

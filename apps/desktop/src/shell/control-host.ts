// Adapter kontrol agent + generate gambar (ADR-077). Di Tauri: perintah Rust yang membaca `<app_data>/control`
// (ditulis plugin Zandor di engine) dan memanggil API gambar provider dengan kunci dari keychain. Di luar Tauri tidak
// ada engine nyata/kunci → `null` (UI menjelaskan, tanpa hasil palsu).
import type { ControlRequest, ControlResponse, ImageSize } from "./agent-control";

export interface ControlHost {
  take(): Promise<ControlRequest[]>;
  reply(id: string, response: ControlResponse): Promise<void>;
  generateImage(input: {
    providerID: string;
    modelID: string;
    prompt: string;
    size: ImageSize;
    root: string;
    rel: string;
  }): Promise<string>;
}

/** Validasi permintaan mentah dari berkas (ditulis proses lain → tidak dipercaya). */
export function toRequest(raw: unknown): ControlRequest | null {
  if (typeof raw !== "object" || raw === null) return null;
  const r = raw as Record<string, unknown>;
  if (typeof r.id !== "string" || typeof r.action !== "string") return null;
  const args =
    typeof r.args === "object" && r.args !== null && !Array.isArray(r.args) ? (r.args as Record<string, unknown>) : {};
  return {
    id: r.id,
    action: r.action,
    args,
    sessionID: typeof r.sessionID === "string" ? r.sessionID : "",
    directory: typeof r.directory === "string" ? r.directory : "",
  };
}

export async function loadControlHost(): Promise<ControlHost | null> {
  const { isTauri, invoke } = await import("@tauri-apps/api/core");
  if (!isTauri()) return null;
  return {
    async take() {
      const raw = await invoke<unknown[]>("control_take");
      return raw.map(toRequest).filter((r): r is ControlRequest => r !== null);
    },
    reply: (id, response) => invoke("control_reply", { id, response }),
    generateImage: (i) =>
      invoke<string>("image_generate", {
        providerId: i.providerID,
        model: i.modelID,
        prompt: i.prompt,
        size: i.size,
        root: i.root,
        rel: i.rel,
      }),
  };
}

// Kunci project untuk memori: sha256 dari path yang dinormalisasi (huruf kecil, `/`, tanpa `/` akhir), 16 heks.
// Plugin (Node/Bun) dan UI (Web Crypto) harus menghasilkan nilai yang sama persis.
import { createHash } from "node:crypto";

export function normalizeProjectPath(path: string): string {
  const p = path.replace(/\\/g, "/").replace(/^\/\/\?\//, "");
  return (p.length > 1 ? p.replace(/\/+$/, "") : p).toLowerCase();
}

export function projectKey(path: string): string {
  return createHash("sha256").update(normalizeProjectPath(path)).digest("hex").slice(0, 16);
}

/** Nama berkas aman untuk id sesi (`ses_abc` → `ses_abc`). */
export function safeId(id: string): string {
  return id.replace(/[^\w-]/g, "_");
}

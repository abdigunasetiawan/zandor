// Engine remote: opencode yang sudah berjalan di mesin lain (VPS, komputer lain), dihubungi lewat URL + Basic Auth.
// Pilihan engine melekat pada project; semua panggilan EngineApi yang membawa `directory` diarahkan ke engine milik
// project itu (router), sehingga ChatStore/ProjectStore tidak perlu tahu ada lebih dari satu engine. Panggilan tanpa
// direktori (model, katalog provider, login) selalu ke engine lokal. Zandor tidak memasang atau menjalankan engine remote.
import type { PtyClient } from "../terminal/pty";
import type { EngineApi } from "./api";

export type RemoteEngine = {
  id: string;
  /** Nama tampilan, mis. "VPS kantor". */
  name: string;
  /** URL dasar server opencode, mis. https://engine.contoh.id atau http://127.0.0.1:4096 (tunnel SSH). */
  url: string;
  username: string;
  password: string;
};

export type RemoteStatus = {
  status: "checking" | "ready" | "error";
  version: string | null;
  error: string | null;
};

const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]", "::1"]);

/**
 * Masalah URL engine remote, atau null bila boleh dipakai. Kata sandi dikirim lewat Basic Auth, jadi URL ke mesin lain
 * wajib HTTPS; HTTP hanya untuk loopback (mis. tunnel `ssh -L 4096:127.0.0.1:4096 vps`).
 */
export function remoteUrlProblem(url: string): "invalid" | "insecure" | null {
  let u: URL;
  try {
    u = new URL(url.trim());
  } catch {
    return "invalid";
  }
  if (u.username || u.password || u.search || u.hash) return "invalid";
  if (u.protocol === "https:") return null;
  if (u.protocol === "http:") return LOOPBACK.has(u.hostname) ? null : "insecure";
  return "invalid";
}

/** URL dirapikan: tanpa garis miring di akhir. */
export const normalizeRemoteUrl = (url: string) => url.trim().replace(/\/+$/, "");

const PTY_BY_DIR = ["list", "create", "resize", "remove", "connect"] as const;
const API_BY_DIR = [
  "listSessions",
  "createSession",
  "renameSession",
  "deleteSession",
  "listFiles",
  "readFile",
  "listMessages",
  "listPermissions",
  "listQuestions",
  "listTodos",
  "replyQuestion",
  "rejectQuestion",
  "sendPrompt",
  "sessionDiff",
  "getRevert",
  "revert",
  "unrevert",
  "fork",
  "findFiles",
  "listCommands",
  "runCommand",
  "summarize",
  "abort",
  "disposeInstance",
  "replyPermission",
  "subscribe",
] as const satisfies ReadonlyArray<keyof EngineApi>;
const API_LOCAL = [
  "health",
  "listModels",
  "listCatalog",
  "listAuthMethods",
  "oauthAuthorize",
  "oauthCallback",
] as const satisfies ReadonlyArray<keyof EngineApi>;

// Semua metode EngineApi harus tercakup salah satu daftar (dicek tipe): metode baru tidak bisa lupa dirutekan.
type Covered = (typeof API_BY_DIR)[number] | (typeof API_LOCAL)[number] | "pty";
const _exhaustive: Exclude<keyof EngineApi, Covered> extends never ? true : never = true;
void _exhaustive;

export class EngineUnavailableError extends Error {}

export type RouteTarget = { kind: "local" } | { kind: "remote"; id: string };

/**
 * EngineApi gabungan. `route(dir)` menentukan engine untuk direktori; engine yang tidak tersedia (lokal belum siap,
 * remote dihapus) menghasilkan galat yang jelas, bukan diam-diam ke engine lain.
 */
export function createRoutedApi(opts: {
  local: EngineApi | null;
  remote: (id: string) => EngineApi | null;
  route: (directory: string) => RouteTarget;
  remoteName?: (id: string) => string;
}): EngineApi {
  const pick = (target: RouteTarget): EngineApi => {
    if (target.kind === "local") {
      if (!opts.local) throw new EngineUnavailableError("engine lokal belum siap");
      return opts.local;
    }
    const api = opts.remote(target.id);
    if (!api)
      throw new EngineUnavailableError(`engine remote "${opts.remoteName?.(target.id) ?? target.id}" tidak ada`);
    return api;
  };
  const forDir = (directory: string) => pick(opts.route(directory));
  // Semua metode (kecuali subscribe) mengembalikan Promise: engine yang tak tersedia menjadi Promise yang ditolak,
  // bukan exception sinkron, agar pola `api.x().catch(...)` tetap menangkapnya.
  const lazy =
    (resolve: (args: unknown[]) => (...a: unknown[]) => unknown) =>
    (...args: unknown[]) => {
      try {
        return resolve(args)(...args);
      } catch (e) {
        return Promise.reject(e);
      }
    };
  const out: Record<string, unknown> = {};
  for (const name of API_BY_DIR) {
    out[name] = lazy((a) => {
      const api = forDir(a[0] as string);
      return (api[name] as (...x: unknown[]) => unknown).bind(api);
    });
  }
  // `subscribe` sinkron dan dipanggil tanpa try oleh store: engine yang tak tersedia = tanpa aliran (disambung lagi
  // saat klien gabungan dibangun ulang), bukan exception.
  out.subscribe = (directory: string, handlers: Parameters<EngineApi["subscribe"]>[1]) => {
    try {
      return forDir(directory).subscribe(directory, handlers);
    } catch {
      return () => {};
    }
  };
  for (const name of API_LOCAL) {
    out[name] = lazy(() => {
      const api = pick({ kind: "local" });
      return (api[name] as (...x: unknown[]) => unknown).bind(api);
    });
  }
  const pty: Record<string, unknown> = {
    shells: lazy(() => {
      const c = pick({ kind: "local" }).pty;
      return c.shells.bind(c);
    }),
  };
  for (const name of PTY_BY_DIR) {
    pty[name] = lazy((a) => {
      const c = forDir(a[0] as string).pty;
      return (c[name] as (...x: unknown[]) => unknown).bind(c);
    });
  }
  out.pty = pty as unknown as PtyClient;
  return out as unknown as EngineApi;
}

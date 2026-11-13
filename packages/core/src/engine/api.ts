// Antarmuka engine yang dibutuhkan UI (Fase 4). Implementasi nyata membungkus SDK resmi @opencode-ai/sdk
// (tipe sama persis dengan engine yang di-pin, ADR-021); test memakai implementasi palsu.
import { createOpencodeClient } from "@opencode-ai/sdk/v2/client";

export type EngineConnectionInfo = { url: string; username: string; password: string };

export type SessionInfo = {
  id: string;
  title: string;
  directory: string;
  time: { created: number; updated: number };
};

export type FileNode = {
  name: string;
  /** Relatif terhadap direktori project. */
  path: string;
  absolute: string;
  type: "file" | "directory";
  ignored: boolean;
};

export interface EngineApi {
  health(): Promise<{ version: string }>;
  listSessions(directory: string): Promise<SessionInfo[]>;
  createSession(directory: string, title?: string): Promise<SessionInfo>;
  renameSession(directory: string, sessionId: string, title: string): Promise<SessionInfo>;
  deleteSession(directory: string, sessionId: string): Promise<void>;
  listFiles(directory: string, path: string): Promise<FileNode[]>;
}

/** Judul bawaan engine untuk sesi baru ("New session - 2026-…") berarti "belum diberi nama". */
export const isDefaultSessionTitle = (title: string) => /^New session - \d{4}-\d{2}-\d{2}T/.test(title);

export function basicAuthHeader(info: Pick<EngineConnectionInfo, "username" | "password">): string {
  const bytes = new TextEncoder().encode(`${info.username}:${info.password}`);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return `Basic ${btoa(bin)}`;
}

/** Respons sukses wajib berisi body; body kosong berarti engine/protokol tidak sesuai harapan. */
function body<T>(r: { data?: T }, what: string): T {
  if (r.data === undefined) throw new Error(`engine mengembalikan respons kosong untuk ${what}`);
  return r.data;
}

const toSession = (s: {
  id: string;
  title: string;
  directory: string;
  time: { created: number; updated: number };
}): SessionInfo => ({
  id: s.id,
  title: s.title,
  directory: s.directory,
  time: { created: s.time.created, updated: s.time.updated },
});

export function createEngineApi(info: EngineConnectionInfo, fetchImpl?: typeof fetch): EngineApi {
  const client = createOpencodeClient({
    baseUrl: info.url,
    headers: { Authorization: basicAuthHeader(info) },
    throwOnError: true,
    ...(fetchImpl ? { fetch: fetchImpl as never } : {}),
  });
  return {
    async health() {
      const r = await client.global.health();
      return { version: body(r, "global.health").version };
    },
    async listSessions(directory) {
      const r = await client.session.list({ directory });
      return body(r, "session.list").map(toSession);
    },
    async createSession(directory, title) {
      const r = await client.session.create({ directory, ...(title ? { title } : {}) });
      return toSession(body(r, "session.create"));
    },
    async renameSession(directory, sessionId, title) {
      const r = await client.session.update({ sessionID: sessionId, directory, title });
      return toSession(body(r, "session.update"));
    },
    async deleteSession(directory, sessionId) {
      await client.session.delete({ sessionID: sessionId, directory });
    },
    async listFiles(directory, path) {
      const r = await client.file.list({ directory, path });
      return body(r, "file.list").map((n) => ({
        name: n.name,
        path: n.path,
        absolute: n.absolute,
        type: n.type,
        ignored: n.ignored,
      }));
    },
  };
}

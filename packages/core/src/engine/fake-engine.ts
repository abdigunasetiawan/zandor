// Engine palsu di memori untuk test (core dan UI). Meniru perilaku yang terbukti pada engine asli:
// sesi terikat direktori, judul bawaan "New session - <ISO>", daftar berkas menurut path relatif.
import type { EngineApi, FileNode, SessionInfo } from "./api";

export type FakeEngine = EngineApi & {
  sessions: Map<string, SessionInfo[]>;
  files: Map<string, Record<string, FileNode[]>>;
  calls: string[];
  /** Jika diisi, semua panggilan berikutnya menolak dengan pesan ini. */
  failWith: string | null;
  tick: number;
};

export function createFakeEngine(): FakeEngine {
  let n = 0;
  const e: FakeEngine = {
    sessions: new Map(),
    files: new Map(),
    calls: [],
    failWith: null,
    tick: 1_000,
    async health() {
      e.calls.push("health");
      if (e.failWith) throw new Error(e.failWith);
      return { version: "1.19.5" };
    },
    async listSessions(dir) {
      e.calls.push(`list:${dir}`);
      if (e.failWith) throw new Error(e.failWith);
      return [...(e.sessions.get(dir) ?? [])];
    },
    async createSession(dir, title) {
      e.calls.push(`create:${dir}`);
      if (e.failWith) throw new Error(e.failWith);
      const t = ++e.tick;
      const s: SessionInfo = {
        id: `ses_${++n}`,
        title: title ?? `New session - ${new Date(t).toISOString()}`,
        directory: dir,
        time: { created: t, updated: t },
      };
      e.sessions.set(dir, [...(e.sessions.get(dir) ?? []), s]);
      return s;
    },
    async renameSession(dir, id, title) {
      e.calls.push(`rename:${id}`);
      if (e.failWith) throw new Error(e.failWith);
      const list = e.sessions.get(dir) ?? [];
      const i = list.findIndex((s) => s.id === id);
      if (i === -1) throw new Error("sesi tidak ada");
      const prev = list[i] as SessionInfo;
      const updated = { ...prev, title, time: { ...prev.time, updated: ++e.tick } };
      list[i] = updated;
      return updated;
    },
    async deleteSession(dir, id) {
      e.calls.push(`delete:${id}`);
      if (e.failWith) throw new Error(e.failWith);
      e.sessions.set(
        dir,
        (e.sessions.get(dir) ?? []).filter((s) => s.id !== id),
      );
    },
    async listFiles(dir, path) {
      e.calls.push(`files:${dir}:${path}`);
      if (e.failWith) throw new Error(e.failWith);
      return e.files.get(dir)?.[path] ?? [];
    },
  };
  return e;
}

// Sesi yang sedang dipilih untuk panel alat khusus sesi (Info, Perubahan, Berkas, Aktivitas): sesi nyata (dengan state chat dari
// store) atau sesi demo bawaan (state statis berlabel). Null bila tidak ada sesi terpilih.
import { type SessionChat, sessionTitle } from "@zandor/core";
import { useMemo } from "react";
import { DEMO_KEY, demoById } from "../demo-sessions";
import { useChat, useProjects, useServices } from "../services";

export type SelectedSession = {
  id: string;
  title: string | null;
  /** Folder kerja sesi ("" untuk demo). */
  directory: string;
  projectName: string | null;
  projectPath: string | null;
  session: SessionChat;
  demo: boolean;
};

export function useSelectedSession(): SelectedSession | null {
  const { projects } = useServices();
  const state = useProjects();
  const chat = useChat();
  const { key, sessionId } = state.selection;
  const demoDef = key === DEMO_KEY ? demoById(sessionId) : undefined;
  const demoSession = useMemo(() => demoDef?.build() ?? null, [demoDef]);
  if (demoDef && demoSession) {
    return {
      id: demoDef.id,
      title: null,
      directory: "",
      projectName: null,
      projectPath: null,
      session: demoSession,
      demo: true,
    };
  }
  const sel = sessionId ? projects.selected() : null;
  const session = sel ? chat.sessions[sel.session.id] : undefined;
  if (!sel || !session) return null;
  return {
    id: sel.session.id,
    title: sessionTitle(sel.session),
    directory: sel.session.directory,
    projectName: sel.project?.name ?? null,
    projectPath: sel.project?.path ?? null,
    session,
    demo: false,
  };
}

/** Path relatif terhadap folder sesi bila berada di dalamnya (tampilan ringkas); selain itu apa adanya. */
export function relativeTo(dir: string, path: string): string {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
  const d = norm(dir);
  const p = norm(path);
  return d && p.toLowerCase().startsWith(`${d.toLowerCase()}/`) ? p.slice(d.length + 1) : p;
}

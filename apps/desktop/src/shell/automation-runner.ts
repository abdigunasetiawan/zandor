// Pelaksana tugas terjadwal (Fase 11): membuat sesi baru (tanpa merebut fokus pengguna), mengirim prompt, lalu memantau
// sesi sampai selesai atau butuh izin dan memberi tahu pengguna. Tugas TIDAK menyetujui izin otomatis.
import { type AutomationRunner, CHAT_KEY, type ChatStore, type ProjectStore } from "@zandor/core";
import type { NotifyHost } from "./notify-host";

export type RunnerMessages = {
  started(name: string): string;
  finished(name: string): string;
  needsPermission(name: string): string;
  failed(name: string, error: string): string;
  title(name: string): string;
};

export type RunnerDeps = {
  projects: ProjectStore;
  chat: ChatStore;
  notify: NotifyHost;
  /** Pesan in-app (toast) selain notifikasi sistem. */
  toast: (text: string, tone: "success" | "warning" | "danger") => void;
  msg: () => RunnerMessages;
};

const WATCH_TIMEOUT_MS = 3 * 60 * 60 * 1000;

export function createAutomationRunner(deps: RunnerDeps): AutomationRunner {
  const { projects, chat } = deps;

  const tell = (title: string, body: string, tone: "success" | "warning" | "danger") => {
    deps.toast(`${title}: ${body}`, tone);
    void deps.notify.notify(title, body).catch(() => {});
  };

  function watch(sessionId: string, name: string) {
    let sawBusy = false;
    let warned = false;
    const stop = (off: () => void, timer: ReturnType<typeof setTimeout>) => {
      off();
      clearTimeout(timer);
    };
    const check = (off: () => void, timer: ReturnType<typeof setTimeout>) => {
      const s = chat.state().sessions[sessionId];
      if (!s) return stop(off, timer);
      if (s.status.type !== "idle" || s.sending) sawBusy = true;
      if (!warned && s.permissions.length > 0) {
        warned = true;
        tell(deps.msg().title(name), deps.msg().needsPermission(name), "warning");
      }
      if (sawBusy && s.status.type === "idle" && !s.sending && s.permissions.length === 0) {
        tell(deps.msg().title(name), deps.msg().finished(name), "success");
        stop(off, timer);
        // lepas dari memori bila pengguna tidak sedang melihat sesi ini
        if (projects.state().selection.sessionId !== sessionId) chat.close(sessionId);
      }
    };
    let off: () => void = () => {};
    const timer = setTimeout(() => stop(off, timer), WATCH_TIMEOUT_MS);
    off = chat.subscribe(() => check(off, timer));
    check(off, timer);
  }

  return {
    ready() {
      const c = chat.state();
      return projects.state().engineReady && c.modelsStatus === "ready" && c.model !== null;
    },
    async run(task) {
      const key = task.projectId ?? CHAT_KEY;
      if (task.projectId && !projects.state().projects.some((p) => p.id === task.projectId))
        throw new Error("project tidak ditemukan (sudah dilepas dari daftar?)");
      const m = deps.msg();
      try {
        const session = await projects.createSession(key, m.title(task.name), { select: false });
        chat.open(session.directory, session.id);
        await chat.send(session.id, task.prompt);
        tell(m.title(task.name), m.started(task.name), "success");
        watch(session.id, task.name);
        return { sessionId: session.id };
      } catch (e) {
        const error = e instanceof Error ? e.message : String(e);
        tell(m.title(task.name), m.failed(task.name, error), "danger");
        throw e;
      }
    },
  };
}

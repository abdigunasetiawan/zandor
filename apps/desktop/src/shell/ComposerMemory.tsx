// Toggle memori dan skill creator di composer. Sesi dalam project berbagi pengaturan project (satu sakelar untuk semua
// sesinya); sesi tanpa project (Chat) diatur per sesi. Di halaman sesi-baru (draft) pilihan Chat ditahan dulu lalu
// diterapkan saat sesi dibuat (ADR-060).
import { memoryEnabled, projectKey, skillCreatorEnabled } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { BookmarkIcon, BrainIcon, IconButton, useToast } from "@zandor/ui";
import { useEffect, useState } from "react";
import { useMemory, useServices } from "./services";

export type PendingMemory = { memory?: boolean; skill?: boolean };

export function ComposerMemory({
  sessionId,
  projectPath,
  pending,
  onPending,
}: {
  /** null = halaman sesi-baru (sesi belum dibuat). */
  sessionId: string | null;
  projectPath: string | null;
  pending: PendingMemory;
  onPending: (p: PendingMemory) => void;
}) {
  const { t } = useI18n();
  const { memory } = useServices();
  const state = useMemory();
  const toast = useToast();
  const [key, setKey] = useState<string | null>(null);
  useEffect(() => {
    void memory.loadSettings();
    let live = true;
    setKey(null);
    if (projectPath) void projectKey(projectPath).then((k) => live && setKey(k));
    return () => {
      live = false;
    };
  }, [memory, projectPath]);

  const s = state.settings;
  const waiting = projectPath !== null && key === null;
  const inProject = key !== null;
  const value = (kind: "memory" | "skill"): boolean => {
    if (!sessionId && !inProject && pending[kind] !== undefined) return pending[kind] as boolean;
    return kind === "memory"
      ? memoryEnabled(s, key, sessionId ?? undefined)
      : skillCreatorEnabled(s, key, sessionId ?? undefined);
  };
  const toggle = (kind: "memory" | "skill") => {
    const on = !value(kind);
    if (!sessionId && !inProject) return onPending({ ...pending, [kind]: on });
    void memory
      .setScoped(kind, on, sessionId ?? "", key)
      .catch((e) => toast.show({ title: String(e), tone: "danger" }));
  };
  const scope = t(inProject ? "memtoggle.scopeProject" : "memtoggle.scopeSession");
  const item = (kind: "memory" | "skill") => {
    const on = value(kind);
    return (
      <IconButton
        label={t(kind === "memory" ? "memtoggle.memory" : "memtoggle.skill")}
        title={`${t(kind === "memory" ? "memtoggle.memory" : "memtoggle.skill")} — ${t(on ? "memtoggle.on" : "memtoggle.off")}. ${scope}`}
        size="sm"
        icon={kind === "memory" ? <BrainIcon /> : <BookmarkIcon />}
        aria-pressed={on}
        className="composer__toggle"
        disabled={waiting}
        onClick={() => toggle(kind)}
      />
    );
  };
  return (
    <div className="composer__toggles">
      {item("memory")}
      {item("skill")}
    </div>
  );
}

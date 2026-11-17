// Panel 4 — Berkas sesi: berkas yang disentuh agent di sesi ini (dibaca/diubah/ditulis), terbaru dulu. Klik membukanya di Panel 3.

import type { SessionFileDiff } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { FilesIcon } from "@zandor/ui";
import { useEffect, useMemo, useState } from "react";
import { FileTypeIcon } from "../FileTypeIcon";
import { useDocActions, useServices } from "../services";
import { mergeSnapshotFiles, touchedFiles } from "../session-timeline";
import { ToolNotice } from "./ToolNotice";
import { relativeTo, useSelectedSession } from "./use-selected-session";

export function SessionFilesPanel() {
  const { t } = useI18n();
  const { projects } = useServices();
  const sel = useSelectedSession();
  const docs = useDocActions();
  const [snap, setSnap] = useState<{ id: string; diffs: SessionFileDiff[] } | null>(null);
  const id = sel?.id ?? null;
  const dir = sel?.directory ?? "";
  const demo = sel?.demo ?? true;
  const busy = sel?.session.status.type !== "idle" || sel?.session.sending === true;
  const messageCount = sel?.session.messages.length ?? 0;

  // Snapshot engine menangkap berkas yang dibuat lewat bash; dimuat ulang tiap giliran selesai.
  // biome-ignore lint/correctness/useExhaustiveDependencies: pemicu = sesi, status sibuk, jumlah pesan
  useEffect(() => {
    const api = projects.engineApi();
    if (!id || demo || !api || busy) return;
    let live = true;
    api.sessionDiff(dir, id).then(
      (diffs) => live && setSnap({ id, diffs }),
      () => live && setSnap({ id, diffs: [] }),
    );
    return () => {
      live = false;
    };
  }, [id, dir, demo, busy, messageCount]);

  const files = useMemo(() => {
    if (!sel) return [];
    const touched = touchedFiles(sel.session.messages);
    if (!snap || snap.id !== sel.id) return touched;
    const lastAt = sel.session.messages.at(-1)?.info.time.created ?? 0;
    return mergeSnapshotFiles(touched, snap.diffs, sel.directory, lastAt);
  }, [sel, snap]);
  if (!sel) return <ToolNotice icon={<FilesIcon size="lg" />}>{t("info.empty")}</ToolNotice>;

  return (
    <div className="sfil">
      {sel.demo && <ToolNotice inline>{t("file.demoNote")}</ToolNotice>}
      {files.length === 0 && <ToolNotice inline>{t("sfil.empty")}</ToolNotice>}
      <ul className="sfil__list" aria-label={t("tools.files")} data-testid="session-files">
        {files.map((f) => {
          const name = relativeTo(sel.directory, f.path);
          const base = name.split("/").at(-1) ?? name;
          return (
            <li key={f.path}>
              <button
                type="button"
                className="sfil__item"
                title={f.path}
                disabled={sel.demo}
                onClick={() => {
                  if (!docs.open(f.path, sel.directory || undefined)) return;
                }}
              >
                <FileTypeIcon name={base} type="file" />
                <span className="sfil__name">
                  <span className="tree__label">{base}</span>
                  {name !== base && (
                    <span className="z-small sfil__dir">{name.slice(0, name.length - base.length - 1)}</span>
                  )}
                </span>
                <span className="sfil__ops">
                  {f.ops.map((op) => (
                    <span key={op} className="gitp__badge" data-kind={op === "read" ? "R" : op === "write" ? "A" : "M"}>
                      {t(`sfil.op.${op}` as never)}
                    </span>
                  ))}
                  {f.count > 1 && <span className="z-small sfil__n">×{f.count}</span>}
                  {f.failed && <span className="z-small sfil__fail">{t("sfil.failed")}</span>}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

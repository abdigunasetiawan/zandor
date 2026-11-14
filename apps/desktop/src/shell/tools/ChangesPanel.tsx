// Panel 4 — Perubahan sesi: berkas yang diubah/dibuat/dihapus OLEH SESI INI menurut snapshot engine (tidak bergantung pada git,
// jadi juga bekerja di folder Chat tanpa repo), dengan diff per berkas.
import type { SessionFileDiff } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, DiffIcon, IconButton, RefreshIcon, Spinner } from "@zandor/ui";
import { useCallback, useEffect, useState } from "react";
import { useDocActions, useServices } from "../services";
import { DiffText } from "./DiffText";
import { ToolNotice } from "./ToolNotice";
import { relativeTo, useSelectedSession } from "./use-selected-session";

type Load = { status: "idle" | "loading" | "ready" | "error"; diffs: SessionFileDiff[]; error: string | null };
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function ChangesPanel() {
  const { t } = useI18n();
  const { projects } = useServices();
  const sel = useSelectedSession();
  const docs = useDocActions();
  const [load, setLoad] = useState<Load>({ status: "idle", diffs: [], error: null });
  const [open, setOpen] = useState<Set<string>>(new Set());
  const id = sel?.id ?? null;
  const dir = sel?.directory ?? "";
  const demo = sel?.demo ?? true;
  const busy = sel?.session.status.type !== "idle" || sel?.session.sending === true;
  const messageCount = sel?.session.messages.length ?? 0;

  const fetchDiffs = useCallback(() => {
    const api = projects.engineApi();
    if (!id || demo || !api) return;
    setLoad((l) => ({ ...l, status: "loading", error: null }));
    api.sessionDiff(dir, id).then(
      (diffs) => setLoad({ status: "ready", diffs, error: null }),
      (e) => setLoad({ status: "error", diffs: [], error: errText(e) }),
    );
  }, [projects, id, dir, demo]);

  // muat saat sesi berganti, dan ulangi setiap giliran agent selesai (idle) atau pesan bertambah
  // biome-ignore lint/correctness/useExhaustiveDependencies: pemicu pemuatan ulang = status sibuk dan jumlah pesan
  useEffect(() => {
    setOpen(new Set());
    fetchDiffs();
  }, [id]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: lihat di atas
  useEffect(() => {
    if (!busy) fetchDiffs();
  }, [busy, messageCount]);

  if (!sel) return <ToolNotice icon={<DiffIcon size="lg" />}>{t("info.empty")}</ToolNotice>;
  if (sel.demo) return <ToolNotice icon={<DiffIcon size="lg" />}>{t("sess.demoNone")}</ToolNotice>;

  const total = load.diffs.reduce((a, d) => ({ add: a.add + d.additions, del: a.del + d.deletions }), {
    add: 0,
    del: 0,
  });
  const toggle = (file: string) =>
    setOpen((prev) => {
      const next = new Set(prev);
      if (!next.delete(file)) next.add(file);
      return next;
    });

  return (
    <div className="chg">
      <div className="chg__bar">
        <span className="z-small" role="status">
          {load.status === "loading" && load.diffs.length === 0 ? (
            <Spinner size="sm" />
          ) : (
            t("chg.summary", { n: load.diffs.length, add: total.add, del: total.del })
          )}
        </span>
        <IconButton label={t("git.refresh")} size="sm" icon={<RefreshIcon />} onClick={fetchDiffs} />
      </div>
      {load.status === "error" && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {t("chg.error", { error: load.error ?? "" })}
        </p>
      )}
      {load.status === "ready" && load.diffs.length === 0 && <ToolNotice inline>{t("chg.empty")}</ToolNotice>}
      <ul className="chg__list" aria-label={t("tools.changes")} data-testid="session-changes">
        {load.diffs.map((d) => {
          const expanded = open.has(d.file);
          const name = relativeTo(dir, d.file);
          return (
            <li key={d.file} className="chg__item">
              <div className="chg__row">
                <button
                  type="button"
                  className="chg__name"
                  aria-expanded={expanded}
                  title={d.file}
                  onClick={() => toggle(d.file)}
                >
                  <span
                    className="gitp__badge"
                    data-kind={d.status === "added" ? "A" : d.status === "deleted" ? "D" : "M"}
                  >
                    {t(`chg.status.${d.status}` as never)}
                  </span>
                  <span className="tree__label">{name}</span>
                </button>
                <span className="chg__count z-small">
                  <span className="chg__add">+{d.additions}</span> <span className="chg__del">−{d.deletions}</span>
                </span>
                {d.status !== "deleted" && (
                  <Button size="sm" onClick={() => docs.open(d.file, dir)}>
                    {t("chg.open")}
                  </Button>
                )}
              </div>
              {expanded &&
                (d.patch ? (
                  <DiffText text={d.patch} testId="session-diff" />
                ) : (
                  <p className="z-small chg__nopatch">{t("chg.noPatch")}</p>
                ))}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

// Panel 4 — Aktivitas sesi: garis waktu seluruh kejadian sesi ini (pesan Anda, berpikir, tool call dengan status dan durasi,
// jawaban, galat, serta izin/pertanyaan yang menunggu). Berbeda dengan Log aktivitas, yang mencatat seluruh aplikasi.
import { useI18n } from "@zandor/i18n/react";
import { ActivityIcon, Button, Select } from "@zandor/ui";
import { useMemo, useState } from "react";
import { requestMessageFocus } from "../message-focus";
import { buildTimeline, type TimelineEntry, type TimelineKind } from "../session-timeline";
import { ToolNotice } from "./ToolNotice";
import { useSelectedSession } from "./use-selected-session";

type Filter = "all" | "user" | "agent" | "tool" | "error";
const FILTERS: Filter[] = ["all", "user", "agent", "tool", "error"];
const matches = (e: TimelineEntry, f: Filter) =>
  f === "all" ||
  (f === "user" && e.kind === "user") ||
  (f === "agent" && (e.kind === "reply" || e.kind === "thinking")) ||
  (f === "tool" && e.kind === "tool") ||
  (f === "error" && (e.kind === "error" || e.status === "error" || e.status === "pending"));

const fmtDuration = (ms: number) => (ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} dtk`);

export function SessionActivityPanel() {
  const { t } = useI18n();
  const sel = useSelectedSession();
  const [filter, setFilter] = useState<Filter>("all");
  const [open, setOpen] = useState<Set<string>>(new Set());
  const timeline = useMemo(() => (sel ? buildTimeline(sel.session.messages, sel.session) : []), [sel]);
  if (!sel) return <ToolNotice icon={<ActivityIcon size="lg" />}>{t("info.empty")}</ToolNotice>;
  const shown = timeline.filter((e) => matches(e, filter));

  const label = (e: TimelineEntry): string => {
    if (e.step) return t(e.step.key);
    return t(`sact.kind.${e.kind}` as never);
  };
  const toggle = (id: string) =>
    setOpen((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  return (
    <div className="sact">
      <div className="actp__bar">
        <Select
          aria-label={t("sact.filter")}
          size="sm"
          items={FILTERS.map((f) => ({ value: f, label: t(`sact.f.${f}` as never) }))}
          value={filter}
          onValueChange={(v) => setFilter(v as Filter)}
        />
        <span className="z-small" role="status">
          {t("act.count", { count: shown.length })}
        </span>
      </div>
      {sel.demo && <ToolNotice inline>{t("file.demoNote")}</ToolNotice>}
      {shown.length === 0 && <ToolNotice inline>{t("sact.empty")}</ToolNotice>}
      <ol className="sact__list" aria-label={t("tools.session")} data-testid="session-activity">
        {shown.map((e) => {
          const expanded = open.has(e.id);
          return (
            <li key={e.id} className="sact__row" data-kind={e.kind as TimelineKind} data-status={e.status}>
              <time className="actp__time z-small" dateTime={e.at ? new Date(e.at).toISOString() : undefined}>
                {e.at ? new Date(e.at).toLocaleTimeString() : "—"}
              </time>
              <div className="sact__main">
                <div className="sact__head">
                  <span className="sact__label">{label(e)}</span>
                  {e.status && e.status !== "completed" && (
                    <span className="sact__status z-small" data-status={e.status}>
                      {t(`sact.status.${e.status}` as never)}
                    </span>
                  )}
                  {e.durationMs !== undefined && <span className="z-small sact__dur">{fmtDuration(e.durationMs)}</span>}
                </div>
                <p className="sact__title">{e.title}</p>
                {expanded && e.detail && <pre className="fold__pre sact__detail">{e.detail}</pre>}
                <div className="sact__actions">
                  {e.detail && (
                    <Button size="sm" variant="ghost" aria-expanded={expanded} onClick={() => toggle(e.id)}>
                      {expanded ? t("sact.hide") : t("sact.detail")}
                    </Button>
                  )}
                  {e.messageId && !sel.demo && (
                    <Button size="sm" variant="ghost" onClick={() => e.messageId && requestMessageFocus(e.messageId)}>
                      {t("sact.jump")}
                    </Button>
                  )}
                </div>
              </div>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

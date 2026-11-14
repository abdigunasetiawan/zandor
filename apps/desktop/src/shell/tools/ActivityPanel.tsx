// Panel 4 — Log aktivitas: event agent (tool call, izin, galat) dan event aplikasi (warn/error, engine, git, terminal).
import type { ActivitySource } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, Select } from "@zandor/ui";
import { useEffect, useRef, useState } from "react";
import { useActivity, useServices } from "../services";
import { ToolNotice } from "./ToolNotice";

const SOURCES: Array<ActivitySource | "all"> = ["all", "app", "engine", "chat", "tool", "git", "terminal"];

export function ActivityPanel() {
  const { t } = useI18n();
  const { activity } = useServices();
  const entries = useActivity();
  const [filter, setFilter] = useState<ActivitySource | "all">("all");
  const scroller = useRef<HTMLUListElement>(null);
  const shown = filter === "all" ? entries : entries.filter((e) => e.source === filter);

  // tetap di bawah saat entri baru masuk, kecuali pengguna menggulir ke atas
  // biome-ignore lint/correctness/useExhaustiveDependencies: dipicu oleh jumlah entri
  useEffect(() => {
    const el = scroller.current;
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight < 80) el.scrollTop = el.scrollHeight;
  }, [shown.length]);

  return (
    <div className="actp">
      <div className="actp__bar">
        <Select
          aria-label={t("act.filter")}
          size="sm"
          items={SOURCES.map((s) => ({ value: s, label: t(`act.src.${s}` as never) }))}
          value={filter}
          onValueChange={(v) => setFilter(v as ActivitySource | "all")}
        />
        <span className="z-small" role="status">
          {t("act.count", { count: shown.length })}
        </span>
        <Button size="sm" onClick={() => activity.clear()} disabled={entries.length === 0}>
          {t("act.clear")}
        </Button>
      </div>
      {shown.length === 0 && <ToolNotice inline>{t("act.empty")}</ToolNotice>}
      <ul className="actp__list" ref={scroller} aria-label={t("tools.log")} data-testid="activity">
        {shown.map((e) => (
          <li key={e.id} className="actp__row" data-level={e.level}>
            <time className="actp__time z-small" dateTime={new Date(e.at).toISOString()}>
              {new Date(e.at).toLocaleTimeString()}
            </time>
            <span className="actp__src z-small">{t(`act.src.${e.source}` as never)}</span>
            <span className="actp__msg">{e.message}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

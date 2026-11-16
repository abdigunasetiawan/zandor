// Panel 4 — Trajectory sesi (ADR-073): buku besar transparan atas proses di balik layar, per giliran. Ikhtisar waktu tiga
// jalur (Masukan / Model / Tool) di atas, daftar rekaman di tengah, dan inspektur (Input / Output / Raw) untuk rekaman
// terpilih. Data murni dari pesan sesi (lihat `trajectory.ts`); tidak ada waktu/rekaman karangan.
import { useI18n } from "@zandor/i18n/react";
import {
  Button,
  ChevronDownIcon,
  ChevronRightIcon,
  CloseIcon,
  CopyIcon,
  IconButton,
  SearchField,
  Tabs,
  TrajectoryIcon,
  useToast,
} from "@zandor/ui";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { requestMessageFocus } from "../message-focus";
import {
  buildTimeline,
  buildTrajectory,
  durationOf,
  formatMs,
  formatTokens,
  matchesQuery,
  type TrajKind,
  type TrajRecord,
  totals,
} from "../trajectory";
import { ToolNotice } from "./ToolNotice";
import { useSelectedSession } from "./use-selected-session";

type Hideable = "thinking" | "context" | "tool" | "step";
const HIDEABLE: Hideable[] = ["context", "thinking", "tool", "step"];
const hideKey = (k: TrajKind): Hideable | null =>
  k === "thinking" || k === "context" || k === "tool" || k === "step" ? k : k === "other" ? "step" : null;

const clock = (ms?: number) =>
  ms === undefined
    ? "—"
    : new Date(ms).toLocaleTimeString(undefined, {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
        hour12: false,
        fractionalSecondDigits: 3,
      } as Intl.DateTimeFormatOptions);

export function TrajectoryPanel() {
  const { t } = useI18n();
  const toast = useToast();
  const sel = useSelectedSession();
  const [query, setQuery] = useState("");
  const [hidden, setHidden] = useState<Set<Hideable>>(new Set());
  const [folded, setFolded] = useState<Set<number>>(new Set());
  const [mode, setMode] = useState<"time" | "sequence">("time");
  const [selected, setSelected] = useState<string | null>(null);
  const [tab, setTab] = useState("summary");
  const listRef = useRef<HTMLDivElement>(null);
  const atBottom = useRef(true);

  const messages = sel?.session.messages;
  const turns = useMemo(() => (messages ? buildTrajectory(messages) : []), [messages]);
  const sum = useMemo(() => totals(turns), [turns]);
  const timeline = useMemo(() => buildTimeline(turns, mode), [turns, mode]);
  const all = useMemo(() => turns.flatMap((x) => x.records), [turns]);
  const current = selected ? all.find((r) => r.id === selected) : undefined;

  // Ganti sesi → reset pilihan dan lipatan.
  const sessionId = sel?.id;
  useEffect(() => {
    if (sessionId === undefined) return;
    setSelected(null);
    setFolded(new Set());
    atBottom.current = true;
  }, [sessionId]);

  // Ikuti ekor selama sesi berjalan, kecuali pengguna menggulir ke atas.
  const recordCount = sum.records;
  useLayoutEffect(() => {
    const el = listRef.current;
    if (el && atBottom.current && recordCount >= 0) el.scrollTop = el.scrollHeight;
  }, [recordCount]);

  if (!sel) return <ToolNotice icon={<TrajectoryIcon size="lg" />}>{t("info.empty")}</ToolNotice>;

  const visible = (r: TrajRecord) => {
    const h = hideKey(r.kind);
    return !(h && hidden.has(h)) && matchesQuery(r, query.trim());
  };
  const toggleIn = <T,>(set: Set<T>, v: T) => {
    const next = new Set(set);
    if (!next.delete(v)) next.add(v);
    return next;
  };
  const allFolded = turns.length > 0 && turns.every((x) => folded.has(x.turn));
  const kindLabel = (k: TrajKind) => t(`traj.kind.${k}` as never);

  const select = (r: TrajRecord) => {
    setSelected(r.id === selected ? null : r.id);
    setFolded((f) => (f.has(r.turn) ? toggleIn(f, r.turn) : f));
    requestAnimationFrame(() =>
      listRef.current?.querySelector(`[data-rec="${CSS.escape(r.id)}"]`)?.scrollIntoView({ block: "nearest" }),
    );
  };

  const copyAll = async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify({ session: sel.id, turns }, null, 2));
      toast.show({ title: t("traj.copied"), tone: "success" });
    } catch {
      toast.show({ title: t("traj.copyFail"), tone: "danger" });
    }
  };

  return (
    <div className="traj" data-testid="trajectory">
      <div className="traj__bar">
        <Button
          size="sm"
          variant="ghost"
          aria-pressed={allFolded}
          onClick={() => setFolded(allFolded ? new Set() : new Set(turns.map((x) => x.turn)))}
        >
          {allFolded ? <ChevronRightIcon size="sm" /> : <ChevronDownIcon size="sm" />}
          {t("traj.turns")}
        </Button>
        {HIDEABLE.map((h) => (
          <Button
            key={h}
            size="sm"
            variant="ghost"
            className="traj__chip"
            aria-pressed={!hidden.has(h)}
            onClick={() => setHidden((s) => toggleIn(s, h))}
          >
            {t(`traj.show.${h}` as never)}
          </Button>
        ))}
        <Button
          size="sm"
          variant="ghost"
          title={t("traj.modeHint")}
          onClick={() => setMode((m) => (m === "time" ? "sequence" : "time"))}
        >
          {mode === "time" ? t("traj.mode.time") : t("traj.mode.sequence")}
        </Button>
        <IconButton size="sm" label={t("traj.copyAll")} icon={<CopyIcon />} onClick={() => void copyAll()} />
        <SearchField
          className="traj__search"
          value={query}
          onValueChange={setQuery}
          placeholder={t("traj.search")}
          aria-label={t("traj.search")}
        />
      </div>

      <p className="traj__stats z-small" role="status">
        {t("traj.stats", {
          turns: turns.length,
          model: sum.modelCalls,
          calls: sum.calls,
          input: formatTokens(sum.tokens.input),
          output: formatTokens(sum.tokens.output),
        })}
        {sum.cost > 0 && ` · $${sum.cost.toFixed(4)}`}
        {sum.errors > 0 && <span className="traj__err"> · {t("traj.errors", { count: sum.errors })}</span>}
      </p>
      {sel.demo && <ToolNotice inline>{t("file.demoNote")}</ToolNotice>}

      {timeline && (
        <section className="traj-ov" aria-label={t("traj.overview")}>
          {(["input", "model", "tools"] as const).map((lane) => (
            <div key={lane} className="traj-ov__lane">
              <span className="traj-ov__label z-small">{t(`traj.lane.${lane}` as never)}</span>
              <div className="traj-ov__track">
                {timeline.boundaries.map((b) => (
                  <span key={b.turn} className="traj-ov__turn" style={{ left: `${b.left}%` }} aria-hidden="true" />
                ))}
                {timeline.spans
                  .filter((s) => s.lane === lane)
                  .map((s) => {
                    const r = all[s.index];
                    if (!r) return null;
                    const d = durationOf(r);
                    return (
                      <button
                        key={s.index}
                        type="button"
                        className="traj-ov__span"
                        data-kind={s.kind}
                        data-error={s.error || undefined}
                        data-active={r.id === selected || undefined}
                        data-dim={(query && !matchesQuery(r, query.trim())) || undefined}
                        style={{ left: `${s.left}%`, width: `${s.width}%` }}
                        title={`${kindLabel(r.kind)}${r.name ? ` ${r.name}` : ""} · ${clock(r.start)}${d !== undefined ? ` · ${formatMs(d)}` : ""}`}
                        aria-label={`${kindLabel(r.kind)} ${r.name ?? ""} ${r.summary}`}
                        onClick={() => select(r)}
                      />
                    );
                  })}
              </div>
            </div>
          ))}
        </section>
      )}

      <div
        ref={listRef}
        className="traj__list"
        onScroll={(e) => {
          const el = e.currentTarget;
          atBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
      >
        {turns.length === 0 && <ToolNotice inline>{t("traj.empty")}</ToolNotice>}
        {turns.map((turn) => {
          const isFolded = folded.has(turn.turn);
          const rows = turn.records.filter(visible);
          if (query && rows.length === 0) return null;
          const dur = turn.start !== undefined && turn.end !== undefined ? turn.end - turn.start : undefined;
          return (
            <section key={turn.turn} className="traj-turn" aria-label={t("traj.turn", { n: turn.turn })}>
              <button
                type="button"
                className="traj-turn__head"
                aria-expanded={!isFolded}
                onClick={() => setFolded((f) => toggleIn(f, turn.turn))}
              >
                {isFolded ? <ChevronRightIcon size="sm" /> : <ChevronDownIcon size="sm" />}
                <span className="z-label">{t("traj.turn", { n: turn.turn })}</span>
                <span className="z-small traj-turn__meta">
                  {t("traj.turnMeta", {
                    records: turn.records.length,
                    model: turn.modelCalls,
                    calls: turn.calls,
                    tokens: formatTokens(turn.tokens.total),
                  })}
                  {dur !== undefined && ` · ${formatMs(dur)}`}
                  {turn.errors > 0 && <span className="traj__err"> · {t("traj.errors", { count: turn.errors })}</span>}
                </span>
              </button>
              {!isFolded && (
                <ol className="traj-turn__rows">
                  {rows.map((r) => {
                    const d = durationOf(r);
                    return (
                      <li key={r.id}>
                        <button
                          type="button"
                          className="traj-row"
                          data-rec={r.id}
                          data-kind={r.kind}
                          data-status={r.status}
                          aria-pressed={r.id === selected}
                          onClick={() => select(r)}
                        >
                          <span className="traj-row__kind">{kindLabel(r.kind)}</span>
                          <span className="traj-row__text">
                            {r.name && <code className="traj-row__name">{r.name}</code>}
                            <span className="traj-row__sum">{r.summary || "—"}</span>
                          </span>
                          {r.preview && <span className="traj-row__out">→ {r.preview}</span>}
                          <span className="traj-row__dur z-small">
                            {r.status === "running" || r.status === "pending"
                              ? t(`sact.status.${r.status}` as never)
                              : d !== undefined
                                ? formatMs(d)
                                : ""}
                          </span>
                        </button>
                      </li>
                    );
                  })}
                </ol>
              )}
            </section>
          );
        })}
      </div>

      {current && (
        <Inspector
          record={current}
          tab={tab}
          onTab={setTab}
          onClose={() => setSelected(null)}
          onJump={sel.demo ? undefined : () => requestMessageFocus(current.messageId)}
        />
      )}
    </div>
  );
}

function Inspector({
  record: r,
  tab,
  onTab,
  onClose,
  onJump,
}: {
  record: TrajRecord;
  tab: string;
  onTab: (v: string) => void;
  onClose: () => void;
  onJump: (() => void) | undefined;
}) {
  const { t } = useI18n();
  const d = durationOf(r);
  const facts: [string, string][] = [
    [t("traj.f.kind"), t(`traj.kind.${r.kind}` as never)],
    ...(r.name ? ([[t("traj.f.name"), r.name]] as [string, string][]) : []),
    ...(r.status ? ([[t("traj.f.status"), r.status]] as [string, string][]) : []),
    [t("traj.f.turn"), String(r.turn)],
    [t("traj.f.start"), clock(r.start)],
    [t("traj.f.end"), clock(r.end)],
    [t("traj.f.duration"), d !== undefined ? formatMs(d) : "—"],
    ...(r.tokens
      ? ([
          [
            t("traj.f.tokens"),
            t("traj.tokens", {
              input: r.tokens.input,
              output: r.tokens.output,
              reasoning: r.tokens.reasoning,
              total: r.tokens.total,
            }),
          ],
        ] as [string, string][])
      : []),
    ...(r.cost !== undefined ? ([[t("traj.f.cost"), `$${r.cost.toFixed(5)}`]] as [string, string][]) : []),
    [t("traj.f.id"), r.id],
  ];
  const pre = (s: string | undefined) =>
    s ? <pre className="fold__pre traj-insp__pre">{s}</pre> : <ToolNotice inline>{t("traj.none")}</ToolNotice>;
  return (
    <section className="traj-insp" aria-label={t("traj.inspector")}>
      <header className="traj-insp__head">
        <span className="z-label">{t(`traj.kind.${r.kind}` as never)}</span>
        {r.name && <code className="traj-row__name">{r.name}</code>}
        <span className="traj-insp__grow" />
        {onJump && (
          <Button size="sm" variant="ghost" onClick={onJump}>
            {t("sact.jump")}
          </Button>
        )}
        <IconButton size="sm" label={t("traj.close")} icon={<CloseIcon />} onClick={onClose} />
      </header>
      <div className="traj-insp__body">
        <Tabs
          aria-label={t("traj.inspector")}
          value={tab}
          onValueChange={onTab}
          items={[
            {
              value: "summary",
              label: t("traj.tab.summary"),
              content: (
                <dl className="traj-insp__facts">
                  {facts.map(([k, v]) => (
                    <div key={k}>
                      <dt className="z-small">{k}</dt>
                      <dd>{v}</dd>
                    </div>
                  ))}
                </dl>
              ),
            },
            { value: "input", label: t("traj.tab.input"), content: pre(r.input) },
            { value: "output", label: t("traj.tab.output"), content: pre(r.output) },
            { value: "raw", label: t("traj.tab.raw"), content: pre(JSON.stringify(r.raw, null, 2)) },
          ]}
        />
      </div>
    </section>
  );
}

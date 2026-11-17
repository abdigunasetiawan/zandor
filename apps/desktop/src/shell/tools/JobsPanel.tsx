// Panel 4 — Jobs (ADR-072): background job yang dijalankan agent (dev server, watcher). Daftar dengan status, port
// (buka di Browser Panel), log yang diperbarui, serta Stop / Jalankan ulang / Hapus. Data dibaca dari plugin engine.
import { useI18n } from "@zandor/i18n/react";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  GlobeIcon,
  IconButton,
  RefreshIcon,
  Select,
  ServerIcon,
  StopIcon,
  TrashIcon,
  Working,
} from "@zandor/ui";
import { useCallback, useEffect, useRef, useState } from "react";
import { isLive, type JobAction, type JobInfo, type JobsHost, loadJobsHost } from "../jobs-host";
import { useServices } from "../services";
import { openInZandorBrowser } from "./browser-bridge";
import { ToolNotice } from "./ToolNotice";
import { useSelectedSession } from "./use-selected-session";

const POLL_MS = 1500;
type Scope = "session" | "all";

export function JobsPanel({ host: injected }: { host?: JobsHost | null } = {}) {
  const { t } = useI18n();
  const { panels } = useServices();
  const sel = useSelectedSession();
  const [host, setHost] = useState<JobsHost | null | undefined>(injected);
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [scope, setScope] = useState<Scope>("session");
  const [open, setOpen] = useState<string | null>(null);

  useEffect(() => {
    if (injected !== undefined) return;
    let alive = true;
    void loadJobsHost().then((h) => alive && setHost(h));
    return () => {
      alive = false;
    };
  }, [injected]);

  const refresh = useCallback(async () => {
    if (!host) return;
    try {
      setJobs(await host.list());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [host]);

  useEffect(() => {
    if (!host) return;
    void refresh();
    const id = setInterval(() => void refresh(), POLL_MS);
    return () => clearInterval(id);
  }, [host, refresh]);

  if (host === undefined) return <ToolNotice icon={<ServerIcon size="lg" />}>{t("jobs.loading")}</ToolNotice>;
  if (host === null) return <ToolNotice icon={<ServerIcon size="lg" />}>{t("jobs.desktopOnly")}</ToolNotice>;

  const act = async (id: string, action: JobAction) => {
    try {
      await host.control(id, action);
      if (action === "remove" && open === id) setOpen(null);
      setTimeout(() => void refresh(), 400);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const sessionId = sel && !sel.demo ? sel.id : null;
  const shown = scope === "session" && sessionId ? jobs.filter((j) => j.sessionID === sessionId) : jobs;
  const running = jobs.filter((j) => isLive(j.status)).length;

  return (
    <div className="jobs">
      <div className="actp__bar">
        <Select
          aria-label={t("jobs.scope")}
          size="sm"
          items={[
            { value: "session", label: t("jobs.scope.session") },
            { value: "all", label: t("jobs.scope.all") },
          ]}
          value={scope}
          onValueChange={(v) => setScope(v as Scope)}
        />
        <span className="z-small jobs__count" role="status">
          {t("jobs.count", { count: shown.length, running })}
        </span>
        <IconButton size="sm" label={t("jobs.refresh")} icon={<RefreshIcon />} onClick={() => void refresh()} />
      </div>
      {error && <ToolNotice inline>{t("jobs.error", { error })}</ToolNotice>}
      {scope === "session" && !sessionId && <ToolNotice inline>{t("jobs.noSession")}</ToolNotice>}
      {shown.length === 0 ? (
        <ToolNotice icon={<ServerIcon size="lg" />}>{t("jobs.empty")}</ToolNotice>
      ) : (
        <ul className="jobs__list" aria-label={t("tools.jobs")}>
          {shown.map((j) => (
            <JobRow
              key={j.id}
              job={j}
              host={host}
              expanded={open === j.id}
              onToggle={() => setOpen(open === j.id ? null : j.id)}
              onAction={(a) => void act(j.id, a)}
              onOpenPort={(p) => openInZandorBrowser(panels, `http://localhost:${p}`)}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function JobRow({
  job,
  host,
  expanded,
  onToggle,
  onAction,
  onOpenPort,
}: {
  job: JobInfo;
  host: JobsHost;
  expanded: boolean;
  onToggle: () => void;
  onAction: (a: JobAction) => void;
  onOpenPort: (port: number) => void;
}) {
  const { t } = useI18n();
  const live = isLive(job.status);
  const status =
    job.exitCode !== null && job.status !== "stopped"
      ? t("jobs.statusCode", { status: t(`jobs.status.${job.status}` as never), code: job.exitCode })
      : t(`jobs.status.${job.status}` as never);
  return (
    <li className="jobs__row" data-status={job.status}>
      <div className="jobs__head">
        <button type="button" className="jobs__toggle" aria-expanded={expanded} onClick={onToggle} title={job.command}>
          {expanded ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
          <span className="jobs__state" aria-hidden="true">
            {live ? <Working variant="grid" /> : <span className="jobs__dot" />}
          </span>
          <span className="jobs__main">
            <span className="jobs__name">{job.name}</span>
            <span className="jobs__meta z-small">
              <span className="jobs__status">{status}</span>
              <span className="jobs__cmd">{job.command}</span>
            </span>
          </span>
        </button>
        <span className="jobs__actions">
          {job.ports.map((p) => (
            <IconButton
              key={p}
              size="sm"
              label={t("jobs.openPort", { port: p })}
              icon={<GlobeIcon />}
              disabled={!live}
              onClick={() => onOpenPort(p)}
            />
          ))}
          {live ? (
            <IconButton size="sm" label={t("jobs.stop")} icon={<StopIcon />} onClick={() => onAction("stop")} />
          ) : (
            <IconButton size="sm" label={t("jobs.remove")} icon={<TrashIcon />} onClick={() => onAction("remove")} />
          )}
          {job.status !== "lost" && (
            <IconButton
              size="sm"
              label={t("jobs.restart")}
              icon={<RefreshIcon />}
              onClick={() => onAction("restart")}
            />
          )}
        </span>
      </div>
      {expanded && <JobLog job={job} host={host} />}
    </li>
  );
}

function JobLog({ job, host }: { job: JobInfo; host: JobsHost }) {
  const { t } = useI18n();
  const [text, setText] = useState<string | null>(null);
  const pre = useRef<HTMLPreElement>(null);
  const live = isLive(job.status);

  useEffect(() => {
    let alive = true;
    const load = () =>
      host.log(job.id).then(
        (s) => {
          if (!alive) return;
          const el = pre.current;
          const atBottom = !el || el.scrollHeight - el.scrollTop - el.clientHeight < 24;
          setText(s);
          if (atBottom)
            requestAnimationFrame(() => {
              if (el) el.scrollTop = el.scrollHeight;
            });
        },
        () => alive && setText(""),
      );
    void load();
    const id = live ? setInterval(load, POLL_MS) : undefined;
    return () => {
      alive = false;
      if (id) clearInterval(id);
    };
  }, [host, job.id, live]);

  return (
    <section className="jobs__detail" aria-label={t("jobs.log", { name: job.name })}>
      <p className="jobs__cwd z-small" title={job.cwd}>
        {job.cwd}
      </p>
      {/* biome-ignore lint/a11y/noNoninteractiveTabindex: area log yang bisa digulir harus bisa dijangkau keyboard. */}
      <pre ref={pre} className="fold__pre jobs__log" tabIndex={0}>
        {text === null ? t("jobs.loadingLog") : text || t("jobs.noOutput")}
      </pre>
    </section>
  );
}

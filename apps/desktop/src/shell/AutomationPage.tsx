// Halaman Automasi (Fase 11): tugas terjadwal yang benar-benar berjalan (selama Zandor terbuka) dan memberi notifikasi.
import { type AutomationTask, type Schedule, validateTask } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowLeftIcon,
  Badge,
  Button,
  Field,
  IconButton,
  Input,
  Select,
  Switch,
  Textarea,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { useEffect, useState } from "react";
import { useAutomation, useProjects, useServices } from "./services";

const DAYS = [0, 1, 2, 3, 4, 5, 6] as const;

function useNow(everyMs = 30_000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(id);
  }, [everyMs]);
  return now;
}

export function AutomationPage() {
  const { t } = useI18n();
  const { nav, automation, notify, projects: projectStore } = useServices();
  const state = useAutomation();
  const { projects } = useProjects();
  const confirm = useConfirm();
  const toast = useToast();
  const now = useNow();
  const [editing, setEditing] = useState<string | "new" | null>(null);
  const [granted, setGranted] = useState<boolean | null>(null);

  useEffect(() => {
    void notify.granted().then(setGranted);
  }, [notify]);

  const nameOf = (id: string | null) =>
    id ? (projects.find((p) => p.id === id)?.name ?? t("auto.projectGone")) : t("chat.noProject");
  const when = (ms: number | null) => (ms === null ? "—" : new Date(ms).toLocaleString());
  const scheduleText = (s: Schedule): string => {
    if (s.kind === "interval") return t("auto.sch.interval", { n: s.everyMinutes });
    if (s.kind === "once") return t("auto.sch.once", { at: when(s.at) });
    const days =
      s.days.length === 0 ? t("auto.sch.everyDay") : s.days.map((d) => t(`auto.day.${d}` as never)).join(", ");
    return t("auto.sch.daily", { time: s.time, days });
  };

  const remove = async (task: AutomationTask) => {
    const ok = await confirm({
      title: t("auto.deleteTitle", { name: task.name }),
      description: t("auto.deleteBody"),
      confirmLabel: t("auto.delete"),
      tone: "danger",
    });
    if (ok) automation.remove(task.id);
  };

  return (
    <main className="mempage">
      <header className="mempage__head">
        <IconButton label={t("mem.back")} icon={<ArrowLeftIcon />} onClick={() => nav.push("/")} />
        <h1 className="z-h3 mempage__title">{t("auto.title")}</h1>
        <Button variant="solid" onClick={() => setEditing("new")}>
          {t("auto.new")}
        </Button>
      </header>
      <p className="chat__note chat__note--warn z-small" role="note">
        {t("auto.openOnly")}
      </p>
      <p className="z-small mempage__muted">{t("auto.noAutoApprove")}</p>
      {notify.available && granted === false && (
        <p className="chat__note z-small" role="status">
          {t("auto.notifOff")}{" "}
          <Button size="sm" onClick={() => void notify.request().then(setGranted)}>
            {t("auto.notifAllow")}
          </Button>
        </p>
      )}
      {!notify.available && <p className="z-small mempage__muted">{t("auto.notifUnavailable")}</p>}

      {editing !== null && (
        <TaskForm
          task={editing === "new" ? null : (state.tasks.find((x) => x.id === editing) ?? null)}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            void notify.granted().then((g) => (g ? undefined : notify.request().then(setGranted)));
          }}
        />
      )}

      {state.tasks.length === 0 ? (
        <p className="z-small mempage__muted">{t("auto.empty")}</p>
      ) : (
        <ul className="mempage__list" aria-label={t("auto.title")}>
          {state.tasks.map((task) => {
            const running = state.running.includes(task.id);
            const next = task.enabled ? automation.next(task) : null;
            return (
              <li key={task.id} className="mempage__card" aria-label={task.name}>
                <div className="mempage__cardhead">
                  <span className="z-label mempage__grow">{task.name}</span>
                  {running && <Badge tone="warning">{t("auto.running")}</Badge>}
                  <Switch
                    label={t("auto.enabled")}
                    checked={task.enabled}
                    onCheckedChange={(on) => automation.setEnabled(task.id, on)}
                  />
                </div>
                <p className="z-small mempage__muted">
                  {scheduleText(task.schedule)} · {nameOf(task.projectId)}
                </p>
                <p className="z-small">{task.prompt.length > 160 ? `${task.prompt.slice(0, 160)}…` : task.prompt}</p>
                <p className="z-small mempage__muted" data-testid={`auto-next-${task.id}`}>
                  {t("auto.next")}: {next === null ? (task.enabled ? t("auto.done") : t("auto.off")) : when(next)} ·{" "}
                  {t("auto.last")}: {when(task.lastRunAt)}
                  <span hidden>{now}</span>
                </p>
                {task.lastError && (
                  <p className="chat__note chat__note--error z-small" role="alert">
                    {t("auto.lastError", { error: task.lastError })}
                  </p>
                )}
                <div className="mempage__actions">
                  <Button
                    size="sm"
                    variant="solid"
                    disabled={running}
                    onClick={() =>
                      void automation.runNow(task.id).then((err) => {
                        if (err) toast.show({ title: t("auto.runFailed", { error: err }), tone: "danger" });
                      })
                    }
                    aria-label={`${t("auto.runNow")} ${task.name}`}
                  >
                    {t("auto.runNow")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => setEditing(task.id)}
                    aria-label={`${t("auto.edit")} ${task.name}`}
                  >
                    {t("auto.edit")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={!task.lastSessionId}
                    onClick={() => {
                      if (!task.lastSessionId) return;
                      nav.push("/");
                      projectStore.select(task.projectId ?? "chat", task.lastSessionId);
                    }}
                    aria-label={`${t("auto.openSession")} ${task.name}`}
                  >
                    {t("auto.openSession")}
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => void remove(task)}
                    aria-label={`${t("auto.delete")} ${task.name}`}
                  >
                    {t("auto.delete")}
                  </Button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </main>
  );
}

function TaskForm({
  task,
  onClose,
  onSaved,
}: {
  task: AutomationTask | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t } = useI18n();
  const { automation } = useServices();
  const { projects } = useProjects();
  const [name, setName] = useState(task?.name ?? "");
  const [prompt, setPrompt] = useState(task?.prompt ?? "");
  const [projectId, setProjectId] = useState<string>(task?.projectId ?? "");
  const [kind, setKind] = useState<Schedule["kind"]>(task?.schedule.kind ?? "daily");
  const [minutes, setMinutes] = useState(String(task?.schedule.kind === "interval" ? task.schedule.everyMinutes : 60));
  const [time, setTime] = useState(task?.schedule.kind === "daily" ? task.schedule.time : "09:00");
  const [days, setDays] = useState<number[]>(task?.schedule.kind === "daily" ? task.schedule.days : []);
  const [onceAt, setOnceAt] = useState(() => {
    const d = task?.schedule.kind === "once" ? new Date(task.schedule.at) : new Date(Date.now() + 3_600_000);
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
  });
  const [error, setError] = useState<string | null>(null);

  const build = (): Schedule =>
    kind === "interval"
      ? { kind, everyMinutes: Number(minutes) }
      : kind === "daily"
        ? { kind, time, days: [...days].sort() }
        : { kind, at: new Date(onceAt).getTime() };

  const save = () => {
    const input = { name, prompt, projectId: projectId || null, schedule: build() };
    const bad = validateTask(input);
    if (bad) return setError(bad);
    const err = task ? automation.update(task.id, input) : "error" in automation.add(input) ? "gagal" : null;
    if (err) return setError(err);
    onSaved();
  };

  return (
    <section className="mempage__card" aria-label={t("auto.form")}>
      <h2 className="z-h4">{task ? t("auto.edit") : t("auto.new")}</h2>
      <Field label={t("auto.name")}>
        {(c) => <Input {...c} value={name} onChange={(e) => setName(e.target.value)} />}
      </Field>
      <Field label={t("auto.project")}>
        {() => (
          <Select
            aria-label={t("auto.project")}
            value={projectId}
            onValueChange={setProjectId}
            items={[
              { value: "", label: t("chat.noProject") },
              ...projects.map((p) => ({ value: p.id, label: p.name })),
            ]}
          />
        )}
      </Field>
      <Field label={t("auto.prompt")} description={t("auto.promptHint")}>
        {(c) => <Textarea {...c} rows={5} value={prompt} onChange={(e) => setPrompt(e.target.value)} />}
      </Field>
      <Field label={t("auto.schedule")}>
        {() => (
          <Select
            aria-label={t("auto.schedule")}
            value={kind}
            onValueChange={(v) => setKind(v as Schedule["kind"])}
            items={[
              { value: "daily", label: t("auto.kind.daily") },
              { value: "interval", label: t("auto.kind.interval") },
              { value: "once", label: t("auto.kind.once") },
            ]}
          />
        )}
      </Field>
      {kind === "interval" && (
        <Field label={t("auto.everyMinutes")}>
          {(c) => <Input {...c} inputMode="numeric" value={minutes} onChange={(e) => setMinutes(e.target.value)} />}
        </Field>
      )}
      {kind === "daily" && (
        <>
          <Field label={t("auto.time")}>
            {(c) => <Input {...c} type="time" value={time} onChange={(e) => setTime(e.target.value)} />}
          </Field>
          <fieldset className="ap__col">
            <legend className="z-label">{t("auto.days")}</legend>
            <p className="z-small mempage__muted">{t("auto.daysHint")}</p>
            <div className="mempage__actions">
              {DAYS.map((d) => (
                <Button
                  key={d}
                  size="sm"
                  variant={days.includes(d) ? "solid" : "outline"}
                  aria-pressed={days.includes(d)}
                  onClick={() => setDays((cur) => (cur.includes(d) ? cur.filter((x) => x !== d) : [...cur, d]))}
                >
                  {t(`auto.day.${d}` as never)}
                </Button>
              ))}
            </div>
          </fieldset>
        </>
      )}
      {kind === "once" && (
        <Field label={t("auto.at")}>
          {(c) => <Input {...c} type="datetime-local" value={onceAt} onChange={(e) => setOnceAt(e.target.value)} />}
        </Field>
      )}
      {error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {error}
        </p>
      )}
      <div className="mempage__actions">
        <Button variant="solid" onClick={save}>
          {t("auto.save")}
        </Button>
        <Button variant="ghost" onClick={onClose}>
          {t("ap.editor.cancel")}
        </Button>
      </div>
    </section>
  );
}

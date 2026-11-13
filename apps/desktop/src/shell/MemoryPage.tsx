// Halaman Memori & Skill (Fase 9): opt-in memori/skill creator, edit berkas memori, antrean persetujuan skill, dan skill
// terpasang. Semua berkas dibaca plugin Zandor di engine setiap giliran, jadi perubahan langsung berlaku.
import {
  type MemoryScope,
  type MemoryState,
  projectKey,
  type SkillEntry,
  type SkillInfo,
  scanSecrets,
} from "@zandor/core";
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
  Tabs,
  Textarea,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { useEffect, useMemo, useState } from "react";
import { useMemory, useProjects, useServices } from "./services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function useFail() {
  const toast = useToast();
  const { t } = useI18n();
  return (e: unknown) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
}

type ProjectRef = { id: string; name: string; path: string; key: string };

/** Project terdaftar + kunci memorinya (sha256 path, dihitung asinkron). */
function useProjectRefs(): ProjectRef[] {
  const { projects } = useProjects();
  const sig = projects.map((p) => `${p.id}\n${p.name}\n${p.path}`).join("\u0000");
  const [keys, setKeys] = useState<Record<string, string>>({});
  // biome-ignore lint/correctness/useExhaustiveDependencies: `sig` mewakili isi `projects`
  useEffect(() => {
    let live = true;
    void Promise.all(projects.map(async (p) => [p.id, await projectKey(p.path)] as const)).then((e) => {
      if (live) setKeys(Object.fromEntries(e));
    });
    return () => {
      live = false;
    };
  }, [sig]);
  return projects
    .filter((p) => keys[p.id])
    .map((p) => ({ id: p.id, name: p.name, path: p.path, key: keys[p.id] as string }));
}

export function MemoryPage() {
  const { t } = useI18n();
  const { nav, memory } = useServices();
  const state = useMemory();
  const refs = useProjectRefs();
  const pathsSig = refs.map((r) => r.path).join("\u0000");
  // biome-ignore lint/correctness/useExhaustiveDependencies: `pathsSig` mewakili isi `refs`
  useEffect(() => {
    void memory.refresh(refs.map((r) => r.path));
  }, [memory, pathsSig]);

  return (
    <main className="mempage">
      <header className="mempage__head">
        <IconButton label={t("mem.back")} icon={<ArrowLeftIcon />} onClick={() => nav.push("/")} />
        <h1 className="z-h3 mempage__title">{t("mem.title")}</h1>
      </header>
      {!memory.connected && (
        <p className="chat__note z-small" role="note">
          {t("mem.preview")}
        </p>
      )}
      {state.error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {state.error}
        </p>
      )}
      {!state.ready ? (
        <p className="z-small">{t("mem.loading")}</p>
      ) : (
        <Tabs
          aria-label={t("mem.title")}
          items={[
            { value: "settings", label: t("mem.tab.settings"), content: <SettingsTab state={state} refs={refs} /> },
            { value: "notes", label: t("mem.tab.notes"), content: <NotesTab state={state} refs={refs} /> },
            {
              value: "queue",
              label: `${t("mem.tab.queue")}${state.queue.some((e) => e.status === "pending") ? ` (${state.queue.filter((e) => e.status === "pending").length})` : ""}`,
              content: <QueueTab state={state} refs={refs} />,
            },
            { value: "skills", label: t("mem.tab.skills"), content: <SkillsTab state={state} refs={refs} /> },
          ]}
        />
      )}
    </main>
  );
}

type TabProps = { state: MemoryState; refs: ProjectRef[] };

function SettingsTab({ state, refs }: TabProps) {
  const { t } = useI18n();
  const { memory } = useServices();
  const fail = useFail();
  const s = state.settings;
  const choice = (key: string) => (typeof s.projects[key] === "boolean" ? (s.projects[key] ? "on" : "off") : "follow");
  return (
    <section className="mempage__section" aria-label={t("mem.tab.settings")}>
      <p className="z-small mempage__muted">{t("mem.settings.intro")}</p>
      <Switch
        label={t("mem.settings.default")}
        checked={s.defaultEnabled}
        onCheckedChange={(on) => void memory.setDefaultEnabled(on).catch(fail)}
      />
      <p className="z-small mempage__muted">{t("mem.settings.defaultHint")}</p>
      <Switch
        label={t("mem.settings.skillCreator")}
        checked={s.skillCreator}
        onCheckedChange={(on) => void memory.setSkillCreator(on).catch(fail)}
      />
      <p className="z-small mempage__muted">{t("mem.settings.skillCreatorHint")}</p>
      <h2 className="z-h4">{t("mem.settings.projects")}</h2>
      {refs.length === 0 ? (
        <p className="z-small mempage__muted">{t("mem.settings.projectsEmpty")}</p>
      ) : (
        <ul className="mempage__list">
          {refs.map((r) => (
            <li key={r.id} className="mempage__row">
              <span className="mempage__grow">{r.name}</span>
              <Select
                aria-label={t("mem.notes.project", { name: r.name })}
                value={choice(r.key)}
                onValueChange={(v) =>
                  void memory.setProjectEnabled(r.key, v === "follow" ? null : v === "on").catch(fail)
                }
                items={[
                  { value: "follow", label: t("mem.settings.follow") },
                  { value: "on", label: t("mem.settings.on") },
                  { value: "off", label: t("mem.settings.off") },
                ]}
              />
            </li>
          ))}
        </ul>
      )}
      <p className="z-small mempage__muted">{t("mem.settings.sessionHint")}</p>
    </section>
  );
}

function NotesTab({ state, refs }: TabProps) {
  const { t } = useI18n();
  const { memory } = useServices();
  const confirm = useConfirm();
  const toast = useToast();
  const fail = useFail();

  const options = useMemo(() => {
    const label = (rel: string): string => {
      if (rel === "global.md") return t("mem.notes.global");
      const pin = /^pins\/(.+)\.md$/.exec(rel);
      if (pin) return t("mem.notes.pin", { id: pin[1] as string });
      const key = /^projects\/(.+)\.md$/.exec(rel)?.[1] ?? "";
      const name = refs.find((r) => r.key === key)?.name ?? state.known[key]?.split(/[\\/]/).filter(Boolean).pop();
      return name ? t("mem.notes.project", { name }) : t("mem.notes.projectUnknown", { key });
    };
    const rels = new Set<string>(["global.md", ...refs.map((r) => `projects/${r.key}.md`)]);
    for (const f of state.files) if (/^(projects|pins)\//.test(f.rel) || f.rel === "global.md") rels.add(f.rel);
    return [...rels].map((rel) => ({ value: rel, label: label(rel) }));
  }, [refs, state.files, state.known, t]);

  const [rel, setRel] = useState("global.md");
  const [text, setText] = useState("");
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: hanya muat ulang saat berkas dipilih berganti
  useEffect(() => {
    let live = true;
    setError(null);
    memory
      .readFile(rel)
      .then((v) => {
        if (live) {
          setText(v);
          setDirty(false);
        }
      })
      .catch(fail);
    return () => {
      live = false;
    };
  }, [memory, rel]);

  const name = options.find((o) => o.value === rel)?.label ?? rel;
  const save = async () => {
    const found = scanSecrets(text);
    if (found.length) return setError(t("mem.notes.secret", { kinds: found.join(", ") }));
    setError(null);
    try {
      await memory.saveFile(rel, text);
      setDirty(false);
      toast.show({ title: t("mem.notes.saved"), tone: "success" });
    } catch (e) {
      fail(e);
    }
  };
  const remove = async () => {
    const ok = await confirm({
      title: t("mem.notes.deleteTitle", { name }),
      description: t("mem.notes.deleteBody"),
      confirmLabel: t("mem.notes.delete"),
      tone: "danger",
    });
    if (!ok) return;
    try {
      await memory.deleteFile(rel);
      setText("");
      setDirty(false);
    } catch (e) {
      fail(e);
    }
  };

  return (
    <section className="mempage__section" aria-label={t("mem.tab.notes")}>
      <Field label={t("mem.notes.file")} description={t("mem.notes.hint")}>
        {() => <Select aria-label={t("mem.notes.file")} value={rel} onValueChange={setRel} items={options} />}
      </Field>
      <Textarea
        aria-label={name}
        rows={12}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          setDirty(true);
        }}
        invalid={error !== null}
      />
      {error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {error}
        </p>
      )}
      <div className="mempage__actions">
        <Button variant="solid" onClick={() => void save()} disabled={!dirty}>
          {t("mem.notes.save")}
        </Button>
        <Button variant="danger" onClick={() => void remove()} disabled={rel === "global.md" && !text}>
          {t("mem.notes.delete")}
        </Button>
      </div>
    </section>
  );
}

function QueueTab({ state, refs }: TabProps) {
  const { t } = useI18n();
  const entries = [...state.queue].sort((a, b) => b.t - a.t);
  if (entries.length === 0) return <p className="z-small mempage__muted">{t("mem.queue.empty")}</p>;
  return (
    <ul className="mempage__list" aria-label={t("mem.tab.queue")}>
      {entries.map((e) => (
        <EntryCard key={e.id} entry={e} refs={refs} />
      ))}
    </ul>
  );
}

const STATUS_TONE = { candidate: "neutral", pending: "warning", approved: "success", rejected: "neutral" } as const;

function EntryCard({ entry, refs }: { entry: SkillEntry; refs: ProjectRef[] }) {
  const { t } = useI18n();
  const { memory } = useServices();
  const toast = useToast();
  const fail = useFail();
  const [name, setName] = useState(entry.name ?? "");
  const [description, setDescription] = useState(entry.description ?? "");
  const [body, setBody] = useState(entry.body ?? "");
  const [error, setError] = useState<string | null>(null);
  const home = refs.find((r) => r.key === entry.project);
  const [target, setTarget] = useState(home ? `project:${home.path}` : "global");
  const pending = entry.status === "pending";

  const run = async (fn: () => Promise<string | null | undefined>) => {
    try {
      setError((await fn()) ?? null);
    } catch (e) {
      fail(e);
    }
  };
  const approve = () =>
    run(async () => {
      // Perubahan yang belum disimpan ikut disetujui: simpan dulu (divalidasi) lalu pasang.
      const edited = name !== entry.name || description !== entry.description || body !== entry.body;
      if (edited) {
        const err = await memory.editDraft(entry.id, { name, description, body });
        if (err) return err;
      }
      const scope: MemoryScope = target === "global" ? "global" : "project";
      const err = await memory.approve(entry.id, scope, scope === "project" ? target.slice("project:".length) : null);
      if (!err) toast.show({ title: t("mem.queue.installed", { path: name }), tone: "success" });
      return err;
    });

  return (
    <li
      className="mempage__card"
      data-status={entry.status}
      aria-label={entry.name ?? t(`mem.queue.trigger.${entry.trigger}`)}
    >
      <div className="mempage__cardhead">
        <Badge tone={STATUS_TONE[entry.status]}>{t(`mem.queue.status.${entry.status}`)}</Badge>
        <span className="z-label">{t(`mem.queue.trigger.${entry.trigger}`)}</span>
        <span className="z-small mempage__muted">{new Date(entry.t).toLocaleString()}</span>
      </div>
      {entry.excerpt && <p className="z-small mempage__muted">{t("mem.queue.excerpt", { text: entry.excerpt })}</p>}
      {pending ? (
        <div className="mempage__form">
          <Field label={t("mem.queue.name")}>
            {(c) => <Input {...c} value={name} onChange={(e) => setName(e.target.value)} />}
          </Field>
          <Field label={t("mem.queue.description")}>
            {(c) => <Input {...c} value={description} onChange={(e) => setDescription(e.target.value)} />}
          </Field>
          <Field label={t("mem.queue.body")}>
            {(c) => <Textarea {...c} rows={8} value={body} onChange={(e) => setBody(e.target.value)} />}
          </Field>
          <Field label={t("mem.queue.scope")}>
            {() => (
              <Select
                aria-label={t("mem.queue.scope")}
                value={target}
                onValueChange={setTarget}
                items={[
                  { value: "global", label: t("mem.queue.scopeGlobal") },
                  ...refs.map((r) => ({ value: `project:${r.path}`, label: t("mem.notes.project", { name: r.name }) })),
                ]}
              />
            )}
          </Field>
          {error && (
            <p className="chat__note chat__note--error z-small" role="alert">
              {error}
            </p>
          )}
        </div>
      ) : (
        entry.name && (
          <p className="z-small">
            <strong>{entry.name}</strong> — {entry.description}
          </p>
        )
      )}
      <div className="mempage__actions">
        {pending && (
          <>
            <Button variant="solid" onClick={() => void approve()}>
              {t("mem.queue.approve")}
            </Button>
            <Button
              variant="ghost"
              onClick={() => void run(() => memory.editDraft(entry.id, { name, description, body }))}
            >
              {t("mem.queue.saveEdit")}
            </Button>
            <Button variant="ghost" onClick={() => void run(async () => void (await memory.reject(entry.id)))}>
              {t("mem.queue.reject")}
            </Button>
          </>
        )}
        <Button variant="danger" onClick={() => void run(async () => void (await memory.removeEntry(entry.id)))}>
          {t("mem.queue.remove")}
        </Button>
      </div>
    </li>
  );
}

function SkillsTab({ state, refs }: TabProps) {
  const { t } = useI18n();
  const groups: Array<{ id: string; title: string; scope: MemoryScope; project: string | null; skills: SkillInfo[] }> =
    [
      { id: "global", title: t("mem.skills.global"), scope: "global", project: null, skills: state.installed.global },
      ...refs.map((r) => ({
        id: r.id,
        title: t("mem.skills.project", { name: r.name }),
        scope: "project" as const,
        project: r.path,
        skills: state.installed.projects[r.path] ?? [],
      })),
    ];
  if (groups.every((g) => g.skills.length === 0))
    return <p className="z-small mempage__muted">{t("mem.skills.empty")}</p>;
  return (
    <div className="mempage__section">
      {groups
        .filter((g) => g.skills.length > 0)
        .map((g) => (
          <section key={g.id} aria-label={g.title}>
            <h2 className="z-h4">{g.title}</h2>
            <ul className="mempage__list">
              {g.skills.map((s) => (
                <SkillRow key={s.name} scope={g.scope} project={g.project} skill={s} />
              ))}
            </ul>
          </section>
        ))}
    </div>
  );
}

function SkillRow({ scope, project, skill }: { scope: MemoryScope; project: string | null; skill: SkillInfo }) {
  const { t } = useI18n();
  const { memory } = useServices();
  const confirm = useConfirm();
  const fail = useFail();
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const open = () => memory.skillRead(scope, project, skill.name).then(setText).catch(fail);
  const save = async () => {
    if (text === null) return;
    const err = await memory.skillSave(scope, project, skill.name, text).catch((e) => errText(e));
    setError(err ?? null);
    if (!err) setText(null);
  };
  const remove = async () => {
    const ok = await confirm({
      title: t("mem.skills.deleteTitle", { name: skill.name }),
      description: t("mem.skills.deleteBody"),
      confirmLabel: t("mem.skills.delete"),
      tone: "danger",
    });
    if (ok) await memory.skillRemove(scope, project, skill.name).catch(fail);
  };

  return (
    <li className="mempage__card" aria-label={skill.name}>
      <div className="mempage__cardhead">
        <span className="z-label mempage__grow">{skill.name}</span>
        {text === null && (
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void open()}
            aria-label={`${t("mem.skills.edit")} ${skill.name}`}
          >
            {t("mem.skills.edit")}
          </Button>
        )}
        <Button
          size="sm"
          variant="danger"
          onClick={() => void remove()}
          aria-label={`${t("mem.skills.delete")} ${skill.name}`}
        >
          {t("mem.skills.delete")}
        </Button>
      </div>
      {text !== null && (
        <>
          <Textarea aria-label={skill.name} rows={12} value={text} onChange={(e) => setText(e.target.value)} />
          {error && (
            <p className="chat__note chat__note--error z-small" role="alert">
              {error}
            </p>
          )}
          <div className="mempage__actions">
            <Button variant="solid" onClick={() => void save()}>
              {t("mem.skills.save")}
            </Button>
            <Button variant="ghost" onClick={() => setText(null)}>
              ✕
            </Button>
          </div>
        </>
      )}
    </li>
  );
}

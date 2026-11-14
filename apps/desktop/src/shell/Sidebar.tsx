// Panel 1: project dan sesi (Fase 4). Sumber kebenaran: ProjectStore (core) — komponen ini hanya tampilan + aksi.
import {
  CHAT_KEY,
  type Project,
  type ProjectKey,
  type SessionInfo,
  type SessionsState,
  sessionTitle,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  BrainIcon,
  Button,
  ChatIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ClockIcon,
  ContextMenu,
  Dialog,
  Field,
  FolderIcon,
  IconButton,
  Input,
  Menu,
  type MenuEntry,
  MoreIcon,
  PlusIcon,
  RefreshIcon,
  SearchField,
  SettingsIcon,
  Spinner,
  Tabs,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { type FormEvent, useMemo, useState } from "react";
import { build } from "../build";
import { FilesTree } from "./FilesTree";
import { useAppearance, useEngineState, useProjects, useServices } from "./services";

type RenameTarget =
  | { kind: "project"; id: string; value: string }
  | { kind: "session"; key: ProjectKey; id: string; value: string };

function useErrorToast() {
  const toast = useToast();
  const { t } = useI18n();
  return (e: unknown) =>
    toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
}

export function Sidebar() {
  const { t } = useI18n();
  const state = useProjects();
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState("sessions");
  const selectedProject = state.projects.find((p) => p.id === state.selection.key) ?? null;

  return (
    <div className="sidebar">
      <div className="sidebar__head">
        <h2 className="z-h4">{t("sb.projects")}</h2>
        <AddProject />
      </div>
      <Tabs
        aria-label={t("panel.sidebar")}
        value={tab}
        onValueChange={setTab}
        items={[
          {
            value: "sessions",
            label: t("sb.tab.sessions"),
            content: (
              <div className="sidebar__tab">
                <SearchField
                  aria-label={t("sb.search")}
                  placeholder={t("sb.search")}
                  value={query}
                  onValueChange={setQuery}
                />
                <SessionsList query={query} />
              </div>
            ),
          },
          { value: "files", label: t("sb.tab.files"), content: <FilesTree project={selectedProject} /> },
        ]}
      />
      <SidebarFooter />
    </div>
  );
}

function AddProject() {
  const { t } = useI18n();
  const { host, projects } = useServices();
  const fail = useErrorToast();
  const [open, setOpen] = useState(false);
  const [path, setPath] = useState("");

  const add = async (p: string) => {
    try {
      await projects.addProject(p);
    } catch (e) {
      fail(e);
    }
  };
  const onClick = async () => {
    if (host.native) {
      const picked = await host.pickFolder().catch((e) => {
        fail(e);
        return null;
      });
      if (picked) await add(picked);
    } else {
      setPath("");
      setOpen(true);
    }
  };
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!path.trim()) return;
    setOpen(false);
    void add(path);
  };
  return (
    <>
      <IconButton label={t("sb.addProject")} size="sm" icon={<PlusIcon />} onClick={() => void onClick()} />
      <Dialog
        open={open}
        onOpenChange={setOpen}
        title={t("sb.addByPathTitle")}
        description={t("sb.addByPathBody")}
        footer={
          <>
            <Button onClick={() => setOpen(false)}>{t("ui.cancel")}</Button>
            <Button variant="solid" type="submit" form="add-project-form" disabled={!path.trim()}>
              {t("sb.add")}
            </Button>
          </>
        }
      >
        <form id="add-project-form" onSubmit={submit}>
          <Field label={t("sb.path")}>
            {(c) => <Input {...c} value={path} onChange={(e) => setPath(e.target.value)} autoFocus />}
          </Field>
        </form>
      </Dialog>
    </>
  );
}

function SessionsList({ query }: { query: string }) {
  const { t } = useI18n();
  const { projects: store } = useServices();
  const state = useProjects();
  const appearance = useAppearance();
  const confirm = useConfirm();
  const fail = useErrorToast();
  const [collapsed, setCollapsed] = useState<Set<ProjectKey>>(new Set());
  const [rename, setRename] = useState<RenameTarget | null>(null);
  const q = query.trim().toLowerCase();

  const groups: Array<{ key: ProjectKey; project: Project | null }> = useMemo(
    () => [
      ...state.projects.map((p) => ({ key: p.id, project: p as Project | null })),
      { key: CHAT_KEY, project: null },
    ],
    [state.projects],
  );

  const toggle = (key: ProjectKey) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(key)) next.add(key);
      return next;
    });

  const run = (p: Promise<unknown>) => void p.catch(fail);

  const removeProject = async (p: Project) => {
    const ok = await confirm({
      title: t("sb.confirmRemoveTitle", { name: p.name }),
      description: t("sb.confirmRemoveBody"),
      confirmLabel: t("sb.removeProject"),
      tone: "danger",
    });
    if (ok) run(store.removeProject(p.id));
  };
  const deleteSession = async (key: ProjectKey, s: SessionInfo) => {
    const ok = await confirm({
      title: t("sb.confirmDeleteTitle", { name: sessionTitle(s) ?? t("sb.untitled") }),
      description: t("sb.confirmDeleteBody"),
      confirmLabel: t("sb.deleteSession"),
      tone: "danger",
    });
    if (ok) run(store.deleteSession(key, s.id));
  };

  const submitRename = (e: FormEvent) => {
    e.preventDefault();
    const r = rename;
    if (!r?.value.trim()) return;
    setRename(null);
    run(r.kind === "project" ? store.renameProject(r.id, r.value) : store.renameSession(r.key, r.id, r.value));
  };

  if (!state.loaded) return <p className="sidebar__note z-small">{t("sb.loading")}</p>;

  return (
    <>
      {state.projects.length === 0 && <p className="sidebar__note z-small">{t("sb.noProjects")}</p>}
      <ul className="tree" aria-label={t("sb.tab.sessions")}>
        {groups.map(({ key, project }) => {
          const group = state.sessions[key];
          const items = (group?.items ?? []).filter(
            (s) => !q || (sessionTitle(s) ?? t("sb.untitled")).toLowerCase().includes(q),
          );
          if (q && items.length === 0) return null;
          const open = q ? true : !collapsed.has(key);
          const label = project ? project.name : t("sb.chatGroup");
          const pstyle = project ? appearance.projects[project.id] : undefined;
          const refresh: MenuEntry = {
            type: "item",
            id: "refresh",
            label: t("sb.refresh"),
            onSelect: () => run(store.refresh(key)),
          };
          const entries: MenuEntry[] = project
            ? [
                {
                  type: "item",
                  id: "rename",
                  label: t("sb.rename"),
                  onSelect: () => setRename({ kind: "project", id: project.id, value: project.name }),
                },
                refresh,
                { type: "separator", id: "sep" },
                {
                  type: "item",
                  id: "remove",
                  label: t("sb.removeProject"),
                  danger: true,
                  onSelect: () => void removeProject(project),
                },
              ]
            : [refresh];
          return (
            <li
              key={key}
              className="tree__group"
              data-styled={pstyle ? "" : undefined}
              style={
                pstyle
                  ? ({
                      "--z-project-border": pstyle.border,
                      "--z-project-bg": pstyle.background,
                    } as React.CSSProperties)
                  : undefined
              }
            >
              <ContextMenu entries={entries} className="tree__row">
                <button
                  type="button"
                  className="tree__toggle"
                  aria-expanded={open}
                  data-selected={state.selection.key === key && !state.selection.sessionId ? "" : undefined}
                  title={project?.path}
                  onClick={() => {
                    toggle(key);
                    store.select(key, null);
                  }}
                >
                  {open ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
                  {pstyle?.logo ? (
                    <span className="tree__logo" aria-hidden="true">
                      {pstyle.logo}
                    </span>
                  ) : project ? (
                    <FolderIcon size="sm" />
                  ) : (
                    <ChatIcon size="sm" />
                  )}
                  <span className="tree__label">{label}</span>
                </button>
                <IconButton
                  label={t("sb.newSessionIn", { name: label })}
                  size="sm"
                  icon={<PlusIcon />}
                  className="tree__action"
                  disabled={!state.engineReady}
                  onClick={() => run(store.createSession(key))}
                />
                <Menu
                  trigger={
                    <IconButton
                      label={t("sb.menuFor", { name: label })}
                      size="sm"
                      icon={<MoreIcon />}
                      className="tree__action"
                    />
                  }
                  entries={entries}
                />
              </ContextMenu>
              {open && (
                <SessionRows
                  keyId={key}
                  group={group}
                  items={items}
                  onDelete={deleteSession}
                  onRename={(s) => setRename({ kind: "session", key, id: s.id, value: sessionTitle(s) ?? "" })}
                />
              )}
            </li>
          );
        })}
      </ul>
      <Dialog
        open={rename !== null}
        onOpenChange={(o) => !o && setRename(null)}
        title={rename?.kind === "project" ? t("sb.renameProjectTitle") : t("sb.renameSessionTitle")}
        footer={
          <>
            <Button onClick={() => setRename(null)}>{t("ui.cancel")}</Button>
            <Button variant="solid" type="submit" form="rename-form" disabled={!rename?.value.trim()}>
              {t("sb.save")}
            </Button>
          </>
        }
      >
        <form id="rename-form" onSubmit={submitRename}>
          <Field label={t("sb.name")}>
            {(c) => (
              <Input
                {...c}
                value={rename?.value ?? ""}
                onChange={(e) => setRename((r) => (r ? { ...r, value: e.target.value } : r))}
                autoFocus
              />
            )}
          </Field>
        </form>
      </Dialog>
    </>
  );
}

function SessionRows({
  keyId,
  group,
  items,
  onDelete,
  onRename,
}: {
  keyId: ProjectKey;
  group: SessionsState | undefined;
  items: SessionInfo[];
  onDelete: (key: ProjectKey, s: SessionInfo) => void;
  onRename: (s: SessionInfo) => void;
}) {
  const { t } = useI18n();
  const { projects: store } = useServices();
  const state = useProjects();
  if (!state.engineReady && (!group || group.status === "idle"))
    return <p className="tree__note z-small">{t("sb.engineWait")}</p>;
  if (group?.status === "loading" && items.length === 0)
    return (
      <p className="tree__note z-small">
        <Spinner size="sm" /> {t("sb.loading")}
      </p>
    );
  if (group?.status === "error")
    return <p className="tree__note tree__note--error z-small">{t("sb.loadError", { error: group.error ?? "" })}</p>;
  if (items.length === 0) return <p className="tree__note z-small">{t("sb.noSessions")}</p>;
  return (
    <ul className="tree__children">
      {items.map((s) => {
        const title = sessionTitle(s) ?? t("sb.untitled");
        const selected = state.selection.sessionId === s.id;
        const entries: MenuEntry[] = [
          { type: "item", id: "rename", label: t("sb.rename"), onSelect: () => onRename(s) },
          {
            type: "item",
            id: "delete",
            label: t("sb.deleteSession"),
            danger: true,
            onSelect: () => onDelete(keyId, s),
          },
        ];
        return (
          <li key={s.id}>
            <ContextMenu entries={entries} className="tree__row">
              <button
                type="button"
                className="tree__session"
                aria-current={selected ? "true" : undefined}
                onClick={() => store.select(keyId, s.id)}
              >
                <span className="tree__label">{title}</span>
              </button>
              <Menu
                trigger={
                  <IconButton
                    label={t("sb.menuFor", { name: title })}
                    size="sm"
                    icon={<MoreIcon />}
                    className="tree__action"
                  />
                }
                entries={entries}
              />
            </ContextMenu>
          </li>
        );
      })}
    </ul>
  );
}

function SidebarFooter() {
  const { t } = useI18n();
  const { host, nav } = useServices();
  const engine = useEngineState();
  const tone =
    engine.status === "ready" ? "ok" : engine.status === "error" || engine.status === "stopped" ? "bad" : "wait";
  return (
    <footer className="sidebar__foot">
      <span className="sidebar__status" data-tone={tone} role="status" title={engine.error ?? undefined}>
        <span className="sidebar__dot" aria-hidden="true" />
        {t(`engine.${engine.status}` as never)}
        {engine.version && engine.status === "ready" ? ` ${engine.version}` : ""}
      </span>
      {(engine.status === "error" || engine.status === "stopped") && host.native && (
        <IconButton label={t("engine.retry")} size="sm" icon={<RefreshIcon />} onClick={() => void host.restart()} />
      )}
      <IconButton label={t("auto.manage")} size="sm" icon={<ClockIcon />} onClick={() => nav.push("/automation")} />
      <IconButton label={t("mem.manage")} size="sm" icon={<BrainIcon />} onClick={() => nav.push("/memory")} />
      <IconButton label={t("pv.manage")} size="sm" icon={<SettingsIcon />} onClick={() => nav.push("/providers")} />
      <span className="z-small sidebar__version">{t("app.version", { version: build.version })}</span>
    </footer>
  );
}

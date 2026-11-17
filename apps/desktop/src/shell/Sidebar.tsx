// Panel 1: project dan sesi (Fase 4). Sumber kebenaran: ProjectStore (core) — komponen ini hanya tampilan + aksi.
import {
  CHAT_KEY,
  type Project,
  type ProjectKey,
  type ProjectStyle,
  pathKey,
  type SessionInfo,
  type SessionsState,
  sessionTitle,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  AppLogo,
  Avatar,
  Badge,
  BrainIcon,
  Button,
  ChatIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ClockIcon,
  CloseIcon,
  CollapseAllIcon,
  ContextMenu,
  CopyIcon,
  Dialog,
  EditIcon,
  ExpandAllIcon,
  EyeIcon,
  Field,
  FileIcon,
  FolderIcon,
  FolderOpenIcon,
  GlobeIcon,
  IconButton,
  ImageIcon,
  Input,
  Kbd,
  Menu,
  type MenuEntry,
  MoreIcon,
  PinIcon,
  PlusIcon,
  RefreshIcon,
  SearchField,
  SearchIcon,
  SettingsIcon,
  Spinner,
  Tabs,
  TrashIcon,
  UserIcon,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { type FormEvent, useMemo, useState } from "react";
import { build } from "../build";
import { useIsDevBuild } from "./app-flavor";
import { DEMO_KEY, DEMO_SESSIONS } from "./demo-sessions";
import { targetIndex, useDragReorder } from "./dnd";
import { type FilesScope, FilesTree } from "./FilesTree";
import { ProjectStyleDialog, projectBackground } from "./ProjectStyleDialog";
import { useProfile } from "./profile";
import { SessionDot, SessionDotFor } from "./SessionDot";
import { useAppearance, useEngineState, useProjects, useServices, useSettings, useSidebarPrefs } from "./services";

type RenameTarget =
  | { kind: "project"; id: string; value: string }
  | { kind: "session"; key: ProjectKey; id: string; value: string };

function useErrorToast() {
  const toast = useToast();
  const { t } = useI18n();
  return (e: unknown) =>
    toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
}

const slashes = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");

/** Folder yang ditampilkan tab Files: folder project terpilih, atau folder kerja sesi Chat yang sedang dibuka. */
export function filesScopeOf(state: ReturnType<typeof useProjects>): FilesScope | null {
  const { key, sessionId } = state.selection;
  const project = state.projects.find((p) => p.id === key);
  if (project) return { id: project.id, name: project.name, path: project.path, root: project.path, prefix: "" };
  if (key !== CHAT_KEY || !state.chatDir) return null;
  const session = sessionId ? state.sessions[CHAT_KEY]?.items.find((s) => s.id === sessionId) : null;
  const dir = session?.directory || state.chatDir;
  const root = slashes(state.chatDir);
  const d = slashes(dir);
  const prefix = d.toLowerCase().startsWith(`${root.toLowerCase()}/`) ? d.slice(root.length + 1) : "";
  return { id: CHAT_KEY, name: prefix || "Chat", path: dir, root: state.chatDir, prefix };
}

export function Sidebar() {
  const { t } = useI18n();
  const state = useProjects();
  const [query, setQuery] = useState("");
  const [searching, setSearching] = useState(false);
  const [tab, setTab] = useState("sessions");
  const [workspaceOpen, setWorkspaceOpen] = useState(true);
  // project yang ditutup; `beforeCollapseAll` menyimpan keadaan sebelum "tutup semua" agar bisa dikembalikan
  const [collapsed, setCollapsed] = useState<Set<ProjectKey>>(new Set());
  const [beforeCollapseAll, setBeforeCollapseAll] = useState<Set<ProjectKey> | null>(null);
  const toggleCollapsed = (key: ProjectKey) => {
    setBeforeCollapseAll(null);
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(key)) next.add(key);
      return next;
    });
  };
  const collapseAllOrRestore = () => {
    if (beforeCollapseAll) {
      setCollapsed(beforeCollapseAll);
      setBeforeCollapseAll(null);
    } else {
      setBeforeCollapseAll(collapsed);
      setCollapsed(new Set([...collapsed, ...state.projects.map((p) => p.id as ProjectKey)]));
    }
  };
  const devBuild = useIsDevBuild();
  const filesScope = useMemo(() => filesScopeOf(state), [state]);

  return (
    <div className="sidebar">
      <div className="sidebar__head">
        <span className="sidebar__logo">
          <AppLogo size="lg" />
        </span>
        <div className="sidebar__brandwrap">
          <h2 className="z-h4 sidebar__brand">{t("app.name")}</h2>
          {devBuild && (
            <Badge tone="warning" title={t("app.devHint")}>
              {t("app.devLabel")}
            </Badge>
          )}
        </div>
        <IconButton
          label={t("sb.searchToggle")}
          size="sm"
          icon={<SearchIcon />}
          aria-pressed={searching}
          onClick={() => {
            setTab("sessions");
            setSearching((v) => !v);
            if (searching) setQuery("");
          }}
        />
      </div>
      {searching && (
        <div className="sidebar__search">
          <SearchField
            aria-label={t("sb.search")}
            placeholder={t("sb.search")}
            value={query}
            onValueChange={setQuery}
            autoFocus
          />
        </div>
      )}
      <Tabs
        aria-label={t("panel.sidebar")}
        value={tab}
        onValueChange={setTab}
        items={[
          {
            value: "sessions",
            label: t("sb.tab.sessions"),
            icon: <ChatIcon size="sm" />,
            content: (
              <div className="sidebar__tab">
                <SidebarActions />
                <button
                  type="button"
                  className="sidebar__workspace"
                  aria-expanded={workspaceOpen}
                  onClick={() => setWorkspaceOpen((v) => !v)}
                >
                  <span className="z-label">{t("sb.workspace")}</span>
                  {workspaceOpen ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
                </button>
                {workspaceOpen && (
                  <div className="sidebar__scroll">
                    <div className="sidebar__section">
                      <header className="sidebar__section-head">
                        <h3 className="z-label">
                          {t("sb.section.projects")} <span className="sidebar__count">({state.projects.length})</span>
                        </h3>
                        <span className="sidebar__section-tools">
                          <AddProject />
                          {state.projects.length > 0 && (
                            <IconButton
                              label={beforeCollapseAll ? t("sb.restoreProjects") : t("sb.collapseProjects")}
                              size="sm"
                              icon={beforeCollapseAll ? <ExpandAllIcon /> : <CollapseAllIcon />}
                              aria-pressed={beforeCollapseAll ? true : undefined}
                              onClick={collapseAllOrRestore}
                            />
                          )}
                        </span>
                      </header>
                      <SessionsList query={query} kind="projects" collapsed={collapsed} onToggle={toggleCollapsed} />
                    </div>
                    <div className="sidebar__section sidebar__section--chats">
                      <SessionsList query={query} kind="chats" collapsed={collapsed} onToggle={toggleCollapsed} />
                    </div>
                  </div>
                )}
              </div>
            ),
          },
          {
            value: "files",
            label: t("sb.tab.files"),
            icon: <FileIcon size="sm" />,
            content: <FilesTree project={filesScope} />,
          },
        ]}
      />
      <SidebarFooter />
    </div>
  );
}

/** Sesi demo bawaan (di dalam daftar Chat): disematkan, permanen (tidak bisa diganti nama/dihapus), hanya-baca. */
function DemoRows() {
  const { t } = useI18n();
  const { projects } = useServices();
  const state = useProjects();
  return (
    <ul className="tree__children sidebar__demo-rows" aria-label={t("sb.section.demo")}>
      {DEMO_SESSIONS.map((d) => (
        <li key={d.id}>
          <div className="tree__row">
            <button
              type="button"
              className="tree__demo"
              title={t(d.subtitleKey)}
              aria-current={state.selection.key === DEMO_KEY && state.selection.sessionId === d.id ? "true" : undefined}
              onClick={() => projects.select(DEMO_KEY, d.id)}
            >
              <SessionDot state="idle" />
              <span className="tree__label">{t(d.titleKey)}</span>
              <PinIcon size="sm" className="tree__pin" />
            </button>
          </div>
        </li>
      ))}
    </ul>
  );
}

function SidebarActions() {
  const { t } = useI18n();
  const { nav, projects } = useServices();
  return (
    <div className="sidebar__actions">
      {/* draf sesi baru tanpa project; folder bisa diganti dari pil lingkup sebelum pesan pertama */}
      <Button className="sidebar__action" onClick={() => projects.select(CHAT_KEY, null)}>
        <PlusIcon size="sm" />
        <span className="sidebar__action-label">{t("sb.newChat")}</span>
        <Kbd keys="Ctrl+N" />
      </Button>
      <Button className="sidebar__action" onClick={() => nav.push("/automation")}>
        <ClockIcon size="sm" />
        <span className="sidebar__action-label">{t("auto.manage")}</span>
      </Button>
    </div>
  );
}

/** Tombol tambah project (pilih folder / path); `labelled` = tombol berteks (dipakai tur pengenalan). */
export function AddProject({ labelled = false, onAdded }: { labelled?: boolean; onAdded?: () => void } = {}) {
  const { t } = useI18n();
  const { host, projects } = useServices();
  const fail = useErrorToast();
  const [open, setOpen] = useState(false);
  const [path, setPath] = useState("");
  const confirm = useConfirm();
  const state = useProjects();

  const add = async (p: string) => {
    try {
      // Folder baru harus dipercaya dulu: agent bisa membaca, mengubah, dan menjalankan perintah di dalamnya.
      const known = state.projects.some((x) => pathKey(x.path) === pathKey(p));
      if (!known) {
        const ok = await confirm({
          title: t("sb.trustTitle"),
          description: t("sb.trustBody", { path: p }),
          confirmLabel: t("sb.trustYes"),
        });
        if (!ok) return;
      }
      await projects.addProject(p);
      onAdded?.();
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
      {labelled ? (
        <Button variant="solid" onClick={() => void onClick()}>
          <PlusIcon size="sm" /> {t("sb.addProject")}
        </Button>
      ) : (
        <IconButton label={t("sb.addProject")} size="sm" icon={<PlusIcon />} onClick={() => void onClick()} />
      )}
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

/** Gaya inline baris project dari gaya kustom (latar warna/gradien/gambar dan border). */
function rowStyle(s: ProjectStyle | undefined): React.CSSProperties | undefined {
  const bg = projectBackground(s);
  if (!bg && !s?.border && !s?.fg) return undefined;
  return {
    ...(bg ? { background: bg } : {}),
    ...(s?.fg ? ({ "--pstyle-fg": s.fg } as React.CSSProperties) : {}),
    ...(s?.border ? { boxShadow: `inset 0 0 0 var(--z-border-width) ${s.border}` } : {}),
  };
}

function SessionsList({
  query,
  kind,
  collapsed,
  onToggle: toggle,
}: {
  query: string;
  kind: "projects" | "chats";
  collapsed: Set<ProjectKey>;
  onToggle: (key: ProjectKey) => void;
}) {
  const { t } = useI18n();
  const settings = useSettings();
  const { projects: store, sidebarPrefs, docHost } = useServices();
  const prefs = useSidebarPrefs();
  const _toast = useToast();
  const [styling, setStyling] = useState<Project | null>(null);
  const state = useProjects();
  const appearance = useAppearance();
  const confirm = useConfirm();
  const fail = useErrorToast();
  const [rename, setRename] = useState<RenameTarget | null>(null);
  const q = query.trim().toLowerCase();

  // urutan project: disematkan dulu, lalu urutan buatan pengguna (membaca `prefs` agar render ulang saat berubah)
  const groups: Array<{ key: ProjectKey; project: Project | null }> =
    kind === "projects"
      ? sidebarPrefs.sortProjects(state.projects, (p) => p.id).map((p) => ({ key: p.id, project: p as Project | null }))
      : [{ key: CHAT_KEY as ProjectKey, project: null as Project | null }];
  void prefs;
  const projectDnd = useDragReorder((dragId, targetId, before) => {
    const ids = groups.map((g) => g.key);
    sidebarPrefs.moveProject(ids, dragId, targetIndex(ids, dragId, targetId, before));
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
    if (!settings.confirmDelete) {
      run(store.deleteSession(key, s.id));
      return;
    }
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
      {kind === "projects" && state.projects.length === 0 && (
        <p className="sidebar__note z-small">{t("sb.noProjects")}</p>
      )}
      <ul className="tree" aria-label={kind === "projects" ? t("sb.section.projects") : t("sb.section.chats")}>
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
            icon: <RefreshIcon size="sm" />,
            label: t("sb.refresh"),
            onSelect: () => run(store.refresh(key)),
          };
          const entries: MenuEntry[] = project
            ? [
                {
                  type: "item",
                  id: "new",
                  icon: <PlusIcon size="sm" />,
                  label: t("sb.newSession"),
                  disabled: !state.engineReady,
                  onSelect: () => run(store.createSession(key)),
                },
                {
                  type: "item",
                  id: "pin",
                  icon: <PinIcon size="sm" />,
                  label: prefs.pinnedProjects.includes(project.id) ? t("sb.unpin") : t("sb.pin"),
                  onSelect: () => sidebarPrefs.toggleProjectPin(project.id),
                },
                {
                  type: "item",
                  id: "rename",
                  icon: <EditIcon size="sm" />,
                  label: t("sb.rename"),
                  onSelect: () => setRename({ kind: "project", id: project.id, value: project.name }),
                },
                {
                  type: "item",
                  id: "style",
                  icon: <ImageIcon size="sm" />,
                  label: t("sb.customize"),
                  onSelect: () => setStyling(project),
                },
                refresh,
                ...(docHost.reveal
                  ? [
                      {
                        type: "item" as const,
                        id: "reveal",
                        icon: <FolderOpenIcon size="sm" />,
                        label: t("sb.reveal"),
                        onSelect: () => void docHost.reveal?.(project.path, ".").catch(fail),
                      },
                    ]
                  : []),
                { type: "separator", id: "sep" },
                {
                  type: "item",
                  id: "remove",
                  icon: <CloseIcon size="sm" />,
                  label: t("sb.removeProject"),
                  danger: true,
                  onSelect: () => void removeProject(project),
                },
              ]
            : [refresh];
          return (
            <li key={key} className="tree__group" {...(kind === "projects" ? projectDnd.itemProps(key) : {})}>
              {kind === "chats" ? (
                <header className="sidebar__section-head">
                  <h3 className="z-label" title={t("sb.chatHint")}>
                    {t("sb.section.chats")} <span className="sidebar__count">({group?.items.length ?? 0})</span>
                  </h3>
                  <IconButton
                    label={t("sb.newSessionIn", { name: label })}
                    size="sm"
                    icon={<PlusIcon />}
                    disabled={!state.engineReady}
                    onClick={() => run(store.createSession(key))}
                  />
                </header>
              ) : (
                <ContextMenu
                  entries={entries}
                  className="tree__row"
                  triggerProps={{
                    style: rowStyle(pstyle),
                    "data-bg": projectBackground(pstyle) ? "" : undefined,
                    "data-fg": pstyle?.fg ? "" : undefined,
                  }}
                >
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
                    {pstyle?.logoImage ? (
                      <img className="tree__logoimg" src={pstyle.logoImage} alt="" />
                    ) : pstyle?.logo ? (
                      <span className="tree__logo" aria-hidden="true">
                        {pstyle.logo}
                      </span>
                    ) : project ? (
                      open ? (
                        <FolderOpenIcon size="sm" />
                      ) : (
                        <FolderIcon size="sm" />
                      )
                    ) : (
                      <ChatIcon size="sm" />
                    )}
                    <span className="tree__label">{label}</span>
                    {project && prefs.pinnedProjects.includes(project.id) && (
                      <PinIcon size="sm" className="tree__pinmark" />
                    )}
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
              )}
              {open && kind === "chats" && settings.showDemos && <DemoRows />}
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
      {styling && <ProjectStyleDialog project={styling} onClose={() => setStyling(null)} />}
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
  const { projects: store, sidebarPrefs } = useServices();
  const prefs = useSidebarPrefs();
  const toast = useToast();
  const state = useProjects();
  // urutan tampil: disematkan dulu, lalu urutan buatan pengguna, lalu bawaan; seret-lepas mengubah urutan buatan
  // biome-ignore lint/correctness/useExhaustiveDependencies: `prefs` memicu pengurutan ulang
  const sorted = useMemo(
    () => sidebarPrefs.sortSessions(keyId, items, (s) => s.id),
    [sidebarPrefs, keyId, items, prefs],
  );
  const dnd = useDragReorder((dragId, targetId, before) => {
    const ids = sorted.map((s) => s.id);
    sidebarPrefs.moveSession(keyId, ids, dragId, targetIndex(ids, dragId, targetId, before));
  });
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
    <ul className="tree__children sidebar__session-rows">
      {sorted.map((s) => {
        const title = sessionTitle(s) ?? t("sb.untitled");
        const selected = state.selection.sessionId === s.id;
        const pinned = prefs.pinnedSessions.includes(s.id);
        const entries: MenuEntry[] = [
          {
            type: "item",
            id: "pin",
            icon: <PinIcon size="sm" />,
            label: pinned ? t("sb.unpin") : t("sb.pin"),
            onSelect: () => sidebarPrefs.toggleSessionPin(s.id),
          },
          {
            type: "item",
            id: "rename",
            icon: <EditIcon size="sm" />,
            label: t("sb.rename"),
            onSelect: () => onRename(s),
          },
          {
            type: "item",
            id: "copyid",
            icon: <CopyIcon size="sm" />,
            label: t("sb.copyId"),
            onSelect: () =>
              void navigator.clipboard
                ?.writeText(s.id)
                .then(() => toast.show({ title: t("sb.copied"), tone: "success", timeoutMs: 1200 })),
          },
          { type: "separator", id: "sep" },
          {
            type: "item",
            id: "delete",
            icon: <TrashIcon size="sm" />,
            label: t("sb.deleteSession"),
            danger: true,
            onSelect: () => onDelete(keyId, s),
          },
        ];
        return (
          <li key={s.id} className="tree__session-item" {...dnd.itemProps(s.id)}>
            <ContextMenu entries={entries} className="tree__row">
              <button
                type="button"
                className="tree__session"
                title={keyId === CHAT_KEY ? t("sb.chatFolderHint", { path: s.directory }) : undefined}
                aria-current={selected ? "true" : undefined}
                onClick={() => store.select(keyId, s.id)}
              >
                <SessionDotFor sessionId={s.id} />
                <span className="tree__label">{title}</span>
                {pinned && <PinIcon size="sm" className="tree__pinmark" />}
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
  const { name, src } = useProfile();
  const tone =
    engine.status === "ready" ? "ok" : engine.status === "error" || engine.status === "stopped" ? "bad" : "wait";
  const entries: MenuEntry[] = [
    {
      type: "item",
      id: "profile",
      icon: <UserIcon size="sm" />,
      label: t("profile.manage"),
      onSelect: () => nav.push("/settings/profile"),
    },
    {
      type: "item",
      id: "settings",
      icon: <SettingsIcon size="sm" />,
      label: t("settings.title"),
      onSelect: () => nav.push("/settings"),
    },
    {
      type: "item",
      id: "providers",
      icon: <GlobeIcon size="sm" />,
      label: t("pv.manage"),
      onSelect: () => nav.push("/settings/models"),
    },
    {
      type: "item",
      id: "memory",
      icon: <BrainIcon size="sm" />,
      label: t("mem.manage"),
      onSelect: () => nav.push("/memory"),
    },
    {
      type: "item",
      id: "appearance",
      icon: <EyeIcon size="sm" />,
      label: t("cmd.appearance"),
      onSelect: () => nav.push("/appearance"),
    },
  ];
  return (
    <footer className="sidebar__foot">
      <Avatar name={name} src={src} size="md" />
      <div className="sidebar__user">
        <span className="sidebar__user-name">{name}</span>
        <span className="z-small sidebar__version">{t("sb.versionShort", { version: build.version })}</span>
        <span
          className={engine.status === "ready" ? "sidebar__status z-visually-hidden" : "sidebar__status"}
          data-tone={tone}
          role="status"
          title={engine.error ?? undefined}
        >
          <span className="sidebar__dot" aria-hidden="true" />
          {t(`engine.${engine.status}` as never)}
          {engine.version && engine.status === "ready" ? ` ${engine.version}` : ""}
        </span>
      </div>
      {(engine.status === "error" || engine.status === "stopped") && host.native && (
        <IconButton label={t("engine.retry")} size="sm" icon={<RefreshIcon />} onClick={() => void host.restart()} />
      )}
      <Menu trigger={<IconButton label={t("sb.user.menu")} size="sm" icon={<MoreIcon />} />} entries={entries} />
    </footer>
  );
}

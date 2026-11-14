// Tab Files: pohon berkas nyata dari project aktif, dimuat malas per folder lewat engine (GET /file?path=). Header:
// nama project (ganti project), berkas/folder baru, tutup/buka semua, muat ulang. Klik berkas membukanya di Panel 3.
import type { FileNode } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Button,
  ChevronDownIcon,
  ChevronRightIcon,
  CollapseAllIcon,
  ExpandAllIcon,
  FilePlusIcon,
  FolderIcon,
  FolderPlusIcon,
  IconButton,
  Input,
  Menu,
  RefreshIcon,
  SearchField,
  Spinner,
  useToast,
} from "@zandor/ui";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FileTypeIcon } from "./FileTypeIcon";
import { useProjects, useServices } from "./services";

/**
 * Yang ditampilkan Files: folder kerja (`path`, dibaca engine) beserta root terdaftar tempat berkas dibuka/dibuat (`root`)
 * dan awalan relatif `path` terhadap `root`. Project: root = path, awalan kosong. Sesi Chat: root = folder Chat,
 * awalan = nama folder sesi (folder sesi bukan root terdaftar, tetapi berada di bawah folder Chat).
 */
export type FilesScope = { id: string; name: string; path: string; root: string; prefix: string };

type Listing = { status: "loading" | "ready" | "error"; nodes: FileNode[]; error: string | null };
type Creating = { parent: string; dir: boolean };

/** Batas folder yang dibuka sekaligus oleh "Buka semua" dan pencarian (melindungi repo besar). */
const MAX_EXPAND = 150;
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const sortNodes = (nodes: FileNode[]) =>
  [...nodes].sort((a, b) => (a.type === b.type ? a.name.localeCompare(b.name) : a.type === "directory" ? -1 : 1));
const underRoot = (prefix: string, path: string) => (!prefix ? path : path === "." ? prefix : `${prefix}/${path}`);
const joinPath = (parent: string, name: string) => (parent === "." ? name : `${parent}/${name}`);

export function FilesTree({ project }: { project: FilesScope | null }) {
  const { t } = useI18n();
  const { projects, docs, docHost, panels } = useServices();
  const state = useProjects();
  const toast = useToast();
  const [listings, setListings] = useState<Record<string, Listing>>({});
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState<Creating | null>(null);
  const [name, setName] = useState("");
  const [capped, setCapped] = useState(false);
  const listingsRef = useRef(listings);
  listingsRef.current = listings;
  const canCreate = typeof docHost.createEntry === "function";

  const fetchDir = useCallback(
    async (path: string, force = false): Promise<FileNode[]> => {
      const api = projects.engineApi();
      if (!project || !api) return [];
      const cached = listingsRef.current[path];
      if (!force && cached?.status === "ready") return cached.nodes;
      setListings((l) => ({ ...l, [path]: { status: "loading", nodes: l[path]?.nodes ?? [], error: null } }));
      try {
        const nodes = await api.listFiles(project.path, path);
        setListings((l) => ({ ...l, [path]: { status: "ready", nodes, error: null } }));
        return nodes;
      } catch (e) {
        setListings((l) => ({ ...l, [path]: { status: "error", nodes: [], error: errText(e) } }));
        return [];
      }
    },
    [project, projects],
  );

  // Ganti project atau status engine → mulai dari nol.
  useEffect(() => {
    setListings({});
    setOpen(new Set());
    setQuery("");
    setCreating(null);
    setCapped(false);
    if (project && state.engineReady) void fetchDir(".", true);
  }, [project, state.engineReady, fetchDir]);

  /** Buka semua folder (kecuali yang diabaikan git) secara bertingkat, dibatasi MAX_EXPAND. */
  const expandAll = useCallback(async () => {
    const opened = new Set<string>();
    let frontier = ["."];
    let hitCap = false;
    while (frontier.length > 0 && !hitCap) {
      const lists = await Promise.all(frontier.map((p) => fetchDir(p)));
      const next: string[] = [];
      for (const nodes of lists)
        for (const n of nodes) {
          if (n.type !== "directory" || n.ignored) continue;
          if (opened.size >= MAX_EXPAND) {
            hitCap = true;
            break;
          }
          opened.add(n.path);
          next.push(n.path);
        }
      frontier = next;
    }
    setOpen(opened);
    setCapped(hitCap);
  }, [fetchDir]);

  const refresh = useCallback(async () => {
    const dirs = [".", ...open];
    await Promise.all(dirs.map((d) => fetchDir(d, true)));
  }, [open, fetchDir]);

  // Mencari butuh daftar yang termuat: saat mulai mengetik, buka semua (sekali, bertahap).
  const searched = useRef(false);
  useEffect(() => {
    if (query.trim() && !searched.current && project) {
      searched.current = true;
      void expandAll();
    }
    if (!query.trim()) searched.current = false;
  }, [query, project, expandAll]);

  const toggle = (node: FileNode) => {
    setOpen((prev) => {
      const next = new Set(prev);
      if (!next.delete(node.path)) {
        next.add(node.path);
        void fetchDir(node.path);
      }
      return next;
    });
  };

  const openFile = (path: string) => {
    if (!project) return;
    docs.open(project.root, underRoot(project.prefix, path));
    panels.actions?.reveal("docs");
  };

  const startCreate = (parent: string, dir: boolean) => {
    if (!canCreate) return;
    setName("");
    setCreating({ parent, dir });
    if (parent !== ".") {
      setOpen((prev) => new Set(prev).add(parent));
      void fetchDir(parent);
    }
  };

  const confirmCreate = async () => {
    if (!creating || !project || !docHost.createEntry) return;
    const value = name.trim();
    if (!value) return setCreating(null);
    try {
      await docHost.createEntry(project.root, underRoot(project.prefix, creating.parent), value, creating.dir);
      await fetchDir(creating.parent, true);
      const made = joinPath(creating.parent, value);
      setCreating(null);
      if (creating.dir) setOpen((prev) => new Set(prev).add(made));
      else openFile(made);
    } catch (e) {
      toast.show({ title: t("files.createFail", { error: errText(e) }), tone: "danger" });
    }
  };

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return null;
    const out: Array<{ node: FileNode; dir: string }> = [];
    for (const [dir, l] of Object.entries(listings))
      for (const n of l.nodes) if (n.name.toLowerCase().includes(q)) out.push({ node: n, dir });
    return out.sort((a, b) => a.node.path.localeCompare(b.node.path)).slice(0, 200);
  }, [query, listings]);

  const projectMenu = state.projects.map((p) => ({
    type: "checkbox" as const,
    id: p.id,
    label: p.name,
    checked: p.id === project?.id,
    onCheckedChange: () => projects.select(p.id, state.selection.key === p.id ? state.selection.sessionId : null),
  }));

  const head = (
    <header className="ftree__head">
      <Menu
        trigger={
          <Button variant="ghost" size="sm" className="ftree__project" aria-label={t("files.switchProject")}>
            <FolderIcon size="sm" />
            <span className="tree__label">{project?.name ?? t("files.pickProject")}</span>
            <ChevronDownIcon size="sm" />
          </Button>
        }
        entries={
          projectMenu.length > 0
            ? projectMenu
            : [{ type: "item", id: "none", label: t("files.noneProject"), disabled: true, onSelect: () => {} }]
        }
      />
      <div className="ftree__actions">
        <IconButton
          size="sm"
          label={canCreate ? t("files.newFile") : t("files.noCreate")}
          icon={<FilePlusIcon size="sm" />}
          disabled={!project || !canCreate}
          onClick={() => startCreate(".", false)}
        />
        <IconButton
          size="sm"
          label={canCreate ? t("files.newFolder") : t("files.noCreate")}
          icon={<FolderPlusIcon size="sm" />}
          disabled={!project || !canCreate}
          onClick={() => startCreate(".", true)}
        />
        <IconButton
          size="sm"
          label={t("files.collapseAll")}
          icon={<CollapseAllIcon size="sm" />}
          disabled={!project || open.size === 0}
          onClick={() => {
            setOpen(new Set());
            setCapped(false);
          }}
        />
        <IconButton
          size="sm"
          label={t("files.expandAll")}
          icon={<ExpandAllIcon size="sm" />}
          disabled={!project}
          onClick={() => void expandAll()}
        />
        <IconButton
          size="sm"
          label={t("files.refresh")}
          icon={<RefreshIcon size="sm" />}
          disabled={!project}
          onClick={() => void refresh()}
        />
      </div>
    </header>
  );

  if (!project)
    return (
      <div className="sidebar__tab ftree">
        {head}
        <p className="sidebar__note z-small">{t("files.noProject")}</p>
      </div>
    );

  const createRow = (parent: string) =>
    creating && creating.parent === parent ? (
      <li className="ftree__create">
        <span className="ftree__spacer" aria-hidden="true" />
        <Input
          size="sm"
          autoFocus
          aria-label={creating.dir ? t("files.nameFolder") : t("files.nameFile")}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => setCreating(null)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void confirmCreate();
            } else if (e.key === "Escape") setCreating(null);
          }}
        />
      </li>
    ) : null;

  const renderDir = (path: string, depth: number) => {
    const l = listings[path];
    if (!l || (l.status === "loading" && l.nodes.length === 0))
      return (
        <p className="tree__note z-small">
          <Spinner size="sm" /> {t("sb.loading")}
        </p>
      );
    if (l.status === "error")
      return <p className="tree__note tree__note--error z-small">{t("files.error", { error: l.error ?? "" })}</p>;
    if (l.nodes.length === 0 && !(creating && creating.parent === path))
      return <p className="tree__note z-small">{t("files.empty")}</p>;
    return (
      <ul className="tree__children ftree__list" aria-label={depth === 0 ? t("files.tree") : undefined}>
        {createRow(path)}
        {sortNodes(l.nodes).map((n) => {
          const isOpen = open.has(n.path);
          return (
            <li key={n.path} data-ignored={n.ignored ? "" : undefined}>
              {n.type === "directory" ? (
                <div className="ftree__row">
                  <button type="button" className="tree__toggle" aria-expanded={isOpen} onClick={() => toggle(n)}>
                    {isOpen ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
                    <FileTypeIcon name={n.name} type="directory" expanded={isOpen} />
                    <span className="tree__label">{n.name}</span>
                  </button>
                  {canCreate && (
                    <span className="ftree__rowactions">
                      <IconButton
                        size="sm"
                        label={t("files.newFileIn", { name: n.name })}
                        icon={<FilePlusIcon size="sm" />}
                        onClick={() => startCreate(n.path, false)}
                      />
                      <IconButton
                        size="sm"
                        label={t("files.newFolderIn", { name: n.name })}
                        icon={<FolderPlusIcon size="sm" />}
                        onClick={() => startCreate(n.path, true)}
                      />
                    </span>
                  )}
                </div>
              ) : (
                <div className="ftree__row">
                  <button type="button" className="tree__file" title={n.path} onClick={() => openFile(n.path)}>
                    <span className="ftree__spacer" aria-hidden="true" />
                    <FileTypeIcon name={n.name} type="file" />
                    <span className="tree__label">{n.name}</span>
                  </button>
                </div>
              )}
              {n.type === "directory" && isOpen && renderDir(n.path, depth + 1)}
            </li>
          );
        })}
      </ul>
    );
  };

  return (
    <div className="sidebar__tab ftree">
      {head}
      <SearchField
        value={query}
        onValueChange={setQuery}
        placeholder={t("files.search")}
        aria-label={t("files.search")}
        className="ftree__search"
      />
      <div className="ftree__tree">
        {matches ? (
          matches.length === 0 ? (
            <p className="tree__note z-small">{t("files.noMatch")}</p>
          ) : (
            <ul className="tree__children">
              {matches.map(({ node, dir }) => (
                <li key={node.path}>
                  <div className="ftree__row">
                    <button
                      type="button"
                      className={node.type === "directory" ? "tree__toggle" : "tree__file"}
                      title={node.path}
                      onClick={() => {
                        if (node.type === "file") return openFile(node.path);
                        setQuery("");
                        setOpen((prev) => new Set(prev).add(node.path));
                        void fetchDir(node.path);
                      }}
                    >
                      <FileTypeIcon name={node.name} type={node.type} />
                      <span className="tree__label">{node.name}</span>
                      {dir !== "." && <span className="ftree__where z-small">{dir}</span>}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )
        ) : (
          renderDir(".", 0)
        )}
      </div>
      {capped && <p className="tree__note z-small">{t("files.partial", { max: MAX_EXPAND })}</p>}
    </div>
  );
}

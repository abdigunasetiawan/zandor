// Tab Files: pohon berkas nyata dari project aktif, dimuat malas per folder lewat engine (GET /file?path=).
import type { FileNode, Project } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { ChevronDownIcon, ChevronRightIcon, Spinner } from "@zandor/ui";
import { useCallback, useEffect, useState } from "react";
import { FileTypeIcon } from "./FileTypeIcon";
import { useProjects, useServices } from "./services";

type Listing = { status: "loading" | "ready" | "error"; nodes: FileNode[]; error: string | null };

export function FilesTree({ project }: { project: Project | null }) {
  const { t } = useI18n();
  const { projects } = useServices();
  const { engineReady } = useProjects();
  const [listings, setListings] = useState<Record<string, Listing>>({});
  const [open, setOpen] = useState<Set<string>>(new Set());

  const load = useCallback(
    (path: string) => {
      const api = projects.engineApi();
      if (!project || !api) return;
      setListings((l) => ({ ...l, [path]: { status: "loading", nodes: l[path]?.nodes ?? [], error: null } }));
      api.listFiles(project.path, path).then(
        (nodes) => setListings((l) => ({ ...l, [path]: { status: "ready", nodes, error: null } })),
        (e) =>
          setListings((l) => ({
            ...l,
            [path]: { status: "error", nodes: [], error: e instanceof Error ? e.message : String(e) },
          })),
      );
    },
    [project, projects],
  );

  // Ganti project atau status engine → mulai dari nol.
  useEffect(() => {
    setListings({});
    setOpen(new Set());
    if (project && engineReady) load(".");
  }, [project, engineReady, load]);

  if (!project) return <p className="sidebar__note z-small">{t("files.noProject")}</p>;

  const toggle = (node: FileNode) => {
    setOpen((prev) => {
      const next = new Set(prev);
      if (!next.delete(node.path)) {
        next.add(node.path);
        if (!listings[node.path]) load(node.path);
      }
      return next;
    });
  };

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
    if (l.nodes.length === 0) return <p className="tree__note z-small">{t("files.empty")}</p>;
    const sorted = [...l.nodes].sort((a, b) =>
      a.type === b.type ? a.name.localeCompare(b.name) : a.type === "directory" ? -1 : 1,
    );
    return (
      <ul className="tree__children" aria-label={depth === 0 ? t("files.tree") : undefined}>
        {sorted.map((n) => {
          const isOpen = open.has(n.path);
          return (
            <li key={n.path} data-ignored={n.ignored ? "" : undefined}>
              {n.type === "directory" ? (
                <button type="button" className="tree__toggle" aria-expanded={isOpen} onClick={() => toggle(n)}>
                  {isOpen ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
                  <FileTypeIcon name={n.name} type="directory" expanded={isOpen} />
                  <span className="tree__label">{n.name}</span>
                </button>
              ) : (
                <span className="tree__file">
                  <FileTypeIcon name={n.name} type="file" />
                  <span className="tree__label">{n.name}</span>
                </span>
              )}
              {n.type === "directory" && isOpen && renderDir(n.path, depth + 1)}
            </li>
          );
        })}
      </ul>
    );
  };

  return <div className="sidebar__tab">{renderDir(".", 0)}</div>;
}

// Panel 3: tab dokumen, path, toolbar, dan penampil per jenis berkas (Fase 7).
import { type DocTab, decodeText, hasRawMode, isTextKind } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Button,
  CloseIcon,
  CodeIcon,
  ContextMenu,
  CopyIcon,
  DownloadIcon,
  EditIcon,
  EyeIcon,
  GlobeIcon,
  IconButton,
  type MenuEntry,
  RefreshIcon,
  SaveIcon,
  SearchIcon,
  Spinner,
  useConfirm,
  useToast,
  WrapIcon,
} from "@zandor/ui";
import { lazy, type ReactNode, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { FileTypeIcon } from "../FileTypeIcon";
import { PanelTabBar } from "../PanelTabBar";
import { useAppearance, useDocs, useServices } from "../services";
import { openInZandorBrowser } from "../tools/browser-bridge";
import { DocCustomize } from "./DocCustomize";
import { groupsForViewer } from "./DocStyleControls";
import { OpenWithMenu } from "./OpenWithMenu";
import { BinaryViewer, HtmlViewer, ImageViewer, JsonViewer, MediaViewer, TableViewer } from "./simple-viewers";
import type { ViewerHandle, ViewerProps } from "./types";

const CodeViewer = lazy(() => import("./CodeViewer"));
const MarkdownViewer = lazy(() => import("./MarkdownViewer"));
const PdfViewer = lazy(() => import("./PdfViewer"));
const DiffViewer = lazy(() => import("./DiffViewer"));
const DocxViewer = lazy(() => import("./office-viewers").then((m) => ({ default: m.DocxViewer })));
const XlsxViewer = lazy(() => import("./office-viewers").then((m) => ({ default: m.XlsxViewer })));
const PptxViewer = lazy(() => import("./office-viewers").then((m) => ({ default: m.PptxViewer })));
const NotebookViewer = lazy(() => import("./office-viewers").then((m) => ({ default: m.NotebookViewer })));

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function viewerFor(
  tab: DocTab,
):
  | "code"
  | "markdown"
  | "image"
  | "pdf"
  | "table"
  | "json"
  | "html"
  | "media"
  | "docx"
  | "xlsx"
  | "pptx"
  | "notebook"
  | "diff"
  | "binary" {
  // diff yang ditinjau (ADR-075) didahulukan untuk berkas teks, kecuali sedang diedit
  if (tab.diff && !tab.editing && isTextKind(tab.kind)) return "diff";
  const raw = tab.mode === "raw" && hasRawMode(tab.kind);
  if (tab.kind === "code" || raw) return "code";
  switch (tab.kind) {
    case "markdown":
      return "markdown";
    case "svg":
    case "image":
      return "image";
    case "pdf":
      return "pdf";
    case "csv":
    case "tsv":
      return "table";
    case "json":
      return "json";
    case "html":
      return "html";
    case "audio":
    case "video":
      return "media";
    case "docx":
    case "xlsx":
    case "pptx":
    case "notebook":
      return tab.kind;
    default:
      return "binary";
  }
}

export function DocPanel({ empty }: { empty: ReactNode }) {
  const { t } = useI18n();
  const { docs, docHost } = useServices();
  const { panels } = useServices();
  const state = useDocs();
  const docStyle = useAppearance().docs;
  const toast = useToast();
  const confirm = useConfirm();
  const [handle, setHandle] = useState<ViewerHandle | null>(null);
  const onHandle = useCallback((h: ViewerHandle | null) => setHandle(h), []);
  const tab = state.tabs.find((x) => x.id === state.activeId) ?? null;
  // Tab dokumen terakhir ditutup → Panel 3 ikut tertutup. Hanya pada peralihan ada → kosong, sehingga membuka panel
  // secara manual tanpa dokumen (tombol panel / Ctrl+3) tetap menampilkan keadaan kosongnya.
  const prevCount = useRef(state.tabs.length);
  useEffect(() => {
    if (prevCount.current > 0 && state.tabs.length === 0) panels.actions?.close("docs");
    prevCount.current = state.tabs.length;
  }, [state.tabs.length, panels]);
  if (!tab) {
    return (
      <div className="docs">
        <PanelTabBar
          closeLabel={t("panel.close.docs")}
          onClose={() => panels.actions?.close("docs")}
          actions={<DocCustomize groups={["code", "prose", "table", "image"]} />}
        />
        {empty}
      </div>
    );
  }
  const bytes = docs.content(tab.id);

  const fail = (e: unknown) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
  const ok = (title: string) => toast.show({ title, tone: "success", timeoutMs: 1500 });

  const copyContent = async () => {
    try {
      // dari byte asli (bukan model editor, yang menormalkan akhir baris)
      const text = bytes && isTextKind(tab.kind) ? decodeText(bytes).text : null;
      if (text === null) return;
      await navigator.clipboard.writeText(text);
      ok(t("doc.copied"));
    } catch (e) {
      fail(e);
    }
  };
  const copyPath = async () => {
    try {
      await navigator.clipboard.writeText(
        `${tab.root.replace(/[\\/]+$/, "")}/${tab.path}`.replace(/\//g, tab.root.includes("\\") ? "\\" : "/"),
      );
      ok(t("doc.copied"));
    } catch (e) {
      fail(e);
    }
  };
  const saveCopy = async () => {
    try {
      if (await docHost.saveCopy(tab.root, tab.path, tab.name, bytes)) ok(t("doc.saved"));
    } catch (e) {
      fail(e);
    }
  };
  const openExternal = async () => {
    try {
      await docHost.openExternal(tab.root, tab.path);
    } catch (e) {
      fail(e);
    }
  };

  /** HTML → live server (aset relatif, skrip, auto-reload saat berkas berubah) di Browser Panel 4. */
  const openLive = async () => {
    try {
      if (!docHost.liveUrl) return;
      openInZandorBrowser(panels, await docHost.liveUrl(tab.root, tab.path));
    } catch (e) {
      fail(e);
    }
  };

  const textual = isTextKind(tab.kind) && viewerFor(tab) === "code";
  const block = docs.editBlock(tab.id);

  const confirmDiscard = async (name: string): Promise<boolean> =>
    confirm({
      title: t("doc.discardTitle", { name }),
      description: t("doc.discardBody"),
      confirmLabel: t("doc.discard"),
      tone: "danger",
    });
  const guarded = async (id: string, name: string, fn: () => void) => {
    if (docs.state().tabs.find((x) => x.id === id)?.dirty && !(await confirmDiscard(name))) return;
    fn();
  };
  const save = async () => {
    const text = handle?.text?.();
    if (text == null) return;
    let r = await docs.save(tab.id, text);
    if (r === "conflict") {
      const ok = await confirm({
        title: t("doc.conflictTitle", { name: tab.name }),
        description: t("doc.conflictBody"),
        confirmLabel: t("doc.overwrite"),
        tone: "danger",
      });
      if (!ok) return;
      r = await docs.save(tab.id, text, true);
    }
    if (r === "saved") ok(t("doc.savedEdit"));
    else if (typeof r === "object") fail(r.error);
  };
  /** Tutup beberapa tab sekaligus; yang belum disimpan dikonfirmasi satu kali. */
  const closeMany = async (mode: "one" | "others" | "right" | "left" | "all", anchorId: string) => {
    const tabs = docs.state().tabs;
    const at = tabs.findIndex((x) => x.id === anchorId);
    const targets = tabs.filter((x, i) =>
      mode === "one"
        ? x.id === anchorId
        : mode === "others"
          ? x.id !== anchorId
          : mode === "right"
            ? i > at
            : mode === "left"
              ? i < at
              : true,
    );
    const dirty = targets.filter((x) => x.dirty);
    if (dirty.length === 1 && dirty[0]) {
      if (!(await confirmDiscard(dirty[0].name))) return;
    } else if (dirty.length > 1) {
      const proceed = await confirm({
        title: t("doc.tabsCloseConfirmTitle", { n: dirty.length }),
        description: t("doc.tabsCloseConfirmBody"),
        confirmLabel: t("doc.discard"),
        tone: "danger",
      });
      if (!proceed) return;
    }
    for (const x of targets) docs.close(x.id);
    // tab yang diklik kanan tetap menjadi tab aktif bila masih ada
    if (mode !== "one" && mode !== "all" && docs.state().tabs.some((x) => x.id === anchorId)) docs.activate(anchorId);
  };
  const startEdit = () => {
    const why = docs.beginEdit(tab.id);
    if (why) toast.show({ title: t(`doc.noEdit.${why}` as never), tone: "danger" });
  };

  return (
    <div
      className="docs"
      data-doc-theme={docStyle.theme && docStyle.theme !== "auto" ? docStyle.theme : undefined}
      data-image-bg={docStyle.imageBg && docStyle.imageBg !== "auto" ? docStyle.imageBg : undefined}
    >
      <PanelTabBar
        closeLabel={t("panel.close.docs")}
        onClose={() => panels.actions?.close("docs")}
        actions={<DocCustomize groups={groupsForViewer(viewerFor(tab))} />}
      >
        <nav className="stabs" aria-label={t("doc.tabs")}>
          {state.tabs.map((x, index) => {
            const active = x.id === state.activeId;
            const last = state.tabs.length - 1;
            const entries: MenuEntry[] = [
              { type: "item", id: "close", label: t("tabs.close"), onSelect: () => void closeMany("one", x.id) },
              {
                type: "item",
                id: "others",
                label: t("tabs.closeOthers"),
                disabled: state.tabs.length < 2,
                onSelect: () => void closeMany("others", x.id),
              },
              {
                type: "item",
                id: "right",
                label: t("tabs.closeRight"),
                disabled: index === last,
                onSelect: () => void closeMany("right", x.id),
              },
              {
                type: "item",
                id: "left",
                label: t("tabs.closeLeft"),
                disabled: index === 0,
                onSelect: () => void closeMany("left", x.id),
              },
              { type: "separator", id: "sep" },
              { type: "item", id: "all", label: t("tabs.closeAll"), onSelect: () => void closeMany("all", x.id) },
            ];
            return (
              <ContextMenu key={x.id} entries={entries} className="stabs__ctx">
                <div className="stabs__tab" data-active={active ? "" : undefined}>
                  <button
                    type="button"
                    className="stabs__label"
                    aria-current={active ? "true" : undefined}
                    title={x.path}
                    onClick={() => docs.activate(x.id)}
                  >
                    <FileTypeIcon name={x.name} type="file" />
                    {x.dirty && (
                      <span role="img" aria-label={t("doc.unsaved")} title={t("doc.unsaved")}>
                        ●
                      </span>
                    )}
                    <span className="stabs__text">{x.name}</span>
                  </button>
                  <button
                    type="button"
                    className="stabs__close"
                    aria-label={t("doc.closeTab", { name: x.name })}
                    onClick={() => void guarded(x.id, x.name, () => docs.close(x.id))}
                  >
                    <CloseIcon size="sm" />
                  </button>
                </div>
              </ContextMenu>
            );
          })}
        </nav>
      </PanelTabBar>
      <div className="docs__path z-small" title={`${tab.root}\\${tab.path}`} data-testid="doc-path">
        {tab.path}
      </div>
      <div className="docs__toolbar" role="toolbar" aria-label={t("doc.toolbar")}>
        {hasRawMode(tab.kind) && (
          <span className="docs__modes">
            <IconButton
              label={t("doc.render")}
              size="sm"
              icon={<EyeIcon />}
              aria-pressed={tab.mode === "render"}
              onClick={() => docs.setMode(tab.id, "render")}
            />
            <IconButton
              label={t("doc.raw")}
              size="sm"
              icon={<CodeIcon />}
              aria-pressed={tab.mode === "raw"}
              onClick={() => docs.setMode(tab.id, "raw")}
            />
          </span>
        )}
        {tab.kind === "html" && (
          <IconButton
            label={docHost.liveUrl ? t("doc.openInBrowser") : t("doc.openInBrowserDesktop")}
            size="sm"
            icon={<GlobeIcon />}
            disabled={!docHost.liveUrl}
            onClick={() => void openLive()}
          />
        )}
        <IconButton
          label={handle?.find ? t("doc.find") : t("doc.findUnavailable")}
          size="sm"
          icon={<SearchIcon />}
          disabled={!handle?.find}
          onClick={() => handle?.find?.()}
        />
        <IconButton
          label={t("doc.wrap")}
          size="sm"
          icon={<WrapIcon />}
          disabled={!textual}
          aria-pressed={tab.wrap}
          onClick={() => docs.toggleWrap(tab.id)}
        />
        <IconButton
          label={t("doc.copy")}
          size="sm"
          icon={<CopyIcon />}
          disabled={!bytes || !isTextKind(tab.kind)}
          onClick={() => void copyContent()}
        />
        <Button size="sm" onClick={() => void copyPath()}>
          {t("doc.copyPath")}
        </Button>
        <IconButton
          label={t("doc.download")}
          size="sm"
          icon={<DownloadIcon />}
          disabled={!bytes}
          onClick={() => void saveCopy()}
        />
        <OpenWithMenu host={docHost} root={tab.root} path={tab.path} textLike={isTextKind(tab.kind)} />
        {textual && !tab.editing && (
          <IconButton
            label={block ? t(`doc.noEdit.${block}` as never) : t("doc.edit")}
            size="sm"
            icon={<EditIcon />}
            disabled={block !== null}
            onClick={startEdit}
          />
        )}
        {tab.editing && (
          <>
            <IconButton
              label={t("doc.save")}
              size="sm"
              icon={<SaveIcon />}
              disabled={!tab.dirty}
              onClick={() => void save()}
            />
            <Button
              size="sm"
              onClick={() => void guarded(tab.id, tab.name, () => docs.endEdit(tab.id))}
              aria-label={t("doc.doneEdit")}
            >
              {t("doc.doneEdit")}
            </Button>
          </>
        )}
        <IconButton
          label={t("doc.reload")}
          size="sm"
          icon={<RefreshIcon />}
          onClick={() => void guarded(tab.id, tab.name, () => docs.reload(tab.id))}
        />
        <IconButton
          label={t("doc.close")}
          size="sm"
          icon={<CloseIcon />}
          onClick={() => void guarded(tab.id, tab.name, () => docs.close(tab.id))}
        />
      </div>
      <div className="docs__body">
        <DocBody
          tab={tab}
          bytes={bytes}
          onHandle={onHandle}
          onSaveRequest={tab.editing ? () => void save() : undefined}
          openExternal={docHost.native ? () => void openExternal() : null}
        />
      </div>
    </div>
  );
}

function DocBody({
  tab,
  bytes,
  onHandle,
  onSaveRequest,
  openExternal,
}: {
  tab: DocTab;
  bytes: Uint8Array | null;
  onHandle: ViewerProps["onHandle"];
  onSaveRequest?: () => void;
  openExternal: (() => void) | null;
}) {
  const { t } = useI18n();
  const { docs } = useServices();
  if (tab.status === "error") {
    return (
      <div className="docview docview--error" role="alert">
        <p className="z-p">{t("doc.error", { error: tab.error ?? "" })}</p>
        <Button onClick={() => docs.reload(tab.id)}>{t("doc.reload")}</Button>
      </div>
    );
  }
  if (!bytes || tab.status === "loading" || tab.status === "idle") {
    return (
      <p className="docview__note z-small" role="status">
        <Spinner size="sm" /> {t("doc.loading")}
      </p>
    );
  }
  const props: ViewerProps = { tab, bytes, onHandle, onSaveRequest };
  const which = viewerFor(tab);
  const fallback = (
    <p className="docview__note z-small" role="status">
      <Spinner size="sm" /> {t("doc.loading")}
    </p>
  );
  switch (which) {
    case "diff":
      return (
        <Suspense fallback={fallback}>
          <DiffViewer key={`${tab.id}:diff`} {...props} />
        </Suspense>
      );
    case "code":
      return (
        <Suspense fallback={fallback}>
          <CodeViewer key={tab.id} {...props} />
        </Suspense>
      );
    case "docx":
      return (
        <Suspense fallback={fallback}>
          <DocxViewer {...props} />
        </Suspense>
      );
    case "xlsx":
      return (
        <Suspense fallback={fallback}>
          <XlsxViewer {...props} />
        </Suspense>
      );
    case "pptx":
      return (
        <Suspense fallback={fallback}>
          <PptxViewer {...props} />
        </Suspense>
      );
    case "notebook":
      return (
        <Suspense fallback={fallback}>
          <NotebookViewer {...props} />
        </Suspense>
      );
    case "markdown":
      return (
        <Suspense fallback={fallback}>
          <MarkdownViewer {...props} />
        </Suspense>
      );
    case "pdf":
      return (
        <Suspense fallback={fallback}>
          <PdfViewer {...props} />
        </Suspense>
      );
    case "image":
      return <ImageViewer {...props} />;
    case "table":
      return <TableViewer {...props} />;
    case "json":
      return <JsonViewer {...props} />;
    case "html":
      return <HtmlViewer {...props} />;
    case "media":
      return <MediaViewer {...props} />;
    default:
      return (
        <BinaryViewer
          {...props}
          extra={
            openExternal && (
              <Button variant="solid" onClick={openExternal}>
                {t("doc.openExternal")}
              </Button>
            )
          }
        />
      );
  }
}

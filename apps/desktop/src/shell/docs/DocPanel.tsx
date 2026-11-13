// Panel 3: tab dokumen, path, toolbar, dan penampil per jenis berkas (Fase 7).
import { type DocTab, decodeText, hasRawMode, isTextKind } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Button,
  CloseIcon,
  CodeIcon,
  CopyIcon,
  DownloadIcon,
  ExternalLinkIcon,
  EyeIcon,
  IconButton,
  RefreshIcon,
  SearchIcon,
  Spinner,
  useToast,
  WrapIcon,
} from "@zandor/ui";
import { lazy, type ReactNode, Suspense, useCallback, useState } from "react";
import { useDocs, useServices } from "../services";
import { BinaryViewer, HtmlViewer, ImageViewer, JsonViewer, MediaViewer, TableViewer } from "./simple-viewers";
import type { ViewerHandle, ViewerProps } from "./types";

const CodeViewer = lazy(() => import("./CodeViewer"));
const MarkdownViewer = lazy(() => import("./MarkdownViewer"));
const PdfViewer = lazy(() => import("./PdfViewer"));

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function viewerFor(
  tab: DocTab,
): "code" | "markdown" | "image" | "pdf" | "table" | "json" | "html" | "media" | "binary" {
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
    default:
      return "binary";
  }
}

export function DocPanel({ empty }: { empty: ReactNode }) {
  const { t } = useI18n();
  const { docs, docHost } = useServices();
  const state = useDocs();
  const toast = useToast();
  const [handle, setHandle] = useState<ViewerHandle | null>(null);
  const onHandle = useCallback((h: ViewerHandle | null) => setHandle(h), []);
  const tab = state.tabs.find((x) => x.id === state.activeId) ?? null;
  if (!tab) return <>{empty}</>;
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

  const textual = isTextKind(tab.kind) && viewerFor(tab) === "code";

  return (
    <div className="docs">
      <nav className="stabs" aria-label={t("doc.tabs")}>
        {state.tabs.map((x) => {
          const active = x.id === state.activeId;
          return (
            <div key={x.id} className="stabs__tab" data-active={active ? "" : undefined}>
              <button
                type="button"
                className="stabs__label"
                aria-current={active ? "true" : undefined}
                title={x.path}
                onClick={() => docs.activate(x.id)}
              >
                {x.name}
              </button>
              <button
                type="button"
                className="stabs__close"
                aria-label={t("doc.closeTab", { name: x.name })}
                onClick={() => docs.close(x.id)}
              >
                <CloseIcon size="sm" />
              </button>
            </div>
          );
        })}
      </nav>
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
        <IconButton
          label={t("doc.openExternal")}
          size="sm"
          icon={<ExternalLinkIcon />}
          disabled={!docHost.native}
          onClick={() => void openExternal()}
        />
        <IconButton label={t("doc.reload")} size="sm" icon={<RefreshIcon />} onClick={() => docs.reload(tab.id)} />
        <IconButton label={t("doc.close")} size="sm" icon={<CloseIcon />} onClick={() => docs.close(tab.id)} />
      </div>
      <div className="docs__body">
        <DocBody
          tab={tab}
          bytes={bytes}
          onHandle={onHandle}
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
  openExternal,
}: {
  tab: DocTab;
  bytes: Uint8Array | null;
  onHandle: ViewerProps["onHandle"];
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
  const props: ViewerProps = { tab, bytes, onHandle };
  const which = viewerFor(tab);
  const fallback = (
    <p className="docview__note z-small" role="status">
      <Spinner size="sm" /> {t("doc.loading")}
    </p>
  );
  switch (which) {
    case "code":
      return (
        <Suspense fallback={fallback}>
          <CodeViewer key={tab.id} {...props} />
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

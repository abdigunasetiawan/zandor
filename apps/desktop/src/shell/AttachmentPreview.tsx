import { type DocKind, type FileAttachment, kindFromName } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, Dialog } from "@zandor/ui";
import { lazy, Suspense, useMemo } from "react";
import { BinaryViewer, HtmlViewer, ImageViewer, JsonViewer, MediaViewer, TableViewer } from "./docs/simple-viewers";
import type { ViewerProps } from "./docs/types";

const CodeViewer = lazy(() => import("./docs/CodeViewer"));
const MarkdownViewer = lazy(() => import("./docs/MarkdownViewer"));
const PdfViewer = lazy(() => import("./docs/PdfViewer"));
const DocxViewer = lazy(() => import("./docs/office-viewers").then((module) => ({ default: module.DocxViewer })));
const XlsxViewer = lazy(() => import("./docs/office-viewers").then((module) => ({ default: module.XlsxViewer })));
const PptxViewer = lazy(() => import("./docs/office-viewers").then((module) => ({ default: module.PptxViewer })));
const NotebookViewer = lazy(() =>
  import("./docs/office-viewers").then((module) => ({ default: module.NotebookViewer })),
);

function kindFor(file: FileAttachment): DocKind {
  return kindFromName(file.filename ?? "") ?? "binary";
}

function PreviewBody({ file, bytes }: { file: FileAttachment; bytes: Uint8Array }) {
  const { t } = useI18n();
  const kind = kindFor(file);
  const tab = useMemo(
    () => ({
      id: file.url,
      root: "",
      path: file.filename ?? "attachment",
      name: file.filename ?? "attachment",
      kind,
      status: "ready" as const,
      error: null,
      size: bytes.length,
      version: 1,
      mode: "render" as const,
      wrap: true,
      modified: null,
      editing: false,
      dirty: false,
    }),
    [bytes.length, file.filename, file.url, kind],
  );
  const props: ViewerProps = { tab, bytes, onHandle: () => {} };
  switch (kind) {
    case "image":
    case "svg":
      return <ImageViewer {...props} />;
    case "pdf":
      return (
        <Suspense fallback={<p className="z-small">{t("chat.previewLoading")}</p>}>
          <PdfViewer {...props} />
        </Suspense>
      );
    case "code":
      return (
        <Suspense fallback={<p className="z-small">{t("chat.previewLoading")}</p>}>
          <CodeViewer {...props} />
        </Suspense>
      );
    case "markdown":
      return (
        <Suspense fallback={<p className="z-small">{t("chat.previewLoading")}</p>}>
          <MarkdownViewer {...props} />
        </Suspense>
      );
    case "csv":
    case "tsv":
      return <TableViewer {...props} />;
    case "json":
      return <JsonViewer {...props} />;
    case "html":
      return <HtmlViewer {...props} />;
    case "audio":
    case "video":
      return <MediaViewer {...props} />;
    case "docx":
    case "xlsx":
    case "pptx":
    case "notebook":
      return (
        <Suspense fallback={<p className="z-small">{t("chat.previewLoading")}</p>}>
          {kind === "docx" ? <DocxViewer {...props} /> : null}
          {kind === "xlsx" ? <XlsxViewer {...props} /> : null}
          {kind === "pptx" ? <PptxViewer {...props} /> : null}
          {kind === "notebook" ? <NotebookViewer {...props} /> : null}
        </Suspense>
      );
    default:
      return <BinaryViewer {...props} />;
  }
}

export function AttachmentPreview({
  file,
  open,
  onOpenChange,
}: {
  file: FileAttachment | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useI18n();
  const bytes = useMemo(() => {
    if (!file?.url.startsWith("data:")) return null;
    try {
      const payload = file.url.slice(file.url.indexOf(",") + 1);
      const binary = atob(payload);
      return Uint8Array.from(binary, (char) => char.charCodeAt(0));
    } catch {
      return null;
    }
  }, [file]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={file?.filename ?? t("chat.attachmentPreview")}
      className="composer__preview-dialog"
      footer={
        <Button size="sm" onClick={() => onOpenChange(false)}>
          {t("ui.close")}
        </Button>
      }
    >
      <div className="composer__attachment-view">
        {!file ? null : file.url.startsWith("file:") ? (
          <p className="z-small">{t("chat.nativeAttachmentPreview")}</p>
        ) : bytes ? (
          <PreviewBody file={file} bytes={bytes} />
        ) : (
          <p className="z-small" role="alert">
            {t("chat.attachmentPreviewError")}
          </p>
        )}
      </div>
    </Dialog>
  );
}

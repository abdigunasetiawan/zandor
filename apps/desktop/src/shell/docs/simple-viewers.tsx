// Penampil ringan: gambar/SVG, audio/video, HTML (sandbox), CSV/TSV (tabel tervirtualisasi), JSON (pohon), biner.

import { useVirtualizer } from "@tanstack/react-virtual";
import { decodeText, mimeFor } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, Switch, useToast } from "@zandor/ui";
import Papa from "papaparse";
import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { useServices } from "../services";
import { requestBrowserOpen } from "../tools/browser-bridge";
import type { ViewerProps } from "./types";

/** Blob URL yang dilepas otomatis saat berkas/komponen berganti. */
function useBlobUrl(bytes: Uint8Array, mime: string): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    const u = URL.createObjectURL(new Blob([bytes as BlobPart], { type: mime }));
    setUrl(u);
    return () => URL.revokeObjectURL(u);
  }, [bytes, mime]);
  return url;
}

export function ImageViewer({ tab, bytes }: ViewerProps) {
  const { t } = useI18n();
  const url = useBlobUrl(bytes, mimeFor(tab.path));
  const [fit, setFit] = useState(true);
  const [error, setError] = useState(false);
  if (error)
    return (
      <p className="docview__note docview__note--error" role="alert">
        {t("doc.imageError")}
      </p>
    );
  return (
    <div className="docview docview--image" data-fit={fit ? "" : undefined}>
      {url && (
        // gambar dimuat dari blob lokal; SVG lewat <img> tidak menjalankan skrip
        <button
          type="button"
          className="docview__imgbtn"
          onClick={() => setFit((f) => !f)}
          title={t("doc.imageToggle")}
        >
          <img src={url} alt={tab.name} data-testid="image" onError={() => setError(true)} />
        </button>
      )}
    </div>
  );
}

export function MediaViewer({ tab, bytes }: ViewerProps) {
  const { t } = useI18n();
  const url = useBlobUrl(bytes, mimeFor(tab.path));
  const [error, setError] = useState(false);
  const common = { controls: true, src: url ?? undefined, onError: () => setError(true) };
  return (
    <div className="docview docview--media">
      {error && (
        <p className="docview__note docview__note--error" role="alert">
          {t("doc.mediaError")}
        </p>
      )}
      {tab.kind === "audio" ? (
        <audio {...common} data-testid="audio" aria-label={tab.name} />
      ) : (
        <video {...common} data-testid="video" aria-label={tab.name}>
          <track kind="captions" />
        </video>
      )}
    </div>
  );
}

export function HtmlViewer({ bytes }: ViewerProps) {
  const { t } = useI18n();
  const { docHost, panels } = useServices();
  const toast = useToast();
  const decoded = useMemo(() => decodeText(bytes), [bytes]);
  const [external, setExternal] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const canExternal = typeof docHost.previewUrl === "function";
  // Opt-in: skrip dan sumber https dimuat dari skema `zpreview` (CSP-nya sendiri) di iframe ber-sandbox tanpa same-origin.
  useEffect(() => {
    if (!external || !docHost.previewUrl) {
      setUrl(null);
      return;
    }
    let alive = true;
    docHost.previewUrl(decoded.text).then(
      (u) => alive && setUrl(u),
      (e) => {
        if (!alive) return;
        setExternal(false);
        toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
      },
    );
    return () => {
      alive = false;
    };
  }, [external, decoded.text, docHost, toast, t]);
  /** Buka HTML ini di Browser Panel 4 (skrip dan sumber https berfungsi di sana). */
  const openInBrowser = async () => {
    try {
      if (!docHost.previewUrl) return;
      const target = await docHost.previewUrl(decoded.text);
      panels.actions?.openTool("browser");
      requestBrowserOpen(target);
    } catch (e) {
      toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
    }
  };
  return (
    <div className="docview docview--html">
      <div className="docview__bar">
        <Button size="sm" disabled={!canExternal} onClick={() => void openInBrowser()}>
          {t("doc.openInBrowser")}
        </Button>
        <Switch
          label={t("doc.htmlExternal")}
          checked={external}
          disabled={!canExternal}
          onCheckedChange={setExternal}
        />
        <p className="docview__note z-small">
          {!canExternal ? t("doc.htmlExternalDesktop") : external ? t("doc.htmlExternalHint") : t("doc.htmlSandbox")}
        </p>
      </div>
      {external && url ? (
        <iframe
          className="docview__frame"
          title="HTML"
          sandbox="allow-scripts allow-popups"
          src={url}
          data-testid="html-frame"
          data-external=""
        />
      ) : (
        /* sandbox kosong: tanpa skrip, tanpa form, tanpa akses origin; srcdoc mewarisi CSP aplikasi */
        <iframe className="docview__frame" title="HTML" sandbox="" srcDoc={decoded.text} data-testid="html-frame" />
      )}
    </div>
  );
}

export function BinaryViewer({ tab, bytes, extra }: ViewerProps & { extra?: ReactNode }) {
  const { t } = useI18n();
  return (
    <div className="docview docview--binary">
      <p className="z-p">{t("doc.binary", { name: tab.name })}</p>
      <p className="z-small">{t("doc.size", { size: formatSize(bytes.length) })}</p>
      {extra}
    </div>
  );
}

export function formatSize(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

// ---------------- CSV / TSV ----------------

const MAX_ROWS = 200_000;
const ROW_HEIGHT = 28;

export function TableViewer({ tab, bytes }: ViewerProps) {
  const { t } = useI18n();
  const decoded = decodeText(bytes);
  const parsed = useMemo(() => {
    const r = Papa.parse<string[]>(decoded.text, {
      delimiter: tab.kind === "tsv" ? "\t" : undefined,
      skipEmptyLines: "greedy",
    });
    const rows = r.data.slice(0, MAX_ROWS);
    const cols = rows.reduce((m, row) => Math.max(m, row.length), 0);
    return { rows, cols, clipped: r.data.length > MAX_ROWS, delimiter: r.meta.delimiter, errors: r.errors.length };
  }, [decoded.text, tab.kind]);

  const scroller = useRef<HTMLDivElement>(null);
  const body = parsed.rows.slice(1);
  const v = useVirtualizer({
    count: body.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });
  if (parsed.rows.length === 0) return <p className="docview__note">{t("doc.tableEmpty")}</p>;
  const template = `repeat(${parsed.cols}, minmax(8rem, 1fr))`;
  return (
    <div className="docview docview--table">
      <p className="docview__note z-small" role="status">
        {t("doc.tableInfo", { rows: parsed.rows.length - 1, cols: parsed.cols })}
        {parsed.clipped ? ` · ${t("doc.tableClipped", { max: MAX_ROWS })}` : ""}
        {decoded.truncated ? ` · ${t("doc.truncated", { mb: 5 })}` : ""}
      </p>
      {/* biome-ignore lint/a11y/noNoninteractiveTabindex: area gulir harus bisa difokus agar bisa digulir dengan keyboard */}
      <section className="dtable" ref={scroller} aria-label={tab.name} tabIndex={0} data-testid="table">
        <div className="dtable__head" style={{ gridTemplateColumns: template }}>
          {Array.from({ length: parsed.cols }, (_, c) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: kolom statis berdasarkan posisi
            <div key={c} className="dtable__cell dtable__cell--head">
              {parsed.rows[0]?.[c] ?? ""}
            </div>
          ))}
        </div>
        <div className="dtable__body" style={{ height: v.getTotalSize() }}>
          {v.getVirtualItems().map((item) => (
            <div
              key={item.key}
              className="dtable__row"
              style={{ gridTemplateColumns: template, transform: `translateY(${item.start}px)` }}
            >
              {Array.from({ length: parsed.cols }, (_, c) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: sel statis berdasarkan posisi
                <div key={c} className="dtable__cell">
                  {body[item.index]?.[c] ?? ""}
                </div>
              ))}
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

// ---------------- JSON ----------------

type Json = null | boolean | number | string | Json[] | { [k: string]: Json };
const PAGE = 100;

function Node({ name, value, depth }: { name: string | null; value: Json; depth: number }) {
  const { t } = useI18n();
  const isObj = typeof value === "object" && value !== null;
  const entries = useMemo<Array<[string, Json]>>(
    () =>
      isObj ? (Array.isArray(value) ? value.map((v, i): [string, Json] => [String(i), v]) : Object.entries(value)) : [],
    [isObj, value],
  );
  const [open, setOpen] = useState(depth < 1);
  const [shown, setShown] = useState(PAGE);
  const label = name === null ? null : <span className="jtree__key">{name}: </span>;
  if (!isObj) {
    const kind = value === null ? "null" : typeof value;
    return (
      <div className="jtree__row">
        {label}
        <span className={`jtree__val jtree__val--${kind}`}>
          {kind === "string" ? JSON.stringify(value) : String(value)}
        </span>
      </div>
    );
  }
  const summary = Array.isArray(value) ? `[${entries.length}]` : `{${entries.length}}`;
  return (
    <div className="jtree__node">
      <button
        type="button"
        className="jtree__toggle"
        onClick={() => setOpen((o) => !o)}
        aria-label={`${name ?? "root"} ${summary}`}
      >
        <span aria-hidden="true">{open ? "▾" : "▸"}</span>
        {label}
        <span className="jtree__sum">{summary}</span>
      </button>
      {open && (
        <div className="jtree__children">
          {entries.slice(0, shown).map(([k, v]) => (
            <Node key={k} name={k} value={v} depth={depth + 1} />
          ))}
          {entries.length > shown && (
            <button type="button" className="jtree__more" onClick={() => setShown((s) => s + PAGE)}>
              {t("doc.jsonMore", { count: entries.length - shown })}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export function JsonViewer({ bytes }: ViewerProps) {
  const { t } = useI18n();
  const decoded = decodeText(bytes);
  const parsed = useMemo((): { value: Json } | { error: string } => {
    try {
      return { value: JSON.parse(decoded.text) as Json };
    } catch (e) {
      return { error: e instanceof Error ? e.message : String(e) };
    }
  }, [decoded.text]);
  if ("error" in parsed) {
    return (
      <p className="docview__note docview__note--error" role="alert">
        {t("doc.jsonInvalid", { error: parsed.error })}
      </p>
    );
  }
  return (
    <section className="docview docview--json" aria-label="JSON" data-testid="json">
      <Node name={null} value={parsed.value} depth={0} />
    </section>
  );
}

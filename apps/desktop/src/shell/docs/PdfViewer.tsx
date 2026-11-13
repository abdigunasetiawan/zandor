// PDF dengan pdf.js (Apache-2.0): worker terpisah, halaman dirender malas saat mendekati area terlihat.
import { useI18n } from "@zandor/i18n/react";
import { Button, IconButton } from "@zandor/ui";
import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { useEffect, useRef, useState } from "react";
import type { ViewerProps } from "./types";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

type Doc = Awaited<ReturnType<typeof pdfjs.getDocument>["promise"]>;

const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3];

function Page({ doc, n, scale, root }: { doc: Doc; n: number; scale: number; root: HTMLElement | null }) {
  const holder = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState<{ w: number; h: number } | null>(null);
  const [visible, setVisible] = useState(n <= 2);

  useEffect(() => {
    if (!holder.current || !root) return;
    const io = new IntersectionObserver((es) => setVisible((v) => v || es.some((e) => e.isIntersecting)), {
      root,
      rootMargin: "100% 0%",
    });
    io.observe(holder.current);
    return () => io.disconnect();
  }, [root]);

  useEffect(() => {
    let cancelled = false;
    let task: { cancel(): void; promise: Promise<unknown> } | null = null;
    void (async () => {
      const page = await doc.getPage(n);
      const vp = page.getViewport({ scale });
      if (cancelled) return;
      setSize({ w: Math.floor(vp.width), h: Math.floor(vp.height) });
      if (!visible || !canvas.current) return;
      const dpr = window.devicePixelRatio || 1;
      const c = canvas.current;
      c.width = Math.floor(vp.width * dpr);
      c.height = Math.floor(vp.height * dpr);
      const transform = dpr === 1 ? undefined : [dpr, 0, 0, dpr, 0, 0];
      task = page.render({ canvas: c, viewport: vp, ...(transform ? { transform } : {}) });
      try {
        await task.promise;
      } catch {
        // dibatalkan saat zoom/unmount
      }
    })();
    return () => {
      cancelled = true;
      task?.cancel();
    };
  }, [doc, n, scale, visible]);

  return (
    <div
      ref={holder}
      className="pdfpage"
      data-page={n}
      style={size ? { width: size.w, height: size.h } : { width: "100%", minHeight: "20rem" }}
    >
      {visible && <canvas ref={canvas} style={size ? { width: size.w, height: size.h } : undefined} />}
    </div>
  );
}

export default function PdfViewer({ bytes }: ViewerProps) {
  const { t } = useI18n();
  const [doc, setDoc] = useState<Doc | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scale, setScale] = useState(1);
  const [current, setCurrent] = useState(1);
  const scroller = useRef<HTMLDivElement>(null);
  const [root, setRoot] = useState<HTMLElement | null>(null);

  useEffect(() => {
    let alive = true;
    setDoc(null);
    setError(null);
    // pdf.js memindahkan (detach) buffer ke worker → beri salinan agar byte di store tetap utuh
    const task = pdfjs.getDocument({ data: bytes.slice() });
    task.promise.then(
      (d) => alive && setDoc(d),
      (e: unknown) => alive && setError(e instanceof Error ? e.message : String(e)),
    );
    return () => {
      alive = false;
      void task.destroy();
    };
  }, [bytes]);

  const onScroll = () => {
    const el = scroller.current;
    if (!el) return;
    const pages = [...el.querySelectorAll<HTMLElement>(".pdfpage")];
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 2) {
      const last = pages.at(-1);
      if (last) setCurrent(Number(last.dataset.page));
      return;
    }
    const mid = el.scrollTop + el.clientHeight / 3;
    const p = pages.find((x) => x.offsetTop + x.offsetHeight > mid);
    if (p) setCurrent(Number(p.dataset.page));
  };

  if (error) {
    return (
      <p className="docview__note docview__note--error" role="alert">
        {t("doc.pdfError", { error })}
      </p>
    );
  }
  if (!doc) return <p className="docview__note z-small">{t("doc.loading")}</p>;

  const zoomIdx = ZOOMS.indexOf(scale);
  const pageNumbers = Array.from({ length: doc.numPages }, (_, i) => i + 1);
  return (
    <div className="docview docview--pdf">
      <div className="pdfbar" role="toolbar" aria-label="PDF">
        <span className="z-small" role="status" data-testid="pdf-pages">
          {t("doc.pdfPage", { n: current, total: doc.numPages })}
        </span>
        <IconButton
          label={t("doc.zoomOut")}
          size="sm"
          icon={<span aria-hidden="true">−</span>}
          disabled={zoomIdx <= 0}
          onClick={() => setScale(ZOOMS[Math.max(0, zoomIdx - 1)] ?? 1)}
        />
        <span className="z-small">{Math.round(scale * 100)}%</span>
        <IconButton
          label={t("doc.zoomIn")}
          size="sm"
          icon={<span aria-hidden="true">+</span>}
          disabled={zoomIdx >= ZOOMS.length - 1}
          onClick={() => setScale(ZOOMS[Math.min(ZOOMS.length - 1, zoomIdx + 1)] ?? 1)}
        />
        <Button size="sm" onClick={() => setScale(1)}>
          100%
        </Button>
      </div>
      <div
        className="pdfpages"
        ref={(el) => {
          scroller.current = el;
          if (el !== root) setRoot(el);
        }}
        onScroll={onScroll}
        data-testid="pdf"
      >
        {pageNumbers.map((n) => (
          <Page key={n} doc={doc} n={n} scale={scale} root={root} />
        ))}
      </div>
    </div>
  );
}

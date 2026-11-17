// PDF dengan pdf.js (Apache-2.0): worker terpisah, halaman dirender malas saat mendekati area terlihat.
// Tiap halaman punya lapisan teks pdf.js (`TextLayer`) di atas kanvas, jadi teks bisa dipilih dan disalin. Pencarian
// memakai teks yang sama (`getTextContent`) dan menyorot hasil di lapisan teks; hasil yang terpecah di dua potongan
// teks PDF tidak ditemukan (batasan jujur, lihat docs/features/documents.md).
import { useI18n } from "@zandor/i18n/react";
import { Button, ChevronDownIcon, ChevronUpIcon, IconButton, SearchField } from "@zandor/ui";
import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ViewerProps } from "./types";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

type Doc = Awaited<ReturnType<typeof pdfjs.getDocument>["promise"]>;

const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3];

/** Teks per potongan (item) tiap halaman; dipakai pencarian. */
type PageTexts = string[][];

/** Jumlah kemunculan `q` (sudah huruf kecil) di `s`, tidak tumpang tindih. */
export function countIn(s: string, q: string): number {
  if (!q) return 0;
  let n = 0;
  let i = s.toLowerCase().indexOf(q);
  while (i !== -1) {
    n++;
    i = s.toLowerCase().indexOf(q, i + q.length);
  }
  return n;
}

/** Bungkus kemunculan `q` di setiap span lapisan teks dengan `<mark>`; mengembalikan daftar mark berurutan. */
function highlight(layer: HTMLElement, q: string): HTMLElement[] {
  for (const m of layer.querySelectorAll("mark.pdfhit")) m.replaceWith(document.createTextNode(m.textContent ?? ""));
  layer.normalize();
  if (!q) return [];
  const marks: HTMLElement[] = [];
  for (const span of layer.querySelectorAll<HTMLElement>("span:not(.markedContent)")) {
    const text = span.textContent ?? "";
    const lower = text.toLowerCase();
    if (!lower.includes(q)) continue;
    const frag = document.createDocumentFragment();
    let at = 0;
    for (let i = lower.indexOf(q); i !== -1; i = lower.indexOf(q, i + q.length)) {
      frag.append(text.slice(at, i));
      const m = document.createElement("mark");
      m.className = "pdfhit";
      m.textContent = text.slice(i, i + q.length);
      frag.append(m);
      marks.push(m);
      at = i + q.length;
    }
    frag.append(text.slice(at));
    span.replaceChildren(frag);
  }
  return marks;
}

function Page({
  doc,
  n,
  scale,
  root,
  query,
  activeHit,
}: {
  doc: Doc;
  n: number;
  scale: number;
  root: HTMLElement | null;
  query: string;
  /** Indeks hasil aktif di halaman ini, atau -1. */
  activeHit: number;
}) {
  const holder = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const textLayer = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<{ w: number; h: number } | null>(null);
  const [visible, setVisible] = useState(n <= 2);
  // naik setiap lapisan teks selesai dirender, supaya sorotan dipasang ulang
  const [textGen, setTextGen] = useState(0);

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
    let text: pdfjs.TextLayer | null = null;
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
      const layer = textLayer.current;
      if (layer) {
        layer.replaceChildren();
        text = new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container: layer, viewport: vp });
        text.render().then(
          () => !cancelled && setTextGen((g) => g + 1),
          () => {}, // dibatalkan saat zoom/unmount
        );
      }
      try {
        await task.promise;
      } catch {
        // dibatalkan saat zoom/unmount
      }
    })();
    return () => {
      cancelled = true;
      task?.cancel();
      text?.cancel();
    };
  }, [doc, n, scale, visible]);

  useEffect(() => {
    const layer = textLayer.current;
    if (!layer || textGen === 0) return;
    const marks = highlight(layer, query);
    const active = marks[activeHit];
    if (active) {
      active.classList.add("pdfhit--active");
      active.scrollIntoView({ block: "center", inline: "nearest" });
    }
  }, [query, activeHit, textGen]);

  // Hasil aktif di halaman yang belum dirender: gulir ke halamannya dulu (lapisan teks menyusul).
  useEffect(() => {
    if (activeHit >= 0 && !visible) holder.current?.scrollIntoView({ block: "start" });
  }, [activeHit, visible]);

  return (
    <div
      ref={holder}
      className="pdfpage"
      data-page={n}
      style={
        size
          ? ({ width: size.w, height: size.h, "--total-scale-factor": scale } as React.CSSProperties)
          : { width: "100%", minHeight: "20rem" }
      }
    >
      {visible && <canvas ref={canvas} style={size ? { width: size.w, height: size.h } : undefined} />}
      {visible && <div ref={textLayer} className="textLayer" />}
    </div>
  );
}

/** Teks semua halaman, dimuat sekali per dokumen saat pengguna pertama kali mencari. */
function usePageTexts(doc: Doc | null, wanted: boolean): PageTexts | null {
  const [texts, setTexts] = useState<PageTexts | null>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: sengaja mengosongkan cache saat dokumen berganti
  useEffect(() => setTexts(null), [doc]);
  useEffect(() => {
    if (!doc || !wanted || texts) return;
    let alive = true;
    void (async () => {
      const out: PageTexts = [];
      for (let i = 1; i <= doc.numPages; i++) {
        const tc = await (await doc.getPage(i)).getTextContent();
        out.push(tc.items.map((it) => ("str" in it ? it.str : "")));
        if (!alive) return;
      }
      setTexts(out);
    })();
    return () => {
      alive = false;
    };
  }, [doc, wanted, texts]);
  return texts;
}

export default function PdfViewer({ bytes, onHandle }: ViewerProps) {
  const { t } = useI18n();
  const [doc, setDoc] = useState<Doc | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scale, setScale] = useState(1);
  const [current, setCurrent] = useState(1);
  const scroller = useRef<HTMLDivElement>(null);
  const [root, setRoot] = useState<HTMLElement | null>(null);
  const findRef = useRef<HTMLInputElement>(null);
  const [rawQuery, setRawQuery] = useState("");
  const query = rawQuery.trim().toLowerCase();
  const [hit, setHit] = useState(0);
  const texts = usePageTexts(doc, query !== "");

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

  // Tombol cari di toolbar dokumen (dan Ctrl+F) memfokuskan kolom cari PDF.
  useEffect(() => {
    if (!doc) return;
    onHandle({
      find: () => {
        findRef.current?.focus();
        findRef.current?.select();
      },
    });
    return () => onHandle(null);
  }, [doc, onHandle]);

  /** Jumlah hasil per halaman (indeks 0 = halaman 1). */
  const perPage = useMemo(
    () => (texts && query ? texts.map((items) => items.reduce((a, s) => a + countIn(s, query), 0)) : null),
    [texts, query],
  );
  const total = perPage?.reduce((a, b) => a + b, 0) ?? 0;
  const hasText = texts ? texts.some((items) => items.some((s) => s.trim())) : true;
  // biome-ignore lint/correctness/useExhaustiveDependencies: kembali ke hasil pertama setiap kata kunci berubah
  useEffect(() => setHit(0), [query]);

  /** Halaman dan indeks lokal untuk hasil ke-`hit`. */
  const active = useMemo(() => {
    if (!perPage || total === 0) return null;
    let k = ((hit % total) + total) % total;
    for (let p = 0; p < perPage.length; p++) {
      const c = perPage[p] ?? 0;
      if (k < c) return { page: p + 1, idx: k };
      k -= c;
    }
    return null;
  }, [perPage, total, hit]);

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
  const step = (d: number) => total > 0 && setHit((h) => (((h + d) % total) + total) % total);
  let findStatus = "";
  if (query) {
    if (!texts) findStatus = t("doc.pdfFindSearching");
    else if (total === 0) findStatus = t("ui.noResults");
    else findStatus = t("doc.pdfFindCount", { i: (((hit % total) + total) % total) + 1, n: total });
  }
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
        {/* biome-ignore lint/a11y/noStaticElementInteractions: menangkap Enter/Shift+Enter dari kolom cari di dalamnya */}
        <div
          className="pdfbar__find"
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            e.preventDefault();
            step(e.shiftKey ? -1 : 1);
          }}
        >
          <SearchField
            value={rawQuery}
            onValueChange={setRawQuery}
            inputRef={findRef}
            placeholder={t("doc.find")}
            aria-label={t("doc.find")}
          />
          <span className="z-small pdfbar__count" role="status" aria-live="polite">
            {findStatus}
          </span>
          <IconButton
            label={t("doc.pdfFindPrev")}
            size="sm"
            icon={<ChevronUpIcon size="sm" />}
            disabled={total === 0}
            onClick={() => step(-1)}
          />
          <IconButton
            label={t("doc.pdfFindNext")}
            size="sm"
            icon={<ChevronDownIcon size="sm" />}
            disabled={total === 0}
            onClick={() => step(1)}
          />
        </div>
      </div>
      {!hasText && (
        <p className="docview__note z-small" role="note">
          {t("doc.pdfNoText")}
        </p>
      )}
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
          <Page
            key={n}
            doc={doc}
            n={n}
            scale={scale}
            root={root}
            query={texts ? query : ""}
            activeHit={active?.page === n ? active.idx : -1}
          />
        ))}
      </div>
    </div>
  );
}

// Penampil DOCX / XLSX / PPTX / Notebook (Fase 11, ADR-039). Best-effort: tampilan struktur dan isi, bukan replika visual
// Office. Semua dirender sebagai elemen React dari data polos hasil parser (tidak ada HTML mentah, tidak ada skrip).
import {
  type DocxBlock,
  type DocxRun,
  decodeText,
  type Notebook,
  parseDocx,
  parseNotebook,
  parsePptx,
  parseXlsx,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Tabs } from "@zandor/ui";
import { type ReactNode, useMemo } from "react";
import { Markdown } from "../Markdown";
import type { ViewerProps } from "./types";

function Failed({ error, name }: { error: string; name: string }) {
  const { t } = useI18n();
  return (
    <div className="docview docview--error" role="alert">
      <p className="z-p">{t("doc.officeFailed", { name, error })}</p>
    </div>
  );
}

function useParsed<T, I>(parse: (input: I) => T, input: I): { value: T | null; error: string | null } {
  return useMemo(() => {
    try {
      return { value: parse(input), error: null };
    } catch (e) {
      return { value: null, error: e instanceof Error ? e.message : String(e) };
    }
  }, [parse, input]);
}

const parseNotebookBytes = (b: Uint8Array) => parseNotebook(decodeText(b).text);

// ---------- DOCX ----------

function Runs({ runs }: { runs: DocxRun[] }) {
  return (
    <>
      {runs.map((r, i) => {
        let node: ReactNode = r.text;
        if (r.image)
          // biome-ignore lint/suspicious/noArrayIndexKey: run statis, urutan tetap
          return <img key={`${i}-img`} className="office__img" src={r.image} alt="" />;
        if (r.bold) node = <strong>{node}</strong>;
        if (r.italic) node = <em>{node}</em>;
        if (r.underline) node = <u>{node}</u>;
        if (r.strike) node = <s>{node}</s>;
        if (r.href)
          node = (
            <a href={r.href} target="_blank" rel="noreferrer noopener">
              {node}
            </a>
          );
        // biome-ignore lint/suspicious/noArrayIndexKey: run statis, urutan tetap
        return <span key={i}>{node}</span>;
      })}
    </>
  );
}

function Blocks({ blocks }: { blocks: DocxBlock[] }) {
  const out: ReactNode[] = [];
  let i = 0;
  while (i < blocks.length) {
    const b = blocks[i] as DocxBlock;
    if (b.type === "table") {
      out.push(
        <table key={i} className="otable">
          <tbody>
            {b.rows.map((row, r) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: struktur statis
              <tr key={r}>
                {row.map((cell, c) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: struktur statis
                  <td key={c}>
                    <Blocks blocks={cell} />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>,
      );
      i++;
    } else if (b.list) {
      // kumpulkan butir berurutan dengan jenis (bernomor/butir) yang sama
      const ordered = b.list.ordered;
      const items: Array<Extract<DocxBlock, { type: "p" }>> = [];
      while (i < blocks.length) {
        const n = blocks[i] as DocxBlock;
        if (n.type !== "p" || !n.list || n.list.ordered !== ordered) break;
        items.push(n);
        i++;
      }
      const L = ordered ? "ol" : "ul";
      out.push(
        <L key={`l${i}`}>
          {items.map((it, k) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: struktur statis
            <li key={k} style={{ marginLeft: `${(it.list?.level ?? 0) * 1.25}rem` }}>
              <Runs runs={it.runs} />
            </li>
          ))}
        </L>,
      );
    } else {
      const content = b.runs.length ? <Runs runs={b.runs} /> : <br />;
      const key = `p${i}`;
      switch (b.style) {
        case "title":
          out.push(
            <h1 key={key} className="z-h1">
              {content}
            </h1>,
          );
          break;
        case "h1":
          out.push(
            <h1 key={key} className="z-h2">
              {content}
            </h1>,
          );
          break;
        case "h2":
          out.push(
            <h2 key={key} className="z-h3">
              {content}
            </h2>,
          );
          break;
        case "h3":
        case "h4":
        case "h5":
        case "h6":
          out.push(
            <h3 key={key} className="z-h4">
              {content}
            </h3>,
          );
          break;
        default:
          out.push(
            <p key={key} className="z-p">
              {content}
            </p>,
          );
      }
      i++;
    }
  }
  return <>{out}</>;
}

export function DocxViewer({ tab, bytes }: ViewerProps) {
  const { t } = useI18n();
  const r = useParsed(parseDocx, bytes);
  if (r.error || !r.value) return <Failed name={tab.name} error={r.error ?? ""} />;
  return (
    <div className="docview docview--office">
      <p className="docview__note z-small" role="status">
        {t("doc.officeBestEffort")}
        {r.value.truncated ? ` · ${t("doc.officeTruncated")}` : ""}
      </p>
      <article className="office__page" aria-label={tab.name} data-testid="docx-view">
        <Blocks blocks={r.value.blocks} />
      </article>
    </div>
  );
}

// ---------- XLSX ----------

export function XlsxViewer({ tab, bytes }: ViewerProps) {
  const { t } = useI18n();
  const r = useParsed(parseXlsx, bytes);
  if (r.error || !r.value) return <Failed name={tab.name} error={r.error ?? ""} />;
  if (r.value.sheets.length === 0) return <p className="docview__note">{t("doc.tableEmpty")}</p>;
  return (
    <div className="docview docview--office">
      <p className="docview__note z-small" role="status">
        {t("doc.officeBestEffort")} · {t("doc.xlsxNote")}
      </p>
      <Tabs
        aria-label={tab.name}
        items={r.value.sheets.map((s, i) => ({
          value: `${i}`,
          label: s.name,
          content: (
            <div className="office__sheet" data-testid="xlsx-view">
              {s.rows.length === 0 ? (
                <p className="docview__note z-small">{t("doc.sheetEmpty")}</p>
              ) : (
                <table className="otable">
                  <tbody>
                    {s.rows.map((row, ri) => (
                      // biome-ignore lint/suspicious/noArrayIndexKey: baris statis
                      <tr key={ri}>
                        {row.map((cell, ci) => (
                          // biome-ignore lint/suspicious/noArrayIndexKey: sel statis
                          <td key={ci}>{cell}</td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              {s.truncated && <p className="docview__note z-small">{t("doc.sheetClipped")}</p>}
            </div>
          ),
        }))}
      />
    </div>
  );
}

// ---------- PPTX ----------

export function PptxViewer({ tab, bytes }: ViewerProps) {
  const { t } = useI18n();
  const r = useParsed(parsePptx, bytes);
  if (r.error || !r.value) return <Failed name={tab.name} error={r.error ?? ""} />;
  return (
    <div className="docview docview--office">
      <p className="docview__note z-small" role="status">
        {t("doc.officeBestEffort")} · {t("doc.pptxNote")}
        {r.value.truncated ? ` · ${t("doc.officeTruncated")}` : ""}
      </p>
      <ol className="office__slides" data-testid="pptx-view" aria-label={tab.name}>
        {r.value.slides.map((s) => (
          <li key={s.index} className="office__slide" aria-label={t("doc.slideLabel", { n: s.index })}>
            <span className="z-small office__slideno">{t("doc.slideLabel", { n: s.index })}</span>
            {s.title && <h2 className="z-h3">{s.title}</h2>}
            {s.paragraphs.map((p, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: baris statis
              <p key={i} className="z-p office__para">
                {p}
              </p>
            ))}
            {s.tables.map((tb, ti) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: tabel statis
              <table key={ti} className="otable">
                <tbody>
                  {tb.map((row, ri) => (
                    // biome-ignore lint/suspicious/noArrayIndexKey: baris statis
                    <tr key={ri}>
                      {row.map((cell, ci) => (
                        // biome-ignore lint/suspicious/noArrayIndexKey: sel statis
                        <td key={ci}>{cell}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            ))}
            {s.images.map((src, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: gambar statis
              <img key={i} className="office__img" src={src} alt="" />
            ))}
          </li>
        ))}
      </ol>
    </div>
  );
}

// ---------- Notebook ----------

function NotebookView({ nb, name }: { nb: Notebook; name: string }) {
  const { t } = useI18n();
  return (
    <article className="nb" aria-label={name} data-testid="notebook-view">
      {nb.truncated && <p className="docview__note z-small">{t("doc.officeTruncated")}</p>}
      {nb.cells.map((c, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: sel notebook statis
        <section key={i} className={`nb__cell nb__cell--${c.type}`} aria-label={t(`doc.nb.${c.type}` as never)}>
          {c.type === "markdown" ? (
            <Markdown text={c.source} />
          ) : (
            <div className="nb__row">
              {c.type === "code" && <span className="nb__count z-small">[{c.executionCount ?? " "}]</span>}
              <pre className="nb__code">
                <code>{c.source}</code>
              </pre>
            </div>
          )}
          {c.outputs.map((o, k) => {
            if (o.kind === "image")
              // biome-ignore lint/suspicious/noArrayIndexKey: output statis
              return <img key={k} className="office__img nb__out" src={o.src} alt={t("doc.nb.output")} />;
            if (o.kind === "skipped")
              return (
                // biome-ignore lint/suspicious/noArrayIndexKey: output statis
                <p key={k} className="nb__out z-small docview__note">
                  {o.reason}
                </p>
              );
            if (o.kind === "error")
              return (
                // biome-ignore lint/suspicious/noArrayIndexKey: output statis
                <pre key={k} className="nb__out nb__out--error" role="alert">
                  {o.name}: {o.message}
                  {"\n"}
                  {o.traceback}
                </pre>
              );
            return (
              // biome-ignore lint/suspicious/noArrayIndexKey: output statis
              <pre key={k} className={`nb__out${o.stream === "stderr" ? " nb__out--stderr" : ""}`}>
                {o.text}
              </pre>
            );
          })}
        </section>
      ))}
    </article>
  );
}

export function NotebookViewer({ tab, bytes }: ViewerProps) {
  const { t } = useI18n();
  const r = useParsed(parseNotebookBytes, bytes);
  if (r.error || !r.value) return <Failed name={tab.name} error={r.error ?? ""} />;
  return (
    <div className="docview docview--office">
      <p className="docview__note z-small" role="status">
        {t("doc.nbNote")}
      </p>
      <NotebookView nb={r.value} name={tab.name} />
    </div>
  );
}

// Renderer Markdown lengkap untuk respons model (Fase 12): GFM (tabel, daftar tugas, coret), KaTeX, blok kode berwarna dengan
// tombol salin, diagram Mermaid, dan tabel yang bisa digulir. Aman secara bawaan: HTML mentah tidak dirender, tautan hanya
// http(s)/mailto, gambar tidak dimuat dari luar (mencegah eksfiltrasi lewat URL), Mermaid strict. Isi dari model tidak dipercaya.
import { useI18n } from "@zandor/i18n/react";
import {
  CodeIcon,
  CopyIcon,
  DownloadIcon,
  FileTextIcon,
  ImageDownIcon,
  ImageIcon,
  SigmaIcon,
  TableIcon,
  WrapIcon,
} from "@zandor/ui";
import dockerfile from "highlight.js/lib/languages/dockerfile";
import powershell from "highlight.js/lib/languages/powershell";
import "katex/dist/katex.min.css";
import { common } from "lowlight";
import { createContext, type ReactNode, useContext, useRef, useState } from "react";
import ReactMarkdown, { type Components, defaultUrlTransform } from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { SAFE_IMAGE_URI } from "../image-uri";
import { ChatLink, isWebLink } from "./ChatLink";
import { Mermaid } from "./Mermaid";
import { type RichAction, RichBar, useRichActions } from "./RichActions";
import { extForLang, fenced, latexOf, stampedName, tableMatrix, toCsv, toMarkdownTable, toTsv } from "./rich-export";

const SAFE_LINK = /^(https?:|mailto:|#)/i;
const LANGUAGES = { ...common, powershell, dockerfile };
const COLLAPSE_LINES = 30;

type HastNode = { type?: string; value?: string; children?: HastNode[] };
const hastText = (n: HastNode | undefined): string =>
  !n ? "" : typeof n.value === "string" ? n.value : (n.children ?? []).map(hastText).join("");

/** Aksi gambar bersama: salin PNG, simpan PNG, simpan SVG. */
function imageActions(
  act: ReturnType<typeof useRichActions>,
  t: ReturnType<typeof useI18n>["t"],
  node: () => HTMLElement | null,
  base: string,
  svg = true,
): RichAction[] {
  return [
    {
      id: "copypng",
      label: t("rich.copyImage"),
      icon: <ImageDownIcon size="sm" />,
      onSelect: () => void act.copyImage(node()),
    },
    {
      id: "png",
      label: t("rich.saveImage"),
      icon: <ImageIcon size="sm" />,
      onSelect: () => void act.saveNode(node(), "png", stampedName(base, "png")),
    },
    ...(svg
      ? [
          {
            id: "svg",
            label: t("rich.saveSvg"),
            icon: <CodeIcon size="sm" />,
            onSelect: () => void act.saveNode(node(), "svg", stampedName(base, "svg")),
          },
        ]
      : []),
  ];
}

function CodeBlock({ lang, raw, children }: { lang: string | null; raw: string; children: ReactNode }) {
  const { t } = useI18n();
  const act = useRichActions();
  const lines = raw.split("\n").length;
  const long = lines > COLLAPSE_LINES;
  const [open, setOpen] = useState(!long);
  const [wrap, setWrap] = useState(false);
  const ref = useRef<HTMLElement>(null);
  const ext = extForLang(lang);
  return (
    <figure ref={ref} className="cblock" data-lang={lang ?? undefined} data-testid="code-block">
      <figcaption className="cblock__head z-small">
        <span className="cblock__lang">{lang ?? t("md.code")}</span>
        <span className="mempage__grow" />
        <RichBar
          className="cblock__actions"
          primary={[
            {
              id: "wrap",
              label: t("rich.wrap"),
              icon: <WrapIcon />,
              pressed: wrap,
              onSelect: () => setWrap((w) => !w),
            },
            { id: "copy", label: t("md.copyCode"), icon: <CopyIcon />, onSelect: () => void act.copyText(raw) },
          ]}
          more={[
            {
              id: "md",
              label: t("rich.copyFenced"),
              icon: <FileTextIcon size="sm" />,
              onSelect: () => void act.copyText(fenced(lang ?? "", raw)),
            },
            {
              id: "file",
              label: t("rich.saveFile", { ext: ext === "Dockerfile" ? ext : `.${ext}` }),
              icon: <DownloadIcon size="sm" />,
              onSelect: () => void act.saveText(stampedName("kode", ext), raw),
            },
            ...imageActions(act, t, () => ref.current, "kode", false),
          ]}
        />
      </figcaption>
      <pre className="cblock__pre" data-collapsed={!open ? "" : undefined} data-wrap={wrap ? "" : undefined}>
        {children}
      </pre>
      {long && (
        <button
          type="button"
          className="cblock__more z-small"
          aria-expanded={open}
          data-no-export=""
          onClick={() => setOpen((o) => !o)}
        >
          {open ? t("md.collapse") : t("md.expand", { n: lines })}
        </button>
      )}
    </figure>
  );
}

function RichTable({ children }: { children?: ReactNode }) {
  const { t } = useI18n();
  const act = useRichActions();
  const box = useRef<HTMLDivElement>(null);
  const table = () => box.current?.querySelector("table") ?? null;
  const rows = () => {
    const el = table();
    return el ? tableMatrix(el) : [];
  };
  const html = () => table()?.outerHTML ?? "";
  return (
    <div ref={box} className="rblock rblock--table">
      <RichBar
        primary={[
          {
            id: "copy",
            label: t("rich.copyTable"),
            icon: <TableIcon />,
            onSelect: () => void act.copyRich(html(), toTsv(rows())),
          },
        ]}
        more={[
          {
            id: "md",
            label: t("rich.copyMarkdown"),
            icon: <FileTextIcon size="sm" />,
            onSelect: () => void act.copyText(toMarkdownTable(rows())),
          },
          {
            id: "tsv",
            label: t("rich.copyTsv"),
            icon: <TableIcon size="sm" />,
            onSelect: () => void act.copyText(toTsv(rows())),
          },
          {
            id: "csv",
            label: t("rich.copyCsv"),
            icon: <CopyIcon size="sm" />,
            onSelect: () => void act.copyText(toCsv(rows())),
          },
          {
            id: "html",
            label: t("rich.copyHtml"),
            icon: <CodeIcon size="sm" />,
            onSelect: () => void act.copyText(html()),
          },
          {
            id: "savecsv",
            label: t("rich.saveCsv"),
            icon: <DownloadIcon size="sm" />,
            onSelect: () => void act.saveText(stampedName("tabel", "csv"), toCsv(rows())),
          },
          {
            id: "savemd",
            label: t("rich.saveMarkdown"),
            icon: <DownloadIcon size="sm" />,
            onSelect: () => void act.saveText(stampedName("tabel", "md"), toMarkdownTable(rows())),
          },
          ...imageActions(act, t, table, "tabel"),
        ]}
      />
      {/* biome-ignore lint/a11y/noNoninteractiveTabindex: area gulir harus bisa difokus agar tabel lebar bisa digulir dengan keyboard */}
      <section className="md__table" tabIndex={0} aria-label={t("rich.table")}>
        <table>{children}</table>
      </section>
    </div>
  );
}

/** Rumus blok KaTeX (`span.katex-display`): salin LaTeX/MathML, salin/simpan sebagai gambar atau SVG, simpan .tex. */
function MathBlock({ latex, children }: { latex: string; children?: ReactNode }) {
  const { t } = useI18n();
  const act = useRichActions();
  const ref = useRef<HTMLSpanElement>(null);
  const formula = () => ref.current?.querySelector<HTMLElement>(".katex-display") ?? null;
  return (
    <span ref={ref} className="rblock rblock--math" data-testid="math-block">
      <RichBar
        primary={[
          { id: "tex", label: t("rich.copyLatex"), icon: <SigmaIcon />, onSelect: () => void act.copyText(latex) },
        ]}
        more={[
          {
            id: "texd",
            label: t("rich.copyLatexDisplay"),
            icon: <CopyIcon size="sm" />,
            onSelect: () => void act.copyText(`$$\n${latex}\n$$`),
          },
          {
            id: "mathml",
            label: t("rich.copyMathml"),
            icon: <CodeIcon size="sm" />,
            onSelect: () => void act.copyText(ref.current?.querySelector("math")?.outerHTML ?? ""),
          },
          ...imageActions(act, t, formula, "rumus"),
          {
            id: "tex-file",
            label: t("rich.saveFile", { ext: ".tex" }),
            icon: <DownloadIcon size="sm" />,
            onSelect: () => void act.saveText(stampedName("rumus", "tex"), latex),
          },
        ]}
      />
      {children}
    </span>
  );
}

/** Gambar data: URI: salin gambar dan simpan berkas aslinya. */
function RichImage({ src, alt }: { src: string; alt: string }) {
  const { t } = useI18n();
  const act = useRichActions();
  const ref = useRef<HTMLImageElement>(null);
  const mime = /^data:([^;,]+)/.exec(src)?.[1] ?? "image/png";
  const ext = (mime.split("/")[1] ?? "png").replace("svg+xml", "svg").replace("jpeg", "jpg");
  const save = async () =>
    act.saveBytes(stampedName("gambar", ext), new Uint8Array(await (await fetch(src)).arrayBuffer()));
  return (
    <span className="rblock rblock--image">
      <RichBar
        primary={[
          {
            id: "copy",
            label: t("rich.copyImage"),
            icon: <CopyIcon />,
            onSelect: () => void act.copyImage(ref.current),
          },
          { id: "save", label: t("rich.saveImage"), icon: <DownloadIcon />, onSelect: () => void save() },
        ]}
      />
      <img ref={ref} className="md__image" src={src} alt={alt} draggable={false} />
    </span>
  );
}

/** Status streaming lewat konteks: objek `components` harus stabil (identitas tetap), jika tidak React me-remount seluruh blok
 * kode/Mermaid pada setiap render dan state (ciut/lebar, diagram) hilang. */
const StreamingContext = createContext(false);

function Pre({ node, children }: { node?: unknown; children?: ReactNode }) {
  const streaming = useContext(StreamingContext);
  const code = (node as HastNode | undefined)?.children?.[0] as
    | (HastNode & { properties?: { className?: string[] } })
    | undefined;
  const lang = code?.properties?.className?.find((c) => c.startsWith("language-"))?.slice("language-".length) ?? null;
  const raw = hastText(code).replace(/\n$/, "");
  // blok kode ("pre > code"): Mermaid dirender, sisanya blok berwarna dengan tombol salin
  if (lang === "mermaid") return <Mermaid code={raw} streaming={streaming} />;
  return (
    <CodeBlock lang={lang} raw={raw}>
      {children}
    </CodeBlock>
  );
}

const COMPONENTS: Components = {
  // tautan web → menu pilihan (browser Zandor / eksternal / salin); mailto dan jangkar tetap tautan biasa
  a: ({ href, children }) =>
    href && isWebLink(href) ? (
      <ChatLink href={href}>{children}</ChatLink>
    ) : href && SAFE_LINK.test(href) ? (
      <a href={href} target="_blank" rel="noreferrer noopener">
        {children}
      </a>
    ) : (
      <span>{children}</span>
    ),
  // hanya data: URI bertipe gambar yang dimuat; gambar dari jaringan tidak diambil (cegah eksfiltrasi lewat URL)
  img: ({ src, alt }) =>
    typeof src === "string" && SAFE_IMAGE_URI.test(src) ? (
      <RichImage src={src} alt={alt ?? ""} />
    ) : (
      <span className="md__img">{alt ?? ""}</span>
    ),
  table: ({ children }) => <RichTable>{children}</RichTable>,
  // rumus blok KaTeX → bilah aksi; rumus sebaris dibiarkan apa adanya
  span: ({ node, className, children, ...rest }) => {
    const latex = className?.split(" ").includes("katex-display") ? latexOf(node as never) : null;
    if (latex !== null) {
      return (
        <MathBlock latex={latex}>
          <span className={className}>{children}</span>
        </MathBlock>
      );
    }
    return (
      <span className={className} {...rest}>
        {children}
      </span>
    );
  },
  pre: Pre,
};

const urlTransform = (url: string) => (SAFE_IMAGE_URI.test(url) ? url : defaultUrlTransform(url));

const REMARK = [remarkGfm, remarkMath];
const REHYPE = [
  [rehypeKatex, { throwOnError: false, strict: "ignore" }],
  [
    rehypeHighlight,
    { languages: LANGUAGES, detect: false, ignoreMissing: true, plainText: ["mermaid", "text", "txt"] },
  ],
] as never;

export default function RichMarkdown({ text, streaming = false }: { text: string; streaming?: boolean }) {
  return (
    <StreamingContext.Provider value={streaming}>
      <div className="md">
        <ReactMarkdown
          remarkPlugins={REMARK}
          rehypePlugins={REHYPE}
          components={COMPONENTS}
          urlTransform={urlTransform}
        >
          {text}
        </ReactMarkdown>
      </div>
    </StreamingContext.Provider>
  );
}

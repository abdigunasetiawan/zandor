// Renderer Markdown lengkap untuk respons model (Fase 12): GFM (tabel, daftar tugas, coret), KaTeX, blok kode berwarna dengan
// tombol salin, diagram Mermaid, dan tabel yang bisa digulir. Aman secara bawaan: HTML mentah tidak dirender, tautan hanya
// http(s)/mailto, gambar tidak dimuat dari luar (mencegah eksfiltrasi lewat URL), Mermaid strict. Isi dari model tidak dipercaya.
import { useI18n } from "@zandor/i18n/react";
import { CopyIcon, IconButton, useToast } from "@zandor/ui";
import dockerfile from "highlight.js/lib/languages/dockerfile";
import powershell from "highlight.js/lib/languages/powershell";
import "katex/dist/katex.min.css";
import { common } from "lowlight";
import { createContext, type ReactNode, useContext, useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { Mermaid } from "./Mermaid";

const SAFE_LINK = /^(https?:|mailto:|#)/i;
const LANGUAGES = { ...common, powershell, dockerfile };
const COLLAPSE_LINES = 30;

type HastNode = { type?: string; value?: string; children?: HastNode[] };
const hastText = (n: HastNode | undefined): string =>
  !n ? "" : typeof n.value === "string" ? n.value : (n.children ?? []).map(hastText).join("");

function CodeBlock({ lang, raw, children }: { lang: string | null; raw: string; children: ReactNode }) {
  const { t } = useI18n();
  const toast = useToast();
  const lines = raw.split("\n").length;
  const long = lines > COLLAPSE_LINES;
  const [open, setOpen] = useState(!long);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(raw);
      toast.show({ title: t("chat.copied"), tone: "success", timeoutMs: 1500 });
    } catch (e) {
      toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
    }
  };
  return (
    <figure className="cblock" data-lang={lang ?? undefined} data-testid="code-block">
      <figcaption className="cblock__head z-small">
        <span className="cblock__lang">{lang ?? t("md.code")}</span>
        <span className="mempage__grow" />
        <IconButton label={t("md.copyCode")} size="sm" icon={<CopyIcon />} onClick={() => void copy()} />
      </figcaption>
      <pre className="cblock__pre" data-collapsed={!open ? "" : undefined}>
        {children}
      </pre>
      {long && (
        <button type="button" className="cblock__more z-small" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
          {open ? t("md.collapse") : t("md.expand", { n: lines })}
        </button>
      )}
    </figure>
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
  a: ({ href, children }) =>
    href && SAFE_LINK.test(href) ? (
      <a href={href} target="_blank" rel="noreferrer noopener">
        {children}
      </a>
    ) : (
      <span>{children}</span>
    ),
  img: ({ alt }) => <span className="md__img">{alt ?? ""}</span>,
  table: ({ children }) => (
    // biome-ignore lint/a11y/noNoninteractiveTabindex: area gulir harus bisa difokus agar tabel lebar bisa digulir dengan keyboard
    <section className="md__table" tabIndex={0} aria-label="tabel">
      <table>{children}</table>
    </section>
  ),
  pre: Pre,
};

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
        <ReactMarkdown remarkPlugins={REMARK} rehypePlugins={REHYPE} components={COMPONENTS}>
          {text}
        </ReactMarkdown>
      </div>
    </StreamingContext.Provider>
  );
}

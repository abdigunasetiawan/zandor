// Markdown (GFM) + KaTeX + Mermaid. Aman: HTML mentah tidak dirender, tautan hanya http(s)/mailto, gambar tidak dimuat
// dari luar, Mermaid berjalan dengan securityLevel "strict". Isi dari agent/berkas pengguna tidak dipercaya.
import { decodeText } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import "katex/dist/katex.min.css";
import { type ReactNode, useEffect, useId, useState } from "react";
import ReactMarkdown from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import type { ViewerProps } from "./types";

const SAFE_LINK = /^(https?:|mailto:|#)/i;

function isDark(): boolean {
  const a = document.documentElement.dataset.theme;
  if (a === "dark") return true;
  if (a === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function Mermaid({ code }: { code: string }) {
  const { t } = useI18n();
  const id = `mmd-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const [state, setState] = useState<{ svg: string } | { error: string } | null>(null);
  useEffect(() => {
    let alive = true;
    setState(null);
    void (async () => {
      try {
        const { default: mermaid } = await import("mermaid");
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: isDark() ? "dark" : "default",
          layout: "dagre", // ELK berlisensi EPL/GPL dan di-stub (ADR-035)
        });
        const { svg } = await mermaid.render(id, code);
        if (alive) setState({ svg });
      } catch (e) {
        if (alive) setState({ error: e instanceof Error ? e.message : String(e) });
      }
    })();
    return () => {
      alive = false;
    };
  }, [code, id]);
  if (!state) return <p className="docview__note z-small">{t("doc.mermaidLoading")}</p>;
  if ("error" in state) {
    return (
      <div className="docview__error" role="alert">
        <p className="z-small">{t("doc.mermaidError", { error: state.error.split("\n")[0] ?? "" })}</p>
        <pre>{code}</pre>
      </div>
    );
  }
  // svg dari mermaid (strict) → DOMPurify di dalam mermaid; berasal dari library, bukan HTML mentah dari berkas.
  // biome-ignore lint/security/noDangerouslySetInnerHtml: SVG dihasilkan mermaid (securityLevel strict, disanitasi DOMPurify), bukan HTML mentah dari berkas
  return <div className="docview__mermaid" data-testid="mermaid" dangerouslySetInnerHTML={{ __html: state.svg }} />;
}

function text(children: ReactNode): string {
  if (typeof children === "string") return children;
  if (Array.isArray(children)) return children.map(text).join("");
  return "";
}

export default function MarkdownViewer({ bytes }: ViewerProps) {
  const decoded = decodeText(bytes);
  const { t } = useI18n();
  return (
    <div className="docview docview--md">
      {decoded.truncated && <p className="docview__note z-small">{t("doc.truncated", { mb: 5 })}</p>}
      <article className="md md--doc" data-testid="markdown">
        <ReactMarkdown
          remarkPlugins={[remarkGfm, remarkMath]}
          rehypePlugins={[[rehypeKatex, { throwOnError: false, strict: "ignore" }]]}
          components={{
            a: ({ href, children }) =>
              href && SAFE_LINK.test(href) ? (
                <a href={href} target="_blank" rel="noreferrer noopener">
                  {children}
                </a>
              ) : (
                <span>{children}</span>
              ),
            img: ({ alt }) => <span className="md__img">{alt ?? ""}</span>,
            code: ({ className, children }) => {
              const lang = /language-(\w+)/.exec(className ?? "")?.[1];
              if (lang === "mermaid") return <Mermaid code={text(children).replace(/\n$/, "")} />;
              return <code className={className}>{children}</code>;
            },
            pre: ({ children }) => <pre>{children}</pre>,
          }}
        >
          {decoded.text}
        </ReactMarkdown>
      </article>
    </div>
  );
}

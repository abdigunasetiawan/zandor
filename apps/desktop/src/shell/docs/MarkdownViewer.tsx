// Markdown (GFM) + KaTeX + Mermaid. Aman: HTML mentah tidak dirender, tautan hanya http(s)/mailto, gambar tidak dimuat
// dari luar, Mermaid berjalan dengan securityLevel "strict". Isi dari agent/berkas pengguna tidak dipercaya.
import { decodeText } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import "katex/dist/katex.min.css";
import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { Mermaid } from "../rich/Mermaid";
import type { ViewerProps } from "./types";

const SAFE_LINK = /^(https?:|mailto:|#)/i;

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

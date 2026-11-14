// Diagram Mermaid yang dipakai chat dan penampil Markdown. Aman: securityLevel "strict" (disanitasi DOMPurify oleh
// mermaid), ELK di-stub (ADR-035) sehingga layout dagre. Saat teks masih mengalir (streaming) diagram belum lengkap, jadi
// sumbernya ditampilkan dulu dan dirender setelah selesai; diagram tidak valid selalu menampilkan galat + sumber.
import { useI18n } from "@zandor/i18n/react";
import { useEffect, useId, useState } from "react";

function isDark(): boolean {
  const a = document.documentElement.dataset.theme;
  if (a === "dark") return true;
  if (a === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function Mermaid({ code, streaming = false }: { code: string; streaming?: boolean }) {
  const { t } = useI18n();
  const id = `mmd-${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const [state, setState] = useState<{ svg: string } | { error: string } | null>(null);

  useEffect(() => {
    if (streaming) return;
    let alive = true;
    setState(null);
    void (async () => {
      try {
        const { default: mermaid } = await import("mermaid");
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: isDark() ? "dark" : "default",
          layout: "dagre",
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
  }, [code, id, streaming]);

  if (streaming) {
    return (
      <div className="mermaid mermaid--pending" data-testid="mermaid-pending">
        <p className="z-small docview__note">{t("doc.mermaidStreaming")}</p>
        <pre>{code}</pre>
      </div>
    );
  }
  if (!state) return <p className="docview__note z-small">{t("doc.mermaidLoading")}</p>;
  if ("error" in state) {
    return (
      <div className="docview__error" role="alert">
        <p className="z-small">{t("doc.mermaidError", { error: state.error.split("\n")[0] ?? "" })}</p>
        <pre>{code}</pre>
      </div>
    );
  }
  // biome-ignore lint/security/noDangerouslySetInnerHtml: SVG dihasilkan mermaid (securityLevel strict, disanitasi DOMPurify), bukan HTML mentah dari model/berkas
  return <div className="docview__mermaid" data-testid="mermaid" dangerouslySetInnerHTML={{ __html: state.svg }} />;
}

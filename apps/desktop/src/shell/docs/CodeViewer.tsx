// Penampil teks/kode (Monaco, read-only). `forceLanguage` dipakai mode "mentah" untuk jenis lain (mis. Markdown mentah).
import { decodeText, languageFor } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { useEffect, useRef } from "react";
import { applyZandorTheme, monaco, monacoLanguage } from "./monaco-setup";
import type { ViewerProps } from "./types";

function isDark(): boolean {
  const a = document.documentElement.dataset.theme;
  if (a === "dark") return true;
  if (a === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export default function CodeViewer({ tab, bytes, onHandle }: ViewerProps) {
  const { t } = useI18n();
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<ReturnType<typeof monaco.editor.create> | null>(null);
  const decoded = decodeText(bytes);

  // buat editor sekali per mount
  // biome-ignore lint/correctness/useExhaustiveDependencies: editor dibuat sekali; isi dan opsi diperbarui di efek lain
  useEffect(() => {
    if (!host.current) return;
    const ed = monaco.editor.create(host.current, {
      value: "",
      readOnly: true,
      domReadOnly: true,
      automaticLayout: true,
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      renderWhitespace: "none",
      fontFamily: getComputedStyle(document.documentElement).getPropertyValue("--z-font-code").trim(),
      fontSize: 13,
      theme: applyZandorTheme(isDark()),
      ariaLabel: tab.name,
      stickyScroll: { enabled: false },
      "semanticHighlighting.enabled": false,
    });
    editor.current = ed;
    onHandle({
      find: () => void ed.getAction("actions.find")?.run(),
      text: () => ed.getModel()?.getValue() ?? null,
    });
    // ikuti pergantian tema aplikasi (atribut data-theme pada <html>)
    const mo = new MutationObserver(() => monaco.editor.setTheme(applyZandorTheme(isDark())));
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "style"] });
    return () => {
      mo.disconnect();
      onHandle(null);
      ed.getModel()?.dispose();
      ed.dispose();
      editor.current = null;
    };
  }, []);

  // isi + bahasa
  useEffect(() => {
    const ed = editor.current;
    if (!ed) return;
    const lang = monacoLanguage(languageFor(tab.path));
    const old = ed.getModel();
    const model = monaco.editor.createModel(decoded.text, lang);
    ed.setModel(model);
    old?.dispose();
  }, [decoded.text, tab.path]);

  useEffect(() => {
    editor.current?.updateOptions({ wordWrap: tab.wrap ? "on" : "off" });
  }, [tab.wrap]);

  return (
    <div className="docview docview--code">
      {decoded.truncated && (
        <p className="docview__note z-small" role="status">
          {t("doc.truncated", { mb: 5 })}
        </p>
      )}
      <div className="docview__editor" ref={host} data-testid="monaco" />
    </div>
  );
}

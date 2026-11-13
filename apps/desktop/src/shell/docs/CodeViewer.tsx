// Penampil teks/kode (Monaco, read-only). `forceLanguage` dipakai mode "mentah" untuk jenis lain (mis. Markdown mentah).
import { decodeText, languageFor } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { useEffect, useRef } from "react";
import { useServices } from "../services";
import { applyEditorTheme, monaco, monacoLanguage } from "./monaco-setup";
import type { ViewerProps } from "./types";

function isDark(): boolean {
  const a = document.documentElement.dataset.theme;
  if (a === "dark") return true;
  if (a === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

const norm = (s: string) => s.replace(/\r\n/g, "\n");

export default function CodeViewer({ tab, bytes, onHandle, onSaveRequest }: ViewerProps) {
  const { t } = useI18n();
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<ReturnType<typeof monaco.editor.create> | null>(null);
  const decoded = decodeText(bytes);
  const tabIdRef = useRef(tab.id);
  tabIdRef.current = tab.id;
  const { appearance, docs } = useServices();
  const baseline = useRef("");
  const saveRef = useRef(onSaveRequest);
  saveRef.current = onSaveRequest;

  const currentTheme = () => {
    const a = appearance.state();
    return applyEditorTheme(a.editorTheme, a.editorThemes, isDark());
  };

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
      theme: currentTheme(),
      ariaLabel: tab.name,
      stickyScroll: { enabled: false },
      "semanticHighlighting.enabled": false,
    });
    editor.current = ed;
    onHandle({
      find: () => void ed.getAction("actions.find")?.run(),
      text: () => ed.getModel()?.getValue() ?? null,
    });
    // Ctrl+S menyimpan (hanya berarti saat mode edit; DocPanel mengabaikannya bila tidak sedang mengedit)
    ed.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => saveRef.current?.());
    ed.onDidChangeModelContent(() => {
      if (ed.getOption(monaco.editor.EditorOption.readOnly)) return;
      docs.markDirty(tabIdRef.current, norm(ed.getValue()) !== baseline.current);
    });
    // ikuti pergantian tema aplikasi (atribut data-theme pada <html>)
    const mo = new MutationObserver(() => monaco.editor.setTheme(currentTheme()));
    // ...dan pilihan tema editor di halaman Tampilan
    const off = appearance.subscribe(() => monaco.editor.setTheme(currentTheme()));
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "style"] });
    return () => {
      mo.disconnect();
      off();
      onHandle(null);
      ed.getModel()?.dispose();
      ed.dispose();
      editor.current = null;
    };
  }, []);

  // isi + bahasa (tab.version memuat ulang isi saat edit dibuang)
  // biome-ignore lint/correctness/useExhaustiveDependencies: tab.version hanya pemicu muat ulang
  useEffect(() => {
    const ed = editor.current;
    if (!ed) return;
    const lang = monacoLanguage(languageFor(tab.path));
    const old = ed.getModel();
    // Setelah menyimpan, isi editor sama dengan teks baru: jangan buat ulang model (kursor/undo tetap).
    if (old && norm(old.getValue()) === norm(decoded.text) && old.getLanguageId() === lang) {
      baseline.current = norm(decoded.text);
      return;
    }
    const model = monaco.editor.createModel(decoded.text, lang);
    baseline.current = norm(decoded.text);
    ed.setModel(model);
    old?.dispose();
  }, [decoded.text, tab.path, tab.version]);

  useEffect(() => {
    editor.current?.updateOptions({ readOnly: !tab.editing, domReadOnly: !tab.editing });
    if (tab.editing) editor.current?.focus();
  }, [tab.editing]);

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

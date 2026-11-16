// Penampil teks/kode (Monaco, read-only). `forceLanguage` dipakai mode "mentah" untuk jenis lain (mis. Markdown mentah).
import { DOC_CODE_SIZE, type DocStyle, decodeText, languageFor } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { useEffect, useRef } from "react";
import { useAppearance, useServices } from "../services";
import { applyEditorTheme, monaco, monacoLanguage } from "./monaco-setup";
import { activateTextMateTheme, ensureTextMateLanguage, textMateLanguage } from "./textmate";
import type { ViewerProps } from "./types";

const codeFont = () => getComputedStyle(document.documentElement).getPropertyValue("--z-font-code").trim();

/** Opsi Monaco dari kustomisasi dokumen (Pengaturan → Dokumen / tombol Kustomisasi Panel 3). */
export function editorOptions(d: DocStyle) {
  const size = d.codeSize ?? DOC_CODE_SIZE.def;
  return {
    fontSize: size,
    lineHeight: Math.round(size * (d.codeLineHeight ?? 1.5)),
    fontFamily: d.codeFont ?? codeFont(),
    fontLigatures: d.ligatures ?? false,
    minimap: { enabled: d.minimap ?? false },
    lineNumbers: (d.lineNumbers ?? true) ? ("on" as const) : ("off" as const),
    renderWhitespace: (d.whitespace ?? false) ? ("all" as const) : ("none" as const),
  };
}

/** Ukuran huruf berikutnya untuk Ctrl+roda mouse (naik = perbesar). */
export function zoomStep(current: number, deltaY: number): number {
  const next = current + (deltaY < 0 ? 1 : -1);
  return Math.min(DOC_CODE_SIZE.max, Math.max(DOC_CODE_SIZE.min, next));
}

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
  const docStyle = useAppearance().docs;
  const baseline = useRef("");
  const saveRef = useRef(onSaveRequest);
  saveRef.current = onSaveRequest;

  const currentTheme = () => {
    const a = appearance.state();
    return applyEditorTheme(a.editorTheme, a.editorThemes, isDark());
  };
  const pathRef = useRef(tab.path);
  pathRef.current = tab.path;
  const syncSeq = useRef(0);
  const modelPath = useRef<string | null>(null);

  /**
   * Terapkan tema + bahasa. Tema terpasang (VS Code) memakai grammar TextMate agar penempatan warna sama dengan VS Code;
   * selama grammar dimuat (atau bila gagal) tokenizer Monarch dipakai sebagai cadangan.
   */
  const syncHighlighting = async () => {
    const ed = editor.current;
    const model = ed?.getModel();
    if (!ed || !model) return;
    const seq = ++syncSeq.current;
    const a = appearance.state();
    const lang = languageFor(pathRef.current);
    const plain = monacoLanguage(lang);
    const custom = a.editorThemes.find((x) => x.id === a.editorTheme);
    const tmLang = custom ? textMateLanguage(lang, pathRef.current) : null;
    if (!custom) {
      monaco.editor.setTheme(currentTheme());
      if (model.getLanguageId() !== plain) monaco.editor.setModelLanguage(model, plain);
      return;
    }
    try {
      const [themeName, langId] = await Promise.all([
        activateTextMateTheme(custom),
        tmLang ? ensureTextMateLanguage(tmLang) : Promise.resolve(plain),
      ]);
      if (seq !== syncSeq.current || ed.getModel() !== model) return;
      monaco.editor.setTheme(themeName);
      if (model.getLanguageId() !== langId) monaco.editor.setModelLanguage(model, langId);
    } catch (e) {
      console.warn("pewarnaan TextMate gagal, memakai Monarch", e);
      if (seq !== syncSeq.current) return;
      monaco.editor.setTheme(currentTheme());
      if (model.getLanguageId() !== plain) monaco.editor.setModelLanguage(model, plain);
    }
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
      scrollBeyondLastLine: false,
      ...editorOptions(appearance.state().docs),
      mouseWheelZoom: false,
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
    // Ctrl+roda mouse = perbesar/perkecil teks (bukan UI); tersimpan sebagai ukuran huruf kode dokumen.
    // Ditangkap di fase capture agar tidak sampai ke gulir Monaco maupun zoom WebView.
    const onWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.deltaY === 0) return;
      e.preventDefault();
      e.stopPropagation();
      const cur = appearance.state().docs.codeSize ?? DOC_CODE_SIZE.def;
      const next = zoomStep(cur, e.deltaY);
      if (next !== cur) appearance.setDoc("codeSize", next === DOC_CODE_SIZE.def ? null : next);
    };
    const el = host.current;
    el.addEventListener("wheel", onWheel, { capture: true, passive: false });
    // ikuti pergantian tema aplikasi (atribut data-theme pada <html>)
    const mo = new MutationObserver(() => void syncHighlighting());
    // ...dan pilihan tema editor di halaman Tampilan
    const off = appearance.subscribe(() => void syncHighlighting());
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "style"] });
    return () => {
      el.removeEventListener("wheel", onWheel, { capture: true });
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
    // (bahasa model bisa `tm-*` saat tema VS Code aktif, jadi bandingkan path, bukan id bahasa)
    if (old && norm(old.getValue()) === norm(decoded.text) && modelPath.current === tab.path) {
      baseline.current = norm(decoded.text);
      return;
    }
    const model = monaco.editor.createModel(decoded.text, lang);
    baseline.current = norm(decoded.text);
    modelPath.current = tab.path;
    ed.setModel(model);
    old?.dispose();
    void syncHighlighting();
  }, [decoded.text, tab.path, tab.version]);

  useEffect(() => {
    editor.current?.updateOptions({ readOnly: !tab.editing, domReadOnly: !tab.editing });
    if (tab.editing) editor.current?.focus();
  }, [tab.editing]);

  useEffect(() => {
    editor.current?.updateOptions(editorOptions(docStyle));
  }, [docStyle]);

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
      <div
        className="docview__editor"
        ref={host}
        data-testid="monaco"
        data-font-size={docStyle.codeSize ?? DOC_CODE_SIZE.def}
      />
    </div>
  );
}

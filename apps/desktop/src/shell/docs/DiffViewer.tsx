// Tinjau perubahan agent di Panel 3 (ADR-075): Monaco diff (kiri = sebelum, kanan = isi berkas saat ini) + bilah
// per bagian (hunk). "Terima" hanya menandai bagian selesai; "Tolak" mengembalikan bagian itu ke isi semula dan
// langsung menyimpan berkas (bisa diurungkan dari toast). Sisi "sebelum" dihitung dengan membalik diff terhadap isi
// berkas saat ini, jadi tidak ada isi karangan: bila diff tidak lagi cocok dengan berkas, penampil mengatakannya.
import {
  decodeText,
  dropHunk,
  formatHunks,
  type Hunk,
  languageFor,
  locateHunk,
  parsePatch,
  reverseApply,
  revertHunk,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Button,
  CheckIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  CloseIcon,
  IconButton,
  UndoIcon,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import { useAppearance, useServices } from "../services";
import { editorOptions } from "./CodeViewer";
import { applyEditorTheme, monaco, monacoLanguage } from "./monaco-setup";
import type { ViewerProps } from "./types";

function isDark(): boolean {
  const a = document.documentElement.dataset.theme;
  if (a === "dark") return true;
  if (a === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export default function DiffViewer({ tab, bytes, onHandle }: ViewerProps) {
  const { t } = useI18n();
  const { docs, appearance } = useServices();
  const docStyle = useAppearance().docs;
  const toast = useToast();
  const confirm = useConfirm();
  const host = useRef<HTMLDivElement>(null);
  const diffEd = useRef<ReturnType<typeof monaco.editor.createDiffEditor> | null>(null);
  const [index, setIndex] = useState(0);
  const [busy, setBusy] = useState(false);

  const current = decodeText(bytes).text;
  const hunks = useMemo(() => parsePatch(tab.diff ?? ""), [tab.diff]);
  const before = useMemo(() => reverseApply(current, hunks), [current, hunks]);
  const at = Math.min(index, Math.max(0, hunks.length - 1));
  const hunk = hunks[at];
  // Menolak = menulis berkas: hanya bila sumber bisa menulis dan isinya aman diedit (UTF-8, tidak terpotong).
  const block = docs.editBlock(tab.id);
  const rejectReason = block === "noWrite" ? t("diff.noWrite") : block ? t("diff.cantWrite") : null;

  // biome-ignore lint/correctness/useExhaustiveDependencies: editor dibuat sekali; isi dan opsi diperbarui di efek lain
  useEffect(() => {
    if (!host.current) return;
    const a = appearance.state();
    const ed = monaco.editor.createDiffEditor(host.current, {
      readOnly: true,
      originalEditable: false,
      automaticLayout: true,
      scrollBeyondLastLine: false,
      renderSideBySide: true,
      useInlineViewWhenSpaceIsLimited: true,
      renderOverviewRuler: false,
      // indikator +/− dan tombol revert di margin memakai font codicon yang tidak dibundel → kotak kosong
      renderIndicators: false,
      renderMarginRevertIcon: false,
      ...editorOptions(a.docs),
      theme: applyEditorTheme(a.editorTheme, a.editorThemes, isDark()),
      ariaLabel: t("diff.aria", { name: tab.name }),
    });
    diffEd.current = ed;
    onHandle({
      find: () => void ed.getModifiedEditor().getAction("actions.find")?.run(),
      text: () => ed.getModifiedEditor().getModel()?.getValue() ?? null,
    });
    const mo = new MutationObserver(() => {
      const s = appearance.state();
      monaco.editor.setTheme(applyEditorTheme(s.editorTheme, s.editorThemes, isDark()));
    });
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "style"] });
    return () => {
      mo.disconnect();
      onHandle(null);
      const m = ed.getModel();
      ed.dispose();
      m?.original.dispose();
      m?.modified.dispose();
      diffEd.current = null;
    };
  }, []);

  useEffect(() => diffEd.current?.updateOptions(editorOptions(docStyle)), [docStyle]);

  // isi kedua sisi
  useEffect(() => {
    const ed = diffEd.current;
    if (!ed || before === null) return;
    const lang = monacoLanguage(languageFor(tab.path));
    const old = ed.getModel();
    ed.setModel({
      original: monaco.editor.createModel(before, lang),
      modified: monaco.editor.createModel(current, lang),
    });
    old?.original.dispose();
    old?.modified.dispose();
  }, [before, current, tab.path]);

  // gulir ke bagian aktif
  useEffect(() => {
    const ed = diffEd.current?.getModifiedEditor();
    if (!ed || !hunk) return;
    const pos = locateHunk(current, hunk);
    if (!pos) return;
    ed.revealLinesInCenter(pos.line, pos.line + Math.max(0, pos.count - 1));
    ed.setPosition({ lineNumber: pos.line, column: 1 });
  }, [hunk, current]);

  const finish = (rest: Hunk[]) => {
    if (rest.length === 0) {
      docs.setDiff(tab.id, null);
      toast.show({ title: t("diff.done"), tone: "success" });
    } else docs.setDiff(tab.id, formatHunks(rest));
  };

  const accept = () => {
    if (!hunk) return;
    finish(dropHunk(hunks, at, false));
  };

  /** Tulis `text` ke berkas; mengembalikan true bila tersimpan. */
  const write = async (text: string): Promise<boolean> => {
    const r = await docs.save(tab.id, text);
    if (r === "saved") return true;
    toast.show({
      title: r === "conflict" ? t("diff.conflict") : t("diff.saveError", { error: r.error }),
      tone: "danger",
    });
    return false;
  };

  const reject = async () => {
    if (!hunk || busy) return;
    const next = revertHunk(current, hunk);
    if (next === null) {
      toast.show({ title: t("diff.stale"), tone: "warning" });
      return;
    }
    setBusy(true);
    const prevText = current;
    const prevPatch = tab.diff ?? null;
    try {
      if (!(await write(next))) return;
      finish(dropHunk(hunks, at, true));
      toast.show({
        title: t("diff.rejected"),
        tone: "neutral",
        action: {
          label: t("diff.undo"),
          onClick: () =>
            void (async () => {
              if (await write(prevText)) docs.setDiff(tab.id, prevPatch);
            })(),
        },
      });
    } finally {
      setBusy(false);
    }
  };

  const rejectAll = async () => {
    if (busy || before === null) return;
    const ok = await confirm({
      title: t("diff.rejectAll.title"),
      description: t("diff.rejectAll.body", { n: hunks.length, name: tab.name }),
      confirmLabel: t("diff.rejectAll"),
      tone: "danger",
    });
    if (!ok) return;
    setBusy(true);
    try {
      if (await write(before)) {
        docs.setDiff(tab.id, null);
        toast.show({ title: t("diff.rejectedAll"), tone: "neutral" });
      }
    } finally {
      setBusy(false);
    }
  };

  const closeDiff = () => docs.setDiff(tab.id, null);

  return (
    <div className="docview ddiff">
      <div className="ddiff__bar" role="toolbar" aria-label={t("diff.toolbar")}>
        <span className="z-small ddiff__pos" role="status" data-testid="diff-pos">
          {hunks.length ? t("diff.pos", { i: at + 1, n: hunks.length }) : t("diff.none")}
        </span>
        <IconButton
          label={t("diff.prev")}
          size="sm"
          icon={<ChevronUpIcon size="sm" />}
          disabled={at <= 0}
          onClick={() => setIndex(at - 1)}
        />
        <IconButton
          label={t("diff.next")}
          size="sm"
          icon={<ChevronDownIcon size="sm" />}
          disabled={at >= hunks.length - 1}
          onClick={() => setIndex(at + 1)}
        />
        <span className="ddiff__actions">
          <Button size="sm" variant="solid" disabled={!hunk || busy} onClick={accept}>
            <CheckIcon size="sm" /> {t("diff.accept")}
          </Button>
          <Button
            size="sm"
            disabled={!hunk || busy || before === null || rejectReason !== null}
            title={rejectReason ?? undefined}
            onClick={() => void reject()}
          >
            <UndoIcon size="sm" /> {t("diff.reject")}
          </Button>
          <Button size="sm" variant="ghost" disabled={busy} onClick={closeDiff}>
            {t("diff.acceptAll")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={busy || before === null || rejectReason !== null}
            title={rejectReason ?? undefined}
            onClick={() => void rejectAll()}
          >
            {t("diff.rejectAll")}
          </Button>
          <IconButton label={t("diff.close")} size="sm" icon={<CloseIcon size="sm" />} onClick={closeDiff} />
        </span>
      </div>
      {rejectReason && before !== null && (
        <p className="docview__note z-small" role="note">
          {rejectReason}
        </p>
      )}
      {before === null && (
        <p className="docview__note z-small" role="alert">
          {t("diff.mismatch")}
        </p>
      )}
      <div ref={host} className="ddiff__editor" data-testid="diff-editor" hidden={before === null} />
    </div>
  );
}

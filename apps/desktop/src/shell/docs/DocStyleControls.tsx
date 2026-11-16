// Kontrol kustomisasi Panel 3 (dokumen), dikelompokkan per jenis penampil: kode/mentah (Monaco), dokumen hasil render
// (Markdown, Word, PowerPoint, notebook), tabel (CSV, Excel, JSON), dan gambar. Dipakai bersama oleh popover
// "Kustomisasi" di Panel 3 (hanya kelompok yang relevan untuk dokumen aktif) dan Pengaturan → Dokumen (semua kelompok).
// Keduanya menulis ke AppearanceStore (`appearance.docs`), jadi selalu sinkron.
import { BUILTIN_EDITOR_THEMES, DOC_CODE_SIZE, DOC_IMAGE_BGS, DOC_THEMES, type DocStyle } from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import { Button, Select, Slider, Switch } from "@zandor/ui";
import { useAppearance, useServices } from "../services";

export type DocStyleGroup = "code" | "prose" | "table" | "image";
export const DOC_STYLE_GROUPS: DocStyleGroup[] = ["code", "prose", "table", "image"];

/** Kunci yang dimiliki tiap kelompok (untuk "Kembalikan bawaan" per kelompok). */
export const GROUP_KEYS: Record<DocStyleGroup, (keyof DocStyle)[]> = {
  code: ["codeSize", "codeLineHeight", "codeFont", "minimap", "lineNumbers", "ligatures", "whitespace"],
  prose: ["theme", "proseFont", "proseSize", "proseLine", "proseWidth"],
  table: ["tableSize"],
  image: ["imageBg"],
};

/** Kelompok kustomisasi untuk penampil aktif (lihat `viewerFor` di DocPanel). */
export function groupsForViewer(viewer: string): DocStyleGroup[] {
  switch (viewer) {
    case "code":
      return ["code"];
    case "markdown":
    case "docx":
    case "pptx":
      return ["prose"];
    case "notebook":
      return ["prose", "code"];
    case "table":
    case "xlsx":
    case "json":
      return ["table"];
    case "image":
      return ["image"];
    default:
      return [];
  }
}

const CODE_FONTS: Array<{ value: string; label: MessageKey }> = [
  { value: "", label: "docStyle.font.codeDefault" },
  { value: "'JetBrains Mono Variable', Consolas, monospace", label: "docStyle.font.jetbrains" },
  { value: "Consolas, 'Courier New', monospace", label: "docStyle.font.consolas" },
  { value: "'Cascadia Code', Consolas, monospace", label: "docStyle.font.cascadia" },
];
const PROSE_FONTS: Array<{ value: string; label: MessageKey }> = [
  { value: "", label: "chatStyle.font.default" },
  { value: "system-ui, 'Segoe UI', sans-serif", label: "chatStyle.font.system" },
  { value: "Georgia, 'Times New Roman', serif", label: "chatStyle.font.serif" },
  { value: "'JetBrains Mono Variable', Consolas, monospace", label: "chatStyle.font.mono" },
];

type NumSpec = {
  key: keyof DocStyle;
  label: MessageKey;
  min: number;
  max: number;
  step: number;
  def: number;
  unit: string;
};
const NUMS: Record<DocStyleGroup, NumSpec[]> = {
  code: [
    {
      key: "codeSize",
      label: "docStyle.codeSize",
      min: DOC_CODE_SIZE.min,
      max: DOC_CODE_SIZE.max,
      step: 1,
      def: DOC_CODE_SIZE.def,
      unit: "",
    },
    { key: "codeLineHeight", label: "docStyle.codeLineHeight", min: 1, max: 2.4, step: 0.05, def: 1.5, unit: "" },
  ],
  prose: [
    { key: "proseSize", label: "docStyle.proseSize", min: 0.6, max: 2, step: 0.025, def: 0.875, unit: "rem" },
    { key: "proseLine", label: "docStyle.proseLine", min: 1, max: 2.4, step: 0.05, def: 1.55, unit: "" },
    { key: "proseWidth", label: "docStyle.proseWidth", min: 28, max: 120, step: 1, def: 48, unit: "rem" },
  ],
  table: [{ key: "tableSize", label: "docStyle.tableSize", min: 0.6, max: 1.6, step: 0.025, def: 0.8125, unit: "rem" }],
  image: [],
};
const BOOLS: Array<{ key: "minimap" | "lineNumbers" | "ligatures" | "whitespace"; label: MessageKey; def: boolean }> = [
  { key: "lineNumbers", label: "docStyle.lineNumbers", def: true },
  { key: "minimap", label: "docStyle.minimap", def: false },
  { key: "ligatures", label: "docStyle.ligatures", def: false },
  { key: "whitespace", label: "docStyle.whitespace", def: false },
];

const round = (n: number) => Math.round(n * 10000) / 10000;
const numOf = (d: DocStyle, s: NumSpec) => {
  const v = d[s.key];
  if (v === undefined) return s.def;
  return typeof v === "number" ? v : Number.parseFloat(String(v));
};

export function DocStyleControls({
  groups = DOC_STYLE_GROUPS,
  compact = false,
}: {
  groups?: DocStyleGroup[];
  compact?: boolean;
}) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const a = useAppearance();
  const d = a.docs;
  const setNum = (s: NumSpec, n: number) => {
    const r = round(n);
    if (r === s.def) return store.setDoc(s.key, null);
    return store.setDoc(s.key, (s.unit ? `${r}${s.unit}` : r) as never);
  };
  const fontSelect = (key: "codeFont" | "proseFont", list: typeof CODE_FONTS, label: MessageKey) => {
    const cur = d[key] ?? "";
    const known = list.some((f) => f.value === cur);
    return (
      <div className="cstyle__row">
        <span className="z-label">{t(label)}</span>
        <Select
          size="sm"
          aria-label={t(label)}
          items={[
            ...list.map((f) => ({ value: f.value || "default", label: t(f.label) })),
            ...(known ? [] : [{ value: cur, label: cur }]),
          ]}
          value={cur || "default"}
          onValueChange={(v) => store.setDoc(key, v === "default" ? null : v)}
        />
      </div>
    );
  };
  const sliders = (g: DocStyleGroup) =>
    NUMS[g].map((s) => {
      const v = numOf(d, s);
      return (
        <div className="cstyle__slider" key={s.key}>
          <div className="cstyle__head">
            <span className="z-label">{t(s.label)}</span>
            <output className="cstyle__value z-small">
              {round(v)}
              {s.unit || (s.key === "codeSize" ? "px" : "")}
            </output>
          </div>
          <Slider
            label={t(s.label)}
            min={s.min}
            max={s.max}
            step={s.step}
            value={v}
            onValueChange={(n) => setNum(s, n)}
          />
        </div>
      );
    });

  if (groups.length === 0) return <p className="z-small cstyle__empty">{t("docStyle.none")}</p>;

  return (
    <div className="cstyle" data-compact={compact ? "" : undefined}>
      {groups.map((g) => (
        <section key={g} className="cstyle__group" aria-label={t(`docStyle.group.${g}` as MessageKey)}>
          <div className="cstyle__grouphead">
            <h3 className="z-label cstyle__grouptitle">{t(`docStyle.group.${g}` as MessageKey)}</h3>
            <Button
              size="sm"
              variant="ghost"
              disabled={!GROUP_KEYS[g].some((k) => d[k] !== undefined)}
              onClick={() => store.resetDocs(GROUP_KEYS[g])}
            >
              {t("chatStyle.reset")}
            </Button>
          </div>
          {g === "code" && (
            <>
              <div className="cstyle__row">
                <span className="z-label">{t("docStyle.editorTheme")}</span>
                <Select
                  size="sm"
                  aria-label={t("docStyle.editorTheme")}
                  items={[
                    { value: "auto", label: t("ap.editor2.auto") },
                    ...BUILTIN_EDITOR_THEMES.map((id) => ({ value: id, label: t("ap.editor2.builtin", { name: id }) })),
                    ...a.editorThemes.map((th) => ({ value: th.id, label: th.name })),
                  ]}
                  value={a.editorTheme}
                  onValueChange={(v) => store.setEditorTheme(v)}
                />
              </div>
              {fontSelect("codeFont", CODE_FONTS, "docStyle.codeFont")}
              {sliders("code")}
              <p className="z-small cstyle__hint">{t("docStyle.zoomHint")}</p>
              {BOOLS.map((b) => (
                <div className="cstyle__row" key={b.key}>
                  <span className="z-label">{t(b.label)}</span>
                  <Switch
                    label=""
                    aria-label={t(b.label)}
                    checked={d[b.key] ?? b.def}
                    onCheckedChange={(c) => store.setDoc(b.key, c === b.def ? null : c)}
                  />
                </div>
              ))}
            </>
          )}
          {g === "prose" && (
            <>
              <div className="cstyle__row">
                <span className="z-label">{t("docStyle.theme")}</span>
                <Select
                  size="sm"
                  aria-label={t("docStyle.theme")}
                  items={DOC_THEMES.map((th) => ({ value: th, label: t(`docStyle.theme.${th}` as MessageKey) }))}
                  value={d.theme ?? "auto"}
                  onValueChange={(v) => store.setDoc("theme", v === "auto" ? null : (v as DocStyle["theme"]))}
                />
              </div>
              {fontSelect("proseFont", PROSE_FONTS, "docStyle.proseFont")}
              {sliders("prose")}
            </>
          )}
          {g === "table" && sliders("table")}
          {g === "image" && (
            <div className="cstyle__row">
              <span className="z-label">{t("docStyle.imageBg")}</span>
              <Select
                size="sm"
                aria-label={t("docStyle.imageBg")}
                items={DOC_IMAGE_BGS.map((b) => ({ value: b, label: t(`docStyle.imageBg.${b}` as MessageKey) }))}
                value={d.imageBg ?? "auto"}
                onValueChange={(v) => store.setDoc("imageBg", v === "auto" ? null : (v as DocStyle["imageBg"]))}
              />
            </div>
          )}
        </section>
      ))}
    </div>
  );
}

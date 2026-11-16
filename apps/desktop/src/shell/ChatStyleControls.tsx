// Kontrol gaya Panel 2 (chat): lebar isi, rasio composer, font, ukuran, ketebalan, opasitas, tinggi baris, jarak huruf,
// jarak antarpesan, ukuran kode, dan preset kerapatan. Dipakai bersama oleh popover "Kustomisasi" di Panel 2 dan halaman
// Pengaturan (kategori Chat) — keduanya menulis ke AppearanceStore (`appearance.chat`), jadi selalu sinkron.
import type { ChatStyle } from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import { Button, Select, Slider } from "@zandor/ui";
import { FontPicker } from "./FontPicker";
import { useAppearance, useServices, useSettings } from "./services";

type NumKey = Exclude<keyof ChatStyle, "font">;
type Spec = {
  key: NumKey;
  label: MessageKey;
  min: number;
  max: number;
  step: number;
  def: number;
  unit: "" | "rem" | "em";
};

// default = nilai token di tokens.css (dipakai bila pengguna belum mengubahnya)
const SPECS: Spec[] = [
  { key: "width", label: "chatStyle.width", min: 28, max: 120, step: 1, def: 46, unit: "rem" },
  { key: "composerRatio", label: "chatStyle.composerRatio", min: 1, max: 1.6, step: 0.05, def: 1.2, unit: "" },
  { key: "size", label: "chatStyle.size", min: 0.6, max: 1.6, step: 0.025, def: 0.875, unit: "rem" },
  { key: "opacity", label: "chatStyle.opacity", min: 0.3, max: 1, step: 0.05, def: 1, unit: "" },
  { key: "lineHeight", label: "chatStyle.lineHeight", min: 1, max: 2.4, step: 0.05, def: 1.55, unit: "" },
  { key: "tracking", label: "chatStyle.tracking", min: -0.05, max: 0.2, step: 0.005, def: 0, unit: "em" },
  { key: "gap", label: "chatStyle.gap", min: 0, max: 3, step: 0.125, def: 0.75, unit: "rem" },
  { key: "codeSize", label: "chatStyle.codeSize", min: 0.6, max: 1.6, step: 0.025, def: 0.8125, unit: "rem" },
];

const WEIGHTS = [300, 400, 500, 600, 700];
const FONTS: Array<{ value: string; label: MessageKey }> = [
  { value: "", label: "chatStyle.font.default" },
  { value: "system-ui, 'Segoe UI', sans-serif", label: "chatStyle.font.system" },
  { value: "Georgia, 'Times New Roman', serif", label: "chatStyle.font.serif" },
  { value: "'JetBrains Mono Variable', Consolas, monospace", label: "chatStyle.font.mono" },
];
const DENSITY: Record<"compact" | "cozy" | "comfy", Partial<ChatStyle>> = {
  compact: { gap: "0.375rem", lineHeight: 1.35 },
  cozy: {},
  comfy: { gap: "1.5rem", lineHeight: 1.8 },
};

const round = (n: number) => Math.round(n * 10000) / 10000;

function current(c: ChatStyle, s: Spec): number {
  const v = c[s.key];
  if (v === undefined) return s.def;
  return typeof v === "number" ? v : Number.parseFloat(v);
}

export function ChatStyleControls({ compact = false }: { compact?: boolean }) {
  const { t } = useI18n();
  const { appearance: store } = useServices();
  const chat = useAppearance().chat;
  // lebar bawaan mengikuti pengaturan "Lebar isi chat" (sempit/normal/lebar) selama slider belum diubah
  const widthDef = { narrow: 38, normal: 46, wide: 56 }[useSettings().chatWidth] ?? 46;
  const specs = SPECS.map((s) => (s.key === "width" ? { ...s, def: widthDef } : s));
  const set = (s: Spec, n: number) => {
    const r = round(n);
    if (r === s.def) return store.setChat(s.key, null);
    return store.setChat(s.key, (s.unit ? `${r}${s.unit}` : r) as never);
  };
  const applyDensity = (d: keyof typeof DENSITY) => {
    store.setChat("gap", null);
    store.setChat("lineHeight", null);
    for (const [k, v] of Object.entries(DENSITY[d])) store.setChat(k as keyof ChatStyle, v as never);
  };
  const density = (Object.keys(DENSITY) as Array<keyof typeof DENSITY>).find(
    (d) => (DENSITY[d].gap ?? undefined) === chat.gap && (DENSITY[d].lineHeight ?? undefined) === chat.lineHeight,
  );
  return (
    <div className="cstyle" data-compact={compact ? "" : undefined}>
      <div className="cstyle__row">
        <span className="z-label">{t("chatStyle.density")}</span>
        <fieldset className="cstyle__seg" aria-label={t("chatStyle.density")}>
          {(Object.keys(DENSITY) as Array<keyof typeof DENSITY>).map((d) => (
            <Button
              key={d}
              size="sm"
              variant={density === d ? "solid" : "ghost"}
              aria-pressed={density === d}
              onClick={() => applyDensity(d)}
            >
              {t(`chatStyle.density.${d}` as MessageKey)}
            </Button>
          ))}
        </fieldset>
      </div>
      <div className="cstyle__row">
        <span className="z-label">{t("chatStyle.font")}</span>
        <FontPicker
          label={t("chatStyle.font")}
          value={chat.font ?? null}
          presets={FONTS}
          onChange={(v) => store.setChat("font", v)}
        />
      </div>
      <div className="cstyle__row">
        <span className="z-label">{t("chatStyle.weight")}</span>
        <Select
          size="sm"
          aria-label={t("chatStyle.weight")}
          items={WEIGHTS.map((w) => ({ value: String(w), label: t(`chatStyle.weight.${w}` as MessageKey) }))}
          value={String(chat.weight ?? 400)}
          onValueChange={(v) => store.setChat("weight", Number(v) === 400 ? null : Number(v))}
        />
      </div>
      {specs.map((s) => {
        const v = current(chat, s);
        return (
          <div className="cstyle__slider" key={s.key}>
            <div className="cstyle__head">
              <span className="z-label">{t(s.label)}</span>
              <output className="cstyle__value z-small">{s.unit ? `${round(v)}${s.unit}` : round(v)}</output>
            </div>
            <Slider
              label={t(s.label)}
              min={s.min}
              max={s.max}
              step={s.step}
              value={v}
              onValueChange={(n) => set(s, n)}
            />
          </div>
        );
      })}
      <div className="cstyle__foot">
        <Button
          size="sm"
          variant="ghost"
          disabled={Object.keys(chat).length === 0}
          onClick={() => {
            for (const k of Object.keys(chat)) store.setChat(k as keyof ChatStyle, null);
          }}
        >
          {t("chatStyle.reset")}
        </Button>
      </div>
    </div>
  );
}

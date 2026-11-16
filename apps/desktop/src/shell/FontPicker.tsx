// Pemilih jenis huruf: pemicu bergaya Select → popover dengan kolom pencarian di atas (langsung terfokus) dan daftar
// font: pilihan bawaan + font yang terinstal di komputer. Desktop: perintah Rust `system_fonts`; browser: Local Font
// Access API bila tersedia; selain itu hanya pilihan bawaan dengan penjelasan jujur.

import { validateFontFamily } from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import { CheckIcon, ChevronDownIcon, Popover, SearchField, Spinner } from "@zandor/ui";
import { useEffect, useMemo, useRef, useState } from "react";

export type FontPreset = { value: string; label: MessageKey };

/** Font yang dibundel aplikasi (@fontsource-variable, OFL) — tersedia tanpa instalasi di komputer pengguna. */
export const BUNDLED_FONTS: Array<{ name: string; value: string }> = [
  { name: "Inter", value: "'Inter Variable', sans-serif" },
  { name: "Geist", value: "'Geist Variable', sans-serif" },
  { name: "IBM Plex Sans", value: "'IBM Plex Sans Variable', sans-serif" },
  { name: "Plus Jakarta Sans", value: "'Plus Jakarta Sans Variable', sans-serif" },
  { name: "JetBrains Mono", value: "'JetBrains Mono Variable', monospace" },
  { name: "Fira Code", value: "'Fira Code Variable', monospace" },
  { name: "Geist Mono", value: "'Geist Mono Variable', monospace" },
  { name: "Source Code Pro", value: "'Source Code Pro Variable', monospace" },
  { name: "Roboto Mono", value: "'Roboto Mono Variable', monospace" },
];

type LoadState = { status: "idle" | "loading" | "ready" | "unavailable" | "error"; fonts: string[]; error?: string };

let cached: LoadState | null = null;
let pending: Promise<LoadState> | null = null;

async function loadSystemFonts(): Promise<LoadState> {
  try {
    const c = await import("@tauri-apps/api/core");
    if (c.isTauri()) return { status: "ready", fonts: await c.invoke<string[]>("system_fonts", { refresh: false }) };
  } catch (e) {
    return { status: "error", fonts: [], error: String(e) };
  }
  const q = (window as unknown as { queryLocalFonts?: () => Promise<Array<{ family: string }>> }).queryLocalFonts;
  if (!q) return { status: "unavailable", fonts: [] };
  try {
    const fams = [...new Set((await q()).map((f) => f.family))].sort((a, b) => a.localeCompare(b));
    // izin ditolak/diabaikan → daftar kosong: anggap tidak tersedia, bukan "tidak ada hasil"
    return fams.length ? { status: "ready", fonts: fams } : { status: "unavailable", fonts: [] };
  } catch (e) {
    return { status: "error", fonts: [], error: String(e) };
  }
}

/** Nilai CSS untuk satu keluarga font terinstal (dikutip, dengan cadangan). */
export function familyValue(name: string): string {
  return `"${name}", sans-serif`;
}

function useSystemFonts(active: boolean): LoadState {
  const [state, setState] = useState<LoadState>(cached ?? { status: "idle", fonts: [] });
  useEffect(() => {
    if (!active || cached) return;
    setState({ status: "loading", fonts: [] });
    pending ??= loadSystemFonts();
    let alive = true;
    void pending.then((r) => {
      if (r.status !== "error") cached = r;
      pending = null;
      if (alive) setState(r);
    });
    return () => {
      alive = false;
    };
  }, [active]);
  return state;
}

export function FontPicker({
  value,
  presets,
  onChange,
  label,
}: {
  /** Nilai CSS font-family saat ini; null = bawaan. */
  value: string | null;
  presets: FontPreset[];
  onChange: (value: string | null) => void;
  label: string;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const sys = useSystemFonts(open);
  const q = query.trim().toLowerCase();

  const presetItems = presets
    .map((p) => ({ value: p.value || null, label: t(p.label) }))
    .filter((p) => !q || p.label.toLowerCase().includes(q));
  const bundledItems = BUNDLED_FONTS.filter((f) => !q || f.name.toLowerCase().includes(q)).map((f) => ({
    value: f.value,
    label: f.name,
  }));
  // font yang namanya tidak lolos validasi (mis. berisi "&") tidak bisa disimpan — tidak ditampilkan
  const systemItems = useMemo(
    () =>
      sys.fonts
        .filter((f) => validateFontFamily(familyValue(f)) === null)
        .filter((f) => !q || f.toLowerCase().includes(q))
        .map((f) => ({ value: familyValue(f), label: f })),
    [sys.fonts, q],
  );

  const currentLabel =
    presets.find((p) => (p.value || null) === value)?.label !== undefined
      ? t(presets.find((p) => (p.value || null) === value)?.label as MessageKey)
      : (BUNDLED_FONTS.find((f) => f.value === value)?.name ?? value?.match(/^"([^"]+)"/)?.[1] ?? value ?? "");

  const pick = (v: string | null) => {
    onChange(v);
    setOpen(false);
    setQuery("");
  };
  const row = (item: { value: string | null; label: string }, preview: boolean) => (
    <li key={item.value ?? "default"}>
      <button
        type="button"
        className="fontpick__item"
        aria-pressed={item.value === value}
        style={preview && item.value ? { fontFamily: item.value } : undefined}
        title={item.label}
        onClick={() => pick(item.value)}
      >
        <span className="fontpick__name">{item.label}</span>
        {item.value === value && <CheckIcon size="sm" />}
      </button>
    </li>
  );

  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (!o) setQuery("");
      }}
      side="bottom"
      align="end"
      title={label}
      hideTitle
      initialFocus={inputRef}
      className="fontpick"
      trigger={
        <button type="button" className="z-select fontpick__trigger" aria-label={label}>
          <span className="fontpick__current" style={value ? { fontFamily: value } : undefined}>
            {currentLabel}
          </span>
          <ChevronDownIcon size="sm" />
        </button>
      }
    >
      <SearchField
        value={query}
        onValueChange={setQuery}
        inputRef={inputRef}
        placeholder={t("fontPicker.search")}
        aria-label={t("fontPicker.search")}
      />
      <div className="fontpick__list">
        {presetItems.length > 0 && <ul className="fontpick__group">{presetItems.map((i) => row(i, true))}</ul>}
        {bundledItems.length > 0 && (
          <>
            <p className="z-label fontpick__heading">{t("fontPicker.bundled")}</p>
            <ul className="fontpick__group">{bundledItems.map((i) => row(i, true))}</ul>
          </>
        )}
        <p className="z-label fontpick__heading">{t("fontPicker.installed")}</p>
        {sys.status === "loading" || sys.status === "idle" ? (
          <p className="z-small fontpick__note">
            <Spinner size="sm" /> {t("fontPicker.loading")}
          </p>
        ) : sys.status === "unavailable" ? (
          <p className="z-small fontpick__note">{t("fontPicker.unavailable")}</p>
        ) : sys.status === "error" ? (
          <p className="z-small fontpick__note" role="alert">
            {t("fontPicker.error", { error: sys.error ?? "" })}
          </p>
        ) : systemItems.length === 0 ? (
          <p className="z-small fontpick__note">{t("ui.noResults")}</p>
        ) : (
          <ul className="fontpick__group">{systemItems.map((i) => row(i, true))}</ul>
        )}
      </div>
    </Popover>
  );
}

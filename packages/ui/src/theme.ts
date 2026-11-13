// Penerapan tema (dark/light/system) dan skala UI (--ui-scale) ke elemen root.
// Penyimpanan ada di SettingsStore (@zandor/core); modul ini hanya menerapkan ke DOM.

export const THEMES = ["dark", "light", "system"] as const;
export type Theme = (typeof THEMES)[number];

/** Langkah skala yang didukung (ADR-025): 75-150%. Harus sama dengan UI_SCALE_VALUES di @zandor/core. */
export const UI_SCALES = [0.75, 0.875, 1, 1.125, 1.25, 1.375, 1.5] as const;
export type UiScale = (typeof UI_SCALES)[number];

/** "system" menghapus atribut sehingga CSS mengikuti prefers-color-scheme. */
export function applyTheme(theme: Theme, root: HTMLElement = document.documentElement) {
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

export function applyScale(scale: UiScale, root: HTMLElement = document.documentElement) {
  root.style.setProperty("--ui-scale", String(scale));
}

/** Skala berikutnya/sebelumnya dalam daftar; berhenti di ujung. */
export function stepScale(current: number, direction: 1 | -1): UiScale {
  const i = (UI_SCALES as readonly number[]).indexOf(current);
  const from = i === -1 ? UI_SCALES.indexOf(1) : i;
  return UI_SCALES[Math.min(UI_SCALES.length - 1, Math.max(0, from + direction))] as UiScale;
}

/**
 * Terapkan variabel CSS kustom (tema/tipografi/chat) sebagai gaya inline di root, sehingga mengalahkan token bawaan
 * untuk kedua mode. Variabel dari panggilan sebelumnya yang tidak ada lagi dihapus. Mengembalikan daftar nama terpasang.
 */
export function applyVars(
  vars: Record<string, string>,
  previous: readonly string[] = [],
  root: HTMLElement = document.documentElement,
): string[] {
  for (const name of previous) if (!(name in vars)) root.style.removeProperty(name);
  for (const [name, value] of Object.entries(vars)) {
    if (name.startsWith("--z-")) root.style.setProperty(name, value);
  }
  return Object.keys(vars).filter((n) => n.startsWith("--z-"));
}

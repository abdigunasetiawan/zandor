// Tema (dark/light/system) dan skala UI (--ui-scale). Logika murni + penyimpanan aman.

export const THEMES = ["dark", "light", "system"] as const;
export type Theme = (typeof THEMES)[number];

/** Langkah skala yang didukung (ADR-025): 75–150%. */
export const UI_SCALES = [0.75, 0.875, 1, 1.125, 1.25, 1.375, 1.5] as const;
export type UiScale = (typeof UI_SCALES)[number];

const THEME_KEY = "zandor.theme";
const SCALE_KEY = "zandor.uiScale";

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // penyimpanan tidak tersedia: preferensi hanya berlaku untuk sesi ini
  }
}

export function storedTheme(): Theme {
  const v = read(THEME_KEY);
  return (THEMES as readonly string[]).includes(v ?? "") ? (v as Theme) : "system";
}

export function storedScale(): UiScale {
  const n = Number(read(SCALE_KEY));
  return (UI_SCALES as readonly number[]).includes(n) ? (n as UiScale) : 1;
}

/** "system" menghapus atribut sehingga CSS mengikuti prefers-color-scheme. */
export function applyTheme(theme: Theme, root: HTMLElement = document.documentElement) {
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
  write(THEME_KEY, theme);
}

export function applyScale(scale: UiScale, root: HTMLElement = document.documentElement) {
  root.style.setProperty("--ui-scale", String(scale));
  write(SCALE_KEY, String(scale));
}

/** Skala berikutnya/sebelumnya dalam daftar; berhenti di ujung. */
export function stepScale(current: number, direction: 1 | -1): UiScale {
  const i = (UI_SCALES as readonly number[]).indexOf(current);
  const from = i === -1 ? UI_SCALES.indexOf(1) : i;
  return UI_SCALES[Math.min(UI_SCALES.length - 1, Math.max(0, from + direction))] as UiScale;
}

export function initPreferences(root: HTMLElement = document.documentElement) {
  applyTheme(storedTheme(), root);
  applyScale(storedScale(), root);
}

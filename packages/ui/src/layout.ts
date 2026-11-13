// Offset popup dalam rem → piksel (API posisi Base UI memakai px). Mengikuti --ui-scale lewat font-size root.
const FALLBACK_ROOT_PX = 16;

export function rootFontPx(): number {
  if (typeof document === "undefined") return FALLBACK_ROOT_PX;
  return Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || FALLBACK_ROOT_PX;
}

/** Fungsi offset untuk `sideOffset` Base UI: dihitung saat popup dibuka. */
export function offsetRem(rem: number): () => number {
  return () => rem * rootFontPx();
}

export const OFFSET_TIGHT = offsetRem(0.25);
export const OFFSET_NORMAL = offsetRem(0.375);

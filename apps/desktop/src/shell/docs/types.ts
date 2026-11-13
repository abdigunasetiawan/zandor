import type { DocTab } from "@zandor/core";

/** Aksi yang bisa dipanggil toolbar pada viewer aktif. Viewer mengisinya lewat `onHandle`. */
export type ViewerHandle = {
  /** Buka pencarian di dalam isi (hanya viewer teks). */
  find?: () => void;
  /** Teks saat ini (untuk "salin isi"). */
  text?: () => string | null;
};

export type ViewerProps = {
  tab: DocTab;
  bytes: Uint8Array;
  onHandle: (h: ViewerHandle | null) => void;
};

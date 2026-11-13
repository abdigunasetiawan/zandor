// Perencana tata letak panel (ADR-025 temuan 2). Hasilnya FUNGSI MURNI dari (lebar, panel, pilihan pengguna):
// tidak bergantung pada urutan reaksi pustaka UI, sehingga deterministik.
//
// Model:
// - `userCollapsed`: panel yang dilipat pengguna (tombol, perintah, atau menyeret). Tidak pernah dibuka otomatis.
// - Panel lain terbuka kecuali tidak muat; yang tidak muat dilipat OTOMATIS menurut `priority`
//   (angka kecil lebih dulu), hanya yang `collapsible`, tidak pernah `protectedId` (panel yang baru dibuka
//   pengguna), dan tidak pernah panel non-collapsible (chat).
// - Karena dihitung ulang dari nol setiap kali, panel yang tadinya dilipat otomatis terbuka kembali sendiri
//   begitu ruang cukup.

export type PanelSpec = {
  id: string;
  /** Lebar minimum saat terbuka, dalam piksel (rem × ukuran root saat ini). */
  minPx: number;
  collapsible: boolean;
  /** Angka kecil = dilipat lebih dulu. */
  priority: number;
};

export type PlanInput = {
  availablePx: number;
  separatorPx: number;
  panels: PanelSpec[];
  userCollapsed: ReadonlySet<string>;
  /** Panel yang tidak boleh dilipat otomatis (biasanya yang terakhir dibuka pengguna). */
  protectedId?: string | null;
};

function requiredPx(open: PanelSpec[], separatorPx: number): number {
  if (open.length === 0) return 0;
  return open.reduce((sum, p) => sum + p.minPx, 0) + separatorPx * (open.length - 1);
}

/** Id panel yang harus dilipat otomatis, berurutan menurut urutan pelipatan. */
export function computeAutoCollapse(input: PlanInput): string[] {
  const { availablePx, separatorPx, panels, userCollapsed, protectedId } = input;
  let open = panels.filter((p) => !userCollapsed.has(p.id));
  const candidates = open
    .filter((p) => p.collapsible && p.id !== protectedId)
    .sort((a, b) => a.priority - b.priority || a.id.localeCompare(b.id));
  const auto: string[] = [];
  for (const c of candidates) {
    if (requiredPx(open, separatorPx) <= availablePx) break;
    open = open.filter((p) => p.id !== c.id);
    auto.push(c.id);
  }
  return auto;
}

/** Himpunan akhir panel yang harus terlipat = pilihan pengguna ∪ pelipatan otomatis. */
export function desiredCollapsed(input: PlanInput): Set<string> {
  return new Set([...input.userCollapsed, ...computeAutoCollapse(input)]);
}

/** Konversi rem → piksel menurut ukuran font root. */
export const remToPx = (rem: number, rootPx: number) => rem * rootPx;

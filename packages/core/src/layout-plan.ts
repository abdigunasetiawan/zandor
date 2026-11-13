// Perencana tata letak panel (ADR-025 temuan 2): hitung panel mana yang harus dilipat/dibuka kembali
// SEBELUM menerapkan ukuran, supaya urutannya deterministik (tidak diserahkan ke reaksi pustaka UI).
//
// Aturan:
// - Panel terbuka butuh lebar minimum; separator antar panel terbuka juga memakai ruang.
// - Bila total melebihi ruang, panel dilipat berurutan menurut `priority` (angka kecil dilipat lebih dulu),
//   hanya yang `collapsible`, dan tidak pernah `protectedId` (panel yang baru saja dibuka pengguna).
// - Panel yang dilipat OLEH PERENCANA (`autoCollapsed`) dibuka lagi otomatis begitu muat, mulai dari
//   prioritas tertinggi. Panel yang dilipat pengguna tidak pernah dibuka otomatis.

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
  /** Panel yang sedang terlipat (oleh siapa pun). */
  collapsed: ReadonlySet<string>;
  /** Subset `collapsed` yang dilipat oleh perencana, kandidat dibuka kembali. */
  autoCollapsed?: ReadonlySet<string>;
  /** Panel yang tidak boleh dilipat otomatis pada putaran ini. */
  protectedId?: string;
};

export type PlanResult = { collapse: string[]; restore: string[] };

function requiredPx(open: PanelSpec[], separatorPx: number): number {
  if (open.length === 0) return 0;
  return open.reduce((sum, p) => sum + p.minPx, 0) + separatorPx * (open.length - 1);
}

export function planLayout(input: PlanInput): PlanResult {
  const { availablePx, separatorPx, panels, collapsed, protectedId } = input;
  const autoCollapsed = input.autoCollapsed ?? new Set<string>();
  const open = panels.filter((p) => !collapsed.has(p.id));
  const collapse: string[] = [];

  // 1) Lipat sampai muat.
  const candidates = open
    .filter((p) => p.collapsible && p.id !== protectedId)
    .sort((a, b) => a.priority - b.priority || a.id.localeCompare(b.id));
  let current = open;
  for (const c of candidates) {
    if (requiredPx(current, separatorPx) <= availablePx) break;
    current = current.filter((p) => p.id !== c.id);
    collapse.push(c.id);
  }

  // 2) Bila tidak ada yang perlu dilipat, buka kembali yang tadinya dilipat otomatis, prioritas tertinggi dulu.
  const restore: string[] = [];
  if (collapse.length === 0) {
    const restorable = panels
      .filter((p) => autoCollapsed.has(p.id) && collapsed.has(p.id))
      .sort((a, b) => b.priority - a.priority || a.id.localeCompare(b.id));
    for (const r of restorable) {
      const next = [...current, r];
      if (requiredPx(next, separatorPx) > availablePx) continue;
      current = next;
      restore.push(r.id);
    }
  }
  return { collapse, restore };
}

/** Konversi rem → piksel menurut ukuran font root. */
export const remToPx = (rem: number, rootPx: number) => rem * rootPx;

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

// ---------------------------------------------------------------------------------------------------------------
// Aturan visibilitas + smart resize (ADR-066). Panel bernomor 1 Sidebar, 2 Chat, 3 Dokumen, 4 Tools, 5 Rel.
// - Panel "utama" (2/3/4) adalah satu-satunya yang boleh tampil sendirian (penuh). Minimal satu panel utama
//   harus selalu terlihat, sehingga kombinasi "hanya 1", "hanya 5", atau "hanya 1 dan 5" tidak pernah terjadi.
// - Smart resize membagi lebar panel yang sedang terlihat:
//   · Sidebar (1) mendapat lebar "secukupnya" (target rem, dibatasi rasio maksimum), sisanya ke panel utama.
//   · 1 panel utama → penuh; 2 panel utama → 50/50; 3 panel utama → panel utama pertama (Chat) lebih besar,
//     dua sisanya sama besar.
//   · Minimum lebar tiap panel dihormati (lebar diambil dari panel lain yang masih di atas minimumnya).

/** Panel utama bawaan (urutan = prioritas; yang pertama mendapat porsi lebih besar saat ketiganya terlihat). */
export const MAIN_PANEL_IDS = ["chat", "docs", "tools"] as const;

/** Boleh menyembunyikan `id`? Tidak boleh bila itu panel utama terakhir yang masih terlihat. */
export function canHidePanel(
  id: string,
  visible: ReadonlySet<string>,
  mainIds: readonly string[] = MAIN_PANEL_IDS,
): boolean {
  if (!mainIds.includes(id)) return true;
  return mainIds.some((m) => m !== id && visible.has(m));
}

export type SmartLayoutInput = {
  /** Lebar grup panel (px), termasuk separator. */
  availablePx: number;
  separatorPx: number;
  /** Id panel yang sedang terlihat. */
  visible: ReadonlySet<string>;
  /** Panel samping (sidebar) yang diberi lebar secukupnya. */
  side: { id: string; targetPx: number; minPx: number; maxRatio: number };
  /** Panel utama berurutan; yang pertama diperbesar saat ≥3 panel utama terlihat. */
  main: Array<{ id: string; minPx: number }>;
  /** Perbandingan panel utama pertama terhadap yang lain saat ≥3 terlihat (bawaan 4:3 → 40/30/30). */
  primaryRatio?: number;
};

/**
 * Tata letak hasil smart resize dalam persen (0..100) untuk setiap panel di `side` + `main`;
 * panel tersembunyi bernilai 0. `null` bila tidak ada panel utama terlihat (kombinasi tidak sah).
 */
export function smartLayout(input: SmartLayoutInput): Record<string, number> | null {
  const { availablePx, separatorPx, visible, side, main } = input;
  const primaryRatio = input.primaryRatio ?? 4 / 3;
  const mains = main.filter((m) => visible.has(m.id));
  if (mains.length === 0) return null;
  const sideOn = visible.has(side.id);
  const count = mains.length + (sideOn ? 1 : 0);
  const content = Math.max(1, availablePx - separatorPx * (count - 1));

  const px: Record<string, number> = {};
  let rest = content;
  if (sideOn) {
    const cap = Math.max(side.minPx, content * side.maxRatio);
    const w = Math.min(Math.max(side.targetPx, side.minPx), cap);
    px[side.id] = w;
    rest -= w;
  }

  // Bobot: 1 atau 2 panel utama → sama rata; ≥3 → panel utama pertama lebih besar.
  const weights = mains.map((_, i) => (mains.length >= 3 && i === 0 ? primaryRatio : 1));
  const total = weights.reduce((a, b) => a + b, 0);
  mains.forEach((m, i) => {
    px[m.id] = (rest * (weights[i] ?? 1)) / total;
  });

  // Hormati minimum: tutup kekurangan dengan mengambil dari panel utama yang masih punya kelebihan.
  for (const m of mains) {
    const deficit = m.minPx - (px[m.id] ?? 0);
    if (deficit <= 0) continue;
    const donors = mains.filter((d) => d.id !== m.id && (px[d.id] ?? 0) > d.minPx);
    const spare = donors.reduce((s, d) => s + ((px[d.id] ?? 0) - d.minPx), 0);
    if (spare <= 0) continue;
    const take = Math.min(deficit, spare);
    for (const d of donors) {
      const share = ((px[d.id] ?? 0) - d.minPx) / spare;
      px[d.id] = (px[d.id] ?? 0) - take * share;
    }
    px[m.id] = (px[m.id] ?? 0) + take;
  }

  const out: Record<string, number> = { [side.id]: 0 };
  for (const m of main) out[m.id] = 0;
  const ids = Object.keys(px);
  let sum = 0;
  ids.forEach((id, i) => {
    const pct =
      i === ids.length - 1
        ? Math.round((100 - sum) * 1000) / 1000
        : Math.round(((px[id] ?? 0) / content) * 100 * 1000) / 1000;
    out[id] = pct;
    sum += pct;
  });
  return out;
}

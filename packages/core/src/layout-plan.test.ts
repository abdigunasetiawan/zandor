import { describe, expect, test } from "vitest";
import { type PanelSpec, planLayout, remToPx } from "./layout-plan";

const rem = 16;
// Sama dengan angka spike S8: min P1 12, P2 22, P3 16, P4 18 rem; separator 6px.
const panels: PanelSpec[] = [
  { id: "p1", minPx: remToPx(12, rem), collapsible: true, priority: 2 },
  { id: "p2", minPx: remToPx(22, rem), collapsible: false, priority: 99 },
  { id: "p3", minPx: remToPx(16, rem), collapsible: true, priority: 1 },
  { id: "p4", minPx: remToPx(18, rem), collapsible: true, priority: 3 },
];
const SEP = 6;
const ALL = 12 + 22 + 16 + 18; // rem
const base = { separatorPx: SEP, panels };

describe("planLayout: melipat", () => {
  test("semua muat → tidak ada perubahan", () => {
    const need = remToPx(ALL, rem) + SEP * 3;
    expect(planLayout({ ...base, availablePx: need, collapsed: new Set() })).toEqual({ collapse: [], restore: [] });
  });

  test("kurang sedikit → hanya P3 (prioritas terendah) yang dilipat", () => {
    const need = remToPx(ALL, rem) + SEP * 3;
    expect(planLayout({ ...base, availablePx: need - 1, collapsed: new Set() }).collapse).toEqual(["p3"]);
  });

  test("urutan lipat P3 → P1 → P4 ketat menurut prioritas; chat (P2) tidak pernah", () => {
    // Muat P2+P4 saja → P3 lalu P1 dilipat (P1 lebih dulu dari P4 walau P1+P2 juga akan muat).
    expect(planLayout({ ...base, availablePx: remToPx(22 + 18, rem) + SEP, collapsed: new Set() }).collapse).toEqual([
      "p3",
      "p1",
    ]);
    expect(planLayout({ ...base, availablePx: 100, collapsed: new Set() }).collapse).toEqual(["p3", "p1", "p4"]);
  });

  test("panel yang dilindungi (baru dibuka pengguna) dilewati; yang berikutnya dilipat", () => {
    const need = remToPx(ALL, rem) + SEP * 3;
    expect(planLayout({ ...base, availablePx: need - 1, collapsed: new Set(), protectedId: "p3" }).collapse).toEqual([
      "p1",
    ]);
  });

  test("panel yang sudah terlipat tidak dihitung dan tidak dilipat ulang", () => {
    const avail = remToPx(12 + 22 + 18, rem) + SEP * 2;
    expect(planLayout({ ...base, availablePx: avail, collapsed: new Set(["p3"]) })).toEqual({
      collapse: [],
      restore: [],
    });
  });

  test("tidak ada yang bisa dilipat lagi → tetap tidak melempar, hasilkan semua yang collapsible", () => {
    const r = planLayout({ ...base, availablePx: 1, collapsed: new Set() });
    expect(r.collapse.sort()).toEqual(["p1", "p3", "p4"]);
  });

  test("hasil deterministik untuk input sama", () => {
    const a = planLayout({ ...base, availablePx: 700, collapsed: new Set() });
    const b = planLayout({ ...base, availablePx: 700, collapsed: new Set() });
    expect(a).toEqual(b);
  });
});

describe("planLayout: membuka kembali", () => {
  test("yang dilipat otomatis dibuka lagi saat ruang cukup; prioritas tertinggi dulu", () => {
    const avail = remToPx(22 + 12 + 18, rem) + SEP * 2; // muat P1+P2+P4; P3 dilipat pengguna
    const r = planLayout({
      ...base,
      availablePx: avail,
      collapsed: new Set(["p1", "p3", "p4"]),
      autoCollapsed: new Set(["p1", "p4"]),
    });
    expect(r.collapse).toEqual([]);
    expect(r.restore).toEqual(["p4", "p1"]); // P4 (prioritas 3) dulu, lalu P1
  });

  test("hanya membuka sebanyak yang muat", () => {
    const avail = remToPx(22 + 18, rem) + SEP; // hanya cukup P2 + P4
    const r = planLayout({
      ...base,
      availablePx: avail,
      collapsed: new Set(["p1", "p3", "p4"]),
      autoCollapsed: new Set(["p1", "p4"]),
    });
    expect(r.restore).toEqual(["p4"]);
  });

  test("panel yang dilipat pengguna tidak dibuka otomatis", () => {
    const r = planLayout({
      ...base,
      availablePx: 5000,
      collapsed: new Set(["p3"]),
      autoCollapsed: new Set(),
    });
    expect(r).toEqual({ collapse: [], restore: [] });
  });

  test("tidak membuka kembali pada putaran yang sama dengan pelipatan", () => {
    const r = planLayout({
      ...base,
      availablePx: remToPx(22, rem),
      collapsed: new Set(["p4"]),
      autoCollapsed: new Set(["p4"]),
    });
    expect(r.collapse.length).toBeGreaterThan(0);
    expect(r.restore).toEqual([]);
  });
});

describe("zoom", () => {
  test("skala 150% (root 24px): minimum membesar sehingga lebih banyak panel terlipat", () => {
    const at = (root: number) =>
      planLayout({
        separatorPx: SEP,
        availablePx: 1366 - 48, // lebar jendela dikurangi rail
        collapsed: new Set(),
        panels: panels.map((p) => ({ ...p, minPx: (p.minPx / rem) * root })),
      }).collapse;
    expect(at(16)).toEqual([]);
    expect(at(20).length).toBeGreaterThan(0);
    expect(at(24).length).toBeGreaterThanOrEqual(at(20).length);
  });
});

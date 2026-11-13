import { describe, expect, test } from "vitest";
import { computeAutoCollapse, desiredCollapsed, type PanelSpec, remToPx } from "./layout-plan";

const rem = 16;
// Angka yang sama dengan spike S8: min P1 12, P2 22, P3 16, P4 18 rem; separator 6px.
const panels: PanelSpec[] = [
  { id: "p1", minPx: remToPx(12, rem), collapsible: true, priority: 2 },
  { id: "p2", minPx: remToPx(22, rem), collapsible: false, priority: 99 },
  { id: "p3", minPx: remToPx(16, rem), collapsible: true, priority: 1 },
  { id: "p4", minPx: remToPx(18, rem), collapsible: true, priority: 3 },
];
const SEP = 6;
const base = { separatorPx: SEP, panels, userCollapsed: new Set<string>() };
const need = (rems: number, seps: number) => remToPx(rems, rem) + SEP * seps;
const ALL = need(12 + 22 + 16 + 18, 3);

describe("computeAutoCollapse", () => {
  test("semua muat → tidak ada yang dilipat", () => {
    expect(computeAutoCollapse({ ...base, availablePx: ALL })).toEqual([]);
  });

  test("kurang sedikit → hanya P3 (prioritas terendah)", () => {
    expect(computeAutoCollapse({ ...base, availablePx: ALL - 1 })).toEqual(["p3"]);
  });

  test("urutan ketat P3 → P1 → P4; chat (P2) tidak pernah", () => {
    expect(computeAutoCollapse({ ...base, availablePx: need(22 + 18, 1) })).toEqual(["p3", "p1"]);
    expect(computeAutoCollapse({ ...base, availablePx: 100 })).toEqual(["p3", "p1", "p4"]);
    expect(computeAutoCollapse({ ...base, availablePx: 1 })).not.toContain("p2");
  });

  test("panel yang dilindungi dilewati; yang berikutnya dilipat", () => {
    expect(computeAutoCollapse({ ...base, availablePx: ALL - 1, protectedId: "p3" })).toEqual(["p1"]);
  });

  test("panel yang dilipat pengguna tidak dihitung dan tidak dilipat ulang", () => {
    expect(
      computeAutoCollapse({ ...base, userCollapsed: new Set(["p3"]), availablePx: need(12 + 22 + 18, 2) }),
    ).toEqual([]);
  });

  test("hasil deterministik dan tidak bergantung pada urutan array panel", () => {
    const a = computeAutoCollapse({ ...base, availablePx: 700 });
    const b = computeAutoCollapse({ ...base, panels: [...panels].reverse(), availablePx: 700 });
    expect(b).toEqual(a);
  });
});

describe("desiredCollapsed (dihitung dari nol → otomatis terbuka kembali)", () => {
  test("ruang menyempit lalu melebar: panel kembali terbuka tanpa campur tangan pengguna", () => {
    const narrow = desiredCollapsed({ ...base, availablePx: need(22 + 18, 1) });
    expect([...narrow].sort()).toEqual(["p1", "p3"]);
    const wide = desiredCollapsed({ ...base, availablePx: ALL });
    expect(wide.size).toBe(0);
  });

  test("pilihan pengguna dipertahankan walau ruang melimpah", () => {
    expect([...desiredCollapsed({ ...base, userCollapsed: new Set(["p3"]), availablePx: 5000 })]).toEqual(["p3"]);
  });

  test("gabungan pilihan pengguna dan pelipatan otomatis", () => {
    const r = desiredCollapsed({ ...base, userCollapsed: new Set(["p4"]), availablePx: need(22 + 12, 1) });
    expect([...r].sort()).toEqual(["p3", "p4"]);
  });

  test("hanya P3 dilipat bila P1+P2+P4 muat", () => {
    const a = desiredCollapsed({ ...base, availablePx: need(22 + 12 + 18, 2) });
    expect([...a]).toEqual(["p3"]);
  });
});

describe("zoom", () => {
  test("root font membesar → minimum membesar → lebih banyak panel terlipat", () => {
    const at = (root: number) =>
      computeAutoCollapse({
        separatorPx: SEP,
        availablePx: 1366 - 48, // lebar jendela dikurangi rail
        userCollapsed: new Set(),
        panels: panels.map((p) => ({ ...p, minPx: (p.minPx / rem) * root })),
      });
    expect(at(16)).toEqual([]);
    expect(at(20).length).toBeGreaterThan(0);
    expect(at(24).length).toBeGreaterThanOrEqual(at(20).length);
    expect(at(24)[0]).toBe("p3"); // P3 selalu yang pertama
  });
});

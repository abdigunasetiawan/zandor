import { describe, expect, test } from "vitest";
import {
  canHidePanel,
  computeAutoCollapse,
  desiredCollapsed,
  type PanelSpec,
  remToPx,
  smartLayout,
} from "./layout-plan";

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

describe("canHidePanel (minimal satu panel utama terlihat)", () => {
  test("panel utama terakhir tidak boleh disembunyikan", () => {
    expect(canHidePanel("chat", new Set(["sidebar", "chat", "rail"]))).toBe(false);
    expect(canHidePanel("tools", new Set(["tools"]))).toBe(false);
  });
  test("panel utama boleh disembunyikan bila masih ada panel utama lain", () => {
    expect(canHidePanel("chat", new Set(["chat", "docs"]))).toBe(true);
  });
  test("sidebar dan rel selalu boleh disembunyikan", () => {
    expect(canHidePanel("sidebar", new Set(["sidebar", "chat"]))).toBe(true);
    expect(canHidePanel("rail", new Set(["rail", "tools"]))).toBe(true);
  });
});

describe("smartLayout", () => {
  const side = { id: "sidebar", targetPx: remToPx(16, rem), minPx: remToPx(12, rem), maxRatio: 0.22 };
  const main = [
    { id: "chat", minPx: remToPx(22, rem) },
    { id: "docs", minPx: remToPx(16, rem) },
    { id: "tools", minPx: remToPx(18, rem) },
  ];
  const run = (visible: string[], availablePx = 2000) =>
    smartLayout({ availablePx, separatorPx: 0, visible: new Set(visible), side, main });

  test("tanpa panel utama → null (kombinasi tidak sah)", () => {
    expect(run(["sidebar"])).toBeNull();
    expect(run([])).toBeNull();
  });

  test("satu panel utama saja → penuh", () => {
    expect(run(["docs"])).toEqual({ sidebar: 0, chat: 0, docs: 100, tools: 0 });
  });

  test("sidebar + satu panel utama → sidebar secukupnya, sisanya ke panel utama", () => {
    const l = run(["sidebar", "tools"]);
    expect(l?.sidebar).toBeCloseTo((256 / 2000) * 100, 2);
    expect(l?.tools).toBeCloseTo(100 - (256 / 2000) * 100, 2);
    expect(l?.chat).toBe(0);
  });

  test("sidebar dibatasi rasio maksimum di jendela sempit, tapi tidak di bawah minimumnya", () => {
    expect(run(["sidebar", "chat"], 1000)?.sidebar).toBeCloseTo(22, 2); // 220px < target 256px
    expect(run(["sidebar", "chat"], 600)?.sidebar).toBeCloseTo(32, 2); // min 192px > 22%
  });

  test("dua panel utama → 50/50", () => {
    expect(run(["chat", "docs"])).toEqual({ sidebar: 0, chat: 50, docs: 50, tools: 0 });
    expect(run(["docs", "tools"])).toEqual({ sidebar: 0, chat: 0, docs: 50, tools: 50 });
  });

  test("tiga panel utama → Chat lebih besar, sisanya sama", () => {
    const l = run(["chat", "docs", "tools"]);
    expect(l?.chat).toBeCloseTo(40, 2);
    expect(l?.docs).toBeCloseTo(30, 2);
    expect(l?.tools).toBeCloseTo(30, 2);
  });

  test("semua panel + sidebar → sidebar secukupnya, Chat terbesar, Dokumen = Tools", () => {
    const l = run(["sidebar", "chat", "docs", "tools"]);
    expect(l?.sidebar).toBeCloseTo(12.8, 2);
    expect(l?.docs).toBeCloseTo(l?.tools ?? 0, 2);
    expect(l?.chat ?? 0).toBeGreaterThan(l?.docs ?? 0);
    const sum = Object.values(l ?? {}).reduce((a, b) => a + b, 0);
    expect(sum).toBeCloseTo(100, 5);
  });

  test("minimum dihormati: panel yang terlalu kecil mengambil dari panel lain", () => {
    // 3 panel utama di 900px: 40/30/30 → tools 270px < min 288px
    const l = run(["chat", "docs", "tools"], 900);
    expect(((l?.tools ?? 0) / 100) * 900).toBeCloseTo(288, 0);
    expect(((l?.chat ?? 0) / 100) * 900).toBeGreaterThanOrEqual(352 - 0.5);
  });
});

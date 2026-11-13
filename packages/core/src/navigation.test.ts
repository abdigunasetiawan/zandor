import { describe, expect, test, vi } from "vitest";
import { NavigationHistory } from "./navigation";

describe("NavigationHistory", () => {
  test("awal: tidak bisa back/forward", () => {
    const h = new NavigationHistory("home");
    expect(h.state()).toEqual({ current: "home", canBack: false, canForward: false });
    expect(h.back()).toBe(false);
    expect(h.forward()).toBe(false);
  });

  test("push, back, forward", () => {
    const h = new NavigationHistory("home");
    h.push("settings");
    h.push("ds");
    expect(h.state()).toEqual({ current: "ds", canBack: true, canForward: false });
    expect(h.back()).toBe(true);
    expect(h.state()).toEqual({ current: "settings", canBack: true, canForward: true });
    expect(h.forward()).toBe(true);
    expect(h.state().current).toBe("ds");
  });

  test("push setelah back membuang entri ke depan", () => {
    const h = new NavigationHistory("a");
    h.push("b");
    h.push("c");
    h.back();
    h.push("d");
    expect(h.state()).toEqual({ current: "d", canBack: true, canForward: false });
    h.back();
    h.back();
    expect(h.state().current).toBe("a");
  });

  test("push ke tujuan yang sama diabaikan (tanpa notifikasi)", () => {
    const h = new NavigationHistory("a");
    const l = vi.fn();
    h.subscribe(l);
    h.push("a");
    expect(l).not.toHaveBeenCalled();
    expect(h.state().canBack).toBe(false);
  });

  test("replace tidak menambah riwayat", () => {
    const h = new NavigationHistory("a");
    h.replace("b");
    expect(h.state()).toEqual({ current: "b", canBack: false, canForward: false });
  });

  test("snapshot stabil bila tidak berubah; berubah setelah navigasi", () => {
    const h = new NavigationHistory("a");
    const s1 = h.state();
    expect(h.state()).toBe(s1);
    h.push("b");
    expect(h.state()).not.toBe(s1);
  });

  test("batas riwayat membuang entri tertua", () => {
    const h = new NavigationHistory<number>(0, Object.is, 3);
    for (const n of [1, 2, 3]) h.push(n);
    expect(h.state().current).toBe(3);
    h.back();
    h.back();
    expect(h.state()).toEqual({ current: 1, canBack: false, canForward: true });
  });

  test("pembanding kustom untuk objek", () => {
    const h = new NavigationHistory({ p: "a" }, (x, y) => x.p === y.p);
    h.push({ p: "a" });
    expect(h.state().canBack).toBe(false);
  });
});

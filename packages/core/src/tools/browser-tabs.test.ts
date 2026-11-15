import { describe, expect, test } from "vitest";
import {
  addBrowserTab,
  browserTabLabel,
  closeBrowserTab,
  initialBrowserTabs,
  MAX_BROWSER_TABS,
  nextBrowserTabId,
  selectBrowserTab,
  setBrowserTabUrl,
} from "./browser";

describe("tab browser", () => {
  test("awal satu tab; tab baru aktif dan id tidak bertabrakan setelah ada yang ditutup", () => {
    let s = initialBrowserTabs();
    expect(s).toEqual({ tabs: [{ id: "b1", url: "" }], active: "b1" });
    s = addBrowserTab(s, "https://a.test/");
    s = addBrowserTab(s);
    expect(s.tabs.map((t) => t.id)).toEqual(["b1", "b2", "b3"]);
    expect(s.active).toBe("b3");
    s = closeBrowserTab(s, "b2");
    expect(nextBrowserTabId(s)).toBe("b2");
  });

  test("batas jumlah tab", () => {
    let s = initialBrowserTabs();
    for (let i = 0; i < 20; i++) s = addBrowserTab(s);
    expect(s.tabs).toHaveLength(MAX_BROWSER_TABS);
    expect(addBrowserTab(s)).toBe(s);
  });

  test("menutup tab aktif memilih tetangga; menutup tab lain tidak mengubah yang aktif", () => {
    let s = initialBrowserTabs();
    s = addBrowserTab(addBrowserTab(s)); // b1 b2 b3, aktif b3
    s = selectBrowserTab(s, "b2");
    expect(closeBrowserTab(s, "b1").active).toBe("b2");
    const closedActive = closeBrowserTab(s, "b2");
    expect(closedActive.tabs.map((t) => t.id)).toEqual(["b1", "b3"]);
    expect(closedActive.active).toBe("b3");
    expect(closeBrowserTab(closedActive, "b3").active).toBe("b1"); // tab terakhir di kanan → ke kiri
  });

  test("menutup tab terakhir membuat tab kosong baru dengan id berbeda (webview lama sudah ditutup)", () => {
    const s = closeBrowserTab(initialBrowserTabs(), "b1");
    expect(s.tabs).toHaveLength(1);
    expect(s.tabs[0]?.id).not.toBe("b1");
    expect(s.active).toBe(s.tabs[0]?.id);
  });

  test("id tak dikenal diabaikan; URL tab diperbarui; judul dari host", () => {
    let s = addBrowserTab(initialBrowserTabs());
    expect(selectBrowserTab(s, "zz")).toBe(s);
    expect(closeBrowserTab(s, "zz")).toBe(s);
    s = setBrowserTabUrl(s, "b2", "https://www.contoh.id/jalan?q=1");
    expect(s.tabs[1]?.url).toBe("https://www.contoh.id/jalan?q=1");
    expect(browserTabLabel(s.tabs[1]?.url ?? "")).toBe("contoh.id");
    expect(browserTabLabel("")).toBe("");
    expect(browserTabLabel("bukan url")).toBe("");
  });
});

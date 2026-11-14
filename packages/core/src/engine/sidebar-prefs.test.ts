import { describe, expect, test } from "vitest";
import { parseSidebarPrefs, SidebarPrefs, sortWithPrefs } from "./sidebar-prefs";

const mem = (init: Record<string, string> = {}) => {
  const data = { ...init };
  return {
    data,
    get: (k: string) => data[k] ?? null,
    set: (k: string, v: string) => {
      data[k] = v;
    },
  };
};
const ids = ["a", "b", "c", "d", "e"];
const id = (s: string) => s;

describe("sortWithPrefs", () => {
  test("disematkan dulu (urutan sematan), lalu urutan buatan, lalu bawaan", () => {
    expect(sortWithPrefs(ids, id, [], [])).toEqual(ids);
    expect(sortWithPrefs(ids, id, ["d", "b"], [])).toEqual(["d", "b", "a", "c", "e"]);
    expect(sortWithPrefs(ids, id, [], ["e", "c"])).toEqual(["e", "c", "a", "b", "d"]);
    expect(sortWithPrefs(ids, id, ["c"], ["e", "a"])).toEqual(["c", "e", "a", "b", "d"]);
    // id di urutan tetapi sudah tidak ada → diabaikan
    expect(sortWithPrefs(["a", "b"], id, [], ["z", "b"])).toEqual(["b", "a"]);
  });
});

describe("SidebarPrefs", () => {
  test("sematkan/lepas, persisten, dan pulih dari penyimpanan", () => {
    const store = mem();
    const p = new SidebarPrefs(store);
    p.toggleSessionPin("s1");
    p.toggleProjectPin("p1");
    expect(p.isSessionPinned("s1") && p.isProjectPinned("p1")).toBe(true);
    const again = new SidebarPrefs(store);
    expect(again.state().pinnedSessions).toEqual(["s1"]);
    again.toggleSessionPin("s1");
    expect(again.isSessionPinned("s1")).toBe(false);
  });

  test("seret-lepas sesi: bergeser di dalam kelompoknya; sematan tetap di atas", () => {
    const p = new SidebarPrefs(mem());
    p.toggleSessionPin("c");
    const view = () => p.sortSessions("chat", ids, id);
    expect(view()).toEqual(["c", "a", "b", "d", "e"]);
    // pindahkan "e" ke indeks 1 (tepat setelah yang disematkan)
    p.moveSession("chat", view(), "e", 1);
    expect(view()).toEqual(["c", "e", "a", "b", "d"]);
    // pindahkan "a" ke akhir
    p.moveSession("chat", view(), "a", 99);
    expect(view()).toEqual(["c", "e", "b", "d", "a"]);
    // item tersemat tidak keluar dari kelompok tersemat (indeks dijepit)
    p.toggleSessionPin("b");
    p.moveSession("chat", view(), "b", 4);
    expect(view().slice(0, 2).sort()).toEqual(["b", "c"]);
    // urutan per kelompok terpisah
    expect(p.sortSessions("proj", ids, id)).toEqual(["c", "b", "a", "d", "e"]); // sematan berlaku global; urutan tidak
    expect(p.state().sessionOrder.proj).toBeUndefined();
  });

  test("seret-lepas project dan lupakan yang dihapus", () => {
    const p = new SidebarPrefs(mem());
    const view = () => p.sortProjects(["x", "y", "z"], id);
    p.moveProject(view(), "z", 0);
    expect(view()).toEqual(["z", "x", "y"]);
    p.forgetProject("z");
    expect(view()).toEqual(["x", "y", "z"]);
    p.toggleSessionPin("s");
    p.moveSession("g", ["s", "t"], "t", 0);
    p.forgetSession("s");
    expect(p.state().pinnedSessions).toEqual([]);
    expect(p.state().sessionOrder.g).toEqual(["t"]);
  });

  test("data rusak atau bertipe salah tidak merusak: jatuh ke kosong", () => {
    expect(parseSidebarPrefs("bukan json").pinnedSessions).toEqual([]);
    expect(parseSidebarPrefs('{"pinnedSessions":[1,"a","a"],"sessionOrder":{"g":"x","h":["y"]}}')).toMatchObject({
      pinnedSessions: ["a"],
      sessionOrder: { h: ["y"] },
    });
  });
});

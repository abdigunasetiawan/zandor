import { describe, expect, test, vi } from "vitest";
import { basicAuthHeader, isDefaultSessionTitle } from "./api";
import { ConnectionStore, fromSnapshot, INITIAL_ENGINE_STATE } from "./connection";
import { createFakeEngine } from "./fake-engine";
import {
  baseName,
  CHAT_KEY,
  isProjectColor,
  normalizePath,
  ProjectStore,
  pathKey,
  type RegistryStorage,
  sessionTitle,
} from "./projects";

const memStorage = (initial: string | null = null) => {
  let v = initial;
  const storage: RegistryStorage & { raw: () => string | null; saves: number } = {
    saves: 0,
    load: async () => v,
    save: async (t) => {
      v = t;
      storage.saves++;
    },
    raw: () => v,
  };
  return storage;
};

let seq = 0;
const make = (storage = memStorage(), opts: Partial<ConstructorParameters<typeof ProjectStore>[0]> = {}) => {
  const warnings: string[] = [];
  const store = new ProjectStore({
    storage,
    newId: () => `p${++seq}`,
    now: () => 5,
    onWarning: (m) => warnings.push(m),
    ...opts,
  });
  return { store, storage, warnings };
};
const flush = () => new Promise((r) => setTimeout(r, 0));

describe("path", () => {
  test.each([
    [String.raw`C:\Users\a\proj\\`, String.raw`C:\Users\a\proj`],
    ["C:/Users/a/proj/", String.raw`C:\Users\a\proj`],
    ["C:\\", "C:\\"],
    ["/home/a/p/", "/home/a/p"],
    ["/", "/"],
    ["  D:\\x  ", "D:\\x"],
  ])("normalizePath(%j)", (input, want) => {
    expect(normalizePath(input)).toBe(want);
  });

  test("pathKey: Windows tidak peka huruf besar/kecil, Unix peka", () => {
    expect(pathKey("C:\\Users\\A")).toBe(pathKey("c:/users/a/"));
    expect(pathKey("/Home/A")).not.toBe(pathKey("/home/a"));
  });

  test("baseName", () => {
    expect(baseName("C:\\Users\\a\\zandor\\")).toBe("zandor");
    expect(baseName("/home/a/proj")).toBe("proj");
    expect(baseName("D:\\")).toBe("D:\\");
  });
});

describe("helper", () => {
  test("judul bawaan engine dikenali; judul user tidak", () => {
    expect(isDefaultSessionTitle("New session - 2026-11-13T13:19:25.866Z")).toBe(true);
    expect(isDefaultSessionTitle("Perbaiki bug login")).toBe(false);
    expect(sessionTitle({ title: "New session - 2026-11-13T13:19:25.866Z" })).toBeNull();
    expect(sessionTitle({ title: "Ganti nama" })).toBe("Ganti nama");
  });
  test("basicAuthHeader (Basic opencode:token)", () => {
    expect(basicAuthHeader({ username: "opencode", password: "x" })).toBe("Basic b3BlbmNvZGU6eA==");
  });
  test("warna project valid", () => {
    expect(isProjectColor("blue")).toBe(true);
    expect(isProjectColor("merah-muda")).toBe(false);
  });
});

describe("ConnectionStore", () => {
  test("snapshot Rust → state; info hanya saat ready", () => {
    const ready = fromSnapshot({
      status: "ready",
      url: "http://127.0.0.1:4096",
      username: "opencode",
      password: "t",
      version: "1.19.5",
      restarts: 1,
      error: null,
    });
    expect(ready.info).toEqual({ url: "http://127.0.0.1:4096", username: "opencode", password: "t" });
    expect(ready.source).toBe("local");
    const starting = fromSnapshot({
      status: "starting",
      url: "",
      username: "opencode",
      password: "",
      version: "",
      restarts: 0,
      error: null,
    });
    expect(starting.info).toBeNull();
    expect(
      fromSnapshot({
        status: "stopped",
        url: "",
        username: "opencode",
        password: "",
        version: "",
        restarts: 0,
        error: null,
      }).source,
    ).toBeNull();
  });
  test("notifikasi hanya bila berubah; snapshot stabil", () => {
    const c = new ConnectionStore();
    const l = vi.fn();
    c.subscribe(l);
    const s0 = c.state();
    c.set({ ...INITIAL_ENGINE_STATE });
    expect(l).not.toHaveBeenCalled();
    c.set({ ...INITIAL_ENGINE_STATE, status: "starting" });
    expect(l).toHaveBeenCalledTimes(1);
    expect(c.state()).not.toBe(s0);
    const s1 = c.state();
    c.set({ ...INITIAL_ENGINE_STATE, status: "starting" });
    expect(c.state()).toBe(s1);
  });
});

describe("ProjectStore: registry", () => {
  test("mulai kosong; tambah project → tersimpan dan terpilih", async () => {
    const { store, storage } = make();
    await store.init();
    expect(store.state()).toMatchObject({ loaded: true, projects: [] });
    const p = await store.addProject("C:/Users/a/zandor/");
    expect(p).toMatchObject({ path: "C:\\Users\\a\\zandor", name: "zandor", color: null });
    expect(store.state().selection).toEqual({ key: p.id, sessionId: null });
    expect(JSON.parse(storage.raw() as string).projects).toHaveLength(1);
  });

  test("folder yang sama (beda huruf/pemisah) tidak digandakan", async () => {
    const { store } = make();
    await store.init();
    const a = await store.addProject("C:\\Work\\App");
    const b = await store.addProject("c:/work/app/");
    expect(b.id).toBe(a.id);
    expect(store.state().projects).toHaveLength(1);
  });

  test("dimuat ulang dari penyimpanan (restart aplikasi)", async () => {
    const storage = memStorage();
    const first = make(storage).store;
    await first.init();
    const p = await first.addProject("C:\\a\\x");
    await first.renameProject(p.id, "X Utama");
    await first.setProjectColor(p.id, "blue");
    const second = make(storage).store;
    await second.init();
    expect(second.state().projects).toEqual([{ ...p, name: "X Utama", color: "blue" }]);
    expect(second.state().selection.key).toBe(p.id);
  });

  test("registry rusak/tidak dikenal → mulai kosong + peringatan; tidak melempar", async () => {
    for (const bad of ["{rusak", '{"version":99,"projects":[]}', "[]"]) {
      const { store, warnings } = make(memStorage(bad));
      await store.init();
      expect(store.state().projects).toEqual([]);
      expect(warnings).toHaveLength(1);
    }
  });

  test("entri registry yang cacat atau ganda dibuang; warna tak dikenal → null", async () => {
    const json = JSON.stringify({
      version: 1,
      projects: [
        { id: "a", path: "C:\\x", name: "x", color: "neon", addedAt: 1 },
        { id: "b", path: "c:/X", name: "dobel", addedAt: 2 },
        { id: "c", name: "tanpa path" },
        { id: "a", path: "C:\\y", name: "id ganda" },
      ],
    });
    const { store } = make(memStorage(json));
    await store.init();
    expect(store.state().projects.map((p) => [p.id, p.color])).toEqual([["a", null]]);
  });

  test("rename: nama kosong ditolak; hapus project hanya melepas dari daftar", async () => {
    const { store } = make();
    await store.init();
    const p = await store.addProject("C:\\a\\x");
    await expect(store.renameProject(p.id, "   ")).rejects.toThrow(/kosong/);
    await expect(store.setProjectColor(p.id, "neon")).rejects.toThrow(/warna/);
    await store.removeProject(p.id);
    expect(store.state().projects).toEqual([]);
    expect(store.state().selection).toEqual({ key: null, sessionId: null });
  });

  test("folder Chat tidak bisa menjadi project", async () => {
    const { store } = make();
    await store.init();
    store.setChatDir("C:\\data\\chats");
    await expect(store.addProject("c:/data/chats/")).rejects.toThrow(/Chat/);
  });
});

describe("ProjectStore: sesi", () => {
  async function ready() {
    const engine = createFakeEngine();
    const ctx = make();
    await ctx.store.init();
    ctx.store.setChatDir("C:\\data\\chats");
    const project = await ctx.store.addProject("C:\\work\\alpha");
    ctx.store.setApi(engine);
    await flush();
    return { ...ctx, engine, project };
  }

  test("saat engine siap semua daftar sesi dimuat; urut terbaru dulu", async () => {
    const engine = createFakeEngine();
    const { store } = make();
    await store.init();
    const p = await store.addProject("C:\\work\\alpha");
    await engine.createSession("C:\\work\\alpha", "lama");
    await engine.createSession("C:\\work\\alpha", "baru");
    store.setChatDir("C:\\data\\chats");
    store.setApi(engine);
    await flush();
    expect(store.state().engineReady).toBe(true);
    expect(store.state().sessions[p.id]?.items.map((s) => s.title)).toEqual(["baru", "lama"]);
    expect(store.state().sessions[CHAT_KEY]).toMatchObject({ status: "ready", items: [] });
  });

  test("buat sesi di project → muncul di atas, terpilih, terikat direktori project", async () => {
    const { store, engine, project } = await ready();
    const s = await store.createSession(project.id);
    expect(s.directory).toBe("C:\\work\\alpha");
    expect(store.state().sessions[project.id]?.items[0]?.id).toBe(s.id);
    expect(store.state().selection).toEqual({ key: project.id, sessionId: s.id });
    expect(store.selected()?.project?.name).toBe("alpha");
    expect(engine.calls).toContain("create:C:\\work\\alpha");
  });

  test("sesi tanpa project memakai direktori Chat", async () => {
    const { store } = await ready();
    const s = await store.createSession(CHAT_KEY);
    expect(s.directory).toBe("C:\\data\\chats");
    expect(store.selected()?.project).toBeNull();
  });

  test("rename dan hapus; menghapus sesi terpilih mengosongkan pilihan", async () => {
    const { store, project } = await ready();
    const s = await store.createSession(project.id);
    await store.renameSession(project.id, s.id, "  Perbaiki login  ");
    expect(store.state().sessions[project.id]?.items[0]?.title).toBe("Perbaiki login");
    await expect(store.renameSession(project.id, s.id, " ")).rejects.toThrow(/kosong/);
    await store.deleteSession(project.id, s.id);
    expect(store.state().sessions[project.id]?.items).toEqual([]);
    expect(store.state().selection).toEqual({ key: project.id, sessionId: null });
  });

  test("operasi tanpa engine siap ditolak dengan pesan jelas", async () => {
    const { store, project } = make().store
      ? await (async () => {
          const m = make();
          await m.store.init();
          const p = await m.store.addProject("C:\\a");
          return { store: m.store, project: p };
        })()
      : (undefined as never);
    await expect(store.createSession(project.id)).rejects.toThrow(/belum siap/);
  });

  test("galat engine saat memuat dicatat per-project, tidak melempar; pulih setelah refresh", async () => {
    const { store, engine, project } = await ready();
    engine.failWith = "koneksi ditolak";
    await store.refresh(project.id);
    expect(store.state().sessions[project.id]).toMatchObject({ status: "error", error: "koneksi ditolak" });
    engine.failWith = null;
    await store.refresh(project.id);
    expect(store.state().sessions[project.id]?.status).toBe("ready");
  });

  test("respons basi dibuang (permintaan lama selesai setelah yang baru)", async () => {
    const { store, engine, project } = await ready();
    let releaseFirst: (v: never[]) => void = () => {};
    const original = engine.listSessions.bind(engine);
    let call = 0;
    engine.listSessions = (dir: string) => {
      call++;
      if (call === 1) return new Promise((r) => (releaseFirst = r as never));
      return original(dir);
    };
    const first = store.refresh(project.id);
    await engine.createSession("C:\\work\\alpha", "terbaru");
    await store.refresh(project.id);
    releaseFirst([]);
    await first;
    expect(store.state().sessions[project.id]?.items.map((s) => s.title)).toEqual(["terbaru"]);
  });

  test("pilihan yang menunjuk sesi yang sudah tidak ada dikosongkan setelah muat ulang", async () => {
    const { store, engine, project } = await ready();
    const s = await store.createSession(project.id);
    engine.sessions.set("C:\\work\\alpha", []); // dihapus di luar Zandor
    await store.refresh(project.id);
    expect(store.state().selection).toEqual({ key: project.id, sessionId: null });
    expect(s.id).toBeTruthy();
  });

  test("engine terputus lalu tersambung lagi → daftar dimuat ulang", async () => {
    const { store, engine, project } = await ready();
    store.setApi(null);
    expect(store.state().engineReady).toBe(false);
    await engine.createSession("C:\\work\\alpha", "dibuat saat putus");
    store.setApi(engine);
    await flush();
    expect(store.state().sessions[project.id]?.items.map((s) => s.title)).toContain("dibuat saat putus");
  });

  test("pilihan sesi tersimpan di registry", async () => {
    const engine = createFakeEngine();
    const storage = memStorage();
    const m = make(storage);
    await m.store.init();
    const p = await m.store.addProject("C:\\work\\alpha");
    m.store.setApi(engine);
    await flush();
    const s = await m.store.createSession(p.id);
    await flush();
    expect(JSON.parse(storage.raw() as string).selection).toEqual({ key: p.id, sessionId: s.id });
  });
});

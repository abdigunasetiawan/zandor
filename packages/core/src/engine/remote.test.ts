import { describe, expect, test } from "vitest";
import type { EngineApi } from "./api";
import { createFakeEngine } from "./fake-engine";
import { ProjectStore, type RegistryStorage } from "./projects";
import { createRoutedApi, remoteUrlProblem } from "./remote";

const flush = () => new Promise((r) => setTimeout(r, 0));
const memStorage = (initial: string | null = null) => {
  let v = initial;
  return { load: async () => v, save: async (t: string) => void (v = t), raw: () => v } satisfies RegistryStorage & {
    raw: () => string | null;
  };
};

describe("engine remote", () => {
  test("URL: HTTPS wajib untuk mesin lain; HTTP hanya loopback; kredensial di URL ditolak", () => {
    expect(remoteUrlProblem("https://engine.contoh.id")).toBeNull();
    expect(remoteUrlProblem("http://127.0.0.1:4096")).toBeNull();
    expect(remoteUrlProblem("http://localhost:4096/")).toBeNull();
    expect(remoteUrlProblem("http://192.168.1.5:4096")).toBe("insecure");
    expect(remoteUrlProblem("https://u:p@engine.id")).toBe("invalid");
    expect(remoteUrlProblem("ftp://engine.id")).toBe("invalid");
    expect(remoteUrlProblem("bukan url")).toBe("invalid");
  });

  test("router: panggilan berdirektori ke engine pemilik, sisanya ke lokal; engine hilang = galat jelas", async () => {
    const local = createFakeEngine();
    const remote = createFakeEngine();
    remote.models = [];
    const api = createRoutedApi({
      local,
      remote: (id) => (id === "r1" ? remote : null),
      route: (d) =>
        d.startsWith("/srv")
          ? { kind: "remote", id: "r1" }
          : d === "/x"
            ? { kind: "remote", id: "hilang" }
            : { kind: "local" },
    });
    await api.createSession("/srv/app", "a");
    await api.createSession("C:\\lokal", "b");
    expect([...remote.sessions.keys()]).toEqual(["/srv/app"]);
    expect([...local.sessions.keys()]).toEqual(["C:\\lokal"]);
    local.models = [{ providerID: "p", modelID: "lokal" } as never];
    expect(await api.listModels()).toEqual(local.models);
    await expect(api.listSessions("/x")).rejects.toThrow(/hilang/);
    expect(api.subscribe("/x", { onEvent: () => {}, onState: () => {} })).toBeTypeOf("function");
    await api.pty.list("/srv/app");
    const noLocal = createRoutedApi({ local: null, remote: () => remote, route: () => ({ kind: "local" }) });
    await expect(noLocal.listModels()).rejects.toThrow(/lokal belum siap/);
  });

  test("ProjectStore: tambah remote (status jujur), project remote dirutekan, hapus ditolak bila dipakai, tersimpan", async () => {
    const local = createFakeEngine();
    const remote = createFakeEngine();
    const storage = memStorage();
    let seq = 0;
    let routed: EngineApi | null = null;
    const store = new ProjectStore({
      storage,
      newId: () => `id${++seq}`,
      createApi: () => remote,
      onApi: (a) => {
        routed = a;
      },
    });
    await store.init();
    store.setApi(local);
    expect(routed).toBe(local); // tanpa remote: klien lokal apa adanya

    await expect(
      store.addRemote({ name: "VPS", url: "http://10.0.0.2:4096", username: "", password: "x" }),
    ).rejects.toThrow(/HTTPS/);
    const r = await store.addRemote({ name: "VPS", url: "https://vps.contoh.id/", username: "", password: "rahasia" });
    expect(r).toMatchObject({ url: "https://vps.contoh.id", username: "opencode" });
    await flush();
    expect(store.state().remoteStatus[r.id]?.status).toBe("ready");
    expect(routed).not.toBe(local);

    const p = await store.addProject("/srv/app", r.id);
    expect(p.engine).toBe(r.id);
    expect(store.routeOf("/srv/app/sub")).toEqual({ kind: "remote", id: r.id });
    expect(store.routeOf("C:\\lain")).toEqual({ kind: "local" });
    await store.createSession(p.id);
    expect(remote.sessions.get("/srv/app")).toHaveLength(1);
    expect(local.sessions.has("/srv/app")).toBe(false);

    await expect(store.removeRemote(r.id)).rejects.toThrow(/dipakai project/);
    expect(JSON.parse(storage.raw() ?? "{}").remotes[0]).toMatchObject({ id: r.id, password: "rahasia" });

    // dimuat ulang dari registry: remote dan pilihan engine project tetap
    const again = new ProjectStore({ storage, createApi: () => remote });
    await again.init();
    expect(again.state().remotes.map((x) => x.name)).toEqual(["VPS"]);
    expect(again.state().projects[0]?.engine).toBe(r.id);

    remote.failWith = "connect ECONNREFUSED";
    await store.checkRemote(r.id);
    expect(store.state().remoteStatus[r.id]).toMatchObject({ status: "error" });
    expect(store.state().remoteStatus[r.id]?.error).toMatch(/ECONNREFUSED/);
  });
});

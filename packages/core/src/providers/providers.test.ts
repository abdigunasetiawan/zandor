import { describe, expect, test, vi } from "vitest";
import { createFakeEngine } from "../engine/fake-engine";
import {
  type LocalFound,
  type ProbeResult,
  type ProviderHost,
  type ProviderSpec,
  ProviderStore,
  type ProviderView,
  pingKey,
  slugifyProviderId,
  validateEndpoint,
} from "./store";

const ok = (over: Partial<ProbeResult> = {}): ProbeResult => ({
  ok: true,
  status: 200,
  latencyMs: 12,
  models: [],
  kind: "ok",
  error: null,
  ...over,
});

function fakeHost() {
  const keys = new Map<string, string>();
  const specs = new Map<string, ProviderSpec>();
  const calls: string[] = [];
  const host: ProviderHost & {
    keys: Map<string, string>;
    calls: string[];
    probeResult: ProbeResult;
    pingResult: ProbeResult | "hang";
    local: LocalFound[];
    opened: string[];
    failSave: string | null;
  } = {
    secure: true,
    keys,
    calls,
    probeResult: ok({ models: ["m1", "m2"] }),
    pingResult: ok(),
    local: [],
    opened: [],
    failSave: null,
    async list(): Promise<ProviderView[]> {
      return [...specs.values()].map((s) => ({ ...s, hasKey: keys.has(s.id) }));
    },
    async save(p, apiKey) {
      calls.push(`save:${p.id}`);
      if (host.failSave) throw new Error(host.failSave);
      specs.set(p.id, p);
      if (apiKey) keys.set(p.id, apiKey);
    },
    async remove(id) {
      calls.push(`remove:${id}`);
      specs.delete(id);
      keys.delete(id);
    },
    async probe(baseUrl, apiKey, providerId) {
      calls.push(`probe:${baseUrl}:${apiKey ?? "-"}:${providerId ?? "-"}`);
      return host.probeResult;
    },
    async ping(providerId, model) {
      calls.push(`ping:${providerId}/${model}`);
      if (host.pingResult === "hang") return new Promise(() => {});
      return host.pingResult;
    },
    async detectLocal() {
      return host.local;
    },
    async openExternal(url) {
      host.opened.push(url);
    },
  };
  return host;
}

describe("helper", () => {
  test("slug: huruf kecil, tanda hubung, unik, tidak kosong", () => {
    expect(slugifyProviderId("Antigravity Tools", [])).toBe("antigravity-tools");
    expect(slugifyProviderId("Antigravity Tools", ["antigravity-tools"])).toBe("antigravity-tools-2");
    expect(slugifyProviderId("  ***  ", [])).toBe("provider");
    expect(slugifyProviderId("Çafé Ünï", [])).toMatch(/^[a-z0-9-]+$/);
    expect(slugifyProviderId("x".repeat(100), []).length).toBeLessThanOrEqual(36);
  });
  test("validateEndpoint hanya menerima http(s)", () => {
    expect(validateEndpoint("http://localhost:8045/v1")).toBeNull();
    expect(validateEndpoint("https://api.x.com/v1")).toBeNull();
    expect(validateEndpoint("ftp://x")).toMatch(/http/);
    expect(validateEndpoint("bukan url")).toMatch(/tidak valid/);
  });
});

describe("ProviderStore: koneksi", () => {
  test("tambah endpoint kustom: id dari nama, base URL dirapikan, kunci ke host, model unik", async () => {
    const host = fakeHost();
    const onChanged = vi.fn();
    const store = new ProviderStore({ host, onChanged });
    const id = await store.saveEndpoint({
      name: "Antigravity Tools",
      kind: "custom",
      baseUrl: " http://localhost:8045/v1/// ",
      apiKey: "sk-rahasia",
      models: [{ id: "a" }, { id: " a " }, { id: "b", name: "Model B" }, { id: " " }],
    });
    expect(id).toBe("antigravity-tools");
    expect(host.keys.get(id)).toBe("sk-rahasia");
    const view = store.state().providers[0];
    expect(view).toMatchObject({ baseUrl: "http://localhost:8045/v1", hasKey: true });
    expect(view?.models).toEqual([{ id: "a" }, { id: "b", name: "Model B" }]);
    expect(onChanged).toHaveBeenCalledTimes(1);
    // kunci tidak pernah ada di state
    expect(JSON.stringify(store.state())).not.toContain("sk-rahasia");
  });

  test("validasi: nama kosong, URL salah, tanpa model → ditolak sebelum menyentuh host", async () => {
    const host = fakeHost();
    const store = new ProviderStore({ host });
    const base = { kind: "custom" as const, baseUrl: "http://x/v1", models: [{ id: "m" }] };
    await expect(store.saveEndpoint({ ...base, name: " " })).rejects.toThrow(/nama/);
    await expect(store.saveEndpoint({ ...base, name: "A", baseUrl: "ftp://x" })).rejects.toThrow(/http/);
    await expect(store.saveEndpoint({ ...base, name: "A", models: [] })).rejects.toThrow(/model/);
    expect(host.calls).toEqual([]);
  });

  test("galat dari host dilempar dan daftar tidak berubah", async () => {
    const host = fakeHost();
    host.failSave = "keychain terkunci";
    const store = new ProviderStore({ host });
    await expect(
      store.saveEndpoint({ name: "A", kind: "custom", baseUrl: "http://x/v1", models: [{ id: "m" }] }),
    ).rejects.toThrow("keychain terkunci");
    expect(store.state().providers).toEqual([]);
  });

  test("simpan ulang tanpa kunci mempertahankan kunci lama", async () => {
    const host = fakeHost();
    const store = new ProviderStore({ host });
    const id = await store.saveEndpoint({
      name: "A",
      kind: "custom",
      baseUrl: "http://x/v1",
      apiKey: "k1",
      models: [{ id: "m" }],
    });
    await store.saveEndpoint({ id, name: "A baru", kind: "custom", baseUrl: "http://x/v1", models: [{ id: "m" }] });
    expect(host.keys.get(id)).toBe("k1");
    expect(store.state().providers[0]?.name).toBe("A baru");
  });

  test("provider katalog (builtin): hanya kunci, tanpa model/URL", async () => {
    const host = fakeHost();
    const store = new ProviderStore({ host });
    await expect(store.saveBuiltin({ id: "openrouter", name: "OpenRouter", apiKey: "  " })).rejects.toThrow(/kunci/);
    await store.saveBuiltin({ id: "openrouter", name: "OpenRouter", apiKey: " sk-or " });
    expect(host.keys.get("openrouter")).toBe("sk-or");
    expect(store.state().providers[0]).toMatchObject({ kind: "builtin", models: [], hasKey: true });
  });

  test("hapus: entri, kunci, dan hasil ping provider itu hilang", async () => {
    const host = fakeHost();
    const store = new ProviderStore({ host });
    const id = await store.saveEndpoint({
      name: "A",
      kind: "custom",
      baseUrl: "http://x/v1",
      apiKey: "k",
      models: [{ id: "m" }],
    });
    await store.ping(id, "m");
    expect(store.state().pings[pingKey(id, "m")]).toBeDefined();
    await store.remove(id);
    expect(store.state().providers).toEqual([]);
    expect(host.keys.size).toBe(0);
    expect(store.state().pings).toEqual({});
  });
});

describe("ProviderStore: probe, deteksi lokal, ping", () => {
  test("probe memakai kunci yang diketik atau kunci tersimpan (id), dan menolak URL tak valid tanpa memanggil host", async () => {
    const host = fakeHost();
    const store = new ProviderStore({ host });
    expect((await store.probe("http://x/v1", "k")).models).toEqual(["m1", "m2"]);
    await store.probe("http://x/v1", undefined, "antigravity");
    expect(host.calls).toEqual(["probe:http://x/v1:k:-", "probe:http://x/v1:-:antigravity"]);
    const bad = await store.probe("ftp://x");
    expect(bad.ok).toBe(false);
    expect(host.calls).toHaveLength(2);
  });

  test("galat dari host diubah menjadi hasil gagal (tidak melempar)", async () => {
    const host = fakeHost();
    host.probe = async () => {
      throw new Error("jaringan putus");
    };
    const r = await new ProviderStore({ host }).probe("http://x/v1");
    expect(r).toMatchObject({ ok: false, error: "jaringan putus" });
  });

  test("deteksi lokal meneruskan hasil; galat → daftar kosong", async () => {
    const host = fakeHost();
    host.local = [{ id: "ollama", name: "Ollama", baseUrl: "http://localhost:11434/v1", models: ["llama3"] }];
    const store = new ProviderStore({ host });
    expect(await store.detectLocal()).toHaveLength(1);
    host.detectLocal = async () => {
      throw new Error("x");
    };
    expect(await store.detectLocal()).toEqual([]);
  });

  test("ping: pending lalu hasil dengan waktu; kegagalan jujur (rate limit)", async () => {
    const host = fakeHost();
    host.pingResult = ok({ ok: false, status: 429, kind: "ratelimit", error: "kuota habis" });
    const store = new ProviderStore({ host, now: () => 123 });
    const p = store.ping("p", "m");
    expect(store.state().pings["p/m"]).toEqual({ status: "pending" });
    await p;
    expect(store.state().pings["p/m"]).toMatchObject({ status: "done", at: 123 });
    expect((store.state().pings["p/m"] as { result: ProbeResult }).result.kind).toBe("ratelimit");
  });

  test("ping yang menggantung berakhir lewat penjaga waktu (tidak ada spinner tanpa akhir)", async () => {
    const host = fakeHost();
    host.pingResult = "hang";
    const store = new ProviderStore({ host, pingWatchdogMs: 20 });
    const r = await store.ping("p", "m");
    expect(r).toMatchObject({ ok: false, kind: "network" });
    expect(store.state().pings["p/m"]).toMatchObject({ status: "done" });
  });
});

describe("ProviderStore: katalog engine dan OAuth", () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));

  test("katalog dimuat saat engine siap; 'tersambung' berasal dari engine", async () => {
    const engine = createFakeEngine();
    engine.catalog = [
      { id: "openrouter", name: "OpenRouter", env: ["OPENROUTER_API_KEY"], connected: true, modelCount: 300 },
      { id: "anthropic", name: "Anthropic", env: ["ANTHROPIC_API_KEY"], connected: false, modelCount: 10 },
    ];
    engine.authMethods = { openai: [{ type: "oauth", label: "ChatGPT", prompts: [] }] };
    const store = new ProviderStore({ host: fakeHost() });
    store.setApi(engine);
    await flush();
    expect(store.state()).toMatchObject({ catalogStatus: "ready", connected: ["openrouter"] });
    expect(store.state().authMethods.openai?.[0]?.label).toBe("ChatGPT");
    store.setApi(null);
    expect(store.state().connected).toEqual([]);
  });

  test("galat katalog dicatat, bukan spinner abadi", async () => {
    const engine = createFakeEngine();
    engine.failWith = "engine mati";
    const store = new ProviderStore({ host: fakeHost() });
    store.setApi(engine);
    await flush();
    expect(store.state()).toMatchObject({ catalogStatus: "error", catalogError: "engine mati" });
  });

  test("OAuth: membuka URL https di browser sistem lalu menyelesaikan dan menyegarkan model", async () => {
    const engine = createFakeEngine();
    const host = fakeHost();
    const onChanged = vi.fn();
    const store = new ProviderStore({ host, onChanged });
    store.setApi(engine);
    await flush();
    const start = await store.startOAuth("openai", 0, { x: "y" });
    expect(start.method).toBe("auto");
    expect(host.opened).toEqual(["https://auth.example.test/authorize"]);
    expect(engine.oauth.starts).toEqual([{ providerId: "openai", method: 0, inputs: { x: "y" } }]);
    await store.finishOAuth("openai", 0);
    expect(engine.oauth.callbacks).toEqual([{ providerId: "openai", method: 0 }]);
    expect(onChanged).toHaveBeenCalled();
    await store.finishOAuth("openai", 1, "kode-123");
    expect(engine.oauth.callbacks[1]).toEqual({ providerId: "openai", method: 1, code: "kode-123" });
  });

  test("OAuth menolak URL non-https dari engine dan tanpa engine", async () => {
    const engine = createFakeEngine();
    engine.oauth.next = { url: "http://jahat.example/x", method: "auto", instructions: "" };
    const host = fakeHost();
    const store = new ProviderStore({ host });
    await expect(store.startOAuth("a", 0)).rejects.toThrow(/belum siap/);
    store.setApi(engine);
    await flush();
    await expect(store.startOAuth("a", 0)).rejects.toThrow(/bukan https/);
    expect(host.opened).toEqual([]);
  });
});

import { describe, expect, test } from "vitest";
import { createFakeEngine } from "../engine/fake-engine";
import { ProjectStore, type RegistryStorage } from "../engine/projects";
import { applyEvent, initialChatState, visibleMessages } from "./reducer";
import { type ChatPrefs, ChatStore } from "./store";
import { type ChatMessage, type EngineEvent, toEngineEvent } from "./types";

const flush = () => new Promise((r) => setTimeout(r, 0));
const DIR = "C:\\work\\alpha";
const prefsOf = (init: Record<string, string> = {}): ChatPrefs & { data: Record<string, string> } => {
  const data = { ...init };
  return {
    data,
    get: (k) => data[k] ?? null,
    set: (k, v) => {
      data[k] = v;
    },
  };
};

async function setup(prefs = prefsOf()) {
  const engine = createFakeEngine();
  engine.models = [
    { providerID: "p", providerName: "P", modelID: "m1", name: "M1", variants: ["low", "high"] },
    { providerID: "p", providerName: "P", modelID: "m2", name: "M2", variants: [] },
  ];
  const store = new ChatStore({ prefs });
  store.setApi(engine);
  await flush();
  store.open(DIR, "s1");
  await flush();
  return { engine, store, prefs };
}

const user = (id: string, text: string, files: Array<{ url: string; mime: string }> = []): ChatMessage => ({
  info: { id, sessionID: "s1", role: "user", time: { created: 1 } },
  parts: [
    { id: `${id}-t`, messageID: id, sessionID: "s1", type: "text", text, synthetic: false },
    ...files.map((f, i) => ({
      id: `${id}-f${i}`,
      messageID: id,
      sessionID: "s1",
      type: "file" as const,
      mime: f.mime,
      url: f.url,
    })),
  ],
});
const asst = (id: string): ChatMessage => ({
  info: { id, sessionID: "s1", role: "assistant", time: { created: 2, completed: 3 } },
  parts: [],
});

describe("mode izin", () => {
  const ask = (permission: string): EngineEvent => ({
    type: "permission.asked",
    request: { id: `per_${permission}`, sessionID: "s1", permission, patterns: [], metadata: {}, always: [] },
  });

  test("bawaan 'ask': tidak ada jawaban otomatis", async () => {
    const { engine } = await setup();
    engine.emit(DIR, ask("edit"));
    await flush();
    expect(engine.replies).toEqual([]);
  });

  test("'auto-edit' hanya menyetujui edit; bash tetap menunggu pengguna", async () => {
    const { engine, store } = await setup();
    store.setPermissionMode("auto-edit");
    engine.emit(DIR, ask("edit"));
    engine.emit(DIR, ask("bash"));
    await flush();
    expect(engine.replies).toEqual([{ requestId: "per_edit", reply: "once" }]);
  });

  test("'auto-all' menyetujui semuanya, dan izin yang sudah menunggu ikut dijawab saat mode berubah", async () => {
    const { engine, store } = await setup();
    engine.emit(DIR, ask("bash"));
    expect(engine.replies).toEqual([]);
    store.setPermissionMode("auto-all");
    await flush();
    expect(engine.replies).toEqual([{ requestId: "per_bash", reply: "once" }]);
  });

  test("mode tersimpan, dipulihkan, dan nilai rusak diabaikan", async () => {
    const a = await setup();
    a.store.setPermissionMode("auto-edit");
    const b = await setup(a.prefs);
    expect(b.store.state().permissionMode).toBe("auto-edit");
    const c = await setup(prefsOf({ "zandor.permissionMode": "bebas" }));
    expect(c.store.state().permissionMode).toBe("ask");
  });
});

describe("varian reasoning", () => {
  test("varian dikirim bersama prompt; ganti ke model tanpa varian itu mengosongkannya", async () => {
    const { engine, store } = await setup();
    store.setVariant("high");
    await store.send("s1", "halo");
    expect(engine.prompts[0]?.variant).toBe("high");
    store.selectModel({ providerID: "p", modelID: "m2" });
    expect(store.state().variant).toBeNull();
    store.setVariant("high"); // m2 tidak punya varian → ditolak
    expect(store.state().variant).toBeNull();
  });
});

describe("lampiran", () => {
  test("prompt hanya berisi lampiran dibolehkan dan membawa file", async () => {
    const { engine, store } = await setup();
    await store.send("s1", "  ", [{ mime: "text/plain", url: "file:///C:/a.txt", filename: "a.txt" }]);
    expect(engine.prompts[0]?.files).toEqual([{ mime: "text/plain", url: "file:///C:/a.txt", filename: "a.txt" }]);
    expect(engine.prompts[0]?.text).toBe("a.txt");
  });
  test("tanpa teks dan tanpa lampiran ditolak", async () => {
    const { store } = await setup();
    await expect(store.send("s1", "  ")).rejects.toThrow(/kosong/);
  });
});

describe("revert, regenerate, fork", () => {
  test("session.updated membawa status revert; pesan sejak id itu disembunyikan", () => {
    const ev = toEngineEvent({
      type: "session.updated",
      properties: { sessionID: "s1", info: { id: "s1", revert: { messageID: "msg_2" } } },
    });
    expect(ev).toEqual({ type: "session.revert", sessionID: "s1", revert: "msg_2" });
    const base = {
      ...initialChatState("s1"),
      messages: [user("msg_1", "a"), asst("msg_1b"), user("msg_2", "b"), asst("msg_3")],
    };
    const s = applyEvent(base, ev as EngineEvent);
    expect(visibleMessages(s).map((m) => m.info.id)).toEqual(["msg_1", "msg_1b"]);
    const cleared = applyEvent(s, { type: "session.revert", sessionID: "s1", revert: null });
    expect(visibleMessages(cleared)).toHaveLength(4);
    expect(applyEvent(cleared, { type: "session.revert", sessionID: "s1", revert: null })).toBe(cleared);
  });

  test("revertTo dan unrevert diteruskan ke engine; ditolak saat sesi berjalan", async () => {
    const { engine, store } = await setup();
    await store.revertTo("s1", "msg_5");
    expect(engine.reverts.get("s1")).toBe("msg_5");
    expect(store.state().sessions.s1?.revert).toBe("msg_5");
    await store.unrevert("s1");
    expect(store.state().sessions.s1?.revert).toBeNull();
    engine.emit(DIR, { type: "session.status", sessionID: "s1", status: { type: "busy" } });
    await expect(store.revertTo("s1", "msg_5")).rejects.toThrow(/hentikan/);
  });

  test("regenerate: revert ke pesan pengguna terakhir lalu kirim ulang teks + lampiran yang sama", async () => {
    const { engine, store } = await setup();
    engine.messages.set("s1", [
      user("msg_1", "pertama"),
      asst("msg_2"),
      user("msg_3", "kedua", [{ url: "file:///C:/x.png", mime: "image/png" }]),
      asst("msg_4"),
    ]);
    store.close("s1");
    store.open(DIR, "s1");
    await flush();
    await store.regenerate("s1");
    expect(engine.reverts.get("s1")).toBe("msg_3");
    expect(engine.prompts.at(-1)).toMatchObject({
      text: "kedua",
      files: [{ mime: "image/png", url: "file:///C:/x.png" }],
    });
  });

  test("regenerate tanpa pesan pengguna ditolak", async () => {
    const { store } = await setup();
    await expect(store.regenerate("s1")).rejects.toThrow(/tidak ada pesan/);
  });

  test("fork meneruskan ke engine dan mengembalikan sesi baru", async () => {
    const { engine, store } = await setup();
    const forked = await store.fork("s1", "msg_2");
    expect(engine.forks).toEqual([{ sessionId: "s1", messageId: "msg_2" }]);
    expect(forked.directory).toBe(DIR);
  });
});

describe("tab sesi (ProjectStore)", () => {
  const mem = () => {
    let v: string | null = null;
    const storage: RegistryStorage & { raw: () => string | null } = {
      load: async () => v,
      save: async (t) => {
        v = t;
      },
      raw: () => v,
    };
    return storage;
  };
  let n = 0;
  const make = (storage = mem()) => ({
    storage,
    store: new ProjectStore({ storage, newId: () => `p${++n}`, now: () => 1 }),
  });

  test("memilih sesi membuka tab; menutup tab aktif pindah ke tetangga; tab terakhir mengosongkan pilihan", async () => {
    const { store } = make();
    await store.init();
    store.select("p", "a");
    store.select("p", "b");
    store.select("p", "c");
    expect(store.state().tabs.map((t) => t.sessionId)).toEqual(["a", "b", "c"]);
    store.closeTab("c");
    expect(store.state().selection.sessionId).toBe("b");
    store.closeTab("a"); // bukan tab aktif
    expect(store.state().selection.sessionId).toBe("b");
    store.closeTab("b");
    expect(store.state().selection).toEqual({ key: "p", sessionId: null });
    expect(store.state().tabs).toEqual([]);
  });

  test("tab dibatasi 12 dan tersimpan/pulih dari registry", async () => {
    const { store, storage } = make();
    await store.init();
    for (let i = 0; i < 15; i++) store.select("p", `s${i}`);
    expect(store.state().tabs).toHaveLength(12);
    expect(store.state().tabs[0]?.sessionId).toBe("s3");
    await flush();
    const again = make(storage).store;
    await again.init();
    expect(again.state().tabs).toHaveLength(12);
  });

  test("adoptSession memasukkan sesi hasil fork ke daftar dan memilihnya", async () => {
    const engine = createFakeEngine();
    const { store } = make();
    await store.init();
    const project = await store.addProject("C:\\w\\x");
    store.setApi(engine);
    await flush();
    const forked = await engine.createSession("C:\\w\\x", "cabang");
    store.adoptSession(project.id, forked);
    expect(store.state().sessions[project.id]?.items[0]?.id).toBe(forked.id);
    expect(store.state().selection.sessionId).toBe(forked.id);
    expect(store.state().tabs.some((t) => t.sessionId === forked.id)).toBe(true);
  });
});

describe("model disematkan", () => {
  test("sematkan/lepas, tersimpan, dipulihkan, dan data rusak diabaikan", async () => {
    const a = await setup();
    a.store.togglePin({ providerID: "p", modelID: "m2" });
    a.store.togglePin({ providerID: "p", modelID: "m1" });
    expect(a.store.state().pinned).toEqual(["p/m2", "p/m1"]);
    a.store.togglePin({ providerID: "p", modelID: "m2" });
    expect(a.store.state().pinned).toEqual(["p/m1"]);
    const b = await setup(a.prefs);
    expect(b.store.state().pinned).toEqual(["p/m1"]);
    const c = await setup(prefsOf({ "zandor.pinnedModels": "{bukan array" }));
    expect(c.store.state().pinned).toEqual([]);
    const d = await setup(prefsOf({ "zandor.pinnedModels": JSON.stringify(["a", 1, "a", null]) }));
    expect(d.store.state().pinned).toEqual(["a"]);
  });
});

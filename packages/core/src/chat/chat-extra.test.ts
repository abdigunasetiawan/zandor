import { describe, expect, test } from "vitest";
import type { EngineApi } from "../engine/api";
import { createFakeEngine } from "../engine/fake-engine";
import { CHAT_KEY, chatFolderName, ProjectStore, type RegistryStorage } from "../engine/projects";
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

  test("izin yang dijawab otomatis tidak pernah masuk state (tanpa kartu berkedip/notifikasi)", async () => {
    const { engine, store } = await setup();
    const seen: number[] = [];
    store.subscribe(() => seen.push(store.state().sessions.s1?.permissions.length ?? 0));
    store.setPermissionMode("auto-all");
    engine.emit(DIR, ask("bash"));
    await flush();
    expect(engine.replies).toEqual([{ requestId: "per_bash", reply: "once" }]);
    expect(store.state().sessions.s1?.permissions).toEqual([]);
    expect(Math.max(0, ...seen)).toBe(0);
  });

  test("bila jawaban otomatis gagal, izin muncul sebagai kartu agar bisa dijawab manual", async () => {
    const { engine, store } = await setup();
    store.setPermissionMode("auto-all");
    engine.failWith = "putus";
    engine.emit(DIR, ask("bash"));
    await flush();
    expect(store.state().sessions.s1?.permissions.map((p) => p.id)).toEqual(["per_bash"]);
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

describe("memuat ulang instance engine (cache skill)", () => {
  test("direktori idle langsung di-dispose; yang sibuk menunggu sampai idle", async () => {
    const { engine, store } = await setup();
    store.reloadInstances([DIR]);
    await flush();
    expect(engine.disposed).toEqual([DIR]);

    engine.emit(DIR, { type: "session.status", sessionID: "s1", status: { type: "busy" } });
    await flush();
    store.reloadInstances([DIR]);
    await flush();
    expect(engine.disposed).toEqual([DIR]); // belum: giliran masih berjalan
    engine.emit(DIR, { type: "session.status", sessionID: "s1", status: { type: "idle" } });
    await flush();
    expect(engine.disposed).toEqual([DIR, DIR]);
  });

  test("tanpa argumen: semua direktori yang dikenal; galat dispose hanya jadi peringatan", async () => {
    const { engine, store } = await setup();
    store.reloadInstances();
    await flush();
    expect(engine.disposed).toContain(DIR);
    engine.failWith = "putus";
    expect(() => store.reloadInstances([DIR])).not.toThrow();
    await flush();
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

  test("menu tab: tutup lainnya / kanan / kiri / semua; pilihan pindah ke tab jangkar bila ikut tertutup", async () => {
    const mk = async () => {
      const { store } = make();
      await store.init();
      for (const id of ["a", "b", "c", "d", "e"]) store.select("p", id);
      return store; // terpilih: e
    };
    const ids = (s: ProjectStore) => s.state().tabs.map((t) => t.sessionId);

    let s = await mk();
    s.closeTabsBy("others", "c");
    expect(ids(s)).toEqual(["c"]);
    expect(s.state().selection.sessionId).toBe("c");

    s = await mk();
    s.closeTabsBy("right", "b"); // e (terpilih) tertutup → pilih b
    expect(ids(s)).toEqual(["a", "b"]);
    expect(s.state().selection.sessionId).toBe("b");

    s = await mk();
    s.select("p", "c");
    s.closeTabsBy("left", "d"); // c terpilih tertutup → pilih d
    expect(ids(s)).toEqual(["d", "e"]);
    expect(s.state().selection.sessionId).toBe("d");

    s = await mk();
    s.closeTabsBy("right", "e"); // tidak ada di kanan → tidak berubah
    s.closeTabsBy("left", "a");
    s.closeTabsBy("others", "tidak-ada");
    expect(ids(s)).toEqual(["a", "b", "c", "d", "e"]);

    s = await mk();
    s.closeTabsBy("all");
    expect(ids(s)).toEqual([]);
    expect(s.state().selection).toEqual({ key: "p", sessionId: null });
  });

  test("sesi Chat: tiap sesi punya folder SENDIRI; daftar mencakup semua folder + folder lama yang berbagi; hapus membersihkan folder kosong", async () => {
    const created: string[] = [];
    const removed: string[] = [];
    const folders = {
      create: async (name: string) => {
        const path = `C:/data/chats/${name}`;
        created.push(path);
        return path;
      },
      list: async () => created,
      removeIfEmpty: async (p: string) => {
        removed.push(p);
      },
    };
    const sessions: Array<{
      id: string;
      title: string;
      directory: string;
      time: { created: number; updated: number };
    }> = [{ id: "lama", title: "Sesi lama", directory: "C:/data/chats", time: { created: 1, updated: 1 } }];
    const api = {
      listSessions: async (d: string) => sessions.filter((s) => s.directory === d),
      createSession: async (d: string, title?: string) => {
        const s = { id: `s${sessions.length}`, title: title ?? "baru", directory: d, time: { created: 5, updated: 5 } };
        sessions.push(s);
        return s;
      },
      renameSession: async (_d: string, id: string, title: string) => {
        const s = sessions.find((x) => x.id === id);
        if (!s) throw new Error("tidak ada");
        s.title = title;
        return s;
      },
      deleteSession: async (_d: string, id: string) => {
        sessions.splice(
          sessions.findIndex((x) => x.id === id),
          1,
        );
      },
    } as unknown as EngineApi;
    const { storage } = make();
    const store = new ProjectStore({
      storage,
      newId: () => "ABCD1234",
      now: () => Date.UTC(2026, 10, 14, 9, 5, 7),
      chatFolders: folders,
    });
    await store.init();
    store.setChatDir("C:/data/chats");
    const shared = store.state().chatDir ?? "";
    for (const x of sessions) x.directory = shared; // sesi lama berbagi folder Chat (bentuk path sudah dinormalisasi store)
    store.setApi(api);
    await flush();
    const a = await store.createSession(CHAT_KEY);
    const b = await store.createSession(CHAT_KEY);
    expect(a.directory).toMatch(/^C:\/data\/chats\/chat-\d{8}-\d{6}-abcd12$/);
    expect(a.directory).toBe(b.directory); // nama dari id/jam yang sama di tes; folder dibuat tiap kali
    expect(created).toHaveLength(2);
    await store.refresh(CHAT_KEY);
    // sesi lama (folder bersama) + sesi di folder masing-masing, tanpa duplikat
    expect(
      store
        .state()
        .sessions[CHAT_KEY]?.items.map((s) => s.id)
        .sort(),
    ).toEqual(["lama", "s1", "s2"]);
    await store.renameSession(CHAT_KEY, "s1", "Judul baru"); // memakai folder milik sesi
    await store.deleteSession(CHAT_KEY, "s1");
    expect(removed).toEqual([a.directory]);
    await store.deleteSession(CHAT_KEY, "lama"); // folder bersama tidak pernah dibersihkan
    expect(removed).toEqual([a.directory]);
  });

  test("nama folder sesi Chat: berurut waktu, aman, dan memangkas akhiran liar", () => {
    expect(chatFolderName(new Date(2026, 10, 14, 9, 5, 7), "AB-cd_12ef99")).toBe("chat-20261114-090507-abcd12");
    expect(chatFolderName(new Date(2026, 0, 2, 3, 4, 5), "!!!")).toBe("chat-20260102-030405-x");
  });

  test("antrean pesan: antre saat sibuk, urutan bisa diubah/diedit/dihapus, dikirim berurutan saat idle", async () => {
    const { engine, store } = await setup();
    const busy = () => engine.emit(DIR, { type: "session.status", sessionID: "s1", status: { type: "busy" } });
    const idle = () => engine.emit(DIR, { type: "session.status", sessionID: "s1", status: { type: "idle" } });
    busy();
    const a = store.enqueue("s1", "  satu ");
    const b = store.enqueue("s1", "dua");
    const c = store.enqueue("s1", "tiga");
    expect(() => store.enqueue("s1", "   ")).toThrow();
    expect(store.queueOf("s1").map((q) => q.text)).toEqual(["satu", "dua", "tiga"]);
    store.moveQueued("s1", c, 0);
    store.editQueued("s1", a, "satu (diedit)");
    store.removeQueued("s1", b);
    expect(store.queueOf("s1").map((q) => q.text)).toEqual(["tiga", "satu (diedit)"]);
    expect(engine.prompts).toHaveLength(0); // sibuk: belum ada yang dikirim
    idle();
    await flush();
    expect(engine.prompts.map((p) => p.text)).toEqual(["tiga"]); // satu per giliran
    expect(store.queueOf("s1").map((q) => q.text)).toEqual(["satu (diedit)"]);
    busy();
    idle();
    await flush();
    expect(engine.prompts.map((p) => p.text)).toEqual(["tiga", "satu (diedit)"]);
    expect(store.queueOf("s1")).toEqual([]);
  });

  test("antrean: Stop menjeda (tidak dikirim diam-diam), lanjutkan mengirim; kirim langsung menghentikan giliran lalu mengirim item itu", async () => {
    const { engine, store } = await setup();
    const busy = () => engine.emit(DIR, { type: "session.status", sessionID: "s1", status: { type: "busy" } });
    const idle = () => engine.emit(DIR, { type: "session.status", sessionID: "s1", status: { type: "idle" } });
    busy();
    store.enqueue("s1", "pertama");
    const second = store.enqueue("s1", "kedua");
    await store.abort("s1");
    idle();
    await flush();
    expect(store.state().queuePaused.s1).toBe(true);
    expect(engine.prompts).toHaveLength(0);
    store.resumeQueue("s1");
    await flush();
    expect(engine.prompts.map((p) => p.text)).toEqual(["pertama"]);

    busy();
    engine.aborted.length = 0;
    await store.sendQueuedNow("s1", second);
    expect(engine.aborted).toEqual(["s1"]);
    expect(store.queueOf("s1").map((q) => q.text)).toEqual(["kedua"]); // menunggu sesi idle
    idle();
    await flush();
    expect(engine.prompts.map((p) => p.text)).toEqual(["pertama", "kedua"]);
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

import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { createFakeEngine } from "../engine/fake-engine";
import { applyEvent, initialChatState, loadMessages, messageText } from "./reducer";
import { type ChatPrefs, ChatStore } from "./store";
import { type ChatMessage, type EngineEvent, type ToolPart, toEngineEvent, toMessage, toPart } from "./types";

const realEvents = (): EngineEvent[] =>
  readFileSync(new URL("./fixtures/real-turn.ndjson", import.meta.url), "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => toEngineEvent(JSON.parse(l)))
    .filter((e): e is EngineEvent => e !== null);

const SID = "ses_e2abb4b5cffe6amKiz5jXnDC7t";
const replay = (events: EngineEvent[], from = initialChatState(SID)) => events.reduce(applyEvent, from);

describe("rekaman SSE asli (satu giliran dengan tool call)", () => {
  const final = replay(realEvents());

  test("urutan pesan: user lalu langkah-langkah asisten (tool call, lalu jawaban)", () => {
    const roles = final.messages.map((m) => m.info.role);
    expect(roles[0]).toBe("user");
    expect(roles.slice(1).every((r) => r === "assistant")).toBe(true);
    expect(roles.length).toBeGreaterThanOrEqual(3);
    expect(final.messages[0] && messageText(final.messages[0])).toContain("catatan.txt");
  });

  test("tool call selesai dengan input/output dari engine", () => {
    const tool = final.messages.flatMap((m) => m.parts).find((p): p is ToolPart => p.type === "tool");
    expect(tool?.tool).toBe("glob");
    expect(tool?.state.status).toBe("completed");
    expect(tool?.state.input).toEqual({ pattern: "**/catatan.txt" });
    expect(tool?.state.output).toContain("catatan.txt");
  });

  test("teks jawaban tersusun dari delta streaming", () => {
    const last = final.messages.at(-1);
    expect(last && messageText(last)).toBe('Isi file catatan.txt hanyalah satu baris yang berbunyi "halo dunia".');
  });

  test("status akhir idle, tanpa sisa delta menggantung, tanpa galat", () => {
    expect(final.status).toEqual({ type: "idle" });
    expect(final.pending).toEqual({});
    expect(final.error).toBeNull();
    expect(final.messages.every((m) => m.info.time.created > 0)).toBe(true);
  });

  test("pesan asisten pertama selesai dengan finish=tool-calls dan token tercatat", () => {
    const first = final.messages[1];
    expect(first?.info.finish).toBe("tool-calls");
    expect(first?.info.tokens?.total).toBeGreaterThan(0);
  });

  test("memutar ulang event yang sama tidak menggandakan part (idempoten untuk part.updated)", () => {
    const events = realEvents().filter((e) => e.type !== "message.part.delta");
    const once = replay(events);
    const twice = replay(events, once);
    expect(twice.messages.map((m) => m.parts.length)).toEqual(once.messages.map((m) => m.parts.length));
  });
});

describe("reducer", () => {
  const msgInfo = (id: string, role: "user" | "assistant" = "assistant") =>
    ({ type: "message.updated", info: { id, sessionID: "s", role, time: { created: 1 } } }) as EngineEvent;
  const textPart = (id: string, messageID: string, text = ""): EngineEvent => ({
    type: "message.part.updated",
    part: { id, messageID, sessionID: "s", type: "text", text, synthetic: false },
  });
  const delta = (partID: string, d: string, messageID = "m1"): EngineEvent => ({
    type: "message.part.delta",
    sessionID: "s",
    messageID,
    partID,
    field: "text",
    delta: d,
  });
  const base = initialChatState("s");

  test("delta yang tiba sebelum part-nya ditahan lalu diterapkan saat part muncul", () => {
    const s = replay([msgInfo("m1"), delta("p1", "ha"), delta("p1", "lo"), textPart("p1", "m1", "")], base);
    expect(s.pending).toEqual({});
    expect(s.messages[0]?.parts[0]).toMatchObject({ text: "halo" });
  });

  test("delta yang sudah tercakup teks part tidak digandakan", () => {
    const s = replay([msgInfo("m1"), delta("p1", "halo"), textPart("p1", "m1", "halo")], base);
    expect(s.messages[0]?.parts[0]).toMatchObject({ text: "halo" });
  });

  test("part tiba sebelum message.updated: pesan sementara dibuat lalu infonya dilengkapi", () => {
    const s = replay([textPart("p1", "m1", "x"), msgInfo("m1")], base);
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]?.info.time.created).toBe(1);
    expect(s.messages[0]?.parts).toHaveLength(1);
  });

  test("event sesi lain diabaikan dan state tidak berubah (referensi sama)", () => {
    const other = {
      ...msgInfo("m9"),
      info: { id: "m9", sessionID: "lain", role: "user", time: { created: 1 } },
    } as EngineEvent;
    expect(applyEvent(base, other)).toBe(base);
    expect(applyEvent(base, { type: "session.status", sessionID: "lain", status: { type: "busy" } })).toBe(base);
  });

  test("status sama tidak membuat state baru; retry membawa pesan", () => {
    const busy = applyEvent(base, { type: "session.status", sessionID: "s", status: { type: "busy" } });
    expect(applyEvent(busy, { type: "session.status", sessionID: "s", status: { type: "busy" } })).toBe(busy);
    const retry = applyEvent(busy, {
      type: "session.status",
      sessionID: "s",
      status: { type: "retry", attempt: 2, message: "rate limit" },
    });
    expect(retry.status).toEqual({ type: "retry", attempt: 2, message: "rate limit" });
  });

  test("izin: diminta lalu dijawab; permintaan ganda tidak digandakan", () => {
    const req = { id: "per_1", sessionID: "s", permission: "edit", patterns: ["a.txt"], metadata: {}, always: [] };
    const asked = applyEvent(base, { type: "permission.asked", request: req });
    expect(applyEvent(asked, { type: "permission.asked", request: req })).toBe(asked);
    expect(asked.permissions).toHaveLength(1);
    const replied = applyEvent(asked, {
      type: "permission.replied",
      sessionID: "s",
      requestID: "per_1",
      reply: "once",
    });
    expect(replied.permissions).toEqual([]);
  });

  test("part dan pesan bisa dihapus", () => {
    const s = replay(
      [
        msgInfo("m1"),
        textPart("p1", "m1", "a"),
        { type: "message.part.removed", sessionID: "s", messageID: "m1", partID: "p1" },
      ],
      base,
    );
    expect(s.messages[0]?.parts).toEqual([]);
    expect(applyEvent(s, { type: "message.removed", sessionID: "s", messageID: "m1" }).messages).toEqual([]);
  });

  test("loadMessages: pesan asisten tanpa completed berarti sesi masih berjalan", () => {
    const running: ChatMessage = {
      info: { id: "m2", sessionID: "s", role: "assistant", time: { created: 2 } },
      parts: [],
    };
    expect(loadMessages(base, [running], []).status).toEqual({ type: "busy" });
    const done: ChatMessage = {
      info: { id: "m2", sessionID: "s", role: "assistant", time: { created: 2, completed: 3 } },
      parts: [],
    };
    expect(loadMessages(base, [done], []).status).toEqual({ type: "idle" });
  });

  test("galat sesi tercatat", () => {
    const s = applyEvent(base, { type: "session.error", sessionID: "s", error: { name: "ApiError", message: "401" } });
    expect(s.error).toBe("401");
  });
});

describe("normalisasi defensif", () => {
  test("input sampah → null, bukan lempar", () => {
    for (const v of [
      null,
      1,
      "x",
      [],
      {},
      { type: "message.updated" },
      { type: "message.part.delta", properties: {} },
    ]) {
      expect(toEngineEvent(v)).toBeNull();
    }
    expect(toMessage({ info: { id: 1 } })).toBeNull();
    expect(toPart({ type: "text" })).toBeNull();
  });
  test("part tak dikenal menjadi 'other' (tidak merusak render)", () => {
    expect(toPart({ id: "p", messageID: "m", sessionID: "s", type: "snapshot" })).toMatchObject({
      type: "other",
      rawType: "snapshot",
    });
  });
  test("galat pesan dibaca dari data.message", () => {
    const info = toMessage({
      info: {
        id: "m",
        sessionID: "s",
        role: "assistant",
        time: { created: 1 },
        error: { name: "ApiError", data: { message: "kunci salah" } },
      },
      parts: [],
    });
    expect(info?.info.error).toEqual({ name: "ApiError", message: "kunci salah" });
  });
});

describe("ChatStore", () => {
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
  const flush = () => new Promise((r) => setTimeout(r, 0));
  const DIR = "C:\\work\\alpha";

  async function setup(prefs = prefsOf()) {
    const engine = createFakeEngine();
    engine.models = [
      { providerID: "p", providerName: "P", modelID: "m1", name: "M1", variants: ["low", "high"] },
      { providerID: "p", providerName: "P", modelID: "m2", name: "M2", variants: [] },
    ];
    const store = new ChatStore({ prefs });
    store.setApi(engine);
    await flush();
    return { engine, store, prefs };
  }

  test("pesan disematkan: per sesi, bisa dilepas, tersimpan dan dipulihkan, data rusak diabaikan", async () => {
    const a = await setup();
    a.store.toggleMessagePin("s1", "m1");
    a.store.toggleMessagePin("s1", "m2");
    a.store.toggleMessagePin("s2", "m9");
    expect(a.store.state().pinnedMessages).toEqual({ s1: ["m1", "m2"], s2: ["m9"] });
    a.store.toggleMessagePin("s1", "m1");
    a.store.toggleMessagePin("s2", "m9");
    expect(a.store.state().pinnedMessages).toEqual({ s1: ["m2"] });
    const b = await setup(a.prefs);
    expect(b.store.state().pinnedMessages).toEqual({ s1: ["m2"] });
    const c = await setup(prefsOf({ "zandor.pinnedMessages": "[1,2]" }));
    expect(c.store.state().pinnedMessages).toEqual({});
  });

  test("model terakhir digunakan: maks 3, terbaru dulu, tanpa duplikat, tersimpan dan dipulihkan", async () => {
    const a = await setup();
    const ref = (id: string) => ({ providerID: "p", modelID: id });
    expect(a.store.state().recent).toEqual([]);
    for (const id of ["m1", "m2", "m3", "m4", "m2"]) a.store.selectModel(ref(id));
    expect(a.store.state().recent).toEqual(["p/m2", "p/m4", "p/m3"]);
    const b = await setup(a.prefs);
    expect(b.store.state().recent).toEqual(["p/m2", "p/m4", "p/m3"]);
    const c = await setup(prefsOf({ "zandor.recentModels": "{rusak" }));
    expect(c.store.state().recent).toEqual([]);
  });

  test("model bawaan pengguna dipakai saat belum ada pilihan, mengalahkan pilihan lama, dan diabaikan bila tak ada", async () => {
    const mk = async (def: { providerID: string; modelID: string } | null, prefs = prefsOf()) => {
      const engine = createFakeEngine();
      engine.models = [
        { providerID: "p", providerName: "P", modelID: "m1", name: "M1", variants: [] },
        { providerID: "p", providerName: "P", modelID: "m2", name: "M2", variants: [] },
      ];
      const store = new ChatStore({ prefs, defaultModel: () => def });
      store.setApi(engine);
      await flush();
      return store;
    };
    expect((await mk({ providerID: "p", modelID: "m2" })).state().model?.modelID).toBe("m2");
    const old = prefsOf({ "zandor.model": JSON.stringify({ providerID: "p", modelID: "m1" }) });
    expect((await mk({ providerID: "p", modelID: "m2" }, old)).state().model?.modelID).toBe("m2");
    expect((await mk({ providerID: "x", modelID: "hilang" })).state().model?.modelID).toBe("m1");
  });

  test("model: bawaan = pertama; pilihan tersimpan dan dipulihkan; model hilang → kembali ke bawaan", async () => {
    const a = await setup();
    expect(a.store.state().model).toEqual({ providerID: "p", modelID: "m1" });
    a.store.selectModel({ providerID: "p", modelID: "m2" });
    const b = await setup(a.prefs);
    expect(b.store.state().model).toEqual({ providerID: "p", modelID: "m2" });
    const c = await setup(prefsOf({ "zandor.model": JSON.stringify({ providerID: "x", modelID: "hilang" }) }));
    expect(c.store.state().model).toEqual({ providerID: "p", modelID: "m1" });
    const d = await setup(prefsOf({ "zandor.model": "{rusak" }));
    expect(d.store.state().model).toEqual({ providerID: "p", modelID: "m1" });
  });

  test("open memuat riwayat dan menampilkan balasan streaming dari event", async () => {
    const { engine, store } = await setup();
    store.open(DIR, "s1");
    await flush();
    expect(store.state().sessions.s1?.loaded).toBe(true);
    const part = (text: string): EngineEvent => ({
      type: "message.part.updated",
      part: { id: "p1", messageID: "m1", sessionID: "s1", type: "text", text, synthetic: false },
    });
    engine.emit(DIR, {
      type: "message.updated",
      info: { id: "m1", sessionID: "s1", role: "assistant", time: { created: 1 } },
    });
    engine.emit(DIR, part(""));
    engine.emit(DIR, {
      type: "message.part.delta",
      sessionID: "s1",
      messageID: "m1",
      partID: "p1",
      field: "text",
      delta: "Halo",
    });
    expect(store.state().sessions.s1?.messages[0]?.parts[0]).toMatchObject({ text: "Halo" });
  });

  test("send mengirim model terpilih; ditolak saat kosong, tanpa model, atau sesi sedang berjalan", async () => {
    const { engine, store } = await setup();
    store.open(DIR, "s1");
    await flush();
    await expect(store.send("s1", "   ")).rejects.toThrow(/kosong/);
    await store.send("s1", "  halo  ");
    expect(engine.prompts).toEqual([
      { directory: DIR, sessionId: "s1", text: "halo", model: { providerID: "p", modelID: "m1" } },
    ]);
    engine.emit(DIR, { type: "session.status", sessionID: "s1", status: { type: "busy" } });
    await expect(store.send("s1", "lagi")).rejects.toThrow(/berjalan/);
    const empty = createFakeEngine();
    const s2 = new ChatStore({ prefs: prefsOf() });
    s2.setApi(empty);
    await flush();
    s2.open(DIR, "x");
    await expect(s2.send("x", "halo")).rejects.toThrow(/model/);
  });

  test("galat saat kirim dicatat di sesi dan dilempar; prompt baru menghapus galat lama", async () => {
    const { engine, store } = await setup();
    store.open(DIR, "s1");
    await flush();
    engine.failWith = "provider mati";
    await expect(store.send("s1", "halo")).rejects.toThrow("provider mati");
    expect(store.state().sessions.s1?.error).toMatch(/provider mati/);
    expect(store.state().sessions.s1?.sending).toBe(false);
    engine.failWith = null;
    await store.send("s1", "coba lagi");
    expect(store.state().sessions.s1?.error).toBeNull();
  });

  test("abort dan balasan izin diteruskan ke engine; izin hilang dari state", async () => {
    const { engine, store } = await setup();
    store.open(DIR, "s1");
    await flush();
    engine.emit(DIR, {
      type: "permission.asked",
      request: { id: "per_1", sessionID: "s1", permission: "edit", patterns: ["a"], metadata: {}, always: [] },
    });
    expect(store.state().sessions.s1?.permissions).toHaveLength(1);
    await store.reply("s1", "per_1", "once");
    expect(engine.replies).toEqual([{ requestId: "per_1", reply: "once" }]);
    expect(store.state().sessions.s1?.permissions).toEqual([]);
    await store.abort("s1");
    expect(engine.aborted).toEqual(["s1"]);
  });

  test("koneksi SSE putus lalu tersambung → riwayat dimuat ulang (event terlewat terkejar)", async () => {
    const { engine, store } = await setup();
    store.open(DIR, "s1");
    await flush();
    const before = engine.calls.filter((c) => c === "messages:s1").length;
    engine.messages.set("s1", [
      { info: { id: "m1", sessionID: "s1", role: "assistant", time: { created: 1, completed: 2 } }, parts: [] },
    ]);
    engine.reconnect(DIR);
    await flush();
    expect(engine.calls.filter((c) => c === "messages:s1").length).toBeGreaterThan(before);
    expect(store.state().sessions.s1?.messages).toHaveLength(1);
    expect(store.state().sessions.s1?.reconnecting).toBe(false);
  });

  test("event pesan yang tiba saat riwayat dimuat memicu muat ulang, bukan hilang", async () => {
    const { engine, store } = await setup();
    let release: () => void = () => {};
    const original = engine.listMessages.bind(engine);
    let calls = 0;
    engine.listMessages = async (d, s) => {
      calls++;
      if (calls === 1)
        await new Promise<void>((r) => {
          release = r;
        });
      return original(d, s);
    };
    store.open(DIR, "s1");
    await flush();
    engine.messages.set("s1", [{ info: { id: "m1", sessionID: "s1", role: "user", time: { created: 1 } }, parts: [] }]);
    engine.emit(DIR, {
      type: "message.updated",
      info: { id: "m1", sessionID: "s1", role: "user", time: { created: 1 } },
    });
    release();
    await flush();
    await flush();
    expect(calls).toBeGreaterThanOrEqual(2);
    expect(store.state().sessions.s1?.messages.map((m) => m.info.id)).toEqual(["m1"]);
  });

  test("close melepas sesi dan menghentikan langganan bila direktori tak dipakai", async () => {
    const { engine, store } = await setup();
    store.open(DIR, "s1");
    await flush();
    expect(engine.subscribers.get(DIR)?.size).toBe(1);
    store.close("s1");
    expect(engine.subscribers.get(DIR)?.size).toBe(0);
    expect(store.state().sessions.s1).toBeUndefined();
  });

  test("aliran event dibatasi (batas koneksi WebView): folder terlama yang idle dilepas, dipasang lagi saat dibuka", async () => {
    const { engine, store } = await setup();
    const dirs = ["/a", "/b", "/c", "/d", "/e", "/f", "/g"];
    for (const [i, d] of dirs.entries()) {
      store.open(d, `s${i}`);
      await flush();
    }
    const open = () => dirs.filter((d) => (engine.subscribers.get(d)?.size ?? 0) > 0);
    expect(open().length).toBeLessThanOrEqual(4);
    expect(open()).toContain("/g"); // yang terbaru tetap terhubung
    expect(open()).not.toContain("/a"); // yang terlama dilepas
    store.open("/a", "s0"); // dibuka lagi → berlangganan lagi (dan yang terlama berikutnya dilepas)
    await flush();
    expect(engine.subscribers.get("/a")?.size).toBe(1);
    expect(open().length).toBeLessThanOrEqual(4);
    store.resume("s1"); // sesi terbuka yang foldernya dilepas: dipasang lagi tanpa open()
    await flush();
    expect(engine.subscribers.get("/b")?.size).toBe(1);
  });

  test("engine mati → langganan dilepas; engine kembali → sesi terbuka berlangganan lagi", async () => {
    const { engine, store } = await setup();
    store.open(DIR, "s1");
    await flush();
    store.setApi(null);
    expect(engine.subscribers.get(DIR)?.size).toBe(0);
    store.setApi(engine);
    await flush();
    expect(engine.subscribers.get(DIR)?.size).toBe(1);
  });
});

import { describe, expect, test } from "vitest";
import { createFakeEngine } from "../engine/fake-engine";
import { applyEvent, initialChatState } from "./reducer";
import { ChatStore } from "./store";
import { type EngineEvent, type QuestionRequest, toEngineEvent, toQuestion, toTodos } from "./types";

const flush = () => new Promise((r) => setTimeout(r, 0));
const DIR = "C:\\work\\alpha";

const RAW = {
  id: "que_1",
  sessionID: "s1",
  questions: [
    {
      question: "Framework mana yang dipakai?",
      header: "Framework",
      options: [
        { label: "React", description: "Pustaka UI" },
        { label: "Svelte", description: "Kompiler UI" },
      ],
    },
    {
      question: "Fitur tambahan?",
      header: "Fitur",
      multiple: true,
      custom: false,
      options: [{ label: "Tes", description: "" }],
    },
  ],
  tool: { messageID: "m1", callID: "c1" },
};

describe("toQuestion / toTodos (normalisasi defensif)", () => {
  test("permintaan valid: bawaan multiple=false dan custom=true", () => {
    const q = toQuestion(RAW) as QuestionRequest;
    expect(q.id).toBe("que_1");
    expect(q.questions[0]).toMatchObject({ header: "Framework", multiple: false, custom: true });
    expect(q.questions[0]?.options.map((o) => o.label)).toEqual(["React", "Svelte"]);
    expect(q.questions[1]).toMatchObject({ multiple: true, custom: false });
    expect(q.tool).toEqual({ messageID: "m1", callID: "c1" });
  });

  test("data rusak ditolak atau dibersihkan", () => {
    expect(toQuestion(null)).toBeNull();
    expect(toQuestion({ questions: [] })).toBeNull();
    // Isi rusak tetap jadi kartu (teks kosong) agar pengguna bisa melewatinya; engine sedang menunggu.
    for (const questions of [[], [{ question: "" }, 5, null], "bukan json", undefined])
      expect(toQuestion({ id: "x", sessionID: "s", questions })?.questions).toEqual([
        { question: "", header: "", options: [], multiple: false, custom: true },
      ]);
    expect(
      toQuestion({ id: "x", sessionID: "s", questions: JSON.stringify([{ question: "Lanjut?" }]) })?.questions[0]
        ?.question,
    ).toBe("Lanjut?");
    const q = toQuestion({
      id: "x",
      sessionID: "s",
      questions: [{ question: "?", options: [{ label: "" }, { label: "ok" }, 3] }],
    });
    expect(q?.questions[0]?.options).toEqual([{ label: "ok", description: "" }]);
    expect(
      toQuestion({ id: "x", sessionID: "s", questions: [{ question: "?", header: "h".repeat(200) }] })?.questions[0]
        ?.header,
    ).toHaveLength(60);
  });

  test("todo: status/prioritas tidak dikenal dinormalkan; non-array → kosong", () => {
    expect(toTodos("x")).toEqual([]);
    expect(
      toTodos([
        { content: "a", status: "in_progress", priority: "high" },
        { content: "b", status: "aneh", priority: "?" },
        { content: "" },
        7,
      ]),
    ).toEqual([
      { content: "a", status: "in_progress", priority: "high" },
      { content: "b", status: "pending", priority: "medium" },
    ]);
  });
});

describe("event SSE → reducer", () => {
  test("question.asked → state; question.replied/rejected menutup; todo.updated mengganti daftar", () => {
    const asked = toEngineEvent({ type: "question.asked", properties: RAW });
    expect(asked?.type).toBe("question.asked");
    let s = initialChatState("s1");
    s = applyEvent(s, asked as EngineEvent);
    expect(s.questions).toHaveLength(1);
    expect(applyEvent(s, asked as EngineEvent)).toBe(s); // idempoten
    const other: EngineEvent = {
      type: "question.asked",
      request: { ...(asked as { request: QuestionRequest }).request, sessionID: "lain" },
    };
    expect(applyEvent(s, other)).toBe(s);

    const closed = toEngineEvent({
      type: "question.replied",
      properties: { sessionID: "s1", requestID: "que_1", answers: [["React"]] },
    });
    expect(closed).toEqual({ type: "question.closed", sessionID: "s1", requestID: "que_1", answered: true });
    expect(applyEvent(s, closed as EngineEvent).questions).toEqual([]);
    const rejected = toEngineEvent({ type: "question.rejected", properties: { sessionID: "s1", requestID: "que_1" } });
    expect(rejected).toMatchObject({ answered: false });
    expect(applyEvent(s, rejected as EngineEvent).questions).toEqual([]);

    const todo = toEngineEvent({
      type: "todo.updated",
      properties: { sessionID: "s1", todos: [{ content: "x", status: "completed", priority: "low" }] },
    });
    expect(applyEvent(s, todo as EngineEvent).todos).toEqual([{ content: "x", status: "completed", priority: "low" }]);
    expect(applyEvent(s, { type: "todo.updated", sessionID: "lain", todos: [] })).toBe(s);
  });
});

describe("ChatStore: pertanyaan dan todo", () => {
  async function setup() {
    const engine = createFakeEngine();
    engine.models = [{ providerID: "p", providerName: "P", modelID: "m1", name: "M1", variants: [] }];
    engine.questions = [
      toQuestion(RAW) as QuestionRequest,
      toQuestion({ ...RAW, id: "que_lain", sessionID: "s2" }) as QuestionRequest,
    ];
    engine.todos.set("s1", [{ content: "tulis tes", status: "in_progress", priority: "high" }]);
    const store = new ChatStore({ prefs: { get: () => null, set: () => {} } });
    store.setApi(engine);
    await flush();
    store.open(DIR, "s1");
    await flush();
    return { engine, store };
  }

  test("muat awal: hanya pertanyaan milik sesi dan todo-nya", async () => {
    const { store } = await setup();
    const s = store.state().sessions.s1;
    expect(s?.questions.map((q) => q.id)).toEqual(["que_1"]);
    expect(s?.todos).toEqual([{ content: "tulis tes", status: "in_progress", priority: "high" }]);
  });

  test("answerQuestion mengirim jawaban ke engine dan menghapus kartu; rejectQuestion juga", async () => {
    const { engine, store } = await setup();
    await store.answerQuestion("s1", "que_1", [["React"], ["Tes", "lainnya"]]);
    expect(engine.questionReplies).toEqual([{ requestId: "que_1", answers: [["React"], ["Tes", "lainnya"]] }]);
    expect(store.state().sessions.s1?.questions).toEqual([]);

    engine.emit(DIR, { type: "question.asked", request: toQuestion({ ...RAW, id: "que_2" }) as QuestionRequest });
    await flush();
    expect(store.state().sessions.s1?.questions.map((q) => q.id)).toEqual(["que_2"]);
    await store.rejectQuestion("s1", "que_2");
    expect(engine.questionReplies.at(-1)).toEqual({ requestId: "que_2", answers: null });
    expect(store.state().sessions.s1?.questions).toEqual([]);
  });

  test("abort menolak pertanyaan tertunda milik sesi (yang tak tampil pun) sebelum abort", async () => {
    const { engine, store } = await setup();
    engine.questions.push(toQuestion({ ...RAW, id: "que_tersembunyi" }) as QuestionRequest);
    await store.abort("s1");
    expect(engine.questionReplies.map((r) => r.requestId).sort()).toEqual(["que_1", "que_tersembunyi"]);
    expect(engine.aborted).toEqual(["s1"]);
    expect(store.state().sessions.s1?.questions).toEqual([]);
  });

  test("galat engine pada jawaban dilempar dan kartu tetap ada (bisa dicoba lagi)", async () => {
    const { engine, store } = await setup();
    engine.failWith = "engine mati";
    await expect(store.answerQuestion("s1", "que_1", [["React"], []])).rejects.toThrow(/engine mati/);
    expect(store.state().sessions.s1?.questions).toHaveLength(1);
  });
});

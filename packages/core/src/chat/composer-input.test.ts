import { describe, expect, test } from "vitest";
import { createFakeEngine } from "../engine/fake-engine";
import {
  activeToken,
  fileUrl,
  isBuiltin,
  mentionAttachment,
  parseSlash,
  rankByQuery,
  replaceToken,
} from "./composer-input";
import { ChatStore } from "./store";

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("token composer", () => {
  test("slash hanya di awal dan sebelum spasi", () => {
    expect(activeToken("/co", 3)).toEqual({ kind: "slash", query: "co", start: 0, end: 3 });
    expect(activeToken("/", 1)?.query).toBe("");
    expect(activeToken("/btw halo", 9)).toBeNull();
    expect(activeToken("a /x", 4)).toBeNull();
  });
  test("mention setelah spasi atau di awal", () => {
    expect(activeToken("lihat @src/ma", 13)).toEqual({ kind: "mention", query: "src/ma", start: 6, end: 13 });
    expect(activeToken("@", 1)).toEqual({ kind: "mention", query: "", start: 0, end: 1 });
    expect(activeToken("email a@b", 9)).toBeNull();
    expect(activeToken("@a b", 4)).toBeNull();
  });
  test("ganti token menambah spasi dan memindah kursor", () => {
    const tok = activeToken("cek @ma lagi", 7);
    expect(tok).not.toBeNull();
    if (!tok) return;
    expect(replaceToken("cek @ma lagi", tok, "@src/main.ts")).toEqual({ text: "cek @src/main.ts lagi", caret: 17 });
    const end = activeToken("/co", 3);
    if (end) expect(replaceToken("/co", end, "/compact")).toEqual({ text: "/compact ", caret: 9 });
  });
});

describe("parseSlash", () => {
  test("nama dan argumen", () => {
    expect(parseSlash("/btw  apa itu X?\nbaris 2")).toEqual({ name: "btw", args: "apa itu X?\nbaris 2" });
    expect(parseSlash("/compact")).toEqual({ name: "compact", args: "" });
    expect(parseSlash("/mcp:prompt arg")).toEqual({ name: "mcp:prompt", args: "arg" });
    expect(parseSlash("bukan")).toBeNull();
    expect(parseSlash("// komentar")).toBeNull();
    expect(isBuiltin("btw")).toBe(true);
    expect(isBuiltin("init")).toBe(false);
  });
  test("ranking awalan sebelum mengandung", () => {
    expect(rankByQuery(["redo", "undo", "help"], "do", (x) => x)).toEqual(["redo", "undo"]);
    expect(rankByQuery(["undo", "redo", "help"], "un", (x) => x)).toEqual(["undo"]);
  });
});

describe("lampiran mention", () => {
  test("jalur Windows menjadi file URL", () => {
    expect(fileUrl("C:\\a b\\x.ts")).toBe("file:///C:/a%20b/x.ts");
    expect(mentionAttachment("C:\\proj\\", "src/app.ts")).toEqual({
      mime: "text/plain",
      url: "file:///C:/proj/src/app.ts",
      filename: "src/app.ts",
    });
    expect(mentionAttachment("/home/u/p", "img/a.PNG").mime).toBe("image/png");
  });
});

describe("aksi store composer", () => {
  async function setup() {
    const engine = createFakeEngine();
    engine.models = [{ providerID: "p", providerName: "P", modelID: "m1", name: "M1", variants: [] }];
    const store = new ChatStore({ prefs: { get: () => null, set: () => {} } });
    store.setApi(engine);
    await flush();
    store.open("D", "s1");
    await flush();
    return { engine, store };
  }
  test("runCommand meneruskan perintah dan model", async () => {
    const { engine, store } = await setup();
    await store.runCommand("s1", "init", "cepat");
    expect(engine.commandRuns).toEqual([{ sessionId: "s1", command: "init", arguments: "cepat" }]);
    expect(store.state().sessions.s1?.sending).toBe(false);
  });
  test("compact memanggil summarize", async () => {
    const { engine, store } = await setup();
    await store.compact("s1");
    expect(engine.summaries).toEqual(["s1"]);
  });
  test("aside mencabangkan seluruh riwayat lalu bertanya di cabang", async () => {
    const { engine, store } = await setup();
    const branch = await store.aside("s1", "omong-omong, apa itu X?");
    expect(engine.forks).toEqual([{ sessionId: "s1" }]);
    expect(engine.prompts.at(-1)).toMatchObject({ sessionId: branch.id, text: "omong-omong, apa itu X?" });
    expect(store.state().sessions[branch.id]).toBeDefined();
  });
  test("findFiles dan listCommands lewat engine", async () => {
    const { engine, store } = await setup();
    engine.findable.set("D", ["src/main.ts", "README.md"]);
    engine.commands = [{ name: "init", hints: [] }];
    expect(await store.findFiles("D", "main")).toEqual(["src/main.ts"]);
    expect(await store.listCommands("D")).toEqual([{ name: "init", hints: [] }]);
  });
});

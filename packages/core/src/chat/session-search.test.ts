import { describe, expect, test } from "vitest";
import type { SessionInfo } from "../engine/api";
import { messageText } from "./reducer";
import { rankResults, SessionSearch, searchSession, snippetAt } from "./session-search";
import type { ChatMessage } from "./types";

const msg = (id: string, role: "user" | "assistant", text: string, synthetic = false): ChatMessage =>
  ({
    info: { id, role, sessionID: "s", time: { created: 1 } },
    parts: [{ id: `${id}-p`, messageID: id, sessionID: "s", type: "text", text, synthetic }],
  }) as unknown as ChatMessage;

const sess = (id: string, title: string, updated = 1): SessionInfo => ({
  id,
  title,
  directory: "C:\\proj",
  time: { created: 1, updated },
});

describe("pencarian sesi", () => {
  test("messageText mengabaikan teks sintetis; snippet menandai kecocokan", () => {
    expect(messageText(msg("m", "user", "halo", true))).toBe("");
    const long = `${"a ".repeat(60)}KUNCI${" b".repeat(60)}`;
    const s = snippetAt(long, long.indexOf("KUNCI"), 5);
    expect(s.snippet.startsWith("…")).toBe(true);
    expect(s.snippet.endsWith("…")).toBe(true);
    expect(s.snippet.slice(s.match[0], s.match[1])).toBe("KUNCI");
  });

  test("searchSession: judul dan pesan, tidak peka huruf besar, satu hit per pesan", () => {
    const item = { key: "p", session: sess("s1", "Refaktor Login") };
    const r = searchSession(
      item,
      [msg("m1", "user", "login gagal, login lagi"), msg("m2", "assistant", "Login OK")],
      "LOGIN",
    );
    expect(r?.titleMatch).toBe(true);
    expect(r?.total).toBe(3);
    expect(r?.hits.map((h) => h.messageId)).toEqual(["m1", "m2"]);
    expect(searchSession(item, [], "tidak ada")).toBeNull();
    expect(searchSession(item, [], "  ")).toBeNull();
  });

  test("rankResults: judul dulu, lalu jumlah, lalu terbaru", () => {
    const mk = (id: string, titleMatch: boolean, total: number, updated: number) => ({
      key: "p",
      session: sess(id, id, updated),
      titleMatch,
      total,
      hits: [],
    });
    expect(
      rankResults([mk("a", false, 5, 9), mk("b", true, 1, 1), mk("c", false, 5, 10)]).map((r) => r.session.id),
    ).toEqual(["b", "c", "a"]);
  });

  test("SessionSearch memuat pesan, cache per updated, gagal dihitung, dibatalkan oleh pencarian baru", async () => {
    const calls: string[] = [];
    const api = {
      listMessages: async (_dir: string, id: string) => {
        calls.push(id);
        if (id === "rusak") throw new Error("x");
        return [msg(`${id}-m`, "assistant", `isi ${id} kata`)];
      },
    };
    const s = new SessionSearch(() => api, 2);
    const list = [
      { key: "p", session: sess("s1", "satu") },
      { key: "p", session: sess("s2", "dua") },
      { key: "c", session: sess("rusak", "rusak") },
    ];
    const updates: number[] = [];
    const r = await s.search(list, "kata", (_r, p) => updates.push(p.done));
    expect(r?.results.map((x) => x.session.id).sort()).toEqual(["s1", "s2"]);
    expect(r?.progress).toEqual({ done: 3, total: 3, failed: 1 });
    expect(updates).toHaveLength(3);
    calls.length = 0;
    await s.search(list, "satu");
    expect(calls).toEqual(["rusak"]); // yang berhasil ter-cache; yang gagal dicoba lagi
    list[0] = { key: "p", session: sess("s1", "satu", 2) };
    calls.length = 0;
    await s.search(list, "satu");
    expect(calls).toContain("s1"); // updated berubah → dimuat ulang

    const first = s.search(list, "isi");
    const second = s.search(list, "dua");
    expect(await first).toBeNull();
    expect((await second)?.results.map((x) => x.session.id)).toEqual(["s2"]);
  });
});

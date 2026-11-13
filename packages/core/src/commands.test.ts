import { describe, expect, test, vi } from "vitest";
import { type CommandDef, CommandRegistry, fuzzyMatch, searchCommands } from "./commands";

const cmd = (id: string, over: Partial<CommandDef> = {}): CommandDef => ({
  id,
  titleKey: `t.${id}`,
  group: "app",
  run: () => {},
  ...over,
});

describe("fuzzyMatch", () => {
  test("subsequence tanpa peduli huruf besar/kecil", () => {
    expect(fuzzyMatch("tpc", "Toggle Panel Chat")).not.toBeNull();
    expect(fuzzyMatch("TPC", "toggle panel chat")).not.toBeNull();
    expect(fuzzyMatch("xyz", "Toggle Panel Chat")).toBeNull();
  });
  test("urutan karakter wajib", () => {
    expect(fuzzyMatch("ba", "ab")).toBeNull();
  });
  test("query kosong cocok dengan skor 0", () => {
    expect(fuzzyMatch("  ", "apa saja")).toEqual({ score: 0, indices: [] });
  });
  test("awal kata dan berurutan mengungguli yang tersebar", () => {
    const awal = fuzzyMatch("pan", "Panel Chat");
    const tersebar = fuzzyMatch("pan", "Pengaturan Aplikasi Nama");
    expect(awal && tersebar && awal.score > tersebar.score).toBe(true);
  });
  test("indeks cocok dikembalikan untuk penyorotan", () => {
    expect(fuzzyMatch("pc", "Panel Chat")?.indices).toEqual([0, 6]);
  });
});

describe("CommandRegistry", () => {
  test("register, list, get, dan pelepas", () => {
    const r = new CommandRegistry();
    const off = r.register(cmd("a"));
    expect(r.list().map((c) => c.id)).toEqual(["a"]);
    expect(r.get("a")?.titleKey).toBe("t.a");
    off();
    expect(r.list()).toEqual([]);
  });
  test("id ganda ditolak", () => {
    const r = new CommandRegistry();
    r.register(cmd("a"));
    expect(() => r.register(cmd("a"))).toThrow(/ganda/);
  });
  test("subscribe dipanggil saat berubah; snapshot stabil bila tidak", () => {
    const r = new CommandRegistry();
    const l = vi.fn();
    r.subscribe(l);
    const empty = r.list();
    expect(r.list()).toBe(empty);
    const off = r.register(cmd("a"));
    expect(l).toHaveBeenCalledTimes(1);
    off();
    off();
    expect(l).toHaveBeenCalledTimes(2);
  });
  test("run: menjalankan, menolak yang tidak ada atau dinonaktifkan", async () => {
    const r = new CommandRegistry();
    const run = vi.fn();
    r.register(cmd("a", { run }));
    r.register(cmd("off", { enabled: () => false, run }));
    expect(await r.run("a")).toBe(true);
    expect(await r.run("off")).toBe(false);
    expect(await r.run("tidak-ada")).toBe(false);
    expect(run).toHaveBeenCalledTimes(1);
    expect(r.isEnabled("off")).toBe(false);
  });
  test("run menunggu perintah async selesai", async () => {
    const r = new CommandRegistry();
    let done = false;
    r.register(
      cmd("slow", {
        run: async () => {
          await new Promise((x) => setTimeout(x, 5));
          done = true;
        },
      }),
    );
    await r.run("slow");
    expect(done).toBe(true);
  });
});

describe("searchCommands", () => {
  const titles: Record<string, string> = {
    "t.chat": "Tampilkan panel chat",
    "t.docs": "Tampilkan panel dokumen",
    "t.zoom": "Perbesar UI",
    "t.theme": "Ganti tema",
  };
  const tr = (k: string) => titles[k] ?? k;
  const commands = [
    cmd("chat", { group: "view" }),
    cmd("docs", { group: "view" }),
    cmd("zoom", { group: "view", keywords: ["zoom in", "besar"] }),
    cmd("theme", { group: "app", enabled: () => false }),
  ];

  test("query kosong: semua, urut kelompok lalu judul", () => {
    expect(searchCommands(commands, "", tr).map((h) => h.command.id)).toEqual(["theme", "zoom", "chat", "docs"]);
  });
  test("menyaring dan mengurutkan menurut skor", () => {
    const hits = searchCommands(commands, "panel", tr).map((h) => h.command.id);
    expect(hits.sort()).toEqual(["chat", "docs"]);
  });
  test("cocok lewat kata kunci", () => {
    expect(searchCommands(commands, "zoom", tr).map((h) => h.command.id)).toEqual(["zoom"]);
  });
  test("perintah nonaktif tetap muncul dengan enabled=false", () => {
    const hit = searchCommands(commands, "tema", tr)[0];
    expect(hit?.command.id).toBe("theme");
    expect(hit?.enabled).toBe(false);
  });
  test("tanpa hasil → kosong", () => {
    expect(searchCommands(commands, "qqq", tr)).toEqual([]);
  });
});

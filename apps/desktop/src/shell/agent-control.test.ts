import { describe, expect, test, vi } from "vitest";
import { type ControlDeps, executeControl, imageFileName, safeRelPath } from "./agent-control";
import { toRequest } from "./control-host";

function deps(over: Partial<ControlDeps> = {}): ControlDeps & { calls: string[] } {
  const calls: string[] = [];
  return {
    calls,
    enabled: () => true,
    status: () => ({ ok: 1 }),
    listCommands: () => [
      { id: "view.toggleChat", title: "Chat", enabled: true },
      { id: "window.close", title: "Tutup", enabled: true },
    ],
    runCommand: async (id) => {
      calls.push(`run:${id}`);
      return id === "view.toggleChat";
    },
    panel: (p, s) => calls.push(`panel:${p}:${s}`),
    openTool: (t) => calls.push(`tool:${t}`),
    openUrl: (u) => calls.push(`url:${u}`),
    openFile: (p, r) => {
      calls.push(`file:${r}:${p}`);
      return true;
    },
    isRoute: (p) => p === "/" || p === "/settings/models",
    navigate: (p) => calls.push(`nav:${p}`),
    listTerminals: () => [],
    closeTerminal: async (id) => id === "pty-1",
    getSettings: () => ({ theme: "dark" }),
    setSetting: (k, v) => calls.push(`set:${k}=${String(v)}`),
    generateImage: async (_p, _s, _r, rel) => rel.replace(".png", ".jpg"),
    knownRoots: () => ["C:/proj"],
    ...over,
  };
}
const req = (action: string, args: Record<string, unknown> = {}) => ({
  id: "ctl-x",
  action,
  args,
  sessionID: "s",
  directory: "C:/proj/",
});

describe("kontrol aplikasi oleh agent (ADR-077)", () => {
  test("dimatikan pengguna → ditolak tanpa efek", async () => {
    const d = deps({ enabled: () => false });
    const r = await executeControl(req("panel", { panel: "chat" }), d);
    expect(r.ok).toBe(false);
    expect(d.calls).toEqual([]);
  });

  test("perintah palet: daftar menyaring perintah terlarang; run menolak window.close", async () => {
    const d = deps();
    const list = await executeControl(req("list_commands"), d);
    expect(list).toEqual({ ok: true, result: [{ id: "view.toggleChat", title: "Chat", enabled: true }] });
    expect((await executeControl(req("run_command", { id: "window.close" }), d)).ok).toBe(false);
    expect((await executeControl(req("run_command", { id: "view.toggleChat" }), d)).ok).toBe(true);
    expect((await executeControl(req("run_command", { id: "tidak.ada" }), d)).ok).toBe(false);
    expect(d.calls).toEqual(["run:view.toggleChat", "run:tidak.ada"]);
  });

  test("panel, alat, URL, halaman divalidasi", async () => {
    const d = deps();
    expect((await executeControl(req("panel", { panel: "docs", state: "open" }), d)).ok).toBe(true);
    expect((await executeControl(req("panel", { panel: "x" }), d)).ok).toBe(false);
    expect((await executeControl(req("open_tool", { tool: "terminal" }), d)).ok).toBe(true);
    expect((await executeControl(req("open_tool", { tool: "rm" }), d)).ok).toBe(false);
    expect((await executeControl(req("open_url", { url: "http://localhost:5173" }), d)).ok).toBe(true);
    expect((await executeControl(req("open_url", { url: "file:///C:/x" }), d)).ok).toBe(false);
    expect((await executeControl(req("open_url", { url: "https://a:b@x.com" }), d)).ok).toBe(false);
    expect((await executeControl(req("navigate", { page: "settings/models" }), d)).ok).toBe(true);
    expect((await executeControl(req("navigate", { page: "rahasia" }), d)).ok).toBe(false);
    expect(d.calls).toEqual(["panel:docs:open", "tool:terminal", "url:http://localhost:5173/", "nav:/settings/models"]);
  });

  test("pengaturan: divalidasi skema; agentControl dilindungi", async () => {
    const d = deps();
    expect((await executeControl(req("set_setting", { key: "theme", value: "light" }), d)).ok).toBe(true);
    expect((await executeControl(req("set_setting", { key: "theme", value: "ungu" }), d)).ok).toBe(false);
    expect((await executeControl(req("set_setting", { key: "agentControl", value: false }), d)).ok).toBe(false);
    expect((await executeControl(req("set_setting", { key: "kunci", value: 1 }), d)).ok).toBe(false);
    expect(d.calls).toEqual(["set:theme=light"]);
  });

  test("terminal: id wajib dan harus ada", async () => {
    const d = deps();
    expect((await executeControl(req("close_terminal", { id: "pty-1" }), d)).ok).toBe(true);
    expect((await executeControl(req("close_terminal", { id: "pty-9" }), d)).ok).toBe(false);
    expect((await executeControl(req("close_terminal"), d)).ok).toBe(false);
  });

  test("generate gambar: folder harus dikenal, path aman, hasil dibuka di Panel 3", async () => {
    const gen = vi.fn(async (_p: string, _s: string, _r: string, rel: string) => rel);
    const d = deps({ generateImage: gen });
    const r = await executeControl(req("generate_image", { prompt: "Kucing oranye!", path: "art/kucing.png" }), d);
    expect(r).toEqual({ ok: true, result: "Gambar disimpan: art/kucing.png (dibuka di Panel 3)." });
    expect(gen).toHaveBeenCalledWith("Kucing oranye!", "1024x1024", "C:/proj", "art/kucing.png");
    expect(d.calls).toEqual(["file:C:/proj:art/kucing.png"]);
    expect((await executeControl(req("generate_image", { prompt: "x", path: "../luar.png" }), d)).ok).toBe(false);
    expect((await executeControl(req("generate_image", { prompt: "x", size: "9x9" }), d)).ok).toBe(false);
    const asing = await executeControl({ ...req("generate_image", { prompt: "x" }), directory: "D:/lain" }, d);
    expect(asing.ok).toBe(false);
    const gagal = deps({
      generateImage: async () => {
        throw new Error("HTTP 401 (kunci API ditolak)");
      },
    });
    expect(await executeControl(req("generate_image", { prompt: "x" }), gagal)).toEqual({
      ok: false,
      error: "HTTP 401 (kunci API ditolak)",
    });
  });

  test("nama berkas gambar dan path relatif aman", () => {
    expect(imageFileName("Kucing Oranye di Bulan!", new Date(2026, 10, 17, 9, 5, 3))).toBe(
      "images/kucing-oranye-di-bulan-20261117-090503.png",
    );
    expect(imageFileName("!!!", new Date(2026, 0, 1, 0, 0, 0))).toBe("images/gambar-20260101-000000.png");
    expect(safeRelPath("./a/b.png")).toBe("a/b.png");
    expect(safeRelPath("a/../b.png")).toBeNull();
    expect(safeRelPath("C:/x.png")).toBeNull();
    expect(safeRelPath("/x.png")).toBeNull();
  });

  test("permintaan mentah divalidasi", () => {
    expect(toRequest(null)).toBeNull();
    expect(toRequest({ id: 1, action: "x" })).toBeNull();
    expect(toRequest({ id: "ctl-a", action: "status", args: [1] })).toEqual({
      id: "ctl-a",
      action: "status",
      args: {},
      sessionID: "",
      directory: "",
    });
  });
});

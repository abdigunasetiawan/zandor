import { describe, expect, test } from "vitest";
import { createFakePty } from "./fake-pty";
import { createPtyClient, mergeShells, pickShell, type ShellOption, shellId, toPtyInfo, toShell } from "./pty";
import { TerminalStore } from "./store";

describe("normalisasi dan pemilihan shell", () => {
  test("toPtyInfo/toShell menolak sampah dan memberi bawaan aman", () => {
    expect(toPtyInfo(null)).toBeNull();
    expect(toPtyInfo({})).toBeNull();
    expect(toPtyInfo({ id: "p1" })).toMatchObject({ id: "p1", title: "terminal", status: "running", pid: null });
    expect(toPtyInfo({ id: "p1", status: "exited", pid: 7 })).toMatchObject({ status: "exited", pid: 7 });
    expect(toShell({ name: "x" })).toBeNull();
    expect(toShell({ path: "C:\\x.exe" })).toMatchObject({ name: "C:\\x.exe", acceptable: true });
  });
  test("PowerShell diutamakan, lalu shell POSIX, lalu yang pertama layak; tanpa shell → null", () => {
    const sh = (name: string, acceptable = true): ShellOption => ({ path: `/${name}`, name, acceptable });
    expect(pickShell([sh("cmd"), sh("powershell")])?.name).toBe("powershell");
    expect(pickShell([sh("cmd"), sh("bash")])?.name).toBe("bash");
    expect(pickShell([sh("fish", false), sh("cmd")])?.name).toBe("cmd");
    expect(pickShell([sh("cmd", false)])).toBeNull();
    expect(pickShell([])).toBeNull();
  });
  test("deteksi lokal + engine digabung: urutan deteksi, exe sama tidak dobel, distro WSL terpisah, penolakan engine dihormati", () => {
    const engine: ShellOption[] = [
      { path: "C:\\Windows\\System32\\cmd.exe", name: "cmd", acceptable: true },
      { path: "C:\\Windows\\System32\\wsl.exe", name: "wsl", acceptable: true },
      { path: "C:\\tools\\nu.exe", name: "nu", acceptable: true },
      { path: "C:\\bad\\fish.exe", name: "fish", acceptable: false },
    ];
    const merged = mergeShells(engine, [
      { id: "powershell", name: "Windows PowerShell", path: "C:\\PS\\powershell.exe", args: ["-NoLogo"], icon: "AAA" },
      { id: "cmd", name: "Command Prompt", path: "c:\\windows\\system32\\CMD.EXE" },
      { id: "wsl", name: "WSL: Ubuntu", path: "C:\\Windows\\System32\\wsl.exe", args: ["-d", "Ubuntu"] },
      { id: "wsl", name: "WSL: Debian", path: "C:\\Windows\\System32\\wsl.exe", args: ["-d", "Debian"] },
      { id: "fish", name: "fish", path: "C:\\bad\\fish.exe" },
    ]);
    expect(merged.map((s) => s.name)).toEqual([
      "Windows PowerShell",
      "Command Prompt",
      "WSL: Ubuntu",
      "WSL: Debian",
      "fish",
      "nu", // hanya dikenal engine → di akhir
    ]);
    expect(merged[0]).toMatchObject({ kind: "powershell", icon: "AAA", args: ["-NoLogo"], acceptable: true });
    expect(merged.find((s) => s.name === "fish")?.acceptable).toBe(false);
    const ids = merged.map(shellId);
    expect(new Set(ids).size).toBe(ids.length); // distro WSL berbagi exe tetapi kuncinya berbeda
    expect(pickShell(merged)?.name).toBe("Windows PowerShell");
    expect(mergeShells(engine, [])).toEqual(engine); // tanpa deteksi (web/engine remote) → daftar engine apa adanya
  });
});

describe("klien /pty", () => {
  type Call = { method: string; url: string; headers: Record<string, string>; body?: unknown };
  function fakeFetch(responses: Array<{ status?: number; body?: unknown }>) {
    const calls: Call[] = [];
    const impl = (async (url: string, init: RequestInit) => {
      calls.push({
        method: init.method ?? "GET",
        url,
        headers: init.headers as Record<string, string>,
        ...(init.body ? { body: JSON.parse(String(init.body)) } : {}),
      });
      const r = responses.shift() ?? { body: null };
      const text = r.body === undefined || r.body === null ? "" : JSON.stringify(r.body);
      return { ok: (r.status ?? 200) < 400, status: r.status ?? 200, text: async () => text } as Response;
    }) as unknown as typeof fetch;
    return { impl, calls };
  }
  const info = { url: "http://127.0.0.1:4096", username: "opencode", password: "pw" };

  test("create mengirim cwd, directory, dan Basic auth; hasil dinormalisasi", async () => {
    const { impl, calls } = fakeFetch([
      { body: { id: "pty_1", title: "ps", command: "powershell.exe", cwd: "C:\\p", status: "running", pid: 5 } },
    ]);
    const c = createPtyClient(info, impl);
    const p = await c.create("C:\\p q", { command: "powershell.exe", args: ["-NoLogo"], title: "ps" });
    expect(p.id).toBe("pty_1");
    expect(calls[0]?.url).toBe("http://127.0.0.1:4096/pty?directory=C%3A%5Cp%20q");
    expect(calls[0]?.headers.authorization).toBe("Basic b3BlbmNvZGU6cHc=");
    expect(calls[0]?.body).toMatchObject({ command: "powershell.exe", cwd: "C:\\p q", args: ["-NoLogo"] });
  });

  test("galat HTTP dilempar dengan metode, path, dan status", async () => {
    const { impl } = fakeFetch([{ status: 500, body: { message: "rusak" } }]);
    await expect(createPtyClient(info, impl).list("C:\\p")).rejects.toThrow(/GET \/pty → HTTP 500/);
  });

  test("resize dan remove memakai id yang di-escape", async () => {
    const { impl, calls } = fakeFetch([{}, {}]);
    const c = createPtyClient(info, impl);
    await c.resize("C:\\p", "pty/1", 120, 30);
    await c.remove("C:\\p", "pty/1");
    expect(calls[0]).toMatchObject({ method: "PUT", body: { size: { cols: 120, rows: 30 } } });
    expect(calls[0]?.url).toContain("/pty/pty%2F1?");
    expect(calls[1]?.method).toBe("DELETE");
  });

  test("connect: tiket sekali pakai (header wajib), URL ws dengan kursor, bingkai teks/biner/kursor", async () => {
    const { impl, calls } = fakeFetch([{ body: { ticket: "t/1" } }]);
    const sockets: FakeWs[] = [];
    class FakeWs {
      binaryType = "";
      readyState = 1;
      onmessage: ((m: { data: unknown }) => void) | null = null;
      onclose: (() => void) | null = null;
      onopen: (() => void) | null = null;
      onerror: (() => void) | null = null;
      sent: string[] = [];
      constructor(public url: string) {
        sockets.push(this);
        queueMicrotask(() => this.onopen?.());
      }
      send(d: string) {
        this.sent.push(d);
      }
      close() {
        this.onclose?.();
      }
    }
    const c = createPtyClient(info, impl, FakeWs as unknown as new (u: string) => WebSocket);
    const sock = await c.connect("C:\\p", "pty_1", 42);
    expect(calls[0]?.headers["x-opencode-ticket"]).toBe("1");
    const ws = sockets[0] as FakeWs;
    expect(ws.url).toBe("ws://127.0.0.1:4096/pty/pty_1/connect?ticket=t%2F1&cursor=42&directory=C%3A%5Cp");
    const got: Array<string | Uint8Array> = [];
    const cursors: number[] = [];
    sock.onData((d) => got.push(d));
    sock.onCursor((n) => cursors.push(n));
    ws.onmessage?.({ data: "halo" });
    ws.onmessage?.({ data: new Uint8Array([104, 105]).buffer });
    ws.onmessage?.({ data: new Uint8Array([0, ...new TextEncoder().encode('{"cursor":99}')]).buffer });
    ws.onmessage?.({ data: new Uint8Array([0, 123]).buffer }); // kursor rusak → diabaikan
    expect(got[0]).toBe("halo");
    expect(new TextDecoder().decode(got[1] as Uint8Array)).toBe("hi");
    expect(cursors).toEqual([99]);
    sock.send("ls\r");
    expect(ws.sent).toEqual(["ls\r"]);
    let closed = 0;
    sock.onClose(() => closed++);
    sock.close();
    expect(closed).toBe(1);
  });

  test("tanpa tiket dari engine → galat jelas", async () => {
    const { impl } = fakeFetch([{ body: {} }]);
    await expect(createPtyClient(info, impl).connect("C:\\p", "x")).rejects.toThrow(/tiket/);
  });
});

describe("TerminalStore", () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));
  async function setup() {
    const pty = createFakePty();
    const store = new TerminalStore();
    store.setClient(pty);
    await flush();
    return { pty, store };
  }

  test("shell dimuat saat klien dipasang; create memakai PowerShell -NoLogo dengan cwd project", async () => {
    const { pty, store } = await setup();
    expect(store.state().shells).toHaveLength(2);
    const t = await store.create("p1", "C:\\work\\alpha");
    expect(pty.created[0]).toMatchObject({ directory: "C:\\work\\alpha", args: ["-NoLogo"], title: "powershell 1" });
    expect(t).toMatchObject({ key: "p1", cwd: "C:\\work\\alpha", status: "running" });
    expect(store.state().activeByKey.p1).toBe(t.id);
    const t2 = await store.create("p1", "C:\\work\\alpha", pty.shellList[1]?.path);
    expect(pty.created[1]?.args).toBeUndefined(); // cmd tanpa argumen
    expect(pty.created[1]?.title).toBe("cmd 2");
    expect(store.state().activeByKey.p1).toBe(t2.id);
    expect(store.tabsOf("p1")).toHaveLength(2);
    expect(store.tabsOf("lain")).toHaveLength(0);
  });

  test("shell hasil deteksi: argumen sendiri dipakai, exe dicatat di tab untuk ikon", async () => {
    const { pty, store } = await setup();
    const wsl = "C:\\Windows\\System32\\wsl.exe";
    store.setDetectedShells([
      { id: "wsl", name: "WSL: Ubuntu", path: wsl, args: ["-d", "Ubuntu"] },
      { id: "wsl", name: "WSL: Debian", path: wsl, args: ["-d", "Debian"] },
    ]);
    expect(store.state().shells.map((s) => s.name)).toEqual(["WSL: Ubuntu", "WSL: Debian", "powershell", "cmd"]);
    const debian = store.state().shells[1];
    if (!debian) throw new Error("tidak ada");
    const t = await store.create("p1", "C:\\w", shellId(debian));
    expect(pty.created[0]).toMatchObject({ command: wsl, args: ["-d", "Debian"], title: "WSL: Debian 1" });
    expect(t.command).toBe(wsl);
  });

  test("galat pembuatan dicatat dan dilempar; tanpa engine ditolak", async () => {
    const { pty, store } = await setup();
    pty.failCreate = "disk penuh";
    await expect(store.create("p1", "C:\\w")).rejects.toThrow("disk penuh");
    expect(store.state().error).toBe("disk penuh");
    expect(store.state().tabs).toEqual([]);
    const lone = new TerminalStore();
    await expect(lone.create("p", "C:\\w")).rejects.toThrow(/belum siap/);
  });

  test("close menghapus tab, memindah aktif ke tetangga, dan menghapus PTY di engine (galat engine diabaikan)", async () => {
    const { pty, store } = await setup();
    const a = await store.create("p1", "C:\\w");
    const b = await store.create("p1", "C:\\w");
    await store.close(b.id);
    expect(pty.removed).toEqual([b.id]);
    expect(store.state().activeByKey.p1).toBe(a.id);
    pty.remove = async () => {
      throw new Error("sudah hilang");
    };
    await store.close(a.id);
    expect(store.state().tabs).toEqual([]);
    expect(store.state().activeByKey.p1).toBeUndefined();
  });

  test("adopt: PTY hidup di engine kembali muncul setelah muat ulang; yang exited/sudah dikenal dilewati", async () => {
    const { pty, store } = await setup();
    pty.ptys.set("old1", { id: "old1", title: "powershell 1", command: "ps", cwd: "C:\\w", status: "running", pid: 1 });
    pty.ptys.set("old2", { id: "old2", title: "x", command: "ps", cwd: "C:\\w", status: "exited", pid: 2 });
    pty.ptys.set("lain", { id: "lain", title: "x", command: "ps", cwd: "C:\\lain", status: "running", pid: 3 });
    expect(await store.adopt("p1", "C:\\w")).toBe(1);
    expect(store.tabsOf("p1").map((t) => t.id)).toEqual(["old1"]);
    expect(store.state().activeByKey.p1).toBe("old1");
    expect(await store.adopt("p1", "C:\\w")).toBe(0); // idempoten
  });

  test("markExited menandai tab tanpa menutupnya", async () => {
    const { store } = await setup();
    const t = await store.create("p1", "C:\\w");
    store.markExited(t.id);
    expect(store.state().tabs[0]?.status).toBe("exited");
    store.markExited("tidak-ada");
  });
});

describe("klien /pty: data sebelum penerima terpasang", () => {
  test("output yang tiba segera setelah tersambung (replay riwayat) ditampung dan dikirim ke penerima pertama", async () => {
    const fetchImpl = (async () => ({
      ok: true,
      status: 200,
      text: async () => JSON.stringify({ ticket: "t" }),
    })) as unknown as typeof fetch;
    let ws: { onmessage: ((m: { data: unknown }) => void) | null; onopen: (() => void) | null } | null = null;
    class W {
      binaryType = "";
      readyState = 1;
      onmessage: ((m: { data: unknown }) => void) | null = null;
      onclose: (() => void) | null = null;
      onopen: (() => void) | null = null;
      onerror: (() => void) | null = null;
      constructor(public url: string) {
        ws = this;
        queueMicrotask(() => {
          this.onopen?.();
          this.onmessage?.({ data: "riwayat-1 " });
          this.onmessage?.({ data: "riwayat-2" });
        });
      }
      send() {}
      close() {}
    }
    const c = createPtyClient(
      { url: "http://x", username: "u", password: "p" },
      fetchImpl,
      W as unknown as new (
        u: string,
      ) => WebSocket,
    );
    const sock = await c.connect("C:p", "p1");
    await new Promise((r) => setTimeout(r, 0)); // pesan tiba sebelum penerima ada
    const got: string[] = [];
    sock.onData((d) => got.push(String(d)));
    expect(got).toEqual(["riwayat-1 ", "riwayat-2"]);
    (ws as unknown as W).onmessage?.({ data: "baru" });
    expect(got.at(-1)).toBe("baru");
  });
});

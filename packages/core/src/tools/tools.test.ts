import { describe, expect, test } from "vitest";
import type { ChatMessage, EngineEvent } from "../chat/types";
import { ActivityLog } from "./activity";
import { type Branch, type FileChange, type GitHost, type GitStatus, GitStore, groupChanges } from "./git";
import { sessionUsage } from "./usage";

const f = (path: string, index: string, worktree: string): FileChange => ({ path, origPath: null, index, worktree });

describe("groupChanges", () => {
  test("staged, unstaged, untracked, dan konflik dipisah; berkas bisa di dua kelompok", () => {
    const g = groupChanges([
      f("a", "M", " "),
      f("b", " ", "M"),
      f("c", "M", "M"),
      f("d", "?", "?"),
      f("e", "U", "U"),
      f("g", "A", "A"),
      f("h", "D", " "),
    ]);
    expect(g.staged.map((x) => x.path)).toEqual(["a", "c", "h"]);
    expect(g.unstaged.map((x) => x.path)).toEqual(["b", "c", "d"]);
    expect(g.conflicts.map((x) => x.path)).toEqual(["e", "g"]);
  });
});

describe("GitStore", () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));
  const repoStatus = (files: FileChange[] = []): GitStatus => ({
    isRepo: true,
    branch: "main",
    upstream: null,
    ahead: 0,
    behind: 0,
    detached: false,
    files,
  });

  function host(initial: FileChange[] = [f("a.txt", "?", "?")]) {
    const calls: string[] = [];
    let files = initial;
    const h: GitHost & { calls: string[]; setFiles(x: FileChange[]): void; fail: string | null; diffText: string } = {
      available: true,
      calls,
      fail: null,
      diffText: "+baris",
      setFiles: (x) => {
        files = x;
      },
      async status() {
        calls.push("status");
        return repoStatus(files);
      },
      async diff(_r, p, staged, untracked) {
        calls.push(`diff:${p}:${staged}:${untracked}`);
        return h.diffText;
      },
      async stage(_r, p) {
        calls.push(`stage:${p.join(",")}`);
        if (h.fail) throw new Error(h.fail);
        files = files.map((x) => (p.includes(x.path) ? f(x.path, "A", " ") : x));
      },
      async unstage(_r, p) {
        calls.push(`unstage:${p.join(",")}`);
      },
      async commit(_r, m) {
        calls.push(`commit:${m}`);
        if (h.fail) throw new Error(h.fail);
        files = [];
        return "abc1234";
      },
      async branches(): Promise<Branch[]> {
        return [{ name: "main", current: true }];
      },
      async switchBranch(_r, n) {
        calls.push(`switch:${n}`);
      },
      async createBranch(_r, n) {
        calls.push(`create:${n}`);
      },
      async log() {
        return [{ hash: "abc", subject: "awal", author: "a", date: "kemarin" }];
      },
    };
    return h;
  }

  test("setRoot memuat status, cabang, dan log; project berganti mengosongkan state lama", async () => {
    const h = host();
    const s = new GitStore(h);
    await s.setRoot("C:\\p");
    expect(s.state()).toMatchObject({ root: "C:\\p", loading: false, error: null });
    expect(s.state().status?.files).toHaveLength(1);
    expect(s.state().branches).toHaveLength(1);
    expect(s.state().log[0]?.subject).toBe("awal");
    await s.setRoot(null);
    expect(s.state()).toMatchObject({ root: null, status: null, branches: [] });
  });

  test("monorepo: akar bukan repo → memakai repo di anak folder; pilihan repo diingat dan status/diff memakai repo aktif", async () => {
    const calls: string[] = [];
    const files: Record<string, FileChange[]> = {
      "C:monoapi": [f("a.ts", " ", "M")],
      "C:monoweb": [f("b.ts", " ", "M")],
    };
    const h = host();
    h.status = async (root: string) => {
      calls.push(`status:${root}`);
      const fl = files[root];
      return fl ? repoStatus(fl) : { ...repoStatus([]), isRepo: false, branch: null };
    };
    h.repos = async () => ["C:monoapi", "C:monoweb"];
    const s = new GitStore(h);
    await s.setRoot("C:mono");
    expect(s.state()).toMatchObject({ projectRoot: "C:mono", root: "C:monoapi" });
    expect(s.state().repos).toEqual(["C:monoapi", "C:monoweb"]);
    expect(s.state().status?.files[0]?.path).toBe("a.ts");
    await s.selectRepo("C:monoweb");
    expect(s.state().root).toBe("C:monoweb");
    expect(s.state().status?.files[0]?.path).toBe("b.ts");
    await s.refresh(); // refresh mengingat repo yang dipilih
    expect(s.state().root).toBe("C:monoweb");
    await s.selectRepo("C:lain"); // bukan anggota daftar → diabaikan
    expect(s.state().root).toBe("C:monoweb");
  });

  test("akar yang sendiri repo tidak mencari anak folder", async () => {
    const h = host();
    const spy = { n: 0 };
    h.repos = async () => {
      spy.n++;
      return ["x"];
    };
    const s = new GitStore(h);
    await s.setRoot("C:p");
    expect(spy.n).toBe(0);
    expect(s.state().repos).toEqual([]);
  });

  test("stage → status dimuat ulang; commit mengembalikan hash dan membersihkan perubahan", async () => {
    const h = host();
    const s = new GitStore(h);
    await s.setRoot("C:\\p");
    await s.stage(["a.txt"]);
    expect(s.state().status?.files[0]?.index).toBe("A");
    expect(await s.commit("pesan")).toBe("abc1234");
    expect(s.state().status?.files).toEqual([]);
    expect(h.calls.filter((c) => c === "status")).toHaveLength(3);
  });

  test("galat aksi dicatat di state dan dilempar; busy kembali false", async () => {
    const h = host();
    const s = new GitStore(h);
    await s.setRoot("C:\\p");
    h.fail = "Author identity unknown";
    await expect(s.commit("x")).rejects.toThrow("Author identity unknown");
    expect(s.state()).toMatchObject({ busy: false, error: "Author identity unknown" });
  });

  test("diff: dimuat untuk berkas, berkas untracked memakai mode --no-index, ditutup saat berkas tak lagi berubah", async () => {
    const h = host([f("a.txt", "?", "?"), f("b.txt", " ", "M")]);
    const s = new GitStore(h);
    await s.setRoot("C:\\p");
    await s.showDiff(f("a.txt", "?", "?"), false);
    expect(s.state().diff).toMatchObject({ path: "a.txt", untracked: true, text: "+baris" });
    expect(h.calls).toContain("diff:a.txt:false:true");
    await s.showDiff(f("b.txt", " ", "M"), false);
    h.setFiles([f("a.txt", "?", "?")]);
    await s.refresh();
    expect(s.state().diff).toBeNull();
  });

  test("tanpa root aksi ditolak; host tidak tersedia tidak memanggil apa pun", async () => {
    const s = new GitStore(host());
    await expect(s.stage(["x"])).rejects.toThrow(/tidak ada project/);
    const off = host();
    off.available = false;
    const s2 = new GitStore(off);
    await s2.setRoot("C:\\p");
    expect(off.calls).toEqual([]);
    expect(s2.available).toBe(false);
  });

  test("respons status basi (project berganti saat memuat) dibuang", async () => {
    const h = host();
    let release: (v: GitStatus) => void = () => {};
    h.status = () =>
      new Promise<GitStatus>((r) => {
        release = r;
      });
    const s = new GitStore(h);
    const first = s.setRoot("C:\\a");
    await flush();
    const resolveA = release;
    h.status = async () => repoStatus([f("baru", "?", "?")]);
    await s.setRoot("C:\\b");
    resolveA(repoStatus([f("lama", "?", "?")]));
    await first;
    expect(s.state().root).toBe("C:\\b");
    expect(s.state().status?.files[0]?.path).toBe("baru");
  });
});

describe("GitStore: push, pull, stash, discard", () => {
  const repo: GitStatus = {
    isRepo: true,
    branch: "main",
    upstream: "origin/main",
    ahead: 1,
    behind: 0,
    detached: false,
    files: [f("a.txt", " ", "M")],
  };
  function host(extra: Partial<GitHost> = {}) {
    const calls: string[] = [];
    let stashes = 0;
    const h: GitHost = {
      available: true,
      status: async () => repo,
      diff: async () => "",
      stage: async () => {},
      unstage: async () => {},
      commit: async () => "abc",
      branches: async () => [{ name: "main", current: true }],
      switchBranch: async () => {},
      createBranch: async () => {},
      log: async () => [],
      push: async (_r) => {
        calls.push("push");
        return "ok";
      },
      pull: async () => {
        calls.push("pull");
        return "Already up to date.";
      },
      stashPush: async (_r, m) => {
        calls.push(`stash:${m}`);
        stashes++;
        return "";
      },
      stashPop: async () => {
        calls.push("pop");
        stashes--;
        return "";
      },
      stashCount: async () => stashes,
      discard: async (_r, p) => {
        calls.push(`discard:${p.join(",")}`);
      },
      ...extra,
    };
    return { h, calls };
  }

  test("operasi jaringan, stash, dan discard memanggil host lalu memuat ulang status dan jumlah stash", async () => {
    const { h, calls } = host();
    const s = new GitStore(h);
    await s.setRoot("C:\r");
    expect([s.canNetwork, s.canStash, s.canDiscard]).toEqual([true, true, true]);
    expect(await s.push()).toBe("ok");
    expect(await s.pull()).toBe("Already up to date.");
    await s.stashPush("");
    expect(s.state().stashes).toBe(1);
    await s.stashPop();
    expect(s.state().stashes).toBe(0);
    await s.discard(["a.txt"]);
    expect(calls).toEqual(["push", "pull", "stash:", "pop", "discard:a.txt"]);
  });

  test("galat host dilaporkan (state.error) dan dilempar; host tanpa dukungan menyembunyikan kemampuan", async () => {
    const { h } = host({
      push: async () => {
        throw new Error("ditolak remote");
      },
    });
    const s = new GitStore(h);
    await s.setRoot("C:\r");
    await expect(s.push()).rejects.toThrow("ditolak remote");
    expect(s.state().error).toBe("ditolak remote");
    expect(s.state().busy).toBe(false);

    const { push: _p, pull: _l, stashPush: _a, stashPop: _o, discard: _d, stashCount: _c, ...bare } = host().h;
    const plain = new GitStore(bare);
    await plain.setRoot("C:\r");
    expect([plain.canNetwork, plain.canStash, plain.canDiscard]).toEqual([false, false, false]);
    await expect(plain.discard(["a"])).rejects.toThrow(/tidak tersedia/);
    expect(plain.state().stashes).toBe(0);
  });
});

describe("sessionUsage", () => {
  const msg = (over: Partial<ChatMessage["info"]>, parts: ChatMessage["parts"] = []): ChatMessage => ({
    info: { id: "m", sessionID: "s", role: "assistant", time: { created: 1000, completed: 3000 }, ...over },
    parts,
  });
  const tool = (name: string, status: "completed" | "error" | "running"): ChatMessage["parts"][number] => ({
    id: `t-${name}-${status}`,
    messageID: "m",
    sessionID: "s",
    type: "tool",
    tool: name,
    callID: "c",
    state: { status, input: {} },
  });

  test("menjumlah token, biaya, pesan, dan tool call; konteks dari giliran terakhir", () => {
    const u = sessionUsage(
      [
        msg({ role: "user", time: { created: 500 } }),
        msg(
          {
            providerID: "p",
            modelID: "m1",
            tokens: { input: 1000, output: 50, reasoning: 0, total: 1050 },
            cost: 0.01,
          },
          [tool("read", "completed"), tool("bash", "error")],
        ),
        msg(
          {
            providerID: "p",
            modelID: "m2",
            tokens: { input: 2000, output: 100, reasoning: 20, total: 2120 },
            cost: 0.02,
          },
          [tool("read", "completed"), tool("edit", "running")],
        ),
      ],
      10_000,
    );
    expect(u.messages).toEqual({ user: 1, assistant: 2 });
    expect(u.tokens).toEqual({ input: 3000, output: 150, reasoning: 20, total: 3170 });
    expect(u.cost).toBeCloseTo(0.03);
    expect(u.model).toEqual({ providerID: "p", modelID: "m2" });
    expect(u.contextTokens).toBe(2000);
    expect(u.contextPercent).toBe(20);
    expect(u.toolCalls).toMatchObject({
      total: 4,
      completed: 2,
      failed: 1,
      running: 1,
      byTool: { read: 2, bash: 1, edit: 1 },
    });
    expect(u.workMs).toBe(4000);
    expect(u.firstAt).toBe(500);
  });

  test("sesi kosong dan biaya tak dilaporkan → null, bukan 0 palsu", () => {
    const u = sessionUsage([]);
    expect(u.cost).toBeNull();
    expect(u.contextTokens).toBeNull();
    expect(u.contextPercent).toBeNull();
    expect(sessionUsage([msg({})]).cost).toBeNull();
  });

  test("persentase konteks dibatasi 100", () => {
    const u = sessionUsage([msg({ tokens: { input: 50_000, output: 0, reasoning: 0, total: 50_000 } })], 10_000);
    expect(u.contextPercent).toBe(100);
  });
});

describe("ActivityLog", () => {
  test("ring buffer, clear, dan sink hanya warn/error", () => {
    const log = new ActivityLog(3, () => 1);
    for (let i = 0; i < 5; i++) log.add("app", "info", `p${i}`);
    expect(log.list().map((e) => e.message)).toEqual(["p2", "p3", "p4"]);
    log.sink({ t: 1, level: "info", scope: "x", msg: "abaikan" });
    log.sink({ t: 1, level: "warn", scope: "x", msg: "penting" });
    expect(log.list().at(-1)?.message).toBe("x: penting");
    log.clear();
    expect(log.list()).toEqual([]);
  });

  test("event engine: tool call dicatat sekali per status; izin dan galat dicatat; delta diabaikan", () => {
    const log = new ActivityLog();
    const part = (status: "running" | "completed"): EngineEvent => ({
      type: "message.part.updated",
      part: {
        id: "p1",
        messageID: "m",
        sessionID: "s",
        type: "tool",
        tool: "read",
        callID: "c",
        state: { status, input: {}, title: "a.txt" },
      },
    });
    log.fromEngineEvent(part("running"));
    log.fromEngineEvent(part("running"));
    log.fromEngineEvent(part("completed"));
    log.fromEngineEvent({
      type: "message.part.delta",
      sessionID: "s",
      messageID: "m",
      partID: "p",
      field: "text",
      delta: "x",
    });
    log.fromEngineEvent({
      type: "permission.asked",
      request: { id: "r", sessionID: "s", permission: "edit", patterns: ["a"], metadata: {}, always: [] },
    });
    log.fromEngineEvent({ type: "session.error", sessionID: "s", error: { name: "E", message: "kunci salah" } });
    expect(log.list().map((e) => [e.source, e.level, e.message])).toEqual([
      ["tool", "info", "read dimulai"],
      ["tool", "info", "read selesai: a.txt"],
      ["chat", "warn", "izin diminta: edit a"],
      ["chat", "error", "galat sesi: kunci salah"],
    ]);
  });

  test("pesan panjang dipotong", () => {
    const log = new ActivityLog();
    log.add("app", "info", "x".repeat(2000));
    expect(log.list()[0]?.message).toHaveLength(500);
  });
});

describe("normalizeBrowserUrl", () => {
  test("tanpa skema → https; skema selain http/https dan URL rusak ditolak", async () => {
    const { normalizeBrowserUrl } = await import("./browser");
    expect(normalizeBrowserUrl("example.com")).toEqual({ ok: true, url: "https://example.com/" });
    expect(normalizeBrowserUrl(" http://localhost:3000/x ")).toEqual({ ok: true, url: "http://localhost:3000/x" });
    for (const bad of [
      "",
      "   ",
      "javascript:alert(1)",
      "file:///C:/x",
      "data:text/html,x",
      "ftp://x.y",
      "tauri://localhost",
      "https://",
      "http://:80/",
    ]) {
      expect(normalizeBrowserUrl(bad).ok, bad).toBe(false);
    }
    expect(normalizeBrowserUrl("a".repeat(5000)).ok).toBe(false);
  });
});

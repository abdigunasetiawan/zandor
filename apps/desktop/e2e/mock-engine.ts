// Engine tiruan di memori untuk e2e, dicegat lewat page.route (tanpa jaringan). Kontrak diverifikasi terhadap engine
// asli di smoke native (e2e-native) dan rekaman SSE asli (packages/core/src/chat/fixtures).
import type { Page } from "@playwright/test";

type Sess = { id: string; title: string; directory: string; time: { created: number; updated: number } };
type Part = Record<string, unknown> & { id: string; messageID: string; type: string };
type Msg = { info: Record<string, unknown> & { id: string; role: string }; parts: Part[] };

export type MockEngine = {
  sessions: Sess[];
  prompts: Array<{ text: string; model?: { providerID: string; modelID: string } }>;
  aborted: string[];
  permissionReplies: Array<{ id: string; reply: string }>;
  /** Jawaban pertanyaan agent (answers null = ditolak/dilewati). */
  questionReplies: Array<{ id: string; answers: string[][] | null }>;
  reverts: string[];
  unreverts: number;
  forks: string[];
  /** Perintah slash engine (GET /command) dan yang dijalankan (POST /session/:id/command). */
  commands: Array<{ name: string; description?: string; source?: string; template: string; hints: string[] }>;
  commandRuns: Array<{ command: string; arguments: string }>;
  summaries: string[];
  /** Isi file yang dikirim bersama prompt (lampiran). */
  files: Array<{ mime: string; url: string; filename?: string }>;
  variants: Array<string | undefined>;
  /** Isi berkas project untuk /file dan /file/content (kunci = path relatif). */
  project: Map<string, { text?: string; base64?: string }>;
  /** Selesaikan giliran yang sedang berjalan (jawaban singkat + status idle), atau gagalkan dengan galat. */
  finishTurn(sessionId: string, error?: { name: string; message: string }): void;
  /** Tambah langkah (tool part) ke giliran yang sedang berjalan sebagai PESAN ASISTEN BARU (seperti engine asli memecah giliran). */
  addStep(sessionId: string, part: Record<string, unknown>): void;
  /** Perubahan berkas per sesi untuk GET /session/:id/diff. */
  sessionDiffs: Map<
    string,
    Array<{ file: string; patch: string; additions: number; deletions: number; status: string }>
  >;
  /** Terminal tiruan: PTY yang dibuat, input yang diterima, dan perubahan ukuran. */
  pty: {
    list: Array<{ id: string; title: string; command: string; cwd: string; status: "running" | "exited"; pid: number }>;
    inputs: string[];
    resizes: Array<{ id: string; cols: number; rows: number }>;
    deleted: string[];
    connects: number;
    /** Akhiri shell: tandai exited dan tutup soket. */
    exit(id: string): void;
  };
  oauth: {
    authorizes: Array<{ id: string; method: number; inputs?: unknown }>;
    callbacks: Array<{ id: string; method: number; code?: string }>;
  };
};

export type MockOptions = {
  /** Semua panggilan gagal 500. */
  fail?: boolean;
  /** Tampilkan tur pengenalan (bawaan: mati). */
  onboarding?: boolean;
  /** Daftar model; default satu model. Kosong = tidak ada provider. */
  models?: Array<{ id: string; name: string; variants?: string[]; limit?: { context: number; output?: number } }>;
  /** Jumlah pasangan pesan (pengguna+asisten) yang sudah ada di setiap sesi baru; untuk uji sesi panjang. */
  preload?: number;
  /** Metode OAuth yang dikembalikan engine tiruan. */
  oauthMethod?: "auto" | "code";
};

/** Respons Markdown kaya untuk uji render (tabel, kode, Mermaid, matematika, daftar tugas, dan konten berbahaya). */
export const RICH_MD = [
  "## Ringkasan",
  "",
  "Contoh **tebal**, _miring_, dan `kode inline`.",
  "",
  "| Nama | Nilai |",
  "| --- | ---: |",
  "| Apel | 3 |",
  "| Jeruk | 10 |",
  "",
  "```ts",
  "const a: number = 1;",
  "function hitung(x: number) { return x * 2; } // komentar",
  "```",
  "",
  "```mermaid",
  "graph TD",
  "  A[Mulai] --> B[Selesai]",
  "```",
  "",
  "Rumus: $E = mc^2$",
  "",
  "- [x] tugas selesai",
  "- [ ] tugas belum",
  "",
  "[aman](https://example.com) dan [jahat](javascript:alert(1))",
  "",
  "<script>window.__pwned = 1</script>",
  "",
  "![gambar luar](https://evil.example/pixel.png)",
  "",
  "```ts",
  ...Array.from({ length: 40 }, (_, i) => `const baris${i} = ${i};`),
  "```",
].join("\n");

export async function mockEngine(page: Page, opts: MockOptions = {}): Promise<MockEngine> {
  const state: MockEngine = {
    finishTurn: () => {},
    addStep: () => {},
    sessions: [],
    prompts: [],
    aborted: [],
    permissionReplies: [],
    questionReplies: [],
    reverts: [],
    unreverts: 0,
    forks: [],
    commands: [{ name: "init", description: "Buat AGENTS.md untuk project", template: "init", hints: [] }],
    commandRuns: [],
    summaries: [],
    files: [],
    variants: [],
    oauth: { authorizes: [], callbacks: [] },
    pty: {
      list: [],
      inputs: [],
      resizes: [],
      deleted: [],
      connects: 0,
      exit: (id: string) => {
        const p = state.pty.list.find((x) => x.id === id);
        if (p) p.status = "exited";
        sockets.get(id)?.close();
      },
    },
    project: new Map(),
    sessionDiffs: new Map(),
  };
  const revertOf = new Map<string, string>();
  const sockets = new Map<string, { close(): void }>();
  const history = new Map<string, string>();
  const messages = new Map<string, Msg[]>();
  const queue: unknown[] = [];
  const permissions: Array<Record<string, unknown>> = [];
  const questions: Array<Record<string, unknown>> = [];
  const todos = new Map<string, unknown[]>();
  let n = 0;
  let clock = Date.now();
  const id = (p: string) => `${p}_${String(++n).padStart(6, "0")}`;
  const models = opts.models ?? [{ id: "m-satu", name: "Model Satu" }];

  await page.addInitScript((tour: boolean) => {
    if (!localStorage.getItem("zandor.engine.override")) {
      localStorage.setItem("zandor.engine.override", JSON.stringify({ url: "http://engine.test", password: "x" }));
      localStorage.setItem("zandor.chatsDir", "C:\\data\\chats");
    }
    // Tur pengenalan tidak muncul otomatis di browser otomatis kecuali tes memintanya.
    if (tour) localStorage.setItem("zandor.e2e.tour", "1");
  }, opts.onboarding ?? false);

  const emit = (type: string, properties: unknown) => queue.push({ id: id("evt"), type, properties });
  state.addStep = (sid, part) => {
    const list = messages.get(sid) ?? [];
    const prev = list.at(-1);
    if (!prev) return;
    prev.info.time = { ...(prev.info.time as object), completed: ++clock } as never;
    emit("message.updated", { sessionID: sid, info: prev.info });
    const next: Msg = {
      info: {
        id: id("msg"),
        sessionID: sid,
        role: "assistant",
        parentID: prev.info.parentID as string,
        modelID: "m-satu",
        providerID: "mock",
        time: { created: ++clock },
      },
      parts: [],
    };
    list.push(next);
    emit("message.updated", { sessionID: sid, info: next.info });
    const p = { id: id("prt"), messageID: next.info.id, sessionID: sid, ...part } as unknown as Part;
    next.parts.push(p);
    emit("message.part.updated", { sessionID: sid, part: p });
  };
  state.finishTurn = (sid, error) => {
    const last = (messages.get(sid) ?? []).at(-1);
    if (last) {
      last.info.time = { ...(last.info.time as object), completed: ++clock } as never;
      if (error) last.info.error = { name: error.name, data: { message: error.message } };
      emit("message.updated", { sessionID: sid, info: last.info });
    }
    emit("session.status", { sessionID: sid, status: { type: "idle" } });
  };

  /** Skenario balasan asisten: teks streaming, plus tool call atau permintaan izin sesuai isi prompt. */
  const respond = (sessionID: string, text: string) => {
    let list = messages.get(sessionID) ?? [];
    const cut = revertOf.get(sessionID);
    if (cut) {
      // Prompt setelah revert membersihkan pesan yang di-revert (perilaku engine asli).
      list = list.filter((m) => m.info.id < cut);
      revertOf.delete(sessionID);
      const s = state.sessions.find((x) => x.id === sessionID);
      if (s) emit("session.updated", { sessionID, info: { ...s } });
    }
    messages.set(sessionID, list);
    const t0 = ++clock;
    const user: Msg = {
      info: { id: id("msg"), sessionID, role: "user", time: { created: t0 } },
      parts: [],
    };
    user.parts.push({ id: id("prt"), messageID: user.info.id, sessionID, type: "text", text });
    list.push(user);
    emit("message.updated", { sessionID, info: user.info });
    emit("message.part.updated", { sessionID, part: user.parts[0] });
    emit("session.status", { sessionID, status: { type: "busy" } });

    let asst: Msg = {
      info: {
        id: id("msg"),
        sessionID,
        role: "assistant",
        parentID: user.info.id,
        modelID: "m-satu",
        providerID: "mock",
        time: { created: ++clock },
      },
      parts: [],
    };
    list.push(asst);
    emit("message.updated", { sessionID, info: asst.info });

    const finish = () => {
      asst.info.time = { ...(asst.info.time as object), completed: ++clock } as never;
      asst.info.finish = "stop";
      asst.info.tokens = { input: 1200, output: 80, reasoning: 0, total: 1280 };
      asst.info.cost = 0.0042;
      emit("message.updated", { sessionID, info: asst.info });
      emit("session.status", { sessionID, status: { type: "idle" } });
    };
    const say = (answer: string) => {
      const part: Part = { id: id("prt"), messageID: asst.info.id, sessionID, type: "text", text: "" };
      asst.parts.push(part);
      emit("message.part.updated", { sessionID, part: { ...part } });
      for (const piece of [answer.slice(0, 6), answer.slice(6)]) {
        part.text += piece;
        emit("message.part.delta", {
          sessionID,
          messageID: asst.info.id,
          partID: part.id,
          field: "text",
          delta: piece,
        });
      }
    };

    if (text.includes("bertahap")) {
      // giliran berjalan: satu langkah baca selesai; langkah berikutnya ditambahkan test lewat addStep
      const part = {
        id: id("prt"),
        messageID: asst.info.id,
        sessionID,
        type: "tool",
        tool: "read",
        callID: id("call"),
        state: {
          status: "completed",
          input: { filePath: ["C:", "Work", "alpha", "src", "a.ts"].join("/"), offset: 1, limit: 3 },
          output: ["<path>a.ts</path>", "<content>", "1: satu", "2: dua", "3: tiga", "</content>"].join(
            String.fromCharCode(10),
          ),
          title: "a.ts",
          metadata: { display: { lineStart: 1, lineEnd: 3, totalLines: 9 } },
          time: { start: 1, end: 2 },
        },
      } as Part;
      asst.parts.push(part);
      emit("message.part.updated", { sessionID, part });
      return; // tetap sibuk
    }
    if (text.includes("bekerja")) {
      // giliran yang sedang berjalan dengan proses (berpikir + pencarian web) dan belum ada jawaban
      for (const part of [
        { id: id("prt"), messageID: asst.info.id, sessionID, type: "reasoning", text: "menimbang langkah" },
        {
          id: id("prt"),
          messageID: asst.info.id,
          sessionID,
          type: "tool",
          tool: "websearch",
          callID: id("call"),
          state: { status: "running", input: { query: "cuaca jakarta" }, title: "", time: { start: 1 } },
        },
      ] as Part[]) {
        asst.parts.push(part);
        emit("message.part.updated", { sessionID, part });
      }
      return; // tetap sibuk sampai di-abort
    }
    for (const [needle, path] of [
      ["buat dokumen", "laporan/ringkasan.md"],
      ["buat kode", "src/app.ts"],
    ] as const) {
      if (!text.includes(needle)) continue;
      asst.parts.push({
        id: id("prt"),
        messageID: asst.info.id,
        sessionID,
        type: "tool",
        tool: "write",
        callID: id("call"),
        state: {
          status: "completed",
          input: { filePath: path },
          output: "ok",
          title: path,
          time: { start: 1, end: 2 },
        },
      });
      emit("message.part.updated", { sessionID, part: asst.parts.at(-1) });
      // seperti engine asli: pesan pertama hanya berisi tool call, jawaban datang di pesan asisten berikutnya
      asst.info.time = { ...(asst.info.time as object), completed: ++clock } as never;
      emit("message.updated", { sessionID, info: asst.info });
      asst = {
        info: {
          id: id("msg"),
          sessionID,
          role: "assistant",
          parentID: user.info.id,
          modelID: "m-satu",
          providerID: "mock",
          time: { created: ++clock },
        },
        parts: [],
      };
      list.push(asst);
      emit("message.updated", { sessionID, info: asst.info });
    }
    if (text.includes("lambat")) return; // tetap sibuk sampai di-abort
    if (text.includes("pertanyaan")) {
      const req = {
        id: id("que"),
        sessionID,
        questions: [
          {
            header: "Framework",
            question: "Framework mana yang dipakai?",
            options: [
              { label: "React", description: "Pustaka UI" },
              { label: "Svelte", description: "Kompiler UI" },
            ],
          },
          {
            header: "Fitur",
            question: "Fitur tambahan apa?",
            multiple: true,
            options: [
              { label: "Tes", description: "Tulis tes" },
              { label: "Lint", description: "Aktifkan lint" },
            ],
          },
        ],
      };
      questions.push(req);
      emit("question.asked", req);
      return;
    }
    if (text.includes("rencana")) {
      const list = [
        { content: "Baca kode", status: "completed", priority: "high" },
        { content: "Tulis tes", status: "in_progress", priority: "medium" },
        { content: "Rilis", status: "pending", priority: "low" },
      ];
      todos.set(sessionID, list);
      emit("todo.updated", { sessionID, todos: list });
    }
    if (text.includes("izin")) {
      const req = {
        id: id("per"),
        sessionID,
        permission: "edit",
        // Berkas rahasia → tetap ditanyakan di mode bawaan "auto-safe"; "aman" → berkas biasa (disetujui otomatis).
        patterns: [text.includes("aman") ? "catatan.txt" : ".env.local"],
        metadata: {},
        always: [text.includes("aman") ? "catatan.txt" : ".env.local"],
      };
      permissions.push(req);
      emit("permission.asked", req);
      return;
    }
    if (text.includes("baca")) {
      const tool: Part = {
        id: id("prt"),
        messageID: asst.info.id,
        sessionID,
        type: "tool",
        tool: "read",
        callID: id("call"),
        state: {
          status: "completed",
          input: { filePath: "catatan.txt" },
          output: "halo dunia",
          title: "catatan.txt",
          time: { start: 1, end: 2 },
        },
      };
      asst.parts.push(tool);
      emit("message.part.updated", { sessionID, part: tool });
    }
    for (const [needle, tool, input, metadata] of [
      ["cari web", "websearch", { query: "cuaca jakarta" }, {}],
      [
        "buat diff",
        "edit",
        { filePath: "a.txt" },
        { diff: ["--- a.txt", "+++ a.txt", "@@ -1 +1 @@", "-lama", "+baru"].join("\n") },
      ],
    ] as const) {
      if (!text.includes(needle)) continue;
      asst.parts.push({
        id: id("prt"),
        messageID: asst.info.id,
        sessionID,
        type: "tool",
        tool,
        callID: id("call"),
        state: { status: "completed", input, output: "ok", title: "", metadata, time: { start: 1, end: 2 } },
      });
      emit("message.part.updated", { sessionID, part: asst.parts.at(-1) });
    }
    if (text.includes("tampilkan kaya")) {
      say(RICH_MD);
      finish();
      return;
    }
    say(text.includes("baca") ? "Isinya halo dunia." : "Halo, ada yang bisa dibantu?");
    finish();
  };

  await page.route("http://engine.test/**", async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const dir = url.searchParams.get("directory") ?? req.headers()["x-opencode-directory"] ?? "";
    const cors = {
      "access-control-allow-origin": "*",
      "access-control-allow-headers": "*",
      "access-control-allow-methods": "*",
    };
    const json = (body: unknown, status = 200) =>
      route.fulfill({ status, headers: { ...cors, "content-type": "application/json" }, body: JSON.stringify(body) });
    if (req.method() === "OPTIONS") return route.fulfill({ status: 204, headers: cors });
    if (opts.fail) return json({ message: "engine rusak" }, 500);
    const p = url.pathname;

    if (p === "/global/health") return json({ healthy: true, version: "1.19.5" });
    if (p === "/provider" && req.method() === "GET") {
      const catalog = [
        { id: "openrouter", name: "OpenRouter", env: ["OPENROUTER_API_KEY"], models: 3 },
        { id: "anthropic", name: "Anthropic", env: ["ANTHROPIC_API_KEY"], models: 5 },
        { id: "cloudflare-workers-ai", name: "Cloudflare Workers AI", env: ["CLOUDFLARE_API_KEY"], models: 8 },
        { id: "openai", name: "OpenAI", env: ["OPENAI_API_KEY"], models: 9 },
      ];
      return json({
        all: catalog.map((c) => ({
          id: c.id,
          name: c.name,
          source: "env",
          env: c.env,
          options: {},
          models: Object.fromEntries(Array.from({ length: c.models }, (_, i) => [`m${i}`, { id: `m${i}` }])),
        })),
        default: {},
        connected: [],
      });
    }
    if (p === "/provider/auth") {
      return json({
        openai: [
          { type: "oauth", label: "ChatGPT Pro/Plus", prompts: [] },
          { type: "api", label: "Manually enter API Key" },
        ],
        "cloudflare-workers-ai": [
          { type: "api", label: "API key", prompts: [{ type: "text", key: "accountId", message: "Account ID" }] },
        ],
      });
    }
    const oauth = p.match(/^\/provider\/([^/]+)\/oauth\/(authorize|callback)$/);
    if (oauth?.[1] && req.method() === "POST") {
      const body = req.postDataJSON() ?? {};
      if (oauth[2] === "authorize") {
        state.oauth.authorizes.push({ id: oauth[1], method: body.method, inputs: body.inputs });
        return json({
          url: "https://auth.example.test/authorize?state=abc",
          method: opts.oauthMethod ?? "auto",
          instructions: "Selesaikan di browser.",
        });
      }
      state.oauth.callbacks.push({ id: oauth[1], method: body.method, code: body.code });
      await new Promise((r) => setTimeout(r, 300));
      return json(true);
    }
    if (p === "/config/providers") {
      const providers =
        models.length === 0
          ? []
          : [
              {
                id: "mock",
                name: "Mock",
                source: "config",
                env: [],
                options: {},
                models: Object.fromEntries(
                  models.map((m) => [
                    m.id,
                    {
                      id: m.id,
                      providerID: "mock",
                      name: m.name,
                      variants: Object.fromEntries((m.variants ?? []).map((v) => [v, {}])),
                      ...(m.limit ? { limit: m.limit } : {}),
                    },
                  ]),
                ),
              },
            ];
      return json({ providers, default: {} });
    }
    if (p === "/event") {
      // Long-poll: tunggu event, kirim sebagai satu tubuh SSE, lalu tutup (klien menyambung ulang).
      for (let i = 0; i < 20 && queue.length === 0; i++) await new Promise((r) => setTimeout(r, 50));
      const events = [{ type: "server.connected", properties: {} }, ...queue.splice(0)];
      return route.fulfill({
        status: 200,
        headers: { ...cors, "content-type": "text/event-stream", "cache-control": "no-cache" },
        body: events.map((e) => `data: ${JSON.stringify(e)}\n\n`).join(""),
      });
    }
    if (p === "/pty/shells") {
      return json([
        {
          path: "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
          name: "powershell",
          acceptable: true,
        },
        { path: "C:\\Windows\\System32\\cmd.exe", name: "cmd", acceptable: true },
      ]);
    }
    if (p === "/pty" && req.method() === "GET") return json(state.pty.list.filter((x) => x.cwd === dir));
    if (p === "/pty" && req.method() === "POST") {
      const body = req.postDataJSON() ?? {};
      const info = {
        id: `pty_${++n}`,
        title: body.title ?? "terminal",
        command: body.command ?? "",
        cwd: body.cwd ?? dir,
        status: "running" as const,
        pid: 1000 + n,
      };
      state.pty.list.push(info);
      return json(info);
    }
    const ptyM = p.match(/^\/pty\/([^/]+)(?:\/(connect-token))?$/);
    if (ptyM?.[1]) {
      const pid = decodeURIComponent(ptyM[1]);
      if (ptyM[2] === "connect-token" && req.method() === "POST") {
        if (req.headers()["x-opencode-ticket"] !== "1") return json({ message: "tiket butuh header" }, 400);
        return json({ ticket: `tk_${pid}` });
      }
      if (req.method() === "PUT") {
        const sz = req.postDataJSON()?.size ?? {};
        state.pty.resizes.push({ id: pid, cols: sz.cols, rows: sz.rows });
        return json(true);
      }
      if (req.method() === "DELETE") {
        state.pty.deleted.push(pid);
        state.pty.list = state.pty.list.filter((x) => x.id !== pid);
        sockets.get(pid)?.close();
        return json(true);
      }
    }
    if (p === "/question" && req.method() === "GET") return json(questions);
    const que = p.match(/^\/question\/([^/]+)\/(reply|reject)$/);
    if (que?.[1] && req.method() === "POST") {
      const body = req.postDataJSON() ?? {};
      const isReply = que[2] === "reply";
      state.questionReplies.push({ id: que[1], answers: isReply ? (body.answers ?? []) : null });
      const i = questions.findIndex((x) => x.id === que[1]);
      const asked = questions[i];
      if (i >= 0) questions.splice(i, 1);
      if (asked) {
        const sessionID = asked.sessionID as string;
        emit(isReply ? "question.replied" : "question.rejected", {
          sessionID,
          requestID: que[1],
          answers: body.answers,
        });
        const last = (messages.get(sessionID) ?? []).at(-1);
        if (last && isReply) {
          const part: Part = {
            id: id("prt"),
            messageID: last.info.id,
            sessionID,
            type: "text",
            text: `Terima kasih, jawaban: ${JSON.stringify(body.answers)}`,
          };
          last.parts.push(part);
          emit("message.part.updated", { sessionID, part });
        }
        if (last) {
          last.info.time = { ...(last.info.time as object), completed: ++clock } as never;
          emit("message.updated", { sessionID, info: last.info });
        }
        emit("session.status", { sessionID, status: { type: "idle" } });
      }
      return json(true);
    }
    const todoGet = p.match(/^\/session\/([^/]+)\/todo$/);
    if (todoGet?.[1] && req.method() === "GET") return json(todos.get(todoGet[1]) ?? []);
    if (p === "/permission" && req.method() === "GET") return json(permissions);
    const perm = p.match(/^\/permission\/([^/]+)\/reply$/);
    if (perm?.[1] && req.method() === "POST") {
      const body = req.postDataJSON() ?? {};
      state.permissionReplies.push({ id: perm[1], reply: body.reply });
      const i = permissions.findIndex((x) => x.id === perm[1]);
      const asked = permissions[i];
      if (i >= 0) permissions.splice(i, 1);
      if (asked) {
        const sessionID = asked.sessionID as string;
        emit("permission.replied", { sessionID, requestID: perm[1], reply: body.reply });
        const last = (messages.get(sessionID) ?? []).at(-1);
        if (last && body.reply !== "reject") {
          const part: Part = { id: id("prt"), messageID: last.info.id, sessionID, type: "text", text: "Sudah diubah." };
          last.parts.push(part);
          emit("message.part.updated", { sessionID, part });
        }
        if (last) {
          last.info.time = { ...(last.info.time as object), completed: ++clock } as never;
          emit("message.updated", { sessionID, info: last.info });
        }
        emit("session.status", { sessionID, status: { type: "idle" } });
      }
      return json(true);
    }

    if (p === "/session" && req.method() === "GET") return json(state.sessions.filter((s) => s.directory === dir));
    if (p === "/session" && req.method() === "POST") {
      const body = req.postDataJSON() ?? {};
      const t = ++clock;
      const s = {
        id: `ses_${++n}`,
        title: body.title ?? `New session - ${new Date(t).toISOString()}`,
        directory: dir,
        time: { created: t, updated: t },
      };
      state.sessions.push(s);
      if (opts.preload) {
        const list: Msg[] = [];
        for (let i = 0; i < opts.preload; i++) {
          const k = String(i).padStart(5, "0");
          const u: Msg = {
            info: { id: `m_${k}_u`, sessionID: s.id, role: "user", time: { created: i * 2 + 1 } },
            parts: [],
          };
          u.parts.push({
            id: `p_${k}_u`,
            messageID: u.info.id,
            sessionID: s.id,
            type: "text",
            text: `pertanyaan ${i}`,
          });
          const a: Msg = {
            info: {
              id: `m_${k}_a`,
              sessionID: s.id,
              role: "assistant",
              modelID: "m-satu",
              providerID: "mock",
              time: { created: i * 2 + 2, completed: i * 2 + 3 },
            },
            parts: [],
          };
          a.parts.push({ id: `p_${k}_a`, messageID: a.info.id, sessionID: s.id, type: "text", text: `jawaban ${i}` });
          list.push(u, a);
        }
        messages.set(s.id, list);
      }
      return json(s);
    }
    const m = p.match(/^\/session\/([^/]+)(?:\/([a-z_]+))?$/);
    if (m?.[1]) {
      const sid = m[1];
      const sub = m[2];
      if (!sub && req.method() === "GET") {
        const s = state.sessions.find((x) => x.id === sid);
        if (!s) return json({ message: "tidak ada" }, 404);
        const cutId = revertOf.get(sid);
        return json({ ...s, ...(cutId ? { revert: { messageID: cutId } } : {}) });
      }
      if (sub === "diff" && req.method() === "GET") return json(state.sessionDiffs.get(sid) ?? []);
      if (sub === "revert" && req.method() === "POST") {
        const body = req.postDataJSON() ?? {};
        state.reverts.push(body.messageID);
        revertOf.set(sid, body.messageID);
        const s = state.sessions.find((x) => x.id === sid);
        if (s) emit("session.updated", { sessionID: sid, info: { ...s, revert: { messageID: body.messageID } } });
        return json(s ?? {});
      }
      if (sub === "unrevert" && req.method() === "POST") {
        state.unreverts++;
        revertOf.delete(sid);
        const s = state.sessions.find((x) => x.id === sid);
        if (s) emit("session.updated", { sessionID: sid, info: { ...s } });
        return json(s ?? {});
      }
      if (sub === "fork" && req.method() === "POST") {
        const body = req.postDataJSON() ?? {};
        state.forks.push(body.messageID);
        const t = ++clock;
        const forked = {
          id: `ses_${++n}`,
          title: "Cabang percakapan",
          directory: dir,
          time: { created: t, updated: t },
        };
        state.sessions.push(forked);
        return json(forked);
      }
      if (!sub && req.method() === "PATCH") {
        const s = state.sessions.find((x) => x.id === sid);
        if (!s) return json({ message: "tidak ada" }, 404);
        Object.assign(s, req.postDataJSON() ?? {});
        return json(s);
      }
      if (!sub && req.method() === "DELETE") {
        const i = state.sessions.findIndex((x) => x.id === sid);
        if (i >= 0) state.sessions.splice(i, 1);
        return json(true);
      }
      if (sub === "message" && req.method() === "GET") return json(messages.get(sid) ?? []);
      if (sub === "prompt_async" && req.method() === "POST") {
        const body = req.postDataJSON() ?? {};
        const text = (body.parts ?? []).map((x: { text?: string }) => x.text ?? "").join("");
        state.prompts.push({ text, ...(body.model ? { model: body.model } : {}) });
        state.variants.push(body.variant);
        for (const part of body.parts ?? []) {
          if (part.type === "file") state.files.push({ mime: part.mime, url: part.url, filename: part.filename });
        }
        respond(sid, text);
        return route.fulfill({ status: 204, headers: cors });
      }
      if (sub === "command" && req.method() === "POST") {
        const body = req.postDataJSON() ?? {};
        state.commandRuns.push({ command: body.command, arguments: body.arguments ?? "" });
        respond(sid, `/${body.command} ${body.arguments ?? ""}`.trim());
        return json({ info: {}, parts: [] });
      }
      if (sub === "summarize" && req.method() === "POST") {
        state.summaries.push(sid);
        return json(true);
      }
      if (sub === "abort" && req.method() === "POST") {
        state.aborted.push(sid);
        const last = (messages.get(sid) ?? []).at(-1);
        if (last) {
          last.info.time = { ...(last.info.time as object), completed: ++clock } as never;
          last.info.error = { name: "MessageAbortedError", data: { message: "dibatalkan" } };
          emit("message.updated", { sessionID: sid, info: last.info });
        }
        emit("session.status", { sessionID: sid, status: { type: "idle" } });
        return json(true);
      }
    }
    if (p === "/command" && req.method() === "GET") return json(state.commands);
    if (p === "/find/file") {
      const q = (url.searchParams.get("query") ?? "").toLowerCase();
      return json([...state.project.keys()].filter((k) => k.toLowerCase().includes(q)).slice(0, 30));
    }
    if (p === "/file/content") {
      const reqPath = url.searchParams.get("path") ?? "";
      // berkas di folder sesi Chat dibuka lewat root folder Chat + awalan `chat-…/`; mock memetakannya ke isi project tiruan
      const f = state.project.get(reqPath) ?? state.project.get(reqPath.replace(/^chat-[^/]+\//, ""));
      if (!f) return json({ name: "NotFoundError", data: { message: "tidak ada" } }, 404);
      if (f.base64 !== undefined) {
        return json({ type: "binary", content: f.base64, encoding: "base64", mimeType: "application/octet-stream" });
      }
      return json({ type: "text", content: f.text ?? "" });
    }
    if (p === "/file" && state.project.size > 0) {
      const path = url.searchParams.get("path") ?? ".";
      const prefix = path === "." ? "" : `${path}/`;
      const seen = new Map<string, boolean>();
      for (const key of state.project.keys()) {
        if (!key.startsWith(prefix)) continue;
        const rest = key.slice(prefix.length);
        const slash = rest.indexOf("/");
        const name = slash === -1 ? rest : rest.slice(0, slash);
        seen.set(name, slash !== -1);
      }
      return json(
        [...seen.entries()].map(([name, isDir]) => ({
          name,
          path: `${prefix}${name}`,
          absolute: `${dir}\\${prefix}${name}`.replace(/\//g, "\\"),
          type: isDir ? "directory" : "file",
          ignored: false,
        })),
      );
    }
    if (p === "/file") {
      const path = url.searchParams.get("path") ?? ".";
      const node = (name: string, rel: string, type: string, ignored = false) => ({
        name,
        path: rel,
        absolute: `${dir}\\${rel}`,
        type,
        ignored,
      });
      const tree: Record<string, unknown[]> = {
        ".": [
          node("src", "src", "directory"),
          node("README.md", "README.md", "file"),
          node("node_modules", "node_modules", "directory", true),
        ],
        src: [node("main.ts", "src/main.ts", "file")],
      };
      return json(tree[path] ?? []);
    }
    return json({ message: `tidak ditangani: ${req.method()} ${p}` }, 404);
  });
  // WebSocket PTY tiruan: prompt, gema ketikan, "perintah" dijawab, riwayat diputar ulang saat tersambung lagi.
  await page.routeWebSocket(/\/pty\/[^/?]+\/connect/, (ws) => {
    const id = decodeURIComponent(new URL(ws.url()).pathname.split("/")[2] ?? "");
    state.pty.connects++;
    sockets.set(id, ws);
    const prompt = "PS C:\\Work> ";
    const hist = history.get(id) ?? prompt;
    history.set(id, hist);
    ws.send(hist);
    let line = "";
    ws.onMessage((m) => {
      const data = String(m);
      state.pty.inputs.push(data);
      for (const ch of data) {
        if (ch === "\r") {
          const out = `\r\nhasil:${line}\r\n${prompt}`;
          line = "";
          history.set(id, (history.get(id) ?? "") + out);
          ws.send(out);
        } else {
          line += ch;
          history.set(id, (history.get(id) ?? "") + ch);
          ws.send(ch);
        }
      }
    });
    ws.onClose(() => sockets.delete(id));
  });
  return state;
}

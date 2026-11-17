// Kontrol aplikasi oleh agent (ADR-077): tool `zandor_app` dan `zandor_image` menulis permintaan ke
// `<controlDir>/requests/<id>.json`; aplikasi Zandor (UI) mengambilnya, menjalankannya, lalu menulis jawaban ke
// `<controlDir>/responses/<id>.json`. Tool menunggu jawaban itu; tanpa jawaban (aplikasi tertutup/kontrol agent
// dimatikan) tool berhenti dengan pesan jujur setelah batas waktu.
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tool } from "@opencode-ai/plugin";

export const APP_ACTIONS = [
  "status",
  "list_commands",
  "run_command",
  "panel",
  "open_tool",
  "open_url",
  "open_file",
  "navigate",
  "list_terminals",
  "close_terminal",
  "get_settings",
  "set_setting",
] as const;
export type AppAction = (typeof APP_ACTIONS)[number] | "generate_image";

export type ControlRequest = {
  id: string;
  action: AppAction;
  args: Record<string, unknown>;
  sessionID: string;
  directory: string;
  at: number;
};

export type ControlResponse = { ok: boolean; result?: unknown; error?: string };

const newId = () => `ctl-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;

export type RequestOptions = { timeoutMs: number; pollMs?: number; signal?: AbortSignal };

/** Kirim permintaan dan tunggu jawaban aplikasi. Tidak pernah melempar: kegagalan menjadi `{ ok: false }`. */
export async function requestControl(
  dir: string,
  req: Omit<ControlRequest, "id" | "at">,
  opts: RequestOptions,
): Promise<ControlResponse> {
  const id = newId();
  const reqDir = join(dir, "requests");
  const resDir = join(dir, "responses");
  mkdirSync(reqDir, { recursive: true });
  mkdirSync(resDir, { recursive: true });
  const file = join(reqDir, `${id}.json`);
  const tmp = `${file}.tmp`;
  writeFileSync(tmp, JSON.stringify({ ...req, id, at: Date.now() }));
  renameSync(tmp, file); // atomik: aplikasi tidak pernah membaca berkas setengah jadi
  const res = join(resDir, `${id}.json`);
  const deadline = Date.now() + opts.timeoutMs;
  const poll = opts.pollMs ?? 200;
  while (Date.now() < deadline) {
    if (opts.signal?.aborted) {
      rmSync(file, { force: true });
      return { ok: false, error: "Dibatalkan." };
    }
    if (existsSync(res)) {
      try {
        const v = JSON.parse(readFileSync(res, "utf8")) as ControlResponse;
        rmSync(res, { force: true });
        return typeof v === "object" && v !== null ? v : { ok: false, error: "Jawaban aplikasi tidak valid." };
      } catch {
        // berkas jawaban sedang ditulis; coba lagi
      }
    }
    await new Promise((r) => setTimeout(r, poll));
  }
  rmSync(file, { force: true });
  return {
    ok: false,
    error:
      "Aplikasi Zandor tidak merespons (jendela tertutup, sedang sibuk, atau kontrol agent dimatikan di Pengaturan → Umum).",
  };
}

const format = (r: ControlResponse) => {
  if (!r.ok) return `Gagal: ${r.error ?? "tidak diketahui"}`;
  if (r.result === undefined || r.result === null) return "Selesai.";
  return typeof r.result === "string" ? r.result : JSON.stringify(r.result, null, 2);
};

export function createControlTools(dir: string) {
  mkdirSync(dir, { recursive: true });
  return {
    zandor_app: tool({
      description: [
        "Operasikan aplikasi Zandor tempat kamu berjalan (bukan sistem berkas). Aksi:",
        "status (panel terbuka, alat aktif, project aktif); list_commands (semua perintah palet + id);",
        "run_command {id}; panel {panel: sidebar|chat|docs|tools|rail, state: open|close|toggle};",
        "open_tool {tool: terminal|jobs|git|changes|files|browser|info|session|trajectory|log};",
        "open_url {url} (http/https di browser Panel 4); open_file {path} (Panel 3, relatif ke folder project);",
        "navigate {page: home|settings|settings/<bagian>|memory|automation|appearance};",
        "list_terminals; close_terminal {id}; get_settings; set_setting {key, value}.",
        "Pakai hanya bila pengguna meminta sesuatu dari tampilan/aplikasi Zandor.",
      ].join(" "),
      args: {
        action: tool.schema.enum(APP_ACTIONS).describe("Aksi yang dijalankan"),
        id: tool.schema.string().optional().describe("id perintah (run_command) atau id terminal (close_terminal)"),
        panel: tool.schema.enum(["sidebar", "chat", "docs", "tools", "rail"]).optional(),
        state: tool.schema.enum(["open", "close", "toggle"]).optional(),
        tool: tool.schema.string().optional().describe("Alat Panel 4 untuk open_tool"),
        url: tool.schema.string().optional(),
        path: tool.schema.string().optional(),
        page: tool.schema.string().optional(),
        key: tool.schema.string().optional().describe("Kunci pengaturan (lihat get_settings)"),
        value: tool.schema.any().optional().describe("Nilai baru pengaturan"),
      },
      async execute(args, ctx) {
        const { action, ...rest } = args;
        const r = await requestControl(
          dir,
          { action, args: rest, sessionID: ctx.sessionID, directory: ctx.directory || ctx.worktree },
          { timeoutMs: 20_000, signal: ctx.abort },
        );
        return format(r);
      },
    }),
    zandor_image: tool({
      description:
        "Buat gambar dari teks dengan model gambar bawaan pengguna (Pengaturan → Provider dan model → Model gambar bawaan). Gambar disimpan sebagai PNG di folder project lalu dibuka di Panel 3. Mengembalikan path berkas.",
      args: {
        prompt: tool.schema.string().min(1).max(4000).describe("Deskripsi gambar"),
        path: tool.schema
          .string()
          .optional()
          .describe("Path .png relatif ke folder project (bawaan: images/<nama>-<waktu>.png)"),
        size: tool.schema.enum(["1024x1024", "1536x1024", "1024x1536"]).optional(),
      },
      async execute(args, ctx) {
        const r = await requestControl(
          dir,
          { action: "generate_image", args, sessionID: ctx.sessionID, directory: ctx.directory || ctx.worktree },
          { timeoutMs: 180_000, signal: ctx.abort },
        );
        return format(r);
      },
    }),
  };
}

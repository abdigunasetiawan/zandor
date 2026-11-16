// Tool `job_*` dan hook prompt untuk background job (ADR-072).
import { mkdirSync } from "node:fs";
import { tool } from "@opencode-ai/plugin";
import { detectPorts, isJobId, JobManager, type JobMeta, resolveCwd } from "./jobs";

const describe = (m: JobMeta, tail: string | null) => {
  const head = [
    `id: ${m.id}`,
    `status: ${m.status}${m.exitCode !== null ? ` (kode ${m.exitCode})` : ""}`,
    `perintah: ${m.command}`,
    `cwd: ${m.cwd}`,
    m.ports.length ? `port: ${m.ports.map((p) => `http://localhost:${p}`).join(", ")}` : "port: (belum terdeteksi)",
  ].join("\n");
  return tail ? `${head}\n--- output terakhir ---\n${tail}` : head;
};

const firstWord = (cmd: string) => cmd.trim().split(/\s+/)[0] ?? cmd;

export function createJobHooks(dir: string) {
  mkdirSync(dir, { recursive: true });
  const jobs = new JobManager({ dir });
  jobs.startPolling();
  const notFound = (id: string) => `Job "${id}" tidak ditemukan. Pakai job_list untuk melihat job yang ada.`;

  return {
    jobs,
    system(sessionID: string | undefined, out: string[]) {
      out.push(
        "Untuk perintah yang tidak selesai sendiri (dev server, watcher, `npm run dev`, `vite`, `--watch`), JANGAN pakai bash: pakai job_start, lalu job_output untuk membaca log. Hentikan dengan job_stop bila tidak diperlukan lagi.",
      );
      const s = sessionID ? jobs.summary(sessionID) : "";
      if (s) out.push(s);
    },
    tool: {
      job_start: tool({
        description:
          "Jalankan perintah lama (dev server, watcher) sebagai background job yang tetap hidup. Mengembalikan id, status, port yang terdeteksi, dan output awal. Pengguna melihat job di panel Jobs.",
        args: {
          command: tool.schema.string().min(1).describe("Perintah shell, mis. `npm run dev`"),
          name: tool.schema.string().optional().describe("Nama singkat, mis. `vite dev`"),
          cwd: tool.schema.string().optional().describe("Folder kerja relatif terhadap folder project"),
          wait_seconds: tool.schema
            .number()
            .min(0)
            .max(60)
            .optional()
            .describe("Tunggu maks. n detik sampai port terdeteksi atau proses keluar (bawaan 8)"),
        },
        async execute(args, ctx) {
          const base = ctx.directory || ctx.worktree;
          const cwd = resolveCwd(base, args.cwd);
          if (!cwd) return `cwd "${args.cwd}" berada di luar folder project; ditolak.`;
          await ctx.ask({
            permission: "bash",
            patterns: [args.command],
            always: [`${firstWord(args.command)} *`],
            metadata: { command: args.command, description: `Background job: ${args.name ?? args.command}` },
          });
          const m = jobs.start({
            command: args.command,
            name: args.name,
            cwd,
            sessionID: ctx.sessionID,
            directory: base,
          });
          ctx.metadata({ title: args.name ?? args.command, metadata: { jobId: m.id } });
          const done = await jobs.wait(m.id, (args.wait_seconds ?? 8) * 1000, (j) => j.ports.length > 0);
          return describe(done ?? m, jobs.output(m.id, 40));
        },
      }),
      job_output: tool({
        description:
          "Baca output terakhir background job. Opsional menunggu sampai teks/regex `wait_for` muncul (mis. `ready|error`).",
        args: {
          id: tool.schema.string().describe("id job"),
          lines: tool.schema.number().min(1).max(1000).optional().describe("Jumlah baris terakhir (bawaan 80)"),
          wait_for: tool.schema.string().optional().describe("Regex (tak peka huruf) yang ditunggu di output"),
          timeout_seconds: tool.schema.number().min(0).max(120).optional().describe("Batas tunggu (bawaan 15)"),
        },
        async execute(args) {
          if (!jobs.get(args.id)) return notFound(args.id);
          let re: RegExp | null = null;
          if (args.wait_for) {
            try {
              re = new RegExp(args.wait_for, "i");
            } catch {
              return `Regex wait_for tidak valid: ${args.wait_for}`;
            }
            const from = (jobs.output(args.id, 100000) ?? "").length;
            await jobs.wait(args.id, (args.timeout_seconds ?? 15) * 1000, (_m, log) => !!re?.test(log.slice(from)));
          }
          const m = jobs.get(args.id);
          return m ? describe(m, jobs.output(args.id, args.lines ?? 80)) : notFound(args.id);
        },
      }),
      job_list: tool({
        description: "Daftar background job sesi ini beserta status dan port.",
        args: {},
        async execute(_a, ctx) {
          const list = jobs.list(ctx.sessionID);
          return list.length ? list.map((m) => describe(m, null)).join("\n\n") : "Tidak ada background job.";
        },
      }),
      job_stop: tool({
        description: "Hentikan background job (beserta proses anaknya).",
        args: { id: tool.schema.string().describe("id job") },
        async execute(args) {
          if (!jobs.stop(args.id)) return notFound(args.id);
          const m = await jobs.wait(args.id, 5000);
          return m ? describe(m, jobs.output(args.id, 10)) : notFound(args.id);
        },
      }),
      job_restart: tool({
        description: "Jalankan ulang background job dengan perintah yang sama.",
        args: {
          id: tool.schema.string().describe("id job"),
          wait_seconds: tool.schema.number().min(0).max(60).optional().describe("Tunggu port (bawaan 8)"),
        },
        async execute(args, ctx) {
          const cur = jobs.get(args.id);
          if (!cur) return notFound(args.id);
          await ctx.ask({
            permission: "bash",
            patterns: [cur.command],
            always: [`${firstWord(cur.command)} *`],
            metadata: { command: cur.command, description: `Jalankan ulang job ${cur.name}` },
          });
          await jobs.restart(args.id);
          const m = await jobs.wait(args.id, (args.wait_seconds ?? 8) * 1000, (j) => j.ports.length > 0);
          return m ? describe(m, jobs.output(args.id, 40)) : notFound(args.id);
        },
      }),
    },
  };
}

export { detectPorts, isJobId };

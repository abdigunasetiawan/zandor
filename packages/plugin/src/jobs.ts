// Background job agent (ADR-072): proses lama (server, watcher) dijalankan sebagai proses anak plugin, bukan lewat
// tool `bash` yang menunggu selesai. Status ditulis ke `<jobsDir>/<id>.json` + `<id>.log` agar UI (Tauri) bisa
// membacanya; aksi UI (stop/restart) datang sebagai berkas kontrol `<jobsDir>/control/<id>.<aksi>`.
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { isAbsolute, join, relative, resolve } from "node:path";
import { isSensitiveEnv } from "./env-guard";

export type JobStatus = "starting" | "running" | "exited" | "failed" | "stopped" | "lost";

export type JobMeta = {
  id: string;
  name: string;
  command: string;
  cwd: string;
  sessionID: string;
  /** Folder project/sesi pemilik (untuk penyaringan di UI). */
  directory: string;
  pid: number | null;
  /** pid proses engine pemilik; job milik engine lain ditandai `lost`. */
  owner: number;
  status: JobStatus;
  exitCode: number | null;
  ports: number[];
  startedAt: number;
  endedAt: number | null;
  restarts: number;
};

export const LOG_LIMIT = 256 * 1024;
const ID_RE = /^job-[a-z0-9]{6,16}$/;
// biome-ignore lint/suspicious/noControlCharactersInRegex: membuang kode warna ANSI dari output.
const ANSI = /\u001b\[[0-9;?]*[ -/]*[@-~]|\u001b\][^\u0007]*\u0007/g;
const PORT_RE = /(?:https?:\/\/)?(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1?\]|::):(\d{2,5})/gi;

export const stripAnsi = (s: string) => s.replace(ANSI, "");

/** Port yang disebut di output (mis. `http://localhost:5173`). */
export function detectPorts(text: string): number[] {
  const out = new Set<number>();
  for (const m of stripAnsi(text).matchAll(PORT_RE)) {
    const n = Number(m[1]);
    if (n > 0 && n < 65536) out.add(n);
  }
  return [...out];
}

/** cwd job: relatif terhadap `base`, tidak boleh keluar dari `base`. */
export function resolveCwd(base: string, cwd?: string): string | null {
  const target = cwd ? (isAbsolute(cwd) ? resolve(cwd) : resolve(base, cwd)) : resolve(base);
  const rel = relative(resolve(base), target);
  if (rel.startsWith("..") || isAbsolute(rel)) return null;
  return target;
}

/** Env proses anak: env engine tanpa kunci sensitif (ADR-022). */
export function childEnv(env: Record<string, string | undefined>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(env)) if (v !== undefined && !isSensitiveEnv(k)) out[k] = v;
  return out;
}

type Live = { meta: JobMeta; proc: ChildProcess | null; log: string; dirty: boolean; stopping: boolean };

export type JobManagerOptions = {
  dir: string;
  /** Pengganti spawn (tes). */
  shell?: { file: string; args: (cmd: string) => string[] };
  now?: () => number;
};

const defaultShell =
  process.platform === "win32"
    ? { file: process.env.ComSpec || "cmd.exe", args: (c: string) => ["/d", "/s", "/c", `"${c}"`] } // /s membuang kutip luar
    : // TODO(port): grup proses (detached + kill(-pid)) agar pohon proses ikut berhenti di unix.
      { file: "/bin/sh", args: (c: string) => ["-c", c] };

export class JobManager {
  private jobs = new Map<string, Live>();
  private timer: ReturnType<typeof setInterval> | null = null;
  private readonly dir: string;
  private readonly control: string;
  private readonly shell: NonNullable<JobManagerOptions["shell"]>;
  private readonly now: () => number;

  constructor(opts: JobManagerOptions) {
    this.dir = opts.dir;
    this.control = join(opts.dir, "control");
    this.shell = opts.shell ?? defaultShell;
    this.now = opts.now ?? Date.now;
    mkdirSync(this.control, { recursive: true });
    this.markLost();
  }

  /** Mulai memantau berkas kontrol dan menulis log secara berkala. */
  startPolling(intervalMs = 700) {
    if (this.timer) return;
    this.timer = setInterval(() => this.tick(), intervalMs);
    (this.timer as { unref?: () => void }).unref?.();
  }

  dispose() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    for (const j of this.jobs.values()) if (j.proc && j.meta.pid) this.kill(j);
    this.flush();
  }

  /** Meta `running`/`starting` milik engine lain (sudah mati) → `lost`. */
  private markLost() {
    for (const f of safeList(this.dir)) {
      if (!f.endsWith(".json")) continue;
      const m = readMeta(join(this.dir, f));
      if (!m || m.owner === process.pid) continue;
      if (m.status === "running" || m.status === "starting") {
        m.status = "lost";
        m.endedAt = m.endedAt ?? this.now();
        writeJson(join(this.dir, f), m);
      }
    }
  }

  start(input: { command: string; name?: string; cwd: string; sessionID: string; directory: string }): JobMeta {
    const id = `job-${Math.random().toString(36).slice(2, 10).padEnd(8, "0")}`;
    const meta: JobMeta = {
      id,
      name: (input.name || input.command).slice(0, 80),
      command: input.command,
      cwd: input.cwd,
      sessionID: input.sessionID,
      directory: input.directory,
      pid: null,
      owner: process.pid,
      status: "starting",
      exitCode: null,
      ports: [],
      startedAt: this.now(),
      endedAt: null,
      restarts: 0,
    };
    const live: Live = { meta, proc: null, log: "", dirty: true, stopping: false };
    this.jobs.set(id, live);
    this.spawn(live);
    return { ...meta };
  }

  private spawn(j: Live) {
    j.stopping = false;
    j.meta.status = "starting";
    j.meta.exitCode = null;
    j.meta.endedAt = null;
    j.meta.ports = [];
    let proc: ChildProcess;
    try {
      proc = spawn(this.shell.file, this.shell.args(j.meta.command), {
        cwd: j.meta.cwd,
        env: childEnv(process.env),
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
        windowsVerbatimArguments: process.platform === "win32",
      });
    } catch (e) {
      this.append(j, `\n[zandor] gagal menjalankan: ${e instanceof Error ? e.message : String(e)}\n`);
      j.meta.status = "failed";
      j.meta.endedAt = this.now();
      this.flush();
      return;
    }
    j.proc = proc;
    j.meta.pid = proc.pid ?? null;
    j.meta.status = "running";
    const onData = (b: Buffer) => this.append(j, b.toString("utf8"));
    proc.stdout?.on("data", onData);
    proc.stderr?.on("data", onData);
    proc.on("error", (e) => {
      this.append(j, `\n[zandor] ${e.message}\n`);
      if (j.proc === proc) this.finish(j, null, "failed");
    });
    proc.on("exit", (code) => {
      if (j.proc !== proc) return;
      this.finish(j, code, j.stopping ? "stopped" : code === 0 ? "exited" : "failed");
    });
    this.flush();
  }

  private finish(j: Live, code: number | null, status: JobStatus) {
    j.proc = null;
    j.meta.exitCode = code;
    j.meta.status = status;
    j.meta.endedAt = this.now();
    j.dirty = true;
    this.flush();
  }

  private append(j: Live, text: string) {
    const clean = stripAnsi(text).replace(/\r(?!\n)/g, "\n");
    j.log = (j.log + clean).slice(-LOG_LIMIT);
    for (const p of detectPorts(clean)) if (!j.meta.ports.includes(p)) j.meta.ports.push(p);
    j.dirty = true;
  }

  private kill(j: Live) {
    const pid = j.meta.pid;
    if (!pid || !j.proc) return;
    j.stopping = true;
    if (process.platform === "win32") {
      // Pohon proses (cmd → node → esbuild, …) harus ikut mati.
      spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], { windowsHide: true });
    } else {
      j.proc.kill("SIGTERM");
    }
  }

  stop(id: string): JobMeta | null {
    const j = this.jobs.get(id);
    if (!j) return null;
    if (j.proc) this.kill(j);
    return { ...j.meta };
  }

  async restart(id: string): Promise<JobMeta | null> {
    const j = this.jobs.get(id);
    if (!j) return null;
    if (j.proc) {
      const proc = j.proc;
      const gone = new Promise<void>((r) => proc.once("exit", () => r()));
      this.kill(j);
      await Promise.race([gone, sleep(5000)]);
    }
    j.meta.restarts += 1;
    this.append(j, `\n[zandor] dijalankan ulang (${j.meta.restarts})\n`);
    this.spawn(j);
    return { ...j.meta };
  }

  /** Hapus job yang sudah berhenti (berkasnya juga). */
  remove(id: string): boolean {
    const j = this.jobs.get(id);
    if (j?.proc) return false;
    this.jobs.delete(id);
    rmSync(join(this.dir, `${id}.json`), { force: true });
    rmSync(join(this.dir, `${id}.log`), { force: true });
    return true;
  }

  get(id: string): JobMeta | null {
    const j = this.jobs.get(id);
    return j ? { ...j.meta } : null;
  }

  list(sessionID?: string): JobMeta[] {
    return [...this.jobs.values()]
      .map((j) => ({ ...j.meta }))
      .filter((m) => !sessionID || m.sessionID === sessionID)
      .sort((a, b) => a.startedAt - b.startedAt);
  }

  /** `lines` baris terakhir output. */
  output(id: string, lines = 80): string | null {
    const j = this.jobs.get(id);
    if (!j) return null;
    const all = j.log.split("\n");
    if (all.length && all[all.length - 1] === "") all.pop();
    return all.slice(-Math.max(1, lines)).join("\n");
  }

  /** Tunggu sampai `until` terpenuhi, proses berhenti, atau waktu habis. */
  async wait(id: string, timeoutMs: number, until?: (j: JobMeta, log: string) => boolean): Promise<JobMeta | null> {
    const deadline = this.now() + timeoutMs;
    for (;;) {
      const j = this.jobs.get(id);
      if (!j) return null;
      const done =
        (j.meta.status !== "running" && j.meta.status !== "starting") ||
        !!until?.(j.meta, j.log) ||
        this.now() >= deadline;
      if (done) {
        this.flush(); // UI langsung melihat status yang sama dengan agent
        return { ...j.meta };
      }
      await sleep(150);
    }
  }

  /** Proses berkas kontrol dari UI lalu tulis status yang berubah. */
  tick() {
    for (const f of safeList(this.control)) {
      const dot = f.lastIndexOf(".");
      const id = f.slice(0, dot);
      const action = f.slice(dot + 1);
      if (!this.jobs.has(id)) continue; // milik instance plugin lain
      rmSync(join(this.control, f), { force: true });
      if (action === "stop") this.stop(id);
      else if (action === "restart") void this.restart(id);
      else if (action === "remove") this.remove(id);
    }
    this.flush();
  }

  flush() {
    for (const j of this.jobs.values()) {
      if (!j.dirty) continue;
      j.dirty = false;
      writeJson(join(this.dir, `${j.meta.id}.json`), j.meta);
      writeAtomic(join(this.dir, `${j.meta.id}.log`), j.log);
    }
  }

  /** Ringkasan untuk disuntik ke prompt sistem (kosong bila sesi tak punya job). */
  summary(sessionID: string): string {
    const jobs = this.list(sessionID);
    if (!jobs.length) return "";
    const rows = jobs.map((m) => {
      const port = m.ports.length ? ` port ${m.ports.join(",")}` : "";
      const code = m.exitCode !== null ? ` kode ${m.exitCode}` : "";
      return `- ${m.id} [${m.status}${code}]${port}: ${m.name}`;
    });
    return `<zandor-jobs>\nBackground job sesi ini (pakai job_output/job_stop/job_restart):\n${rows.join("\n")}\n</zandor-jobs>`;
  }
}

export const isJobId = (s: string) => ID_RE.test(s);

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function safeList(dir: string): string[] {
  try {
    return existsSync(dir) ? readdirSync(dir) : [];
  } catch {
    return [];
  }
}

function readMeta(file: string): JobMeta | null {
  try {
    const v = JSON.parse(readFileSync(file, "utf8")) as JobMeta;
    return typeof v?.id === "string" ? v : null;
  } catch {
    return null;
  }
}

function writeAtomic(file: string, text: string) {
  const tmp = `${file}.tmp`;
  writeFileSync(tmp, text);
  renameSync(tmp, file);
}

const writeJson = (file: string, v: unknown) => writeAtomic(file, JSON.stringify(v));

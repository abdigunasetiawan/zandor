import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, test } from "vitest";
import { childEnv, detectPorts, JobManager, type JobMeta, resolveCwd, stripAnsi } from "./jobs";

const dirs: string[] = [];
const managers: JobManager[] = [];
const tmp = () => {
  const d = mkdtempSync(join(tmpdir(), "zjobs-"));
  dirs.push(d);
  return d;
};
const make = (dir: string) => {
  const m = new JobManager({ dir });
  managers.push(m);
  return m;
};
afterEach(() => {
  for (const m of managers.splice(0)) m.dispose();
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});

// Skrip node kecil sebagai "server": ditulis ke berkas lalu dijalankan lewat shell sistem (seperti `npm run dev`).
let n = 0;
const node = (code: string) => {
  const f = join(tmp(), `s${n++}.cjs`);
  writeFileSync(f, code);
  return `"${process.execPath}" "${f}"`;
};

describe("util job", () => {
  test("deteksi port dari output, termasuk ANSI", () => {
    expect(detectPorts("  ➜  Local:   \u001b[36mhttp://localhost:\u001b[1m5173\u001b[22m/\u001b[39m")).toEqual([5173]);
    expect(detectPorts(stripAnsi("Local: http://localhost:5173/ and http://127.0.0.1:8080"))).toEqual([5173, 8080]);
    expect(detectPorts("listening on 0.0.0.0:3000")).toEqual([3000]);
    expect(detectPorts("versi 1.2:3")).toEqual([]);
  });

  test("cwd tidak boleh keluar dari folder project", () => {
    const base = tmp();
    expect(resolveCwd(base)).toBe(base);
    expect(resolveCwd(base, "web")).toBe(join(base, "web"));
    expect(resolveCwd(base, "..")).toBeNull();
    expect(resolveCwd(base, tmpdir())).toBeNull();
  });

  test("env anak tanpa secret", () => {
    expect(childEnv({ PATH: "/bin", ZANDOR_SECRET_X: "sk", OPENCODE_SERVER_PASSWORD: "pw" })).toEqual({ PATH: "/bin" });
  });
});

describe("JobManager", () => {
  test("menjalankan, mendeteksi port, menulis berkas, lalu berhenti", async () => {
    const dir = tmp();
    const jobs = make(dir);
    const m = jobs.start({
      command: node("console.log('ready on http://localhost:4321');setInterval(()=>{},1000)"),
      cwd: dir,
      sessionID: "s1",
      directory: dir,
    });
    const up = await jobs.wait(m.id, 10000, (j) => j.ports.length > 0);
    expect(up?.status).toBe("running");
    expect(up?.ports).toEqual([4321]);
    jobs.flush();
    const meta = JSON.parse(readFileSync(join(dir, `${m.id}.json`), "utf8")) as JobMeta;
    expect(meta.ports).toEqual([4321]);
    expect(readFileSync(join(dir, `${m.id}.log`), "utf8")).toContain("ready on");
    expect(jobs.summary("s1")).toContain(m.id);
    expect(jobs.summary("lain")).toBe("");

    jobs.stop(m.id);
    const down = await jobs.wait(m.id, 10000);
    expect(down?.status).toBe("stopped");
  }, 20000);

  test("proses yang gagal tercatat dengan kode keluar", async () => {
    const dir = tmp();
    const jobs = make(dir);
    const m = jobs.start({
      command: node("console.error('boom');process.exit(3)"),
      cwd: dir,
      sessionID: "s",
      directory: dir,
    });
    const end = await jobs.wait(m.id, 10000);
    expect(end?.status).toBe("failed");
    expect(end?.exitCode).toBe(3);
    expect(jobs.output(m.id)).toContain("boom");
  }, 20000);

  test("berkas kontrol dari UI: restart lalu stop", async () => {
    const dir = tmp();
    const jobs = make(dir);
    const m = jobs.start({
      command: node("console.log('hi');setInterval(()=>{},1000)"),
      cwd: dir,
      sessionID: "s",
      directory: dir,
    });
    await jobs.wait(m.id, 10000, (_j, log) => log.includes("hi"));
    writeFileSync(join(dir, "control", `${m.id}.restart`), "");
    jobs.tick();
    expect(existsSync(join(dir, "control", `${m.id}.restart`))).toBe(false);
    await new Promise((r) => setTimeout(r, 300));
    await jobs.wait(m.id, 10000, (j) => j.restarts === 1 && j.status === "running");
    expect(jobs.get(m.id)?.restarts).toBe(1);
    writeFileSync(join(dir, "control", `${m.id}.stop`), "");
    jobs.tick();
    expect((await jobs.wait(m.id, 10000))?.status).toBe("stopped");
    writeFileSync(join(dir, "control", `${m.id}.remove`), "");
    jobs.tick();
    expect(existsSync(join(dir, `${m.id}.json`))).toBe(false);
  }, 30000);

  test("job milik engine lain yang masih 'running' ditandai lost", () => {
    const dir = tmp();
    const stale: Partial<JobMeta> = { id: "job-abcdef12", status: "running", owner: -1, endedAt: null };
    writeFileSync(join(dir, "job-abcdef12.json"), JSON.stringify(stale));
    make(dir);
    expect((JSON.parse(readFileSync(join(dir, "job-abcdef12.json"), "utf8")) as JobMeta).status).toBe("lost");
  });
});

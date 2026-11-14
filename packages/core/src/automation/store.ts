// Store automasi terjadwal (Fase 11). Tugas berjalan HANYA selama Zandor terbuka (tidak ada layanan latar); saat aplikasi
// dibuka kembali, tugas yang terlewat dijalankan sekali. Tugas tidak pernah menyetujui izin sendiri: bila agent butuh
// izin, sesi menunggu pengguna (dan pengguna diberi notifikasi).
import { isDue, nextRun, type Schedule, validateSchedule } from "./schedule";

export type AutomationTask = {
  id: string;
  name: string;
  /** id project terdaftar, atau null = percakapan tanpa project. */
  projectId: string | null;
  prompt: string;
  schedule: Schedule;
  enabled: boolean;
  createdAt: number;
  lastRunAt: number | null;
  lastSessionId: string | null;
  lastError: string | null;
};

export type TaskInput = Pick<AutomationTask, "name" | "projectId" | "prompt" | "schedule">;

export interface AutomationRunner {
  /** Buat sesi dan kirim prompt. Mengembalikan id sesi; melempar bila gagal (engine belum siap, project hilang, dst). */
  run(task: AutomationTask): Promise<{ sessionId: string }>;
  /** Engine/model siap menerima tugas sekarang? Bila tidak, tick ditunda (tugas tidak ditandai gagal). */
  ready(): boolean;
}

export type AutomationStorage = { get(): string | null; set(v: string): void };

export const MAX_TASKS = 50;
const NAME_MAX = 60;
const PROMPT_MAX = 4000;

export function validateTask(t: Partial<TaskInput>, now = Date.now()): string | null {
  if (typeof t.name !== "string" || t.name.trim() === "" || t.name.length > NAME_MAX)
    return `nama wajib (maks ${NAME_MAX} karakter)`;
  if (typeof t.prompt !== "string" || t.prompt.trim() === "" || t.prompt.length > PROMPT_MAX)
    return `prompt wajib (maks ${PROMPT_MAX} karakter)`;
  if (t.projectId !== null && typeof t.projectId !== "string") return "project tidak valid";
  return validateSchedule(t.schedule, now);
}

export type AutomationState = { tasks: AutomationTask[]; running: string[] };

type Persisted = { version: 1; tasks: AutomationTask[] };

function sanitize(raw: unknown): AutomationTask[] {
  const p = raw as Persisted | null;
  if (!p || !Array.isArray(p.tasks)) return [];
  const out: AutomationTask[] = [];
  for (const t of p.tasks.slice(0, MAX_TASKS)) {
    if (!t || typeof t.id !== "string") continue;
    // Jadwal "once" yang sudah lewat tetap valid untuk tugas lama: validasi memakai now=0.
    if (validateTask(t, 0) !== null) continue;
    out.push({
      id: t.id,
      name: t.name,
      projectId: t.projectId ?? null,
      prompt: t.prompt,
      schedule: t.schedule,
      enabled: t.enabled !== false,
      createdAt: typeof t.createdAt === "number" ? t.createdAt : 0,
      lastRunAt: typeof t.lastRunAt === "number" ? t.lastRunAt : null,
      lastSessionId: typeof t.lastSessionId === "string" ? t.lastSessionId : null,
      lastError: typeof t.lastError === "string" ? t.lastError.slice(0, 300) : null,
    });
  }
  return out;
}

export class AutomationStore {
  private snap: AutomationState = { tasks: [], running: [] };
  private listeners = new Set<() => void>();
  private runner: AutomationRunner | null = null;

  constructor(
    private readonly storage: AutomationStorage,
    private readonly clock: () => number = Date.now,
    private readonly newId: () => string = () => `at_${Math.random().toString(36).slice(2, 10)}`,
  ) {
    try {
      const t = storage.get();
      this.snap = { tasks: sanitize(t ? JSON.parse(t) : null), running: [] };
    } catch {
      this.snap = { tasks: [], running: [] };
    }
  }

  setRunner(r: AutomationRunner | null) {
    this.runner = r;
  }

  state(): AutomationState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private commit(tasks: AutomationTask[], running = this.snap.running) {
    this.snap = { tasks, running };
    try {
      this.storage.set(JSON.stringify({ version: 1, tasks } satisfies Persisted));
    } catch {
      // penyimpanan penuh: tetap berlaku untuk sesi ini
    }
    for (const l of this.listeners) l();
  }

  /** Tambah tugas. Mengembalikan galat atau id tugas. */
  add(input: TaskInput): { error: string } | { id: string } {
    const err = validateTask(input, this.clock());
    if (err) return { error: err };
    if (this.snap.tasks.length >= MAX_TASKS) return { error: `maksimal ${MAX_TASKS} tugas` };
    const task: AutomationTask = {
      id: this.newId(),
      name: input.name.trim(),
      projectId: input.projectId,
      prompt: input.prompt,
      schedule: input.schedule,
      enabled: true,
      createdAt: this.clock(),
      lastRunAt: null,
      lastSessionId: null,
      lastError: null,
    };
    this.commit([...this.snap.tasks, task]);
    return { id: task.id };
  }

  /** Ubah isi tugas (jadwal baru dihitung dari sekarang). */
  update(id: string, input: TaskInput): string | null {
    const err = validateTask(input, this.clock());
    if (err) return err;
    if (!this.snap.tasks.some((t) => t.id === id)) return "tugas tidak ditemukan";
    this.commit(
      this.snap.tasks.map((t) =>
        t.id === id
          ? { ...t, ...input, name: input.name.trim(), createdAt: this.clock(), lastRunAt: null, lastError: null }
          : t,
      ),
    );
    return null;
  }

  setEnabled(id: string, enabled: boolean) {
    this.commit(
      this.snap.tasks.map((t) =>
        // mengaktifkan lagi tidak mengejar kejadian saat nonaktif
        t.id === id ? { ...t, enabled, ...(enabled && !t.enabled ? { createdAt: this.clock() } : {}) } : t,
      ),
    );
  }

  remove(id: string) {
    this.commit(this.snap.tasks.filter((t) => t.id !== id));
  }

  next(task: AutomationTask): number | null {
    return nextRun(task, this.clock());
  }

  /** Jalankan semua tugas yang jatuh tempo. Mengembalikan id yang dimulai. Dipanggil berkala (mis. tiap 30 dtk). */
  async tick(): Promise<string[]> {
    const runner = this.runner;
    if (!runner?.ready()) return [];
    const now = this.clock();
    const due = this.snap.tasks.filter((t) => t.enabled && !this.snap.running.includes(t.id) && isDue(t, now));
    const started: string[] = [];
    for (const task of due) {
      started.push(task.id);
      await this.execute(task, runner, now);
    }
    return started;
  }

  /** "Jalankan sekarang" (tidak mengubah jadwal reguler selain mencatat lastRunAt). */
  async runNow(id: string): Promise<string | null> {
    const runner = this.runner;
    const task = this.snap.tasks.find((t) => t.id === id);
    if (!task) return "tugas tidak ditemukan";
    if (!runner?.ready()) return "engine belum siap";
    if (this.snap.running.includes(id)) return "tugas sedang berjalan";
    await this.execute(task, runner, this.clock());
    return this.snap.tasks.find((t) => t.id === id)?.lastError ?? null;
  }

  private async execute(task: AutomationTask, runner: AutomationRunner, now: number) {
    this.commit(this.snap.tasks, [...this.snap.running, task.id]);
    let patch: Partial<AutomationTask>;
    try {
      const r = await runner.run(task);
      patch = { lastRunAt: now, lastSessionId: r.sessionId, lastError: null };
    } catch (e) {
      // Gagal tetap dihitung sebagai percobaan (lastRunAt) agar tidak mengulang tiap detak; galat ditampilkan.
      patch = { lastRunAt: now, lastError: (e instanceof Error ? e.message : String(e)).slice(0, 300) };
    }
    this.commit(
      this.snap.tasks.map((t) => (t.id === task.id ? { ...t, ...patch } : t)),
      this.snap.running.filter((x) => x !== task.id),
    );
  }
}

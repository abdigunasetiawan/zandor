import { describe, expect, test } from "vitest";
import { isDue, nextRun, type Schedule, validateSchedule } from "./schedule";
import { type AutomationRunner, AutomationStore, type AutomationTask, validateTask } from "./store";

// Waktu lokal: Rabu 11 Nov 2026 pukul h:m (bulan 0-based).
const at = (day: number, h: number, m = 0) => new Date(2026, 10, day, h, m, 0, 0).getTime();
const task = (schedule: Schedule, over: Partial<{ createdAt: number; lastRunAt: number | null }> = {}) => ({
  schedule,
  createdAt: at(11, 8),
  lastRunAt: null as number | null,
  ...over,
});
const MIN = 60_000;

describe("validateSchedule", () => {
  test("interval: batas 5 menit – 7 hari, bilangan bulat", () => {
    expect(validateSchedule({ kind: "interval", everyMinutes: 5 })).toBeNull();
    expect(validateSchedule({ kind: "interval", everyMinutes: 4 })).toMatch(/interval/);
    expect(validateSchedule({ kind: "interval", everyMinutes: 10081 })).toMatch(/interval/);
    expect(validateSchedule({ kind: "interval", everyMinutes: 7.5 })).toMatch(/interval/);
  });
  test("harian: jam HH:MM dan hari 0–6", () => {
    expect(validateSchedule({ kind: "daily", time: "09:30", days: [] })).toBeNull();
    expect(validateSchedule({ kind: "daily", time: "9:30", days: [] })).toMatch(/HH:MM/);
    expect(validateSchedule({ kind: "daily", time: "24:00", days: [] })).toMatch(/HH:MM/);
    expect(validateSchedule({ kind: "daily", time: "09:30", days: [7] })).toMatch(/0–6/);
  });
  test("sekali: tidak boleh di masa lalu; jenis tak dikenal ditolak", () => {
    const now = at(11, 10);
    expect(validateSchedule({ kind: "once", at: now + 5 * MIN }, now)).toBeNull();
    expect(validateSchedule({ kind: "once", at: now - 5 * MIN }, now)).toMatch(/lewat/);
    expect(validateSchedule({ kind: "cron" })).toMatch(/tidak dikenal/);
    expect(validateSchedule(null)).toMatch(/tidak valid/);
  });
});

describe("isDue / nextRun", () => {
  test("interval: dihitung dari lastRunAt, atau createdAt bila belum pernah jalan", () => {
    const t = task({ kind: "interval", everyMinutes: 30 });
    expect(isDue(t, at(11, 8, 29))).toBe(false);
    expect(isDue(t, at(11, 8, 30))).toBe(true);
    const ran = { ...t, lastRunAt: at(11, 8, 30) };
    expect(isDue(ran, at(11, 8, 59))).toBe(false);
    expect(isDue(ran, at(11, 9, 0))).toBe(true);
    expect(nextRun(ran, at(11, 8, 40))).toBe(at(11, 9, 0));
  });

  test("harian: jalan sekali per kejadian; kejadian sebelum dibuat tidak dikejar", () => {
    const s: Schedule = { kind: "daily", time: "09:00", days: [] };
    const fresh = task(s, { createdAt: at(11, 9, 30) }); // dibuat setelah jam 09:00 hari ini
    expect(isDue(fresh, at(11, 12))).toBe(false);
    expect(isDue(fresh, at(12, 9, 0))).toBe(true);
    const ran = task(s, { createdAt: at(10, 8), lastRunAt: at(11, 9, 0) });
    expect(isDue(ran, at(11, 23))).toBe(false);
    expect(isDue(ran, at(12, 9, 1))).toBe(true);
  });

  test("harian: terlewat beberapa hari → hanya satu kali jatuh tempo (kejar sekali)", () => {
    const t = task({ kind: "daily", time: "09:00", days: [] }, { createdAt: at(1, 8), lastRunAt: at(3, 9) });
    expect(isDue(t, at(11, 12))).toBe(true);
    expect(isDue({ ...t, lastRunAt: at(11, 12) }, at(11, 13))).toBe(false);
  });

  test("harian dengan hari terpilih: hanya Sen/Rab/Jum", () => {
    const s: Schedule = { kind: "daily", time: "09:00", days: [1, 3, 5] };
    const t = task(s, { createdAt: at(1, 8) });
    // 11 Nov 2026 = Rabu (3) → terpilih; 12 Nov = Kamis (4) tidak
    expect(isDue({ ...t, lastRunAt: at(10, 9) }, at(11, 9, 5))).toBe(true);
    expect(isDue({ ...t, lastRunAt: at(11, 9) }, at(12, 12))).toBe(false);
    expect(nextRun({ ...t, lastRunAt: at(11, 9) }, at(11, 10))).toBe(at(13, 9)); // Jumat
  });

  test("sekali: jalan satu kali lalu selesai", () => {
    const t = task({ kind: "once", at: at(11, 10) });
    expect(isDue(t, at(11, 9, 59))).toBe(false);
    expect(isDue(t, at(11, 10, 1))).toBe(true);
    expect(isDue({ ...t, lastRunAt: at(11, 10, 1) }, at(11, 11))).toBe(false);
    expect(nextRun({ ...t, lastRunAt: at(11, 10, 1) }, at(11, 11))).toBeNull();
    expect(nextRun(t, at(11, 9))).toBe(at(11, 10));
  });
});

describe("validateTask", () => {
  const ok = {
    name: "Ringkas berita",
    projectId: null,
    prompt: "ringkas",
    schedule: { kind: "interval", everyMinutes: 60 } as Schedule,
  };
  test("lolos dan galat per kolom", () => {
    expect(validateTask(ok)).toBeNull();
    expect(validateTask({ ...ok, name: " " })).toMatch(/nama/);
    expect(validateTask({ ...ok, prompt: "" })).toMatch(/prompt/);
    expect(validateTask({ ...ok, prompt: "x".repeat(4001) })).toMatch(/prompt/);
    expect(validateTask({ ...ok, schedule: { kind: "interval", everyMinutes: 1 } })).toMatch(/interval/);
  });
});

describe("AutomationStore", () => {
  const mem = () => {
    let v: string | null = null;
    return {
      get: () => v,
      set: (x: string) => {
        v = x;
      },
    };
  };
  function make(opts: { ready?: boolean; fail?: string } = {}) {
    const clock = { now: at(11, 8) };
    const storage = mem();
    let n = 0;
    const store = new AutomationStore(
      storage,
      () => clock.now,
      () => `t${++n}`,
    );
    const calls: AutomationTask[] = [];
    const runner: AutomationRunner = {
      ready: () => opts.ready ?? true,
      async run(t) {
        calls.push(t);
        if (opts.fail) throw new Error(opts.fail);
        return { sessionId: `ses-${t.id}` };
      },
    };
    store.setRunner(runner);
    return { store, clock, calls, storage };
  }
  const input = (schedule: Schedule) => ({ name: "Tugas", projectId: null, prompt: "kerjakan", schedule });

  test("tick menjalankan tugas yang jatuh tempo sekali, mencatat sesi, dan tidak mengulang", async () => {
    const { store, clock, calls } = make();
    const r = store.add(input({ kind: "interval", everyMinutes: 30 }));
    expect("id" in r).toBe(true);
    expect(await store.tick()).toEqual([]);
    clock.now = at(11, 8, 31);
    expect(await store.tick()).toEqual(["t1"]);
    expect(calls).toHaveLength(1);
    expect(store.state().tasks[0]).toMatchObject({
      lastSessionId: "ses-t1",
      lastError: null,
      lastRunAt: at(11, 8, 31),
    });
    expect(await store.tick()).toEqual([]);
    clock.now = at(11, 9, 2);
    expect(await store.tick()).toEqual(["t1"]);
  });

  test("tidak jalan saat nonaktif, saat runner belum siap, dan tugas tidak ditandai gagal karenanya", async () => {
    const off = make();
    off.store.add(input({ kind: "interval", everyMinutes: 5 }));
    off.store.setEnabled("t1", false);
    off.clock.now = at(11, 9);
    expect(await off.store.tick()).toEqual([]);

    const notReady = make({ ready: false });
    notReady.store.add(input({ kind: "interval", everyMinutes: 5 }));
    notReady.clock.now = at(11, 9);
    expect(await notReady.store.tick()).toEqual([]);
    expect(notReady.store.state().tasks[0]?.lastError).toBeNull();
    expect(notReady.store.state().tasks[0]?.lastRunAt).toBeNull();
  });

  test("mengaktifkan kembali tidak mengejar kejadian saat nonaktif", async () => {
    const { store, clock, calls } = make();
    store.add(input({ kind: "interval", everyMinutes: 10 }));
    store.setEnabled("t1", false);
    clock.now = at(11, 12);
    store.setEnabled("t1", true);
    expect(await store.tick()).toEqual([]);
    expect(calls).toHaveLength(0);
    clock.now = at(11, 12, 11);
    expect(await store.tick()).toEqual(["t1"]);
  });

  test("galat runner dicatat, tidak melempar, dan tidak diulang tiap detak", async () => {
    const { store, clock, calls } = make({ fail: "project tidak ditemukan" });
    store.add(input({ kind: "interval", everyMinutes: 30 }));
    clock.now = at(11, 9);
    await store.tick();
    expect(store.state().tasks[0]?.lastError).toBe("project tidak ditemukan");
    await store.tick();
    expect(calls).toHaveLength(1);
  });

  test("runNow berjalan walau belum jatuh tempo; menolak bila tidak ada/engine belum siap/sedang berjalan", async () => {
    const { store, calls } = make();
    store.add(input({ kind: "daily", time: "23:00", days: [] }));
    expect(await store.runNow("t1")).toBeNull();
    expect(calls).toHaveLength(1);
    expect(await store.runNow("zzz")).toMatch(/tidak ditemukan/);
    expect(await make({ ready: false }).store.runNow("t1")).toMatch(/tidak ditemukan|belum siap/);
    const nr = make({ ready: false });
    nr.store.add(input({ kind: "interval", everyMinutes: 5 }));
    expect(await nr.store.runNow("t1")).toBe("engine belum siap");
  });

  test("tugas yang sedang berjalan tidak dijalankan ganda", async () => {
    const clock = { now: at(11, 8) };
    let release: (v: { sessionId: string }) => void = () => {};
    let calls = 0;
    const store = new AutomationStore(mem(), () => clock.now);
    store.setRunner({
      ready: () => true,
      run: () => {
        calls++;
        return new Promise((r) => {
          release = r;
        });
      },
    });
    store.add(input({ kind: "interval", everyMinutes: 5 }));
    clock.now = at(11, 8, 10);
    const first = store.tick();
    await Promise.resolve();
    expect(store.state().running).toHaveLength(1);
    expect(await store.tick()).toEqual([]);
    release({ sessionId: "s" });
    await first;
    expect(calls).toBe(1);
    expect(store.state().running).toEqual([]);
  });

  test("persisten: dimuat ulang; data rusak diabaikan; ubah tugas mengatur ulang jadwal; batas jumlah", () => {
    const { store, storage } = make();
    store.add(input({ kind: "interval", everyMinutes: 30 }));
    expect(new AutomationStore(storage).state().tasks).toHaveLength(1);
    expect(store.update("t1", { ...input({ kind: "interval", everyMinutes: 60 }), name: "Baru" })).toBeNull();
    expect(store.update("t1", { ...input({ kind: "interval", everyMinutes: 1 }) })).toMatch(/interval/);
    expect(store.update("zzz", input({ kind: "interval", everyMinutes: 60 }))).toMatch(/tidak ditemukan/);
    expect(new AutomationStore({ get: () => "{rusak", set: () => {} }).state().tasks).toEqual([]);
    expect(
      new AutomationStore({
        get: () => JSON.stringify({ tasks: [{ id: "x", name: "", prompt: "p", schedule: {} }] }),
        set: () => {},
      }).state().tasks,
    ).toEqual([]);
    store.remove("t1");
    expect(store.state().tasks).toEqual([]);
    for (let i = 0; i < 50; i++) expect("id" in store.add(input({ kind: "interval", everyMinutes: 60 }))).toBe(true);
    expect(store.add(input({ kind: "interval", everyMinutes: 60 }))).toEqual({ error: "maksimal 50 tugas" });
  });
});

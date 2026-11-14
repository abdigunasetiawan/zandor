import { describe, expect, test } from "vitest";
import { type OpenWorkDevice, type OpenWorkHost, type OpenWorkPoll, type ProviderHost, ProviderStore } from "./store";

const flush = () => new Promise((r) => setTimeout(r, 0));
const DEVICE: OpenWorkDevice = {
  userCode: "ABCD-1234",
  verificationUri: "https://app.openworklabs.com/device",
  interval: 5,
  expiresIn: 600,
};

function setup(script: OpenWorkPoll[], opts: { startError?: string } = {}) {
  const calls: string[] = [];
  const queue = [...script];
  const ow: OpenWorkHost = {
    async start() {
      calls.push("start");
      if (opts.startError) throw new Error(opts.startError);
      return DEVICE;
    },
    async poll() {
      calls.push("poll");
      const next = queue.shift();
      if (!next) throw new Error("tanpa skenario");
      return next;
    },
    async cancel() {
      calls.push("cancel");
    },
  };
  const host = {
    secure: true,
    openwork: ow,
    list: async () => {
      calls.push("list");
      return [];
    },
    save: async () => {},
    remove: async () => {},
    probe: async () => ({ ok: true, status: 200, latencyMs: 1, models: [], kind: "ok", error: null }),
    ping: async () => ({ ok: true, status: 200, latencyMs: 1, models: [], kind: "ok", error: null }),
    detectLocal: async () => [],
    openExternal: async () => {},
  } as unknown as ProviderHost;
  // penjadwal tiruan: fungsi yang dijadwalkan dijalankan manual lewat `tick()` dengan jeda yang tercatat
  const timers: Array<{ fn: () => void; ms: number; cancelled: boolean }> = [];
  let changed = 0;
  const store = new ProviderStore({
    host,
    now: () => 1_000_000,
    onChanged: () => {
      changed++;
    },
    schedule: (fn, ms) => {
      const t = { fn, ms, cancelled: false };
      timers.push(t);
      return () => {
        t.cancelled = true;
      };
    },
  });
  const tick = async () => {
    const t = timers.filter((x) => !x.cancelled).at(-1);
    if (!t) throw new Error("tidak ada timer aktif");
    t.cancelled = true;
    t.fn();
    await flush();
  };
  return { store, calls, timers, tick, changed: () => changed };
}

describe("masuk dengan OpenWork (device flow)", () => {
  test("alur sukses: kode tampil → menunggu → disetujui → provider dimuat ulang dan model chat diperbarui", async () => {
    const { store, calls, timers, tick, changed } = setup([
      { status: "pending" },
      { status: "connected", account: "Budi · Tim", models: 7 },
    ]);
    await store.startOpenWork();
    expect(store.state().openwork).toEqual({
      status: "waiting",
      userCode: "ABCD-1234",
      verificationUri: DEVICE.verificationUri,
      expiresAt: 1_000_000 + 600_000,
    });
    expect(timers[0]?.ms).toBe(5000);
    await tick(); // pending → jadwalkan lagi
    expect(store.state().openwork.status).toBe("waiting");
    expect(timers.at(-1)?.ms).toBe(5000);
    await tick(); // connected
    expect(store.state().openwork).toEqual({ status: "connected", account: "Budi · Tim", models: 7 });
    expect(calls).toEqual(["start", "poll", "poll", "list"]);
    expect(changed()).toBe(1);
  });

  test("slow_down memperlambat interval; interval minimum 3 detik", async () => {
    const { store, timers, tick } = setup([
      { status: "slowDown", interval: 10 },
      { status: "slowDown", interval: 1 },
    ]);
    await store.startOpenWork();
    await tick();
    expect(timers.at(-1)?.ms).toBe(10_000);
    await tick();
    expect(timers.at(-1)?.ms).toBe(3000);
  });

  test("ditolak, kedaluwarsa, dan galat dilaporkan apa adanya dan menghentikan polling", async () => {
    for (const [r, want] of [
      [{ status: "denied", message: "ditolak admin" }, "ditolak admin"],
      [{ status: "expired" }, "Kode kedaluwarsa. Mulai lagi untuk mendapat kode baru."],
      [{ status: "error", message: "org_credential_missing" }, "org_credential_missing"],
    ] as const) {
      const { store, timers, tick } = setup([r]);
      await store.startOpenWork();
      await tick();
      expect(store.state().openwork).toEqual({ status: "error", message: want });
      expect(timers.filter((t) => !t.cancelled)).toHaveLength(0);
    }
  });

  test("galat saat memulai (OpenWork tidak terjangkau) → error, tanpa polling", async () => {
    const { store, timers } = setup([], { startError: "OpenWork tidak terjangkau" });
    await store.startOpenWork();
    expect(store.state().openwork).toEqual({ status: "error", message: "OpenWork tidak terjangkau" });
    expect(timers).toHaveLength(0);
  });

  test("batal: timer dihentikan, host diberi tahu, dan poll yang terlambat tidak mengubah state", async () => {
    const { store, calls, timers, tick } = setup([{ status: "connected", account: "x", models: 1 }]);
    await store.startOpenWork();
    const t = timers[0];
    await store.cancelOpenWork();
    expect(store.state().openwork).toEqual({ status: "idle" });
    expect(t?.cancelled).toBe(true);
    expect(calls).toContain("cancel");
    // timer lama terlanjur berjalan setelah dibatalkan → diabaikan
    t?.fn();
    await flush();
    expect(store.state().openwork).toEqual({ status: "idle" });
    await expect(tick()).rejects.toThrow(/tidak ada timer aktif/);
  });

  test("memulai ulang membatalkan alur sebelumnya", async () => {
    const { store, timers } = setup([{ status: "connected", account: "x", models: 1 }]);
    await store.startOpenWork();
    await store.startOpenWork();
    expect(timers[0]?.cancelled).toBe(true);
    expect(timers.filter((t) => !t.cancelled)).toHaveLength(1);
  });

  test("tanpa host OpenWork (build web): pesan jujur", async () => {
    const store = new ProviderStore({
      host: {
        secure: false,
        list: async () => [],
        save: async () => {},
        remove: async () => {},
        probe: async () => ({}) as never,
        ping: async () => ({}) as never,
        detectLocal: async () => [],
        openExternal: async () => {},
      },
    });
    expect(store.openworkAvailable).toBe(false);
    await store.startOpenWork();
    expect(store.state().openwork).toMatchObject({
      status: "error",
      message: expect.stringContaining("hanya tersedia di aplikasi desktop"),
    });
  });
});

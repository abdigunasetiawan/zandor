// Penjadwalan murni untuk automasi (Fase 11). Tidak ada timer di sini: store memanggil `isDue` pada setiap detak.
// Semua perhitungan memakai waktu LOKAL pengguna (jam dinding), karena jadwal harian berarti "jam 09:00 saya".

export type Schedule =
  | { kind: "interval"; everyMinutes: number }
  | { kind: "daily"; time: string; days: number[] }
  | { kind: "once"; at: number };

export const MIN_INTERVAL_MINUTES = 5;
export const MAX_INTERVAL_MINUTES = 7 * 24 * 60;

const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/;

/** Galat validasi jadwal (Bahasa Indonesia) atau null. `days` = 0 (Minggu) … 6 (Sabtu); kosong = setiap hari. */
export function validateSchedule(s: unknown, now = Date.now()): string | null {
  if (!s || typeof s !== "object") return "jadwal tidak valid";
  const o = s as Record<string, unknown>;
  if (o.kind === "interval") {
    const n = o.everyMinutes;
    if (typeof n !== "number" || !Number.isInteger(n) || n < MIN_INTERVAL_MINUTES || n > MAX_INTERVAL_MINUTES)
      return `interval harus bilangan bulat ${MIN_INTERVAL_MINUTES}–${MAX_INTERVAL_MINUTES} menit`;
    return null;
  }
  if (o.kind === "daily") {
    if (typeof o.time !== "string" || !TIME.test(o.time)) return "jam harus berformat HH:MM (24 jam)";
    if (!Array.isArray(o.days) || o.days.some((d) => !Number.isInteger(d) || (d as number) < 0 || (d as number) > 6))
      return "hari harus berisi angka 0–6";
    return null;
  }
  if (o.kind === "once") {
    if (typeof o.at !== "number" || !Number.isFinite(o.at)) return "waktu tidak valid";
    if (o.at < now - 60_000) return "waktu sudah lewat";
    return null;
  }
  return "jenis jadwal tidak dikenal";
}

/** Kejadian harian terakhir yang ≤ `now` (ms), memperhatikan hari yang dipilih; null bila tidak ada dalam 8 hari terakhir. */
function lastDailyOccurrence(time: string, days: number[], now: number): number | null {
  const m = TIME.exec(time);
  if (!m) return null;
  const [h, mi] = [Number(m[1]), Number(m[2])];
  for (let back = 0; back < 8; back++) {
    const d = new Date(now);
    d.setDate(d.getDate() - back);
    d.setHours(h, mi, 0, 0);
    if (d.getTime() > now) continue;
    if (days.length === 0 || days.includes(d.getDay())) return d.getTime();
  }
  return null;
}

export type ScheduledTaskLike = { schedule: Schedule; createdAt: number; lastRunAt: number | null };

/**
 * Apakah tugas jatuh tempo? Kebijakan "kejar sekali": bila aplikasi tertutup melewati beberapa kejadian, yang dijalankan
 * hanya SATU kali saat aplikasi dibuka (bukan semuanya). Kejadian sebelum tugas dibuat tidak pernah dijalankan.
 */
export function isDue(task: ScheduledTaskLike, now: number): boolean {
  const s = task.schedule;
  if (s.kind === "once") return task.lastRunAt === null && s.at <= now;
  if (s.kind === "interval") return now >= (task.lastRunAt ?? task.createdAt) + s.everyMinutes * 60_000;
  const occ = lastDailyOccurrence(s.time, s.days, now);
  if (occ === null) return false;
  return occ >= task.createdAt && (task.lastRunAt === null || task.lastRunAt < occ);
}

/** Waktu jalan berikutnya (ms) atau null (sudah selesai/tidak ada). */
export function nextRun(task: ScheduledTaskLike, now: number): number | null {
  const s = task.schedule;
  if (s.kind === "once") return task.lastRunAt === null ? s.at : null;
  if (s.kind === "interval") return (task.lastRunAt ?? task.createdAt) + s.everyMinutes * 60_000;
  const m = TIME.exec(s.time);
  if (!m) return null;
  for (let ahead = 0; ahead < 8; ahead++) {
    const d = new Date(now);
    d.setDate(d.getDate() + ahead);
    d.setHours(Number(m[1]), Number(m[2]), 0, 0);
    if (d.getTime() <= now) continue;
    if (s.days.length === 0 || s.days.includes(d.getDay())) return d.getTime();
  }
  return null;
}

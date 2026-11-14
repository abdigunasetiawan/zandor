// Logika murni tab "Uji (ping)" pemilih model: kunci status, pengurutan, dan pengelompokan. Tanpa React agar mudah diuji.
import type { ModelOption, PingState } from "@zandor/core";

/** `pingable`: false untuk provider katalog (dikelola engine, tidak bisa diuji dari klien). */
export type PingRow = { model: ModelOption; state: PingState | undefined; pingable: boolean };
export type PingSort = "name" | "provider" | "status" | "latency";
export type PingGroup = "none" | "provider" | "status";

/** Kunci status dari state ping: belum diuji, sedang menguji, atau hasil (ok / jenis galat). */
export function statusKey(state: PingState | undefined): string {
  if (!state) return "untested";
  if (state.status === "pending") return "pending";
  return state.result.ok ? "ok" : state.result.kind;
}

/** Kunci status baris, termasuk "unsupported" untuk model yang tidak bisa diuji. */
export const rowStatus = (r: PingRow): string => (r.pingable ? statusKey(r.state) : "unsupported");

// Urutan tampil status: berhasil dulu, lalu galat, lalu yang sedang/belum diuji, terakhir yang tidak bisa diuji.
const STATUS_ORDER = [
  "ok",
  "ratelimit",
  "auth",
  "notfound",
  "server",
  "network",
  "invalid",
  "pending",
  "untested",
  "unsupported",
];
const statusRank = (k: string) => {
  const i = STATUS_ORDER.indexOf(k);
  return i === -1 ? STATUS_ORDER.length : i;
};

const latencyOf = (s: PingState | undefined) =>
  s && s.status === "done" ? s.result.latencyMs : Number.POSITIVE_INFINITY;

export function sortRows(rows: PingRow[], by: PingSort): PingRow[] {
  const byName = (a: PingRow, b: PingRow) => a.model.name.localeCompare(b.model.name);
  const cmp: Record<PingSort, (a: PingRow, b: PingRow) => number> = {
    name: byName,
    provider: (a, b) => a.model.providerName.localeCompare(b.model.providerName) || byName(a, b),
    status: (a, b) => statusRank(rowStatus(a)) - statusRank(rowStatus(b)) || byName(a, b),
    latency: (a, b) => {
      const x = latencyOf(a.state);
      const y = latencyOf(b.state);
      if (x === y) return byName(a, b);
      return x < y ? -1 : 1;
    },
  };
  return [...rows].sort(cmp[by]);
}

export type RowGroup = { id: string; rows: PingRow[] };

/** Kelompokkan baris (urutan di dalam grup mengikuti urutan masukan). Urutan grup: provider → nama; status → STATUS_ORDER. */
export function groupRows(rows: PingRow[], by: PingGroup): RowGroup[] {
  if (by === "none") return [{ id: "", rows }];
  const map = new Map<string, PingRow[]>();
  for (const r of rows) {
    const id = by === "provider" ? r.model.providerID : rowStatus(r);
    map.set(id, [...(map.get(id) ?? []), r]);
  }
  const groups = [...map.entries()].map(([id, rs]) => ({ id, rows: rs }));
  return by === "provider"
    ? groups.sort((a, b) => (a.rows[0]?.model.providerName ?? "").localeCompare(b.rows[0]?.model.providerName ?? ""))
    : groups.sort((a, b) => statusRank(a.id) - statusRank(b.id));
}

export type PingSummary = { ok: number; failed: number; pending: number; untested: number; unsupported: number };

export function summarize(rows: PingRow[]): PingSummary {
  const out = { ok: 0, failed: 0, pending: 0, untested: 0, unsupported: 0 };
  for (const r of rows) {
    const k = rowStatus(r);
    if (k === "ok") out.ok++;
    else if (k === "pending") out.pending++;
    else if (k === "untested") out.untested++;
    else if (k === "unsupported") out.unsupported++;
    else out.failed++;
  }
  return out;
}

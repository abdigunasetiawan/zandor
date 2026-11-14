import type { ModelOption, PingState } from "@zandor/core";
import { describe, expect, test } from "vitest";
import { groupRows, type PingRow, rowStatus, sortRows, statusKey, summarize } from "./model-ping";

const m = (providerID: string, modelID: string, name = modelID): ModelOption => ({
  providerID,
  providerName: providerID.toUpperCase(),
  modelID,
  name,
  variants: [],
});
const ok = (latencyMs: number): PingState => ({
  status: "done",
  at: 1,
  result: { ok: true, status: 200, latencyMs, models: ["x"], kind: "ok", error: null },
});
const bad = (kind: "auth" | "network"): PingState => ({
  status: "done",
  at: 1,
  result: { ok: false, status: 401, latencyMs: 5, models: [], kind, error: "x" },
});
const base: Array<Omit<PingRow, "pingable">> = [
  { model: m("b", "m2"), state: ok(300) },
  { model: m("a", "m1"), state: bad("auth") },
  { model: m("a", "m3"), state: undefined },
  { model: m("b", "m4"), state: { status: "pending" } },
  { model: m("a", "m5"), state: ok(100) },
];
const rows: PingRow[] = base.map((r) => ({ ...r, pingable: true }));

describe("ping tab: status, sort, group", () => {
  test("kunci status", () => {
    expect(statusKey(undefined)).toBe("untested");
    expect(statusKey({ status: "pending" })).toBe("pending");
    expect(statusKey(ok(1))).toBe("ok");
    expect(statusKey(bad("network"))).toBe("network");
  });
  test("urut status: berhasil → galat → menguji → belum diuji (nama sebagai pembeda)", () => {
    expect(sortRows(rows, "status").map((r) => r.model.modelID)).toEqual(["m2", "m5", "m1", "m4", "m3"]);
  });
  test("urut waktu respons: tercepat dulu (termasuk uji yang gagal), yang belum ada hasil di akhir", () => {
    expect(sortRows(rows, "latency").map((r) => r.model.modelID)).toEqual(["m1", "m5", "m2", "m3", "m4"]);
  });
  test("urut nama dan provider", () => {
    expect(sortRows(rows, "name").map((r) => r.model.modelID)).toEqual(["m1", "m2", "m3", "m4", "m5"]);
    expect(sortRows(rows, "provider").map((r) => r.model.providerID)).toEqual(["a", "a", "a", "b", "b"]);
  });
  test("grup provider dan status; tanpa grup = satu grup", () => {
    expect(groupRows(rows, "none")).toHaveLength(1);
    expect(groupRows(rows, "provider").map((g) => [g.id, g.rows.length])).toEqual([
      ["a", 3],
      ["b", 2],
    ]);
    expect(groupRows(rows, "status").map((g) => g.id)).toEqual(["ok", "auth", "pending", "untested"]);
  });
  test("ringkasan", () => {
    expect(summarize(rows)).toEqual({ ok: 2, failed: 1, pending: 1, untested: 1, unsupported: 0 });
  });
  test("model provider katalog (tidak bisa diuji) tetap tampil: status 'unsupported', diurutkan terakhir", () => {
    const withBuiltin: PingRow[] = [...rows, { model: m("opencode", "big", "big"), state: undefined, pingable: false }];
    const last = withBuiltin.find((r) => !r.pingable);
    expect(last && rowStatus(last)).toBe("unsupported");
    expect(sortRows(withBuiltin, "status").at(-1)?.model.providerID).toBe("opencode");
    expect(groupRows(withBuiltin, "status").at(-1)?.id).toBe("unsupported");
    expect(summarize(withBuiltin).unsupported).toBe(1);
  });
});

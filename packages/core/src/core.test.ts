import { describe, expect, test } from "vitest";
import { buildInfo, createLogger, createMemorySink, REDACTED, redact } from "./index";

describe("logger", () => {
  test("menyaring level di bawah minimum", () => {
    const mem = createMemorySink();
    const log = createLogger("app", { level: "info", sinks: [mem.sink], now: () => 1 });
    log.debug("tidak tampil");
    log.info("tampil");
    log.error("galat", { kode: 1 });
    expect(mem.entries().map((e) => [e.level, e.msg])).toEqual([
      ["info", "tampil"],
      ["error", "galat"],
    ]);
  });

  test("child menambah scope", () => {
    const mem = createMemorySink();
    createLogger("app", { sinks: [mem.sink] })
      .child("engine")
      .warn("x");
    expect(mem.entries()[0]?.scope).toBe("app.engine");
  });

  test("ring buffer membuang entri tertua", () => {
    const mem = createMemorySink(2);
    const log = createLogger("a", { sinks: [mem.sink] });
    for (const m of ["1", "2", "3"]) log.info(m);
    expect(mem.entries().map((e) => e.msg)).toEqual(["2", "3"]);
  });

  test("sink yang melempar tidak menjatuhkan pemanggil", () => {
    const mem = createMemorySink();
    const log = createLogger("a", {
      sinks: [
        () => {
          throw new Error("rusak");
        },
        mem.sink,
      ],
    });
    expect(() => log.error("x")).not.toThrow();
    expect(mem.entries()).toHaveLength(1);
  });

  test("secret disamarkan di pesan dan data", () => {
    const mem = createMemorySink();
    const log = createLogger("a", { sinks: [mem.sink] });
    log.info("kirim Bearer abcdefghijklmnop ke sk-1234567890abcdef", {
      apiKey: "sk-rahasia",
      nested: { OPENCODE_SERVER_PASSWORD: "x", url: "http://127.0.0.1:4096" },
      headers: [{ Authorization: "Basic b3BlbmNvZGU6eA==" }],
    });
    const e = mem.entries()[0];
    expect(e?.msg).toBe(`kirim ${REDACTED} ke ${REDACTED}`);
    expect(e?.data).toEqual({
      apiKey: REDACTED,
      nested: { OPENCODE_SERVER_PASSWORD: REDACTED, url: "http://127.0.0.1:4096" },
      headers: [{ Authorization: REDACTED }],
    });
  });

  test("redact aman terhadap siklus dan Error", () => {
    const a: Record<string, unknown> = { nama: "a" };
    a.diri = a;
    expect(redact(a)).toEqual({ nama: "a", diri: "[siklus]" });
    const r = redact(new Error("token sk-abcdefghijkl")) as { message: string };
    expect(r.message).toBe(`token ${REDACTED}`);
  });
});

describe("buildInfo", () => {
  test("dev = 0.0.<jumlah commit>", () => {
    expect(buildInfo({ packageVersion: "0.1.0", commitCount: "42", isRelease: false })).toEqual({
      version: "0.0.42",
      commitCount: 42,
      channel: "dev",
    });
  });
  test("rilis = semver paket", () => {
    expect(buildInfo({ packageVersion: "0.1.0", commitCount: 42, isRelease: true }).version).toBe("0.1.0");
  });
  test("jumlah commit tidak valid → 0.0.0 tanpa crash", () => {
    expect(buildInfo({ packageVersion: "0.1.0", commitCount: "abc", isRelease: false }).version).toBe("0.0.0");
    expect(buildInfo({ packageVersion: "0.1.0", commitCount: null, isRelease: false }).commitCount).toBeNull();
  });
  test("versi paket wajib semver", () => {
    expect(() => buildInfo({ packageVersion: "v1", commitCount: 1, isRelease: true })).toThrow();
  });
});

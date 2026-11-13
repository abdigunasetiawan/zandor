import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test, vi } from "vitest";
import {
  defaultSettings,
  SETTING_KEYS,
  SETTINGS_SCHEMA,
  type SettingsStorage,
  SettingsStore,
  schemaJson,
  schemaMarkdown,
  validateSetting,
  validateSettings,
} from "./settings";

function memoryStorage(initial: string | null = null): SettingsStorage & { raw: () => string | null } {
  let v = initial;
  return {
    get: () => v,
    set: (x) => {
      v = x;
    },
    raw: () => v,
  };
}

describe("skema", () => {
  test("default valid terhadap skemanya sendiri", () => {
    expect(validateSettings(defaultSettings())).toMatchObject({ valid: true, errors: [] });
  });
  test("setiap properti punya judul, deskripsi, default, dan metadata UI", () => {
    for (const k of SETTING_KEYS) {
      const p = SETTINGS_SCHEMA.properties[k];
      expect(p.title.length, k).toBeGreaterThan(0);
      expect(p.description.length, k).toBeGreaterThan(0);
      expect(p["x-zandor"].titleKey, k).toMatch(/^[a-z]+\./);
      expect(validateSetting(k, p.default), k).toBeNull();
    }
  });
  test("additionalProperties false", () => {
    expect(SETTINGS_SCHEMA.additionalProperties).toBe(false);
  });
});

describe("validateSetting", () => {
  test.each([
    ["language", "fr"],
    ["language", 1],
    ["theme", "biru"],
    ["uiScale", 2],
    ["uiScale", "1"],
    ["uiScale", Number.NaN],
    ["rememberLayout", "ya"],
    ["tidakAda", 1],
  ])("%s = %j ditolak", (k, v) => {
    expect(validateSetting(k, v)).not.toBeNull();
  });
  test.each([
    ["language", "en"],
    ["theme", "dark"],
    ["uiScale", 1.25],
    ["rememberLayout", false],
  ])("%s = %j diterima", (k, v) => {
    expect(validateSetting(k, v)).toBeNull();
  });
});

describe("validateSettings", () => {
  test("nilai salah dilaporkan tetapi sisanya tetap dipakai; value selalu utuh", () => {
    const r = validateSettings({ language: "en", theme: "ungu", zzz: 1 });
    expect(r.valid).toBe(false);
    expect(r.errors).toHaveLength(2);
    expect(r.value).toEqual({ ...defaultSettings(), language: "en" });
  });
  test("bukan objek", () => {
    for (const bad of [null, 1, "x", [1]]) expect(validateSettings(bad).valid).toBe(false);
  });
});

describe("SettingsStore", () => {
  test("default saat kosong; set menyimpan hanya nilai eksplisit", () => {
    const storage = memoryStorage();
    const s = new SettingsStore({ storage });
    expect(s.all()).toEqual(defaultSettings());
    s.set("theme", "dark");
    expect(JSON.parse(storage.raw() as string)).toEqual({ theme: "dark" });
    expect(s.isExplicit("theme")).toBe(true);
    expect(s.isExplicit("language")).toBe(false);
  });

  test("dimuat ulang dari penyimpanan", () => {
    const s = new SettingsStore({ storage: memoryStorage('{"uiScale":1.5,"language":"en"}') });
    expect(s.get("uiScale")).toBe(1.5);
    expect(s.get("language")).toBe("en");
    expect(s.isExplicit("language")).toBe(true);
  });

  test("nilai tersimpan yang tidak valid dibuang per properti dan dilaporkan; yang valid tetap", () => {
    const onInvalid = vi.fn();
    const s = new SettingsStore({ storage: memoryStorage('{"theme":"ungu","language":"en","x":1}'), onInvalid });
    expect(s.get("theme")).toBe("system");
    expect(s.get("language")).toBe("en");
    expect(onInvalid).toHaveBeenCalledOnce();
  });

  test("JSON rusak → default + dilaporkan, tidak melempar", () => {
    const onInvalid = vi.fn();
    const s = new SettingsStore({ storage: memoryStorage("{rusak"), onInvalid });
    expect(s.all()).toEqual(defaultSettings());
    expect(onInvalid).toHaveBeenCalledOnce();
  });

  test("set nilai tidak valid melempar dan tidak mengubah apa pun", () => {
    const storage = memoryStorage();
    const s = new SettingsStore({ storage });
    expect(() => s.set("uiScale", 3 as never)).toThrow(/uiScale/);
    expect(s.get("uiScale")).toBe(1);
    expect(storage.raw()).toBeNull();
  });

  test("reset menghapus dari penyimpanan dan mengembalikan default", () => {
    const storage = memoryStorage();
    const s = new SettingsStore({ storage });
    s.set("theme", "light");
    s.reset("theme");
    expect(s.get("theme")).toBe("system");
    expect(JSON.parse(storage.raw() as string)).toEqual({});
  });

  test("subscribe: dipanggil saat berubah, snapshot stabil bila tidak", () => {
    const s = new SettingsStore({ storage: memoryStorage() });
    const l = vi.fn();
    s.subscribe(l);
    const a = s.all();
    expect(s.all()).toBe(a);
    s.set("theme", "dark");
    expect(l).toHaveBeenCalledTimes(1);
    expect(s.all()).not.toBe(a);
    s.set("theme", "dark"); // sama → tidak memicu
    expect(l).toHaveBeenCalledTimes(1);
  });

  test("galat penyimpanan tidak menjatuhkan set", () => {
    const s = new SettingsStore({
      storage: {
        get: () => null,
        set: () => {
          throw new Error("penuh");
        },
      },
    });
    expect(() => s.set("theme", "dark")).not.toThrow();
    expect(s.get("theme")).toBe("dark");
  });
});

describe("dokumentasi dihasilkan dari skema", () => {
  const manual = (f: string) => readFileSync(join(import.meta.dirname, "..", "..", "..", "docs", "manual", f), "utf8");

  test("settings.schema.json di docs/manual sama dengan skema (jalankan: bun scripts/gen-settings-docs.ts)", () => {
    expect(manual("settings.schema.json").replace(/\r\n/g, "\n")).toBe(schemaJson());
  });
  test("settings.md di docs/manual sama dengan hasil generator", () => {
    expect(manual("settings.md").replace(/\r\n/g, "\n")).toBe(schemaMarkdown());
  });
});

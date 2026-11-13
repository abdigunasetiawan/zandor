import { describe, expect, test } from "vitest";
import {
  catalogs,
  createTranslator,
  DEFAULT_LOCALE,
  detectLocale,
  LOCALES,
  type MessageKey,
  placeholders,
} from "./index";

describe("katalog", () => {
  const keys = Object.keys(catalogs.id).sort();

  test.each(LOCALES)("%s punya kunci yang persis sama dengan id", (locale) => {
    expect(Object.keys(catalogs[locale]).sort()).toEqual(keys);
  });

  test.each(LOCALES)("%s tidak punya pesan kosong", (locale) => {
    for (const [k, v] of Object.entries(catalogs[locale])) expect(v.trim(), k).not.toBe("");
  });

  test.each(LOCALES)("%s memakai placeholder yang sama dengan id", (locale) => {
    for (const k of keys as MessageKey[]) {
      expect(placeholders(catalogs[locale][k]), `${locale}:${k}`).toEqual(placeholders(catalogs.id[k]));
    }
  });
});

describe("translator", () => {
  test("mengisi placeholder", () => {
    expect(createTranslator("id")("app.version", { version: "0.0.7" })).toBe("Versi 0.0.7");
    expect(createTranslator("en")("app.version", { version: "0.0.7" })).toBe("Version 0.0.7");
  });

  test("placeholder tanpa nilai dibiarkan terlihat", () => {
    expect(createTranslator("id")("app.version")).toBe("Versi {version}");
  });
});

describe("detectLocale", () => {
  test.each([
    ["id-ID", "id"],
    ["en-US", "en"],
    ["EN", "en"],
    ["fr-FR", DEFAULT_LOCALE],
    ["", DEFAULT_LOCALE],
    [undefined, DEFAULT_LOCALE],
  ])("%s → %s", (tag, want) => {
    expect(detectLocale(tag)).toBe(want);
  });
});

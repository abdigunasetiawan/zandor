import { en } from "./locales/en";
import { id, type MessageKey, type Messages } from "./locales/id";

export type { MessageKey, Messages };
export const LOCALES = ["id", "en"] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "id";

export const catalogs: Record<Locale, Messages> = { id, en };

export type Params = Record<string, string | number>;
export type Translate = (key: MessageKey, params?: Params) => string;

const PLACEHOLDER = /\{(\w+)\}/g;

/** Daftar placeholder `{nama}` dalam sebuah pesan (terurut, unik). */
export function placeholders(message: string): string[] {
  return [...new Set([...message.matchAll(PLACEHOLDER)].map((m) => m[1] as string))].sort();
}

/** Pilih locale dari tag bahasa (mis. `navigator.language`), jatuh ke default. */
export function detectLocale(tag: string | undefined | null): Locale {
  const base = (tag ?? "").toLowerCase().split(/[-_]/)[0];
  return (LOCALES as readonly string[]).includes(base ?? "") ? (base as Locale) : DEFAULT_LOCALE;
}

export function createTranslator(locale: Locale): Translate {
  const messages = catalogs[locale];
  return (key, params) =>
    messages[key].replace(PLACEHOLDER, (whole, name: string) => {
      const value = params?.[name];
      if (value === undefined) {
        // Placeholder tanpa nilai adalah bug pemanggil: tampilkan apa adanya agar terlihat di UI/test.
        return whole;
      }
      return String(value);
    });
}

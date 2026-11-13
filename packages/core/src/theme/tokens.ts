// Kontrak tema Zandor (Fase 10): daftar token yang boleh diubah dan validator nilainya. Semua nilai dari berkas
// impor/UI WAJIB lewat validator ini sebelum menyentuh CSS (cegah injeksi `;`, `url(...)`, `expression`, dsb.).

export const COLOR_KEYS = [
  "bg",
  "surface",
  "raised",
  "hover",
  "active",
  "border",
  "border-strong",
  "fg",
  "fg-muted",
  "fg-subtle",
  "accent",
  "on-accent",
  "focus",
  "backdrop",
  "danger",
  "success",
  "warning",
  "shadow-color",
] as const;
export type ColorKey = (typeof COLOR_KEYS)[number];
export type ColorMap = Partial<Record<ColorKey, string>>;

export const TYPE_VARS = ["h1", "h2", "h3", "h4", "p", "small", "link", "button", "label", "code"] as const;
export type TypeVar = (typeof TYPE_VARS)[number];
export const TYPE_PROPS = ["size", "weight", "lh", "tracking"] as const;
export type TypeProp = (typeof TYPE_PROPS)[number];

export const FONT_SLOTS = ["ui", "code", "terminal"] as const;
export type FontSlot = (typeof FONT_SLOTS)[number];

const HEX = /^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const FUNC_COLOR = /^(?:rgb|rgba|hsl|hsla)\(\s*[\d.]+%?(?:\s*[, ]\s*[\d.]+%?){2}(?:\s*[,/]\s*[\d.]+%?)?\s*\)$/i;

export const isColor = (v: unknown): v is string => typeof v === "string" && (HEX.test(v) || FUNC_COLOR.test(v));

const num = (v: string) => Number.parseFloat(v);

/** Validasi nilai satu properti tipografi. null = valid. */
export function validateTypeValue(prop: TypeProp, v: unknown): string | null {
  if (typeof v !== "string") return "harus string";
  if (prop === "size") {
    if (!/^\d*\.?\d+rem$/.test(v)) return "ukuran harus dalam rem, mis. 0.875rem";
    const n = num(v);
    return n >= 0.5 && n <= 4 ? null : "ukuran harus 0.5rem–4rem";
  }
  if (prop === "tracking") {
    if (!/^-?\d*\.?\d+em$/.test(v) && v !== "0") return "jarak huruf harus dalam em, mis. -0.01em";
    const n = num(v);
    return n >= -0.1 && n <= 0.5 ? null : "jarak huruf harus -0.1em–0.5em";
  }
  if (prop === "weight") {
    if (!/^\d{3}$/.test(v)) return "ketebalan harus 100–900";
    const n = Number(v);
    return n >= 100 && n <= 900 ? null : "ketebalan harus 100–900";
  }
  if (!/^\d*\.?\d+$/.test(v)) return "tinggi baris harus angka, mis. 1.5";
  const n = num(v);
  return n >= 1 && n <= 3 ? null : "tinggi baris harus 1–3";
}

/** Daftar font: nama dan koma saja (tanpa `;`, `{`, `(`, `url`). */
export function validateFontFamily(v: unknown): string | null {
  if (typeof v !== "string" || v.trim() === "") return "nama font kosong";
  if (v.length > 200) return "nama font terlalu panjang";
  if (!/^[\p{L}\p{N} ,"'._-]+$/u.test(v)) return "nama font hanya boleh huruf, angka, spasi, koma, tanda kutip";
  if (/url|expression|import/i.test(v)) return "nama font tidak valid";
  return null;
}

export type ThemeDef = {
  /** kebab-case; tidak boleh sama dengan preset bawaan. */
  id: string;
  name: string;
  dark?: ColorMap;
  light?: ColorMap;
};

const ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

/** Bersihkan objek tema mentah (impor). Mengembalikan tema valid atau daftar galat; nilai salah ditolak (tidak diperbaiki diam-diam). */
export function sanitizeTheme(raw: unknown): { theme: ThemeDef | null; errors: string[] } {
  const errors: string[] = [];
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    return { theme: null, errors: ["tema harus berupa objek JSON"] };
  }
  const o = raw as Record<string, unknown>;
  if (typeof o.id !== "string" || !ID.test(o.id) || o.id.length > 40) errors.push("id harus kebab-case (maks 40)");
  if (typeof o.name !== "string" || o.name.trim() === "" || o.name.length > 40)
    errors.push("name wajib (maks 40 karakter)");
  const modes: Pick<ThemeDef, "dark" | "light"> = {};
  for (const mode of ["dark", "light"] as const) {
    const m = o[mode];
    if (m === undefined) continue;
    if (m === null || typeof m !== "object" || Array.isArray(m)) {
      errors.push(`${mode}: harus objek`);
      continue;
    }
    const out: ColorMap = {};
    for (const [k, v] of Object.entries(m)) {
      if (!(COLOR_KEYS as readonly string[]).includes(k)) errors.push(`${mode}.${k}: token tidak dikenal`);
      else if (!isColor(v)) errors.push(`${mode}.${k}: warna tidak valid`);
      else out[k as ColorKey] = v;
    }
    modes[mode] = out;
  }
  if (!modes.dark && !modes.light) errors.push("tema harus punya `dark` dan/atau `light`");
  if (errors.length) return { theme: null, errors };
  return { theme: { id: o.id as string, name: (o.name as string).trim(), ...modes }, errors: [] };
}

// ---- kontras WCAG (untuk peringatan di editor dan uji preset) ----

function rgb(hex: string): [number, number, number] | null {
  if (!HEX.test(hex)) return null;
  let h = hex.slice(1);
  if (h.length <= 4) h = [...h].map((c) => c + c).join("");
  return [0, 2, 4].map((i) => Number.parseInt(h.slice(i, i + 2), 16)) as [number, number, number];
}

function luminance(c: [number, number, number]): number {
  const [r, g, b] = c.map((v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Rasio kontras WCAG 2 antara dua warna heks; null bila bukan heks. */
export function contrastRatio(a: string, b: string): number | null {
  const x = rgb(a);
  const y = rgb(b);
  if (!x || !y) return null;
  const [hi, lo] = [luminance(x), luminance(y)].sort((p, q) => q - p) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

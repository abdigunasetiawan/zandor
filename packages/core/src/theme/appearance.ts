// Penampilan yang bisa dikustomisasi (Fase 10): preset/tema kustom, tipografi per variabel, font, panel chat, dan gaya
// section project. Murni data + store; penerapan ke DOM ada di aplikasi. Semua masukan divalidasi (tokens.ts).
import { parseCombo } from "../shortcuts";
import { codeVarsFor } from "./code-colors";
import { validateGradientCss } from "./gradient";
import { DEFAULT_PRESET, PRESETS, presetById } from "./presets";
import {
  type ColorKey,
  type FontSlot,
  isColor,
  sanitizeTheme,
  type ThemeDef,
  TYPE_PROPS,
  TYPE_VARS,
  type TypeProp,
  type TypeVar,
  validateFontFamily,
  validateTypeValue,
} from "./tokens";
import { BUILTIN_EDITOR_THEMES, type EditorTheme, sanitizeEditorTheme } from "./vscode-theme";

export type ChatStyle = {
  /** 0.3–1 */
  opacity?: number;
  /** 100–900 */
  weight?: number;
  /** rem */
  size?: string;
  /** rem: jarak antarpesan */
  gap?: string;
  font?: string;
  /** rem: lebar kolom isi chat (28–120) */
  width?: string;
  /** 1–2.4: tinggi baris teks pesan */
  lineHeight?: number;
  /** em: jarak antarhuruf (-0.05–0.2) */
  tracking?: string;
  /** rem: ukuran huruf kode di pesan */
  codeSize?: string;
  /** 1–1.6: lebar composer relatif terhadap lebar isi */
  composerRatio?: number;
};

/** Semua kunci gaya chat (urutan tampil di UI dan sanitasi). */
export const CHAT_STYLE_KEYS = [
  "width",
  "composerRatio",
  "font",
  "size",
  "weight",
  "opacity",
  "lineHeight",
  "tracking",
  "gap",
  "codeSize",
] as const satisfies readonly (keyof ChatStyle)[];

/** Tema baca dokumen hasil render (Markdown, Office, notebook). "auto" = ikut tema aplikasi. */
export const DOC_THEMES = ["auto", "paper", "sepia", "night"] as const;
export type DocTheme = (typeof DOC_THEMES)[number];
/** Latar penampil gambar. */
export const DOC_IMAGE_BGS = ["auto", "checker", "light", "dark"] as const;
export type DocImageBg = (typeof DOC_IMAGE_BGS)[number];

/** Kustomisasi Panel 3 (dokumen), dikelompokkan per jenis penampil. */
export type DocStyle = {
  /** px: ukuran huruf editor kode/mentah (Monaco), 8–40. Ctrl+roda mouse mengubahnya. */
  codeSize?: number;
  /** 1–2.4: tinggi baris editor (kelipatan ukuran huruf) */
  codeLineHeight?: number;
  codeFont?: string;
  minimap?: boolean;
  lineNumbers?: boolean;
  ligatures?: boolean;
  whitespace?: boolean;
  /** Tema baca dokumen hasil render */
  theme?: DocTheme;
  proseFont?: string;
  /** rem: 0.6–2 */
  proseSize?: string;
  /** 1–2.4 */
  proseLine?: number;
  /** rem: lebar kolom baca 28–120 */
  proseWidth?: string;
  /** rem: ukuran huruf tabel/JSON 0.6–1.6 */
  tableSize?: string;
  imageBg?: DocImageBg;
};

export const DOC_STYLE_KEYS = [
  "codeSize",
  "codeLineHeight",
  "codeFont",
  "minimap",
  "lineNumbers",
  "ligatures",
  "whitespace",
  "theme",
  "proseFont",
  "proseSize",
  "proseLine",
  "proseWidth",
  "tableSize",
  "imageBg",
] as const satisfies readonly (keyof DocStyle)[];

export const DOC_CODE_SIZE = { min: 8, max: 40, def: 13 } as const;

const remIn = (v: unknown, lo: number, hi: number, what: string): string | null => {
  if (typeof v !== "string" || !/^\d*\.?\d+rem$/.test(v)) return "nilai harus dalam rem, mis. 0.9rem";
  const n = Number.parseFloat(v);
  return n >= lo && n <= hi ? null : `${what} harus ${lo}rem–${hi}rem`;
};

export function validateDoc(key: keyof DocStyle, v: unknown): string | null {
  switch (key) {
    case "codeSize":
      return typeof v === "number" && Number.isInteger(v) && v >= DOC_CODE_SIZE.min && v <= DOC_CODE_SIZE.max
        ? null
        : `ukuran huruf kode harus ${DOC_CODE_SIZE.min}–${DOC_CODE_SIZE.max}`;
    case "codeLineHeight":
    case "proseLine":
      return typeof v === "number" && v >= 1 && v <= 2.4 ? null : "tinggi baris harus 1–2.4";
    case "codeFont":
    case "proseFont":
      return validateFontFamily(v);
    case "minimap":
    case "lineNumbers":
    case "ligatures":
    case "whitespace":
      return typeof v === "boolean" ? null : "nilai harus boolean";
    case "theme":
      return (DOC_THEMES as readonly unknown[]).includes(v) ? null : "tema dokumen tidak dikenal";
    case "imageBg":
      return (DOC_IMAGE_BGS as readonly unknown[]).includes(v) ? null : "latar gambar tidak dikenal";
    case "proseSize":
      return remIn(v, 0.6, 2, "ukuran");
    case "proseWidth":
      return remIn(v, 28, 120, "lebar");
    case "tableSize":
      return remIn(v, 0.6, 1.6, "ukuran");
  }
}

/**
 * Gaya tampilan satu project di sidebar. `background` = warna, `gradient` = string CSS gradien (dari generator), `image` =
 * gambar latar (data URL kecil). Urutan prioritas tampil: image > gradient > background.
 */
export type ProjectStyle = {
  border?: string;
  background?: string;
  logo?: string;
  gradient?: string;
  image?: string;
  /** Logo berupa gambar (data URL kecil); menggantikan logo teks bila ada. */
  logoImage?: string;
  /** Posisi vertikal gambar latar dalam persen (0 = atas, 100 = bawah); bawaan 50. */
  imageY?: string;
  /** Skala gambar latar dalam persen terhadap `cover` (100–300); bawaan 100. */
  imageZoom?: string;
  /** Lapisan di atas latar agar teks terbaca: penuh atau gradasi (kiri pekat → kanan transparan). */
  overlay?: string;
  /** Warna lapisan overlay; bawaan hitam. */
  overlayColor?: string;
  /** Kepekatan overlay dalam persen (0–100); bawaan 50. */
  overlayOpacity?: string;
  /** Warna teks dan ikon baris project (folder, +, ⋯). */
  fg?: string;
};

export const PROJECT_OVERLAYS = ["full", "gradient", "gradient-rtl", "gradient-ttb", "gradient-btt"] as const;
export const DEFAULT_OVERLAY_COLOR = "#000000";

function intIn(v: unknown, min: number, max: number): string | null {
  const n = typeof v === "string" && /^\d{1,3}$/.test(v) ? Number(v) : Number.NaN;
  return n >= min && n <= max ? null : `angka harus ${min}–${max}`;
}

/** Batas panjang data URL gambar gaya project (~0,7 MB). */
export const MAX_STYLE_IMAGE_CHARS = 700_000;
const IMAGE_DATA_URL = /^data:image\/(?:png|jpeg|webp|gif);base64,[A-Za-z0-9+/=]+$/;
export function validateImageDataUrl(v: unknown, maxChars = MAX_STYLE_IMAGE_CHARS): string | null {
  if (typeof v !== "string" || !IMAGE_DATA_URL.test(v)) return "gambar harus berupa data URL png/jpeg/webp/gif";
  return v.length > maxChars ? "gambar terlalu besar" : null;
}

/** Gaya ilustrasi avatar (DiceBear, dirender lokal). Hanya gaya berlisensi CC0/CC BY 4.0. */
export const AVATAR_STYLES = [
  "lorelei",
  "notionists",
  "openPeeps",
  "adventurer",
  "bigSmile",
  "funEmoji",
  "pixelArt",
  "thumbs",
] as const;
export type AvatarStyle = (typeof AVATAR_STYLES)[number];
export type ProfileAvatar = { kind: "image"; src: string } | { kind: "illustration"; style: AvatarStyle; seed: string };
/** Profil pengguna lokal. Nama kosong → nama akun OS; avatar kosong → inisial. */
export type Profile = { name?: string; avatar?: ProfileAvatar };

export const MAX_PROFILE_NAME = 40;
export const MAX_AVATAR_IMAGE_CHARS = 200_000;

export function validateProfileName(v: unknown): string | null {
  if (typeof v !== "string" || v.trim() === "") return "nama kosong";
  if ([...v].length > MAX_PROFILE_NAME) return `nama maksimal ${MAX_PROFILE_NAME} karakter`;
  return [...v].some((c) => (c.codePointAt(0) ?? 0) < 32) ? "nama mengandung karakter terlarang" : null;
}

export function validateProfileAvatar(v: unknown): string | null {
  if (!v || typeof v !== "object") return "avatar tidak valid";
  const a = v as Record<string, unknown>;
  if (a.kind === "image") return validateImageDataUrl(a.src, MAX_AVATAR_IMAGE_CHARS);
  if (a.kind === "illustration") {
    if (!(AVATAR_STYLES as readonly unknown[]).includes(a.style)) return "gaya avatar tidak dikenal";
    return typeof a.seed === "string" && /^[\w-]{1,64}$/.test(a.seed) ? null : "seed avatar tidak valid";
  }
  return "avatar tidak valid";
}

export type Appearance = {
  version: 1;
  preset: string;
  customThemes: ThemeDef[];
  typography: Partial<Record<TypeVar, Partial<Record<TypeProp, string>>>>;
  fonts: Partial<Record<FontSlot, string>>;
  chat: ChatStyle;
  /** Kustomisasi Panel 3 (dokumen). */
  docs: DocStyle;
  projects: Record<string, ProjectStyle>;
  /** Tema editor (Monaco): id bawaan Monaco atau tema VS Code yang diimpor. */
  editorTheme: string;
  /** Tema editor yang diimpor (berkas atau Open VSX). */
  editorThemes: EditorTheme[];
  /** Override shortcut: id binding → combo (mis. "Ctrl+Shift+K"). */
  shortcuts: Record<string, string>;
  profile: Profile;
};

export const DEFAULT_APPEARANCE: Appearance = {
  version: 1,
  preset: DEFAULT_PRESET,
  customThemes: [],
  typography: {},
  fonts: {},
  chat: {},
  docs: {},
  projects: {},
  editorTheme: "auto",
  editorThemes: [],
  shortcuts: {},
  profile: {},
};

export const MAX_EDITOR_THEMES = 20;
const CHAT_LIMITS = { opacity: [0.3, 1], weight: [100, 900] } as const;

export function validateChat(key: keyof ChatStyle, v: unknown): string | null {
  if (key === "opacity")
    return typeof v === "number" && v >= CHAT_LIMITS.opacity[0] && v <= CHAT_LIMITS.opacity[1]
      ? null
      : "opasitas harus 0.3–1";
  if (key === "weight")
    return typeof v === "number" && Number.isInteger(v) && v >= 100 && v <= 900 ? null : "ketebalan harus 100–900";
  if (key === "font") return validateFontFamily(v);
  if (key === "lineHeight") return typeof v === "number" && v >= 1 && v <= 2.4 ? null : "tinggi baris harus 1–2.4";
  if (key === "composerRatio") return typeof v === "number" && v >= 1 && v <= 1.6 ? null : "rasio composer harus 1–1.6";
  if (key === "tracking") {
    if (typeof v !== "string" || !/^-?\d*\.?\d+em$/.test(v)) return "nilai harus dalam em, mis. 0.01em";
    const n = Number.parseFloat(v);
    return n >= -0.05 && n <= 0.2 ? null : "jarak huruf harus -0.05em–0.2em";
  }
  if (typeof v !== "string" || !/^\d*\.?\d+rem$/.test(v)) return "nilai harus dalam rem, mis. 0.9rem";
  const n = Number.parseFloat(v);
  if (key === "size") return n >= 0.6 && n <= 2 ? null : "ukuran harus 0.6rem–2rem";
  if (key === "codeSize") return n >= 0.6 && n <= 1.6 ? null : "ukuran kode harus 0.6rem–1.6rem";
  if (key === "width") return n >= 28 && n <= 120 ? null : "lebar harus 28rem–120rem";
  return n >= 0 && n <= 3 ? null : "jarak harus 0rem–3rem";
}

/** Logo project: teks/emoji pendek (maks 4 titik kode), tanpa karakter markup. */
export function validateLogo(v: unknown): string | null {
  if (typeof v !== "string" || v.trim() === "") return "logo kosong";
  if ([...v].length > 4) return "logo maksimal 4 karakter";
  return /[<>&"'`\\]/.test(v) || [...v].some((c) => c.codePointAt(0) !== undefined && (c.codePointAt(0) as number) < 32)
    ? "logo mengandung karakter terlarang"
    : null;
}

export function validateProjectStyle(key: keyof ProjectStyle, v: unknown): string | null {
  if (key === "logo") return validateLogo(v);
  if (key === "gradient") return validateGradientCss(v);
  if (key === "image") return validateImageDataUrl(v);
  if (key === "logoImage") return validateImageDataUrl(v, 120_000);
  if (key === "imageY") return intIn(v, 0, 100);
  if (key === "imageZoom") return intIn(v, 100, 300);
  if (key === "overlayOpacity") return intIn(v, 0, 100);
  if (key === "overlay") return (PROJECT_OVERLAYS as readonly unknown[]).includes(v) ? null : "overlay tidak dikenal";
  return isColor(v) ? null : "warna tidak valid";
}

export type Mode = "dark" | "light";

export function findTheme(a: Appearance, id: string): ThemeDef | null {
  return presetById(id) ?? a.customThemes.find((t) => t.id === id) ?? null;
}

/** Variabel CSS (tanpa awalan nilai kosong) untuk mode efektif. Hanya yang menyimpang dari default yang dikembalikan. */
export function computeVars(a: Appearance, mode: Mode): Record<string, string> {
  const vars: Record<string, string> = {};
  const theme = findTheme(a, a.preset);
  const colors = ((theme as { dark?: object; light?: object } | null)?.[mode] ?? {}) as Partial<
    Record<ColorKey, string>
  >;
  for (const [k, v] of Object.entries(colors)) if (isColor(v)) vars[`--z-color-${k}`] = v;
  for (const tv of TYPE_VARS)
    for (const tp of TYPE_PROPS) {
      const v = a.typography[tv]?.[tp];
      if (v !== undefined && validateTypeValue(tp, v) === null) vars[`--z-type-${tv}-${tp}`] = v;
    }
  for (const [slot, v] of Object.entries(a.fonts)) if (validateFontFamily(v) === null) vars[`--z-font-${slot}`] = v;
  const c = a.chat;
  if (c.opacity !== undefined && validateChat("opacity", c.opacity) === null)
    vars["--z-chat-opacity"] = String(c.opacity);
  if (c.weight !== undefined && validateChat("weight", c.weight) === null) vars["--z-chat-weight"] = String(c.weight);
  if (c.size !== undefined && validateChat("size", c.size) === null) vars["--z-chat-size"] = c.size;
  if (c.gap !== undefined && validateChat("gap", c.gap) === null) vars["--z-chat-gap"] = c.gap;
  if (c.font !== undefined && validateChat("font", c.font) === null) vars["--z-chat-font"] = c.font;
  const extra: Array<[keyof ChatStyle, string]> = [
    ["width", "--z-chat-width"],
    ["lineHeight", "--z-chat-line"],
    ["tracking", "--z-chat-tracking"],
    ["codeSize", "--z-chat-code-size"],
    ["composerRatio", "--z-chat-composer-ratio"],
  ];
  for (const [k, name] of extra) if (c[k] !== undefined && validateChat(k, c[k]) === null) vars[name] = String(c[k]);
  const d = a.docs;
  const docVars: Array<[keyof DocStyle, string]> = [
    ["proseFont", "--z-doc-font"],
    ["proseSize", "--z-doc-size"],
    ["proseLine", "--z-doc-line"],
    ["proseWidth", "--z-doc-width"],
    ["tableSize", "--z-doc-table-size"],
  ];
  for (const [k, name] of docVars) if (d[k] !== undefined && validateDoc(k, d[k]) === null) vars[name] = String(d[k]);
  // warna sintaks blok kode chat mengikuti tema editor aktif (sama dengan penampil kode)
  Object.assign(vars, codeVarsFor(a.editorTheme, a.editorThemes));
  return vars;
}

/** Bersihkan objek tersimpan: bagian yang salah dibuang, sisanya dipertahankan. */
export function sanitizeAppearance(raw: unknown): Appearance {
  const out: Appearance = structuredClone(DEFAULT_APPEARANCE);
  if (raw === null || typeof raw !== "object") return out;
  const o = raw as Record<string, unknown>;
  if (Array.isArray(o.customThemes)) {
    for (const t of o.customThemes) {
      const r = sanitizeTheme(t);
      if (r.theme && !presetById(r.theme.id) && !out.customThemes.some((x) => x.id === r.theme?.id))
        out.customThemes.push(r.theme);
    }
  }
  if (typeof o.preset === "string" && findTheme(out, o.preset)) out.preset = o.preset;
  const typo = o.typography as Record<string, Record<string, unknown>> | undefined;
  if (typo && typeof typo === "object") {
    for (const tv of TYPE_VARS)
      for (const tp of TYPE_PROPS) {
        const v = typo[tv]?.[tp];
        if (typeof v === "string" && validateTypeValue(tp, v) === null) {
          const cur = out.typography[tv] ?? {};
          cur[tp] = v;
          out.typography[tv] = cur;
        }
      }
  }
  const fonts = o.fonts as Record<string, unknown> | undefined;
  if (fonts && typeof fonts === "object")
    for (const slot of ["ui", "code", "terminal"] as const)
      if (typeof fonts[slot] === "string" && validateFontFamily(fonts[slot]) === null)
        out.fonts[slot] = fonts[slot] as string;
  const chat = o.chat as Record<string, unknown> | undefined;
  if (chat && typeof chat === "object")
    for (const k of CHAT_STYLE_KEYS)
      if (chat[k] !== undefined && validateChat(k, chat[k]) === null)
        (out.chat as Record<string, unknown>)[k] = chat[k];
  const docs = o.docs as Record<string, unknown> | undefined;
  if (docs && typeof docs === "object")
    for (const k of DOC_STYLE_KEYS)
      if (docs[k] !== undefined && validateDoc(k, docs[k]) === null) (out.docs as Record<string, unknown>)[k] = docs[k];
  const projects = o.projects as Record<string, Record<string, unknown>> | undefined;
  if (projects && typeof projects === "object")
    for (const [id, st] of Object.entries(projects)) {
      if (!st || typeof st !== "object") continue;
      const clean: ProjectStyle = {};
      for (const k of [
        "border",
        "background",
        "logo",
        "gradient",
        "image",
        "logoImage",
        "imageY",
        "imageZoom",
        "overlay",
        "overlayColor",
        "overlayOpacity",
        "fg",
      ] as const)
        if (st[k] !== undefined && validateProjectStyle(k, st[k]) === null) clean[k] = st[k] as string;
      if (Object.keys(clean).length) out.projects[id] = clean;
    }
  const sc = o.shortcuts as Record<string, unknown> | undefined;
  if (sc && typeof sc === "object")
    for (const [id, combo] of Object.entries(sc)) {
      if (!/^[\w.:-]{1,60}$/.test(id) || typeof combo !== "string") continue;
      try {
        parseCombo(combo);
        out.shortcuts[id] = combo;
      } catch {
        // combo rusak dibuang
      }
    }
  if (Array.isArray(o.editorThemes))
    for (const raw of o.editorThemes.slice(0, MAX_EDITOR_THEMES)) {
      const t = sanitizeEditorTheme(raw);
      if (
        t &&
        !out.editorThemes.some((x) => x.id === t.id) &&
        !(BUILTIN_EDITOR_THEMES as readonly string[]).includes(t.id)
      )
        out.editorThemes.push(t);
    }
  if (
    typeof o.editorTheme === "string" &&
    (o.editorTheme === "auto" ||
      (BUILTIN_EDITOR_THEMES as readonly string[]).includes(o.editorTheme) ||
      out.editorThemes.some((t) => t.id === o.editorTheme))
  )
    out.editorTheme = o.editorTheme;
  const prof = o.profile as Record<string, unknown> | undefined;
  if (prof && typeof prof === "object") {
    if (validateProfileName(prof.name) === null) out.profile.name = (prof.name as string).trim();
    if (validateProfileAvatar(prof.avatar) === null) {
      const a = prof.avatar as ProfileAvatar;
      out.profile.avatar =
        a.kind === "image" ? { kind: "image", src: a.src } : { kind: "illustration", style: a.style, seed: a.seed };
    }
  }
  return out;
}

export type AppearanceStorage = { get(): string | null; set(v: string): void };

export class AppearanceStore {
  private snap: Appearance;
  private listeners = new Set<() => void>();

  constructor(private readonly storage: AppearanceStorage) {
    let raw: unknown = null;
    try {
      const t = storage.get();
      raw = t ? JSON.parse(t) : null;
    } catch {
      raw = null;
    }
    this.snap = sanitizeAppearance(raw);
  }

  state(): Appearance {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private commit(next: Appearance) {
    this.snap = next;
    try {
      this.storage.set(JSON.stringify(next));
    } catch {
      // penyimpanan penuh: berlaku untuk sesi ini saja
    }
    for (const l of this.listeners) l();
  }

  private edit(fn: (a: Appearance) => void) {
    const next = structuredClone(this.snap);
    fn(next);
    this.commit(next);
  }

  /** Semua tema yang bisa dipilih (preset bawaan + kustom). */
  themes(): Array<{ id: string; name: string; builtin: boolean }> {
    return [
      ...PRESETS.map((p) => ({ id: p.id, name: p.name, builtin: true })),
      ...this.snap.customThemes.map((t) => ({ id: t.id, name: t.name, builtin: false })),
    ];
  }

  setPreset(id: string): void {
    if (!findTheme(this.snap, id)) throw new Error(`tema tidak dikenal: ${id}`);
    this.edit((a) => {
      a.preset = id;
    });
  }

  /** Simpan/ganti tema kustom (id tidak boleh sama dengan preset bawaan). Mengembalikan galat atau null. */
  saveCustomTheme(def: unknown): string[] | null {
    const r = sanitizeTheme(def);
    if (!r.theme) return r.errors;
    const t = r.theme;
    if (presetById(t.id)) return [`id "${t.id}" dipakai preset bawaan`];
    this.edit((a) => {
      const i = a.customThemes.findIndex((x) => x.id === t.id);
      if (i >= 0) a.customThemes[i] = t;
      else a.customThemes.push(t);
    });
    return null;
  }

  removeCustomTheme(id: string): void {
    this.edit((a) => {
      a.customThemes = a.customThemes.filter((t) => t.id !== id);
      if (a.preset === id) a.preset = DEFAULT_PRESET;
    });
  }

  /** Impor dari teks JSON. Id yang bentrok dengan preset diberi akhiran `-impor`. */
  importTheme(text: string): { ok: true; id: string } | { ok: false; errors: string[] } {
    let raw: unknown;
    try {
      raw = JSON.parse(text);
    } catch {
      return { ok: false, errors: ["bukan JSON yang valid"] };
    }
    if (text.length > 100_000) return { ok: false, errors: ["berkas terlalu besar (maks 100 KB)"] };
    const r = sanitizeTheme(raw);
    if (!r.theme) return { ok: false, errors: r.errors };
    const theme = presetById(r.theme.id) ? { ...r.theme, id: `${r.theme.id}-impor` } : r.theme;
    const err = this.saveCustomTheme(theme);
    return err ? { ok: false, errors: err } : { ok: true, id: theme.id };
  }

  /** Ekspor tema (preset atau kustom) sebagai JSON yang bisa diimpor ulang. */
  exportTheme(id: string): string {
    const t = findTheme(this.snap, id);
    if (!t) throw new Error(`tema tidak dikenal: ${id}`);
    return `${JSON.stringify({ id: t.id, name: t.name, dark: t.dark, light: t.light }, null, 2)}\n`;
  }

  /** `null` = kembali ke default. Mengembalikan galat atau null. */
  setTypography(v: TypeVar, prop: TypeProp, value: string | null): string | null {
    if (value !== null) {
      const err = validateTypeValue(prop, value);
      if (err) return err;
    }
    this.edit((a) => {
      const cur = { ...(a.typography[v] ?? {}) };
      if (value === null) delete cur[prop];
      else cur[prop] = value;
      if (Object.keys(cur).length) a.typography[v] = cur;
      else delete a.typography[v];
    });
    return null;
  }

  setFont(slot: FontSlot, value: string | null): string | null {
    if (value !== null) {
      const err = validateFontFamily(value);
      if (err) return err;
    }
    this.edit((a) => {
      if (value === null) delete a.fonts[slot];
      else a.fonts[slot] = value;
    });
    return null;
  }

  setChat<K extends keyof ChatStyle>(key: K, value: ChatStyle[K] | null): string | null {
    if (value !== null) {
      const err = validateChat(key, value);
      if (err) return err;
    }
    this.edit((a) => {
      if (value === null) delete a.chat[key];
      else a.chat[key] = value;
    });
    return null;
  }

  /** Kustomisasi Panel 3; `null` = kembali ke bawaan. */
  setDoc<K extends keyof DocStyle>(key: K, value: DocStyle[K] | null): string | null {
    if (value !== null) {
      const err = validateDoc(key, value);
      if (err) return err;
    }
    this.edit((a) => {
      if (value === null) delete a.docs[key];
      else a.docs[key] = value;
    });
    return null;
  }

  /** Hapus semua kustomisasi dokumen (atau hanya kunci yang disebut). */
  resetDocs(keys?: readonly (keyof DocStyle)[]): void {
    this.edit((a) => {
      if (!keys) a.docs = {};
      else for (const k of keys) delete a.docs[k];
    });
  }

  setProjectStyle(projectId: string, key: keyof ProjectStyle, value: string | null): string | null {
    if (value !== null) {
      const err = validateProjectStyle(key, value);
      if (err) return err;
    }
    this.edit((a) => {
      const cur = { ...(a.projects[projectId] ?? {}) };
      if (value === null) delete cur[key];
      else cur[key] = value;
      if (Object.keys(cur).length) a.projects[projectId] = cur;
      else delete a.projects[projectId];
    });
    return null;
  }

  /** Nama tampilan pengguna; null/kosong = kembali ke nama akun OS. */
  setProfileName(name: string | null): string | null {
    const v = name?.trim() || null;
    if (v !== null) {
      const err = validateProfileName(v);
      if (err) return err;
    }
    this.edit((a) => {
      if (v === null) delete a.profile.name;
      else a.profile.name = v;
    });
    return null;
  }

  /** Avatar pengguna (gambar unggahan atau ilustrasi); null = inisial. */
  setProfileAvatar(avatar: ProfileAvatar | null): string | null {
    if (avatar !== null) {
      const err = validateProfileAvatar(avatar);
      if (err) return err;
    }
    this.edit((a) => {
      if (avatar === null) delete a.profile.avatar;
      else a.profile.avatar = avatar;
    });
    return null;
  }

  /** Simpan override shortcut (validasi bentrok dilakukan ShortcutRegistry oleh pemanggil). */
  setShortcut(id: string, combo: string | null): string | null {
    if (combo !== null) {
      try {
        parseCombo(combo);
      } catch (e) {
        return e instanceof Error ? e.message : String(e);
      }
    }
    this.edit((a) => {
      if (combo === null) delete a.shortcuts[id];
      else a.shortcuts[id] = combo;
    });
    return null;
  }

  /** Pilih tema editor: "auto", bawaan Monaco, atau id tema terpasang. */
  setEditorTheme(id: string): void {
    const ok =
      id === "auto" ||
      (BUILTIN_EDITOR_THEMES as readonly string[]).includes(id) ||
      this.snap.editorThemes.some((t) => t.id === id);
    if (!ok) throw new Error(`tema editor tidak dikenal: ${id}`);
    this.edit((a) => {
      a.editorTheme = id;
    });
  }

  /** Tambah/ganti tema editor terpasang. Mengembalikan galat atau null. */
  addEditorTheme(theme: unknown): string | null {
    const t = sanitizeEditorTheme(theme);
    if (!t) return "tema editor tidak valid";
    if ((BUILTIN_EDITOR_THEMES as readonly string[]).includes(t.id)) return `id "${t.id}" dipakai tema bawaan`;
    if (!this.snap.editorThemes.some((x) => x.id === t.id) && this.snap.editorThemes.length >= MAX_EDITOR_THEMES)
      return `maksimal ${MAX_EDITOR_THEMES} tema editor terpasang`;
    this.edit((a) => {
      const i = a.editorThemes.findIndex((x) => x.id === t.id);
      if (i >= 0) a.editorThemes[i] = t;
      else a.editorThemes.push(t);
    });
    return null;
  }

  removeEditorTheme(id: string): void {
    this.edit((a) => {
      a.editorThemes = a.editorThemes.filter((t) => t.id !== id);
      if (a.editorTheme === id) a.editorTheme = "auto";
    });
  }

  /** Atur ulang tampilan. Profil (nama/avatar) bukan bagian tampilan, jadi dipertahankan. */
  resetAll(): void {
    this.commit({ ...structuredClone(DEFAULT_APPEARANCE), profile: structuredClone(this.snap.profile) });
  }
}

// Tema editor (Monaco) dari tema VS Code (Fase 10): konversi JSON/JSONC → definisi Monaco yang sudah divalidasi, dan
// ekstraksi tema dari paket .vsix (Open VSX). Tidak ada kode dari ekstensi yang dijalankan: hanya berkas JSON tema yang dibaca.
import { unzipSync } from "fflate";
import { isColor } from "./tokens";

export type EditorThemeBase = "vs" | "vs-dark" | "hc-black" | "hc-light";
export type EditorTokenRule = { token: string; foreground?: string; background?: string; fontStyle?: string };
export type EditorTheme = {
  /** kebab-case, unik. */
  id: string;
  name: string;
  base: EditorThemeBase;
  colors: Record<string, string>;
  rules: EditorTokenRule[];
  /** Asal (mis. "openvsx:ns.nama@1.0.0" atau "berkas"). */
  source: string;
};

export const BUILTIN_EDITOR_THEMES = ["vs", "vs-dark", "hc-black", "hc-light"] as const;
export const MAX_THEME_BYTES = 400_000;
const MAX_COLORS = 1000;
const MAX_RULES = 2000;

/** Hapus komentar `//` `/* *​/` dan koma penutup (JSONC) tanpa menyentuh isi string. */
export function stripJsonc(text: string): string {
  let out = "";
  let i = 0;
  const n = text.length;
  while (i < n) {
    const c = text[i] as string;
    if (c === '"') {
      let j = i + 1;
      while (j < n && text[j] !== '"') j += text[j] === "\\" ? 2 : 1;
      out += text.slice(i, j + 1);
      i = j + 1;
    } else if (c === "/" && text[i + 1] === "/") {
      while (i < n && text[i] !== "\n") i++;
    } else if (c === "/" && text[i + 1] === "*") {
      const end = text.indexOf("*/", i + 2);
      i = end === -1 ? n : end + 2;
    } else {
      out += c;
      i++;
    }
  }
  return out.replace(/,(\s*[}\]])/g, "$1");
}

const slug = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40);

const baseFor = (type: unknown, uiTheme?: unknown): EditorThemeBase => {
  const t = String(type ?? uiTheme ?? "").toLowerCase();
  if (t === "light" || t === "vs") return "vs";
  if (t === "hc-light" || t === "hc_light") return "hc-light";
  if (t === "hc" || t === "hc-black" || t === "hc_black") return "hc-black";
  return "vs-dark";
};

const hexNoHash = (v: unknown): string | undefined => {
  if (typeof v !== "string" || !isColor(v) || !v.startsWith("#")) return undefined;
  return v.slice(1);
};

/** Ubah teks tema VS Code menjadi `EditorTheme`. `fallbackName` dipakai bila tema tidak punya nama. */
export function convertVsCodeTheme(
  text: string,
  options: { name?: string; uiTheme?: string; source?: string } = {},
): { theme: EditorTheme | null; errors: string[] } {
  if (text.length > MAX_THEME_BYTES) return { theme: null, errors: ["berkas tema terlalu besar (maks 400 KB)"] };
  let raw: unknown;
  try {
    raw = JSON.parse(stripJsonc(text));
  } catch {
    return { theme: null, errors: ["bukan JSON tema yang valid"] };
  }
  if (raw === null || typeof raw !== "object" || Array.isArray(raw))
    return { theme: null, errors: ["tema harus berupa objek"] };
  const o = raw as Record<string, unknown>;
  const name = String(options.name ?? o.name ?? "")
    .trim()
    .slice(0, 40);
  if (!name) return { theme: null, errors: ["tema tidak punya nama"] };
  const id = slug(name);
  if (!id) return { theme: null, errors: ["nama tema tidak valid"] };

  const colors: Record<string, string> = {};
  if (o.colors && typeof o.colors === "object") {
    for (const [k, v] of Object.entries(o.colors as Record<string, unknown>)) {
      if (Object.keys(colors).length >= MAX_COLORS) break;
      if (/^[\w.-]{1,80}$/.test(k) && isColor(v)) colors[k] = v;
    }
  }
  const rules: EditorTokenRule[] = [];
  const tokenColors = Array.isArray(o.tokenColors) ? o.tokenColors : [];
  for (const tc of tokenColors as Array<Record<string, unknown>>) {
    const settings = (tc?.settings ?? {}) as Record<string, unknown>;
    const foreground = hexNoHash(settings.foreground);
    const background = hexNoHash(settings.background);
    const fontStyle =
      typeof settings.fontStyle === "string" && /^[a-z ]{0,40}$/.test(settings.fontStyle)
        ? settings.fontStyle
        : undefined;
    if (!foreground && !background && fontStyle === undefined) continue;
    const scopes = Array.isArray(tc.scope) ? tc.scope : typeof tc.scope === "string" ? tc.scope.split(",") : [""];
    for (const sc of scopes as unknown[]) {
      if (rules.length >= MAX_RULES) break;
      const token = typeof sc === "string" ? (sc.trim().split(/\s+/).pop() ?? "") : "";
      if (token !== "" && !/^[\w.+-]{1,100}$/.test(token)) continue;
      rules.push({
        token,
        ...(foreground && { foreground }),
        ...(background && { background }),
        ...(fontStyle !== undefined && { fontStyle }),
      });
    }
  }
  if (Object.keys(colors).length === 0 && rules.length === 0)
    return { theme: null, errors: ["tema tidak memuat warna atau aturan token yang valid"] };
  return {
    theme: { id, name, base: baseFor(o.type, options.uiTheme), colors, rules, source: options.source ?? "berkas" },
    errors: [],
  };
}

/** Validasi ulang objek tersimpan (data dari localStorage tidak dipercaya). */
export function sanitizeEditorTheme(raw: unknown): EditorTheme | null {
  if (raw === null || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  if (typeof o.id !== "string" || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(o.id) || o.id.length > 40) return null;
  if (typeof o.name !== "string" || o.name.trim() === "" || o.name.length > 40) return null;
  if (!(BUILTIN_EDITOR_THEMES as readonly string[]).includes(o.base as string)) return null;
  const colors: Record<string, string> = {};
  for (const [k, v] of Object.entries((o.colors ?? {}) as Record<string, unknown>))
    if (Object.keys(colors).length < MAX_COLORS && /^[\w.-]{1,80}$/.test(k) && isColor(v)) colors[k] = v;
  const rules: EditorTokenRule[] = [];
  for (const r of (Array.isArray(o.rules) ? o.rules : []) as Array<Record<string, unknown>>) {
    if (rules.length >= MAX_RULES || typeof r?.token !== "string" || !/^[\w.+-]{0,100}$/.test(r.token)) continue;
    const hex = (v: unknown) => (typeof v === "string" && /^[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(v) ? v : undefined);
    const fg = hex(r.foreground);
    const bg = hex(r.background);
    const fs = typeof r.fontStyle === "string" && /^[a-z ]{0,40}$/.test(r.fontStyle) ? r.fontStyle : undefined;
    rules.push({
      token: r.token,
      ...(fg && { foreground: fg }),
      ...(bg && { background: bg }),
      ...(fs !== undefined && { fontStyle: fs }),
    });
  }
  return {
    id: o.id,
    name: o.name.trim(),
    base: o.base as EditorThemeBase,
    colors,
    rules,
    source: typeof o.source === "string" ? o.source.slice(0, 120) : "berkas",
  };
}

export type VsixThemeResult = { themes: EditorTheme[]; skipped: string[]; errors: string[] };

/** Baca tema dari bytes .vsix: `extension/package.json` → `contributes.themes[]` → berkas JSON tema. */
export function themesFromVsix(bytes: Uint8Array, source: string): VsixThemeResult {
  const result: VsixThemeResult = { themes: [], skipped: [], errors: [] };
  let files: Record<string, Uint8Array>;
  try {
    files = unzipSync(bytes, {
      filter: (f) => f.originalSize <= MAX_THEME_BYTES * 2 && /^extension\/.*\.(json|jsonc)$/i.test(f.name),
    });
  } catch {
    result.errors.push("paket .vsix tidak bisa dibuka");
    return result;
  }
  const dec = new TextDecoder();
  const pkgBytes = files["extension/package.json"];
  if (!pkgBytes) {
    result.errors.push("package.json tidak ditemukan di paket");
    return result;
  }
  let pkg: { contributes?: { themes?: Array<{ label?: string; uiTheme?: string; path?: string }> } };
  try {
    pkg = JSON.parse(stripJsonc(dec.decode(pkgBytes)));
  } catch {
    result.errors.push("package.json rusak");
    return result;
  }
  const entries = pkg.contributes?.themes ?? [];
  if (entries.length === 0) result.errors.push("ekstensi ini tidak memuat tema warna");
  for (const e of entries.slice(0, 12)) {
    const rel = typeof e.path === "string" ? e.path.replace(/^\.\//, "").replace(/\\/g, "/") : "";
    if (!rel || rel.includes("..")) {
      result.skipped.push(`${e.label ?? "?"}: path tidak valid`);
      continue;
    }
    const data = files[`extension/${rel}`];
    if (!data) {
      result.skipped.push(`${e.label ?? rel}: berkas tema tidak ada`);
      continue;
    }
    const r = convertVsCodeTheme(dec.decode(data), { name: e.label, uiTheme: e.uiTheme, source });
    if (r.theme) result.themes.push(r.theme);
    else result.skipped.push(`${e.label ?? rel}: ${r.errors[0]}`);
  }
  return result;
}

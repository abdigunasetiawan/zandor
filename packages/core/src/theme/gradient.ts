// Generator gradien untuk gaya project: spesifikasi terstruktur (jenis, sudut, titik warna) ⇄ string CSS, validasi ketat string
// CSS gradien (hanya token gradien yang aman; tidak ada url(), var(), atau fungsi lain), dan templat siap pakai.

export type GradientStop = { color: string; pos: number };
export type GradientSpec = { type: "linear" | "radial"; angle: number; stops: GradientStop[] };

const HEX = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
export const MIN_STOPS = 2;
export const MAX_STOPS = 5;

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

/** String CSS dari spesifikasi. Warna harus hex; posisi 0–100 (%), sudut 0–360. */
export function gradientCss(spec: GradientSpec): string {
  const stops = spec.stops
    .slice(0, MAX_STOPS)
    .map((s) => `${s.color} ${Math.round(clamp(s.pos, 0, 100))}%`)
    .join(", ");
  return spec.type === "radial"
    ? `radial-gradient(circle at center, ${stops})`
    : `linear-gradient(${Math.round(clamp(spec.angle, 0, 360))}deg, ${stops})`;
}

/** Validasi spesifikasi (dipakai generator sebelum menyimpan). */
export function validateGradientSpec(spec: GradientSpec): string | null {
  if (spec.type !== "linear" && spec.type !== "radial") return "jenis gradien tidak dikenal";
  if (!Number.isFinite(spec.angle)) return "sudut harus angka";
  if (spec.stops.length < MIN_STOPS || spec.stops.length > MAX_STOPS)
    return `gradien butuh ${MIN_STOPS}–${MAX_STOPS} titik warna`;
  for (const s of spec.stops) {
    if (!HEX.test(s.color)) return "warna titik harus berupa hex (#rrggbb)";
    if (!Number.isFinite(s.pos) || s.pos < 0 || s.pos > 100) return "posisi titik harus 0–100";
  }
  return null;
}

const STOP_RE = /^(#[0-9a-f]{3,8})\s+(\d{1,3})%$/i;

/**
 * Baca string CSS buatan `gradientCss` kembali menjadi spesifikasi (agar gradien tersimpan bisa disunting). null bila bentuknya
 * bukan hasil generator ini.
 */
export function parseGradient(css: string): GradientSpec | null {
  const lin = css.match(/^linear-gradient\((\d{1,3})deg,\s*(.+)\)$/);
  const rad = css.match(/^radial-gradient\(circle at center,\s*(.+)\)$/);
  const m = lin ?? rad;
  const body = lin ? lin[2] : rad ? rad[1] : undefined;
  if (!m || body === undefined) return null;
  const stops: GradientStop[] = [];
  for (const part of body.split(/,\s*/)) {
    const sm = part.trim().match(STOP_RE);
    if (!sm?.[1] || !sm[2]) return null;
    stops.push({ color: sm[1], pos: Number(sm[2]) });
  }
  const spec: GradientSpec = { type: lin ? "linear" : "radial", angle: lin?.[1] ? Number(lin[1]) : 0, stops };
  return validateGradientSpec(spec) === null ? spec : null;
}

/**
 * Validasi string gradien CSS yang disimpan/diimpor: hanya `linear-gradient`/`radial-gradient` dengan token yang diizinkan
 * (angka, deg, %, warna hex/rgb/hsl, kata kunci posisi/bentuk). Menolak url(), var(), calc(), dan fungsi/pemisah lain.
 */
export function validateGradientCss(v: unknown): string | null {
  if (typeof v !== "string" || v.length === 0 || v.length > 400) return "gradien tidak valid";
  const m = v.match(/^(linear|radial)-gradient\((.*)\)$/s);
  if (!m?.[2]) return "gradien harus linear-gradient(...) atau radial-gradient(...)";
  // buang fungsi warna yang sah dulu; sisanya hanya boleh angka/satuan/kata kunci/koma/spasi/#hex
  const stripped = m[2].replace(/(?:rgb|rgba|hsl|hsla)\(\s*[\d.,%\s/-]+\)/gi, "0");
  if (/[()]/.test(stripped)) return "gradien mengandung fungsi yang tidak diizinkan";
  const tokens = stripped.split(/[\s,]+/).filter(Boolean);
  const KEYWORDS =
    /^(circle|ellipse|at|to|center|top|bottom|left|right|closest-side|farthest-side|closest-corner|farthest-corner)$/i;
  for (const tok of tokens) {
    if (HEX.test(tok) || /^-?\d{1,4}(\.\d+)?(deg|%|px|rem)?$/.test(tok) || KEYWORDS.test(tok) || tok === "0") continue;
    if (/^[a-z]{3,20}$/i.test(tok)) continue; // nama warna CSS (red, tomato, …)
    return `token gradien tidak diizinkan: ${tok}`;
  }
  return null;
}

export type GradientTemplate = { id: string; name: string; spec: GradientSpec };

const T = (
  id: string,
  name: string,
  type: "linear" | "radial",
  angle: number,
  ...stops: Array<[string, number]>
): GradientTemplate => ({
  id,
  name,
  spec: { type, angle, stops: stops.map(([color, pos]) => ({ color, pos })) },
});

/** Templat gradien siap pakai (nilai ditulis sendiri). */
export const GRADIENT_TEMPLATES: readonly GradientTemplate[] = [
  T("aurora", "Aurora", "linear", 135, ["#0f766e", 0], ["#2563eb", 55], ["#7c3aed", 100]),
  T("sunset", "Senja", "linear", 120, ["#f97316", 0], ["#ec4899", 60], ["#7c3aed", 100]),
  T("ocean", "Samudra", "linear", 160, ["#0ea5e9", 0], ["#1d4ed8", 100]),
  T("forest", "Hutan", "linear", 150, ["#14532d", 0], ["#65a30d", 100]),
  T("ember", "Bara", "linear", 135, ["#7f1d1d", 0], ["#ea580c", 100]),
  T("graphite", "Grafit", "linear", 160, ["#18181b", 0], ["#52525b", 100]),
  T("mint", "Mint", "linear", 120, ["#99f6e4", 0], ["#a7f3d0", 50], ["#fef9c3", 100]),
  T("peach", "Persik", "linear", 135, ["#fecaca", 0], ["#fed7aa", 100]),
  T("lavender", "Lavender", "linear", 145, ["#ddd6fe", 0], ["#fbcfe8", 100]),
  T("midnight", "Tengah malam", "linear", 180, ["#020617", 0], ["#1e3a8a", 100]),
  T("spotlight", "Sorot", "radial", 0, ["#6366f1", 0], ["#1e1b4b", 100]),
  T("glow", "Cahaya", "radial", 0, ["#fde68a", 0], ["#f59e0b", 55], ["#7c2d12", 100]),
];

/** Nilai bawaan generator (disimpan di core agar komponen UI tidak memuat warna mentah). */
export const DEFAULT_GRADIENT: GradientSpec = {
  type: "linear",
  angle: 135,
  stops: [
    { color: "#2563eb", pos: 0 },
    { color: "#7c3aed", pos: 100 },
  ],
};
export const DEFAULT_PROJECT_COLOR = "#1e293b";
export const NEW_STOP_COLOR = "#f97316";
export const FALLBACK_PICKER_COLOR = "#000000";

/** HSL (0–360, 0–1, 0–1) → "#rrggbb". */
export function hslToHex(h: number, s: number, l: number): string {
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    const v = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(v * 255)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

/** Gradien acak yang enak dilihat: dua rona berdekatan dengan saturasi/terang sedang. `rand` untuk tes. */
export function randomGradient(rand: () => number = Math.random): GradientSpec {
  const hue = Math.floor(rand() * 360);
  return {
    type: "linear",
    angle: Math.floor(rand() * 360),
    stops: [
      { color: hslToHex(hue, 0.65, 0.5), pos: 0 },
      { color: hslToHex((hue + 50) % 360, 0.65, 0.4), pos: 100 },
    ],
  };
}

// Aturan "tidak ada warna/ukuran yang di-hardcode" (roadmap Fase 2) dan "tidak ada dialog bawaan OS" (PRD §4.4).
// Logika murni agar bisa diuji; CLI ada di check-design-tokens.ts.

export type Violation = { file: string; line: number; rule: string; text: string };

/** Hanya tokens.css yang boleh menulis nilai mentah. */
export const TOKEN_FILE = /(^|[\\/])tokens\.css$/;

const HEX_COLOR = /(?<![\w&])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})(?![\w-])/;
const COLOR_FN = /\b(?:rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color)\(/;
// Ukuran piksel selain garis 1px. (Titik desimal dan 0px diperlakukan sebagai ukuran juga, kecuali "0".)
const PX_VALUE = /(?<![\w.#-])(?:\d*\.\d+|\d+)px\b/g;
const NATIVE_DIALOG =
  /\b(?:window|globalThis|self)\.(?:alert|confirm|prompt)\s*\(|(?<![\w.$])(?:alert|prompt)\s*\(|(?<![\w.$])confirm\s*\(\s*["'`]/;

/** Hapus komentar blok/baris supaya contoh di komentar tidak dianggap pelanggaran. */
function stripComments(text: string, isCss: boolean): string {
  const noBlock = text.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "));
  if (isCss) return noBlock;
  return noBlock.replace(/(^|[^:"'`\\])\/\/.*$/gm, (_m, pre: string) => pre);
}

export function findViolations(file: string, text: string): Violation[] {
  const isCss = file.endsWith(".css");
  const isToken = TOKEN_FILE.test(file);
  const out: Violation[] = [];
  const lines = stripComments(text, isCss).split("\n");
  lines.forEach((raw, i) => {
    const line = raw;
    const push = (rule: string) => out.push({ file, line: i + 1, rule, text: raw.trim() });

    if (!isToken) {
      if (HEX_COLOR.test(line)) push("warna hex di luar tokens.css");
      if (COLOR_FN.test(line)) push("fungsi warna (rgb/hsl/…) di luar tokens.css");
      for (const m of line.matchAll(PX_VALUE)) {
        if (m[0] !== "1px") {
          push(`ukuran ${m[0]} di luar tokens.css (pakai token rem)`);
          break;
        }
      }
    }
    if (NATIVE_DIALOG.test(line)) push("dialog bawaan OS (alert/confirm/prompt) — pakai useConfirm/Dialog");
  });
  return out;
}

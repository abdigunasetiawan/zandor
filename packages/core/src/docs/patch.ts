// Diff unified dari engine (snapshot sesi atau metadata tool edit) → hunk yang bisa ditinjau di Panel 3 (ADR-075).
// Tidak memakai nomor baris secara buta: isi hunk dicocokkan dengan teks berkas saat ini (dekat posisi yang diharapkan
// dulu, lalu di seluruh berkas bila cocok tepat satu kali), jadi tetap benar walau berkas sudah bergeser sedikit.

export type Hunk = {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  /** Baris hunk dengan awalan ` `, `-`, atau `+` (tanpa baris `\ No newline`). */
  lines: string[];
};

const HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

/** Urai diff unified menjadi hunk. Header berkas (`Index:`, `===`, `---`, `+++`) diabaikan. */
export function parsePatch(patch: string): Hunk[] {
  const hunks: Hunk[] = [];
  let cur: Hunk | null = null;
  for (const raw of patch.split(/\r?\n/)) {
    const m = HEADER.exec(raw);
    if (m) {
      cur = {
        oldStart: Number(m[1]),
        oldLines: m[2] === undefined ? 1 : Number(m[2]),
        newStart: Number(m[3]),
        newLines: m[4] === undefined ? 1 : Number(m[4]),
        lines: [],
      };
      hunks.push(cur);
      continue;
    }
    if (!cur) continue;
    const oldSeen = cur.lines.filter((l) => l[0] !== "+").length;
    const newSeen = cur.lines.filter((l) => l[0] !== "-").length;
    const c = raw[0];
    if (c === "-" || c === "+" || c === " ") cur.lines.push(raw);
    // baris konteks kosong yang spasinya terpangkas, hanya selama hunk belum lengkap
    else if (raw === "" && oldSeen < cur.oldLines && newSeen < cur.newLines) cur.lines.push(" ");
  }
  return hunks.filter((h) => h.lines.some((l) => l[0] !== " "));
}

/** Sisi "sebelum" (konteks + hapus) dan "sesudah" (konteks + tambah) sebuah hunk. */
export function hunkSides(h: Hunk): { before: string[]; after: string[] } {
  const before: string[] = [];
  const after: string[] = [];
  for (const l of h.lines) {
    if (l[0] !== "+") before.push(l.slice(1));
    if (l[0] !== "-") after.push(l.slice(1));
  }
  return { before, after };
}

function splitLines(text: string): { lines: string[]; eol: string } {
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  return { lines: text.split(/\r?\n/), eol };
}

function matchesAt(lines: string[], at: number, want: string[]): boolean {
  if (at < 0 || at + want.length > lines.length) return false;
  for (let i = 0; i < want.length; i++) if (lines[at + i] !== want[i]) return false;
  return true;
}

/** Indeks baris (0-based) tempat `want` berada, mencari dari `hint` keluar; null bila tidak ada atau ambigu. */
function locate(lines: string[], want: string[], hint: number): number | null {
  if (want.length === 0) return Math.max(0, Math.min(hint, lines.length));
  for (let d = 0; d <= 50; d++) {
    if (matchesAt(lines, hint - d, want)) return hint - d;
    if (d && matchesAt(lines, hint + d, want)) return hint + d;
  }
  let found: number | null = null;
  for (let i = 0; i + want.length <= lines.length; i++) {
    if (!matchesAt(lines, i, want)) continue;
    if (found !== null) return null; // muncul lebih dari sekali: jangan menebak
    found = i;
  }
  return found;
}

/** Posisi hunk di teks saat ini (baris 1-based awal dan jumlah baris sisi "sesudah"), atau null bila tidak ditemukan. */
export function locateHunk(text: string, h: Hunk): { line: number; count: number } | null {
  const { lines } = splitLines(text);
  const { after } = hunkSides(h);
  const at = locate(lines, after, h.newStart - 1);
  return at === null ? null : { line: at + 1, count: after.length };
}

/** Kembalikan satu hunk ke isi sebelum diubah. null bila isi hunk tidak lagi ada di teks (berkas sudah berubah). */
export function revertHunk(text: string, h: Hunk): string | null {
  const { lines, eol } = splitLines(text);
  const { before, after } = hunkSides(h);
  const at = locate(lines, after, h.newStart - 1);
  if (at === null) return null;
  lines.splice(at, after.length, ...before);
  return lines.join(eol);
}

/** Teks "sebelum" untuk seluruh patch (hunk dibalik dari bawah ke atas). null bila ada hunk yang tidak cocok. */
export function reverseApply(text: string, hunks: Hunk[]): string | null {
  let out = text;
  for (const h of [...hunks].sort((a, b) => b.newStart - a.newStart)) {
    const next = revertHunk(out, h);
    if (next === null) return null;
    out = next;
  }
  return out;
}

/**
 * Buang hunk ke-`i` dari daftar. Bila hunk itu dikembalikan (`reverted`), posisi hunk sesudahnya digeser sebesar
 * selisih panjangnya supaya pencarian dekat posisi tetap tepat.
 */
export function dropHunk(hunks: Hunk[], i: number, reverted: boolean): Hunk[] {
  const h = hunks[i];
  if (!h) return hunks;
  const shift = reverted ? h.oldLines - h.newLines : 0;
  return hunks
    .filter((_, k) => k !== i)
    .map((x) => (shift && x.newStart > h.newStart ? { ...x, newStart: x.newStart + shift } : x));
}

/** Tulis ulang hunk menjadi teks diff unified (tanpa header berkas); `parsePatch(formatHunks(h))` setara `h`. */
export function formatHunks(hunks: Hunk[]): string {
  return hunks
    .map((h) => `@@ -${h.oldStart},${h.oldLines} +${h.newStart},${h.newLines} @@\n${h.lines.join("\n")}\n`)
    .join("");
}

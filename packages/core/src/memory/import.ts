// Impor memori dari AI lain (onboarding). Pengguna menempel jawaban beberapa AI (ChatGPT, Gemini, Claude, …) yang diminta
// menulis fakta `- kunci: nilai`. Fakta digabung dengan memori global yang sudah ada; kunci yang sama dengan nilai berbeda
// (mis. "nama: Adi" dari Gemini vs "nama: Abdi" dari ChatGPT) menjadi KONFLIK yang harus dipilih pengguna — tidak pernah
// diputuskan diam-diam. Baris yang tampak memuat secret dibuang. Hasil akhir = isi baru `global.md` (satu fakta per baris).
import { scanSecrets } from "./store";

/** Sumber memori: id stabil + nama tampilan (mis. "ChatGPT"). `zandor` = memori yang sudah ada. */
export type ImportSource = { id: string; label: string; text: string };

export type ParsedLine = { key: string | null; label: string | null; value: string };

export type FactOption = { value: string; sources: string[] };
/** Fakta berkunci: satu opsi = sepakat; >1 opsi = konflik. */
export type KeyedFact = { key: string; label: string; options: FactOption[] };
export type MergedMemory = {
  keyed: KeyedFact[];
  /** Fakta tanpa kunci (kalimat bebas), sudah dihapus duplikatnya. */
  free: FactOption[];
  /** Jumlah baris yang dibuang karena tampak memuat secret. */
  secrets: number;
};
/** Pilihan untuk konflik kunci tunggal: indeks opsi, atau satu nilai tulisan pengguna. */
export type Resolution = number | { custom: string };

export const MAX_IMPORT_FACTS = 300;
const MAX_LINE = 500;
const MAX_KEY = 40;

const normValue = (v: string) =>
  v
    .toLowerCase()
    .replace(/\s+/g, " ")
    .replace(/[.。]+$/, "")
    .trim();
export const normKey = (k: string) =>
  k
    .toLowerCase()
    .replace(/[*_`"]/g, "")
    .replace(/\s+/g, " ")
    .trim();

/** Satu baris jawaban AI → fakta. Pagar kode, judul, dan baris kosong dilewati. */
export function parseMemoryText(text: string): ParsedLine[] {
  const out: ParsedLine[] = [];
  for (const raw of text.split(/\r?\n/)) {
    let line = raw.trim();
    if (!line || line.startsWith("```") || line.startsWith("#")) continue;
    line = line
      .replace(/^([-*•]|\d+[.)])\s+/, "")
      .replace(/^\*\*(.+?)\*\*/, "$1")
      .trim();
    if (!line || line.length > MAX_LINE) continue;
    const m = /^([^:：]{1,40})[:：]\s*(.+)$/.exec(line);
    const label = m?.[1]?.replace(/[*_`"]/g, "").trim();
    if (m && label && label.length <= MAX_KEY && !/^https?$/i.test(label)) {
      out.push({ key: normKey(label), label, value: (m[2] ?? "").trim() });
    } else out.push({ key: null, label: null, value: line });
  }
  return out;
}

/** Gabungkan sumber (urutan = prioritas tampilan; taruh memori yang ada lebih dulu). */
export function mergeMemory(sources: readonly ImportSource[]): MergedMemory {
  const keyed = new Map<string, KeyedFact>();
  const free = new Map<string, FactOption>();
  let secrets = 0;
  let count = 0;
  for (const src of sources) {
    for (const f of parseMemoryText(src.text)) {
      if (scanSecrets(`${f.label ?? ""} ${f.value}`).length) {
        secrets++;
        continue;
      }
      if (count >= MAX_IMPORT_FACTS) break;
      const nv = normValue(f.value);
      if (!nv) continue;
      if (f.key && f.label) {
        let k = keyed.get(f.key);
        if (!k) {
          k = { key: f.key, label: f.label, options: [] };
          keyed.set(f.key, k);
          count++;
        }
        const same = k.options.find((o) => normValue(o.value) === nv);
        if (same) {
          if (!same.sources.includes(src.id)) same.sources.push(src.id);
        } else k.options.push({ value: f.value, sources: [src.id] });
      } else {
        const same = free.get(nv);
        if (same) {
          if (!same.sources.includes(src.id)) same.sources.push(src.id);
        } else {
          free.set(nv, { value: f.value, sources: [src.id] });
          count++;
        }
      }
    }
  }
  return { keyed: [...keyed.values()], free: [...free.values()], secrets };
}

/**
 * Kunci bernilai TUNGGAL (hanya satu yang benar): nilai berbeda = konflik yang wajib dipilih. Kunci lain — termasuk
 * yang dibuat sendiri oleh AI — dianggap bernilai BANYAK (proyek, alat, minat…): nilainya digabung, bukan konflik.
 * Salah tebak ke "banyak" hanya menghasilkan nilai berlebih yang mudah dibuang; salah tebak ke "tunggal" bisa
 * membuang informasi. Mengikuti kunci standar di prompt impor (ID + EN).
 */
export const SINGLE_KEYS: ReadonlySet<string> = new Set([
  "nama",
  "nama lengkap",
  "name",
  "full name",
  "panggilan",
  "nickname",
  "pekerjaan",
  "occupation",
  "job",
  "perusahaan",
  "company",
  "lokasi",
  "location",
  "zona waktu",
  "time zone",
  "timezone",
  "gaya jawaban",
  "answer style",
]);
export const isSingleKey = (key: string) => SINGLE_KEYS.has(normKey(key));

/** Konflik = kunci tunggal dengan lebih dari satu nilai. */
export const conflictsOf = (m: MergedMemory) => m.keyed.filter((k) => isSingleKey(k.key) && k.options.length > 1);

/** Kunci bernilai banyak (ditampilkan sebagai daftar yang bisa dicentang/ditambah). */
export const listsOf = (m: MergedMemory) => m.keyed.filter((k) => !isSingleKey(k.key));

/** Pilihan untuk daftar bernilai banyak: nilai yang DIBUANG (dinormalisasi) + nilai tambahan tulisan pengguna. */
export type ListPick = { excluded: string[]; extra: string[] };
export type ListPicks = Readonly<Record<string, ListPick>>;

export const valueId = normValue;
const oneLine = (v: string) => v.replace(/\s+/g, " ").trim();

/** Konflik yang belum punya pilihan (atau "Tulis sendiri" yang masih kosong). */
export function unresolved(m: MergedMemory, res: Readonly<Record<string, Resolution>>): KeyedFact[] {
  return conflictsOf(m).filter((k) => {
    const r = res[k.key];
    return r === undefined || (typeof r === "object" && !r.custom.trim());
  });
}

/** Isi `global.md` baru: satu fakta per baris (`- Label: nilai`). Konflik tanpa pilihan dilewati. */
export function buildMemory(m: MergedMemory, res: Readonly<Record<string, Resolution>>, lists: ListPicks = {}): string {
  const lines: string[] = [];
  for (const k of m.keyed) {
    if (!isSingleKey(k.key)) {
      // daftar: semua nilai kecuali yang dibuang, lalu tambahan pengguna (tanpa duplikat)
      const pick = lists[k.key];
      const seen = new Set<string>();
      const values = [...k.options.map((o) => o.value), ...(pick?.extra ?? []).map(oneLine)];
      for (const v of values) {
        const id = normValue(v);
        if (!id || seen.has(id) || pick?.excluded.includes(id)) continue;
        seen.add(id);
        lines.push(`- ${k.label}: ${v}`);
      }
      continue;
    }
    if (k.options.length === 1) {
      lines.push(`- ${k.label}: ${k.options[0]?.value}`);
      continue;
    }
    const r = res[k.key];
    if (r === undefined) continue;
    if (typeof r === "object") {
      const v = oneLine(r.custom);
      if (v) lines.push(`- ${k.label}: ${v}`);
      continue;
    }
    const o = k.options[r];
    if (o) lines.push(`- ${k.label}: ${o.value}`);
  }
  for (const f of m.free) lines.push(`- ${f.value}`);
  return lines.length ? `${lines.join("\n")}\n` : "";
}

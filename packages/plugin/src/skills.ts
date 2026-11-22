// Antrean skill creator (PRD §4.7): kandidat dari pemicu → draf dari model → persetujuan pengguna.
// Status: candidate (pemicu tercatat, belum ada isi) → pending (draf siap ditinjau) → approved | rejected.
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { scanSecrets } from "./secrets";

export type SkillTrigger = "correction" | "tool-count" | "reflect" | "manual";
export type SkillStatus = "candidate" | "pending" | "approved" | "rejected";

export type SkillEntry = {
  id: string;
  t: number;
  status: SkillStatus;
  trigger: SkillTrigger;
  sessionID?: string;
  project?: string;
  excerpt?: string;
  count?: number;
  name?: string;
  description?: string;
  body?: string;
};

export const CORRECTION = /\b(salah|bukan begitu|jangan (lagi|begitu)|keliru|that'?s wrong|not like that)\b/i;
export const REFLECT = /^\s*\/reflect\b/i;
/** Prompt internal aplikasi (judul otomatis, langkah berikutnya) — bukan ucapan pengguna, jangan jadi pemicu skill. */
export const INTERNAL_PROMPT =
  /^\s*Anda (memberi judul pada sebuah percakapan|membantu pengguna melanjutkan percakapan)/i;
export const TOOL_COUNT_THRESHOLD = 5;
const NAME = /^[a-z0-9]+(-[a-z0-9]+)*$/;

export function readQueue(file: string): SkillEntry[] {
  if (!existsSync(file)) return [];
  const out: SkillEntry[] = [];
  for (const line of readFileSync(file, "utf8").split("\n")) {
    if (!line.trim()) continue;
    try {
      const e = JSON.parse(line) as SkillEntry;
      // Berkas lama (spike) memakai status "draft" tanpa id.
      if (!e.id) e.id = `legacy-${e.t}`;
      if ((e.status as string) === "draft") e.status = "candidate";
      out.push(e);
    } catch {
      // baris rusak dilewati
    }
  }
  return out;
}

export function writeQueue(file: string, entries: SkillEntry[]): void {
  writeFileSync(file, entries.map((e) => JSON.stringify(e)).join("\n") + (entries.length ? "\n" : ""));
}

export function addCandidate(file: string, e: Omit<SkillEntry, "id" | "t" | "status">): SkillEntry {
  const entry: SkillEntry = {
    id: `sk_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
    t: Date.now(),
    status: "candidate",
    ...e,
  };
  appendFileSync(file, `${JSON.stringify(entry)}\n`);
  return entry;
}

export type DraftInput = { name: string; description: string; body: string };

/** Validasi draf dari model: nama, deskripsi, panjang, dan pemindai secret. Mengembalikan pesan galat atau null. */
export function validateDraft(d: DraftInput): string | null {
  if (!NAME.test(d.name) || d.name.length > 64)
    return "Nama skill harus huruf kecil/angka dengan tanda hubung (maks 64), mis. `tulis-changelog`.";
  if (d.description.trim().length < 10 || d.description.length > 300) return "Deskripsi harus 10–300 karakter.";
  if (d.body.trim().length < 20 || d.body.length > 8000) return "Isi skill harus 20–8000 karakter.";
  const found = scanSecrets(`${d.name}\n${d.description}\n${d.body}`);
  if (found.length)
    return `Ditolak: draf tampak memuat secret (${found.join(", ")}). Hapus nilai rahasia lalu coba lagi.`;
  return null;
}

/** Isi draf ke kandidat terbuka milik sesi (bila ada), atau buat entri baru berstatus pending. */
export function submitDraft(file: string, d: DraftInput, sessionID: string | undefined, project: string): SkillEntry {
  const all = readQueue(file);
  const open = sessionID
    ? [...all].reverse().find((e) => e.status === "candidate" && e.sessionID === sessionID)
    : undefined;
  if (open) {
    Object.assign(open, { status: "pending", ...d, project });
    writeQueue(file, all);
    return open;
  }
  const entry: SkillEntry = {
    id: `sk_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
    t: Date.now(),
    status: "pending",
    trigger: "manual",
    sessionID,
    project,
    ...d,
  };
  all.push(entry);
  writeQueue(file, all);
  return entry;
}

/** Isi berkas SKILL.md untuk draf yang disetujui. */
export function skillMarkdown(e: Pick<SkillEntry, "name" | "description" | "body">): string {
  const desc = (e.description ?? "").replace(/\s+/g, " ").replace(/"/g, '\\"');
  return `---\nname: ${e.name}\ndescription: "${desc}"\n---\n\n${(e.body ?? "").trim()}\n`;
}

export const skillHint = (trigger: string, auto = false) =>
  `<zandor-skill-hint>\nPemicu skill creator terdeteksi (${trigger}). Setelah menyelesaikan permintaan user saat ini, bila ada prosedur atau preferensi yang bisa dipakai ulang, panggil tool skill_propose sekali dengan nama (kebab-case), deskripsi (kapan dipakai), dan isi langkah singkat. Jangan memasukkan secret, token, atau data pribadi. Lewati bila tidak ada yang layak. Setelah mengusulkan, ringkas isi draf langsung di balasan; jangan membaca berkas antrean atau folder data Zandor (di luar project). ${auto ? "Skill langsung dipasang tanpa persetujuan; user bisa menghapusnya di Pengaturan > Memori dan skill, jadi buat yang benar-benar berguna." : "User menyetujui draf lewat Pengaturan > Memori dan skill."}\n</zandor-skill-hint>`;

/** Pasang skill langsung (mode otomatis). Tidak menimpa skill yang sudah ada. Mengembalikan jalur atau pesan galat. */
export function installSkill(root: string, d: DraftInput): { path: string } | { error: string } {
  const folder = join(root, d.name);
  const file = join(folder, "SKILL.md");
  if (existsSync(file)) return { error: `Skill "${d.name}" sudah ada; pakai nama lain.` };
  try {
    mkdirSync(folder, { recursive: true });
    writeFileSync(file, skillMarkdown(d));
    return { path: file };
  } catch (e) {
    return { error: `Gagal memasang skill: ${(e as Error).message}` };
  }
}

/** Catat skill yang dipasang otomatis ke antrean sebagai riwayat (status approved). */
export function recordAutoInstall(file: string, d: DraftInput, sessionID: string | undefined, project: string): void {
  const entry: SkillEntry = {
    id: `sk_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
    t: Date.now(),
    status: "approved",
    trigger: "manual",
    sessionID,
    project,
    ...d,
  };
  appendFileSync(
    file,
    `${JSON.stringify(entry)}
`,
  );
}

/** Batas skill yang boleh dibuat otomatis per sesi (cegah banjir skill). */
export const AUTO_SKILL_LIMIT = 3;

/** Mode otomatis (ala Hermes): model sendiri menilai, tanpa pemicu dari user. Disuntik tiap giliran. */
export const AUTO_SKILL_HINT = [
  "<zandor-skill-auto>",
  'Skill creator otomatis aktif. SEBELUM menulis jawaban akhir, putuskan: apakah giliran ini memenuhi salah satu kriteria? (a) user menyebut format/gaya/aturan yang berulang ("biasa", "selalu", "format tetap"); (b) tugas multi-langkah yang berhasil dan polanya bisa dipakai ulang; (c) kamu memulihkan diri dari galat dengan cara yang layak diingat. Bila ya, WAJIB panggil skill_propose SEKARANG (satu kali) dengan nama kebab-case, deskripsi kapan dipakai, dan langkah ringkas; skill langsung terpasang tanpa persetujuan, lalu sebut singkat di jawabanmu bahwa kamu membuat skill itu. Bila tidak, lanjut tanpa menyebut ini. Jangan membuat skill untuk tugas sepele atau sekali pakai, jangan duplikat skill yang sudah ada, jangan memasukkan secret atau data pribadi.',
  "</zandor-skill-auto>",
].join("\n");

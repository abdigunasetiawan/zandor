// Registry perintah: sumber tunggal untuk menu, command palette, dan pintasan keyboard.
// Judul disimpan sebagai kunci i18n; pemanggil yang menerjemahkan (core tidak bergantung pada paket i18n).

export type CommandDef = {
  /** Unik, mis. "view.toggleChat". */
  id: string;
  /** Kunci i18n judul. */
  titleKey: string;
  /** Kelompok untuk pengurutan/label di palette, mis. "view", "app". */
  group: string;
  /** Combo pintasan (format shortcuts.ts), opsional. Dibindingkan otomatis oleh UI. */
  shortcut?: string;
  /** Kata kunci tambahan untuk pencarian (sudah dalam bahasa aktif atau netral). */
  keywords?: string[];
  /** false → tampil redup di palette dan tidak bisa dijalankan. */
  enabled?: () => boolean;
  run: () => void | Promise<void>;
};

export class CommandRegistry {
  private commands = new Map<string, CommandDef>();
  private listeners = new Set<() => void>();
  private snapshot: CommandDef[] = [];

  private changed() {
    this.snapshot = [...this.commands.values()];
    for (const l of this.listeners) l();
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Daftarkan perintah. Melempar bila id sudah ada. Mengembalikan fungsi pelepas. */
  register(command: CommandDef): () => void {
    if (this.commands.has(command.id)) throw new Error(`id perintah ganda: ${command.id}`);
    this.commands.set(command.id, command);
    this.changed();
    return () => {
      if (this.commands.delete(command.id)) this.changed();
    };
  }

  /** Daftar perintah. Identitas array stabil selama tidak ada perubahan. */
  list(): CommandDef[] {
    return this.snapshot;
  }

  get(id: string): CommandDef | undefined {
    return this.commands.get(id);
  }

  isEnabled(id: string): boolean {
    const c = this.commands.get(id);
    return !!c && (c.enabled ? c.enabled() : true);
  }

  /** Jalankan perintah; false bila tidak ada atau sedang dinonaktifkan. */
  async run(id: string): Promise<boolean> {
    const c = this.commands.get(id);
    if (!c || (c.enabled && !c.enabled())) return false;
    await c.run();
    return true;
  }
}

// ---------- Pencarian fuzzy ----------

export type FuzzyMatch = { score: number; indices: number[] };

const isBoundary = (text: string, i: number) => i === 0 || /[\s\-_./:]/.test(text[i - 1] ?? "");

/**
 * Cocok bila semua karakter `query` muncul berurutan di `text` (tanpa memperhatikan huruf besar/kecil).
 * Skor lebih tinggi = lebih baik: bonus untuk awal kata, karakter berurutan, dan kecocokan awal teks.
 */
export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
  const q = query.trim().toLowerCase();
  if (!q) return { score: 0, indices: [] };
  const t = text.toLowerCase();
  const indices: number[] = [];
  let score = 0;
  let from = 0;
  let prev = -2;
  for (const ch of q) {
    if (ch === " ") continue;
    const at = t.indexOf(ch, from);
    if (at === -1) return null;
    indices.push(at);
    score += 1;
    if (at === prev + 1) score += 3; // berurutan
    if (isBoundary(t, at)) score += 2; // awal kata
    if (at === 0) score += 2;
    prev = at;
    from = at + 1;
  }
  // Teks lebih pendek dan kecocokan lebih rapat lebih baik.
  const span = (indices.at(-1) ?? 0) - (indices[0] ?? 0) + 1;
  score -= (span - indices.length) * 0.5;
  score -= text.length * 0.02;
  return { score, indices };
}

export type CommandHit = { command: CommandDef; title: string; match: FuzzyMatch; enabled: boolean };

/** Cari perintah menurut judul terjemahan + kata kunci. Query kosong → semua, urut kelompok lalu judul. */
export function searchCommands(
  commands: CommandDef[],
  query: string,
  translate: (key: string) => string,
): CommandHit[] {
  const hits: CommandHit[] = [];
  for (const command of commands) {
    const title = translate(command.titleKey);
    const enabled = command.enabled ? command.enabled() : true;
    const byTitle = fuzzyMatch(query, title);
    let match = byTitle;
    if (!match) {
      for (const k of command.keywords ?? []) {
        const m = fuzzyMatch(query, k);
        if (m && (!match || m.score > match.score)) match = { score: m.score - 2, indices: [] };
      }
    }
    if (match) hits.push({ command, title, match, enabled });
  }
  return hits.sort((a, b) => {
    if (!query.trim()) return a.command.group.localeCompare(b.command.group) || a.title.localeCompare(b.title);
    return b.match.score - a.match.score || a.title.localeCompare(b.title);
  });
}

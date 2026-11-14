// Preferensi tampilan daftar sidebar: sesi/project yang disematkan (pin) dan urutan buatan pengguna (seret-lepas). Murni data
// + persistensi lewat `prefs`; pengurutan ditentukan di sini agar bisa diuji: disematkan dulu (urutan sematan), lalu urutan
// buatan pengguna, lalu urutan bawaan (apa adanya dari pemanggil).

export type SidebarPrefsStorage = { get(key: string): string | null; set(key: string, value: string): void };
export type SidebarPrefsState = {
  pinnedSessions: string[];
  pinnedProjects: string[];
  /** Urutan project buatan pengguna (id). */
  projectOrder: string[];
  /** Urutan sesi per kelompok (kunci project atau "chat"). */
  sessionOrder: Record<string, string[]>;
};

const PREF_KEY = "zandor.sidebarPrefs";
const MAX = 2000;

const strings = (v: unknown): string[] =>
  Array.isArray(v) ? [...new Set(v.filter((x): x is string => typeof x === "string"))].slice(0, MAX) : [];

export function parseSidebarPrefs(raw: string | null): SidebarPrefsState {
  const empty: SidebarPrefsState = { pinnedSessions: [], pinnedProjects: [], projectOrder: [], sessionOrder: {} };
  if (!raw) return empty;
  try {
    const o = JSON.parse(raw) as Record<string, unknown>;
    const orders: Record<string, string[]> = {};
    const so = o.sessionOrder;
    if (so && typeof so === "object")
      for (const [k, v] of Object.entries(so as Record<string, unknown>)) {
        const list = strings(v);
        if (list.length) orders[k] = list;
      }
    return {
      pinnedSessions: strings(o.pinnedSessions),
      pinnedProjects: strings(o.pinnedProjects),
      projectOrder: strings(o.projectOrder),
      sessionOrder: orders,
    };
  } catch {
    return empty;
  }
}

/** Urutkan `items` (urutan bawaan): disematkan dulu menurut urutan sematan, lalu yang punya urutan buatan, lalu sisanya. */
export function sortWithPrefs<T>(items: T[], idOf: (t: T) => string, pinned: string[], order: string[]): T[] {
  const pinRank = new Map(pinned.map((id, i) => [id, i]));
  const ordRank = new Map(order.map((id, i) => [id, i]));
  const base = new Map(items.map((t, i) => [idOf(t), i]));
  return [...items].sort((a, b) => {
    const ia = idOf(a);
    const ib = idOf(b);
    const pa = pinRank.get(ia);
    const pb = pinRank.get(ib);
    if (pa !== undefined || pb !== undefined) {
      if (pa === undefined) return 1;
      if (pb === undefined) return -1;
      return pa - pb;
    }
    const oa = ordRank.get(ia);
    const ob = ordRank.get(ib);
    if (oa !== undefined && ob !== undefined) return oa - ob;
    if (oa !== undefined) return -1; // yang diurutkan pengguna mendahului yang belum
    if (ob !== undefined) return 1;
    return (base.get(ia) ?? 0) - (base.get(ib) ?? 0);
  });
}

export class SidebarPrefs {
  private snap: SidebarPrefsState;
  private listeners = new Set<() => void>();

  constructor(private readonly storage: SidebarPrefsStorage) {
    this.snap = parseSidebarPrefs(storage.get(PREF_KEY));
  }

  state(): SidebarPrefsState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private set(next: SidebarPrefsState) {
    this.snap = next;
    try {
      this.storage.set(PREF_KEY, JSON.stringify(next));
    } catch {
      // penyimpanan penuh/tidak tersedia: tetap berlaku selama aplikasi berjalan
    }
    for (const l of this.listeners) l();
  }

  isSessionPinned(id: string): boolean {
    return this.snap.pinnedSessions.includes(id);
  }
  isProjectPinned(id: string): boolean {
    return this.snap.pinnedProjects.includes(id);
  }

  toggleSessionPin(id: string): void {
    const cur = this.snap.pinnedSessions;
    this.set({ ...this.snap, pinnedSessions: cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id] });
  }
  toggleProjectPin(id: string): void {
    const cur = this.snap.pinnedProjects;
    this.set({ ...this.snap, pinnedProjects: cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id] });
  }

  sortProjects<T>(items: T[], idOf: (t: T) => string): T[] {
    return sortWithPrefs(items, idOf, this.snap.pinnedProjects, this.snap.projectOrder);
  }
  sortSessions<T>(group: string, items: T[], idOf: (t: T) => string): T[] {
    return sortWithPrefs(items, idOf, this.snap.pinnedSessions, this.snap.sessionOrder[group] ?? []);
  }

  /**
   * Pindahkan `id` ke posisi `toIndex` di antara `visibleIds` (urutan yang sedang tampil, sudah terurut). Menyematkan/melepas
   * sematan tidak berubah: item hanya bergeser DI DALAM kelompoknya (disematkan atau tidak). Urutan penuh disimpan.
   */
  private reorder(visibleIds: string[], id: string, toIndex: number, pinned: string[]): string[] {
    const isPinned = pinned.includes(id);
    const group = visibleIds.filter((x) => pinned.includes(x) === isPinned);
    const from = group.indexOf(id);
    if (from < 0) return visibleIds;
    const to = Math.max(0, Math.min(group.length - 1, toIndex));
    const next = [...group];
    next.splice(from, 1);
    next.splice(to, 0, id);
    // gabungkan kembali: kelompok disematkan dulu, lalu tidak disematkan (sesuai pengurutan)
    const other = visibleIds.filter((x) => pinned.includes(x) !== isPinned);
    return isPinned ? [...next, ...other] : [...other, ...next];
  }

  moveProject(visibleIds: string[], id: string, toIndex: number): void {
    // indeks tujuan relatif terhadap kelompoknya
    const isPinned = this.snap.pinnedProjects.includes(id);
    const offset = isPinned ? 0 : visibleIds.filter((x) => this.snap.pinnedProjects.includes(x)).length;
    this.set({ ...this.snap, projectOrder: this.reorder(visibleIds, id, toIndex - offset, this.snap.pinnedProjects) });
  }

  moveSession(group: string, visibleIds: string[], id: string, toIndex: number): void {
    const isPinned = this.snap.pinnedSessions.includes(id);
    const offset = isPinned ? 0 : visibleIds.filter((x) => this.snap.pinnedSessions.includes(x)).length;
    const order = this.reorder(visibleIds, id, toIndex - offset, this.snap.pinnedSessions);
    this.set({ ...this.snap, sessionOrder: { ...this.snap.sessionOrder, [group]: order } });
  }

  /** Buang sesi yang sudah tidak ada (dihapus) dari sematan dan urutan. */
  forgetSession(id: string): void {
    const sessionOrder: Record<string, string[]> = {};
    for (const [k, v] of Object.entries(this.snap.sessionOrder)) sessionOrder[k] = v.filter((x) => x !== id);
    this.set({ ...this.snap, pinnedSessions: this.snap.pinnedSessions.filter((x) => x !== id), sessionOrder });
  }
  forgetProject(id: string): void {
    this.set({
      ...this.snap,
      pinnedProjects: this.snap.pinnedProjects.filter((x) => x !== id),
      projectOrder: this.snap.projectOrder.filter((x) => x !== id),
    });
  }
}

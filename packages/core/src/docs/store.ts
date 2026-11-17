// Tab dokumen Panel 3 (Fase 7). State tab (metadata) bersifat reaktif; isi berkas (byte) disimpan di luar state dan
// diambil lewat `content(id)`. Hanya beberapa tab terakhir menyimpan isi di memori; sisanya dimuat ulang saat dibuka.
import {
  classify,
  type DocKind,
  type DocMode,
  decodeText,
  defaultMode,
  fileName,
  hasRawMode,
  isTextKind,
} from "./kinds";

export type FileSource = {
  /** Byte persis berkas `path` (relatif) di dalam `root`. Menolak dengan pesan jelas bila gagal. */
  read(root: string, path: string): Promise<Uint8Array>;
  /** Ukuran + waktu modifikasi (ms). Opsional: dipakai mendeteksi perubahan di disk sebelum menyimpan. */
  stat?(root: string, path: string): Promise<{ size: number; modified: number }>;
  /**
   * Simpan teks UTF-8 ke berkas yang sudah ada. `expectedModified` ≠ waktu di disk → menolak dengan pesan berawalan
   * `KONFLIK:`. Mengembalikan waktu modifikasi baru. Opsional: tanpa ini editor tidak tersedia (mis. build web).
   */
  write?(root: string, path: string, text: string, expectedModified: number | null): Promise<number>;
};

/** Alasan berkas tidak bisa diedit (kunci i18n `doc.noEdit.*` di UI), atau null bila bisa. */
export type EditBlock = "noWrite" | "kind" | "notLoaded" | "encoding" | "truncated" | "binary";
export type SaveResult = "saved" | "conflict" | { error: string };

export type DocTab = {
  id: string;
  root: string;
  /** Relatif terhadap root, pemisah `/`. */
  path: string;
  name: string;
  kind: DocKind;
  status: "idle" | "loading" | "ready" | "error";
  error: string | null;
  size: number;
  /** Naik setiap kali isi (re)load, dipakai viewer untuk memuat ulang. */
  version: number;
  mode: DocMode;
  wrap: boolean;
  /** Waktu modifikasi di disk saat terakhir dibaca/disimpan (ms), bila host mendukung. */
  modified: number | null;
  editing: boolean;
  dirty: boolean;
  /** Diff unified yang sedang ditinjau di tab ini (ADR-075); tidak dipersistenkan. */
  diff?: string | null;
};

export type DocsState = { tabs: DocTab[]; activeId: string | null };

export type DocPrefs = { get(key: string): string | null; set(key: string, value: string): void };
export type DocStoreOptions = { source: FileSource; prefs?: DocPrefs; maxCached?: number; maxTabs?: number };

const PREF_KEY = "zandor.docTabs";
type Persisted = { active: string | null; tabs: Array<{ root: string; path: string; mode: DocMode; wrap: boolean }> };

export const docId = (root: string, path: string) => `${root}\u0000${path}`;
const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class DocStore {
  private snap: DocsState = { tabs: [], activeId: null };
  private listeners = new Set<() => void>();
  private cache = new Map<string, Uint8Array>(); // urutan penyisipan = urutan pemakaian (terlama dulu)
  private seq = new Map<string, number>();
  private readonly maxCached: number;
  private readonly maxTabs: number;
  private restored = false;

  constructor(private readonly options: DocStoreOptions) {
    this.maxCached = options.maxCached ?? 4;
    this.maxTabs = options.maxTabs ?? 16;
  }

  state(): DocsState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private update(patch: Partial<DocsState>) {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l();
    this.persist();
  }

  private patchTab(id: string, fn: (t: DocTab) => DocTab) {
    const tabs = this.snap.tabs.map((t) => (t.id === id ? fn(t) : t));
    this.update({ tabs });
  }

  private persist() {
    if (!this.options.prefs) return;
    const data: Persisted = {
      active: this.snap.activeId,
      tabs: this.snap.tabs.map((t) => ({ root: t.root, path: t.path, mode: t.mode, wrap: t.wrap })),
    };
    try {
      this.options.prefs.set(PREF_KEY, JSON.stringify(data));
    } catch {
      // persistensi hanya kenyamanan
    }
  }

  /** Pulihkan tab dari sesi sebelumnya (tanpa memuat isinya); tab aktif langsung dimuat. */
  restore(): void {
    if (this.restored) return;
    this.restored = true;
    const raw = this.options.prefs?.get(PREF_KEY);
    if (!raw || this.snap.tabs.length > 0) return;
    let data: Persisted;
    try {
      data = JSON.parse(raw) as Persisted;
    } catch {
      return;
    }
    if (!data || !Array.isArray(data.tabs)) return;
    const tabs: DocTab[] = [];
    for (const t of data.tabs.slice(0, this.maxTabs)) {
      if (typeof t?.root !== "string" || typeof t.path !== "string") continue;
      const id = docId(t.root, t.path);
      if (tabs.some((x) => x.id === id)) continue;
      const kind = classify(t.path, null);
      tabs.push({
        id,
        root: t.root,
        path: t.path,
        name: fileName(t.path),
        kind,
        status: "idle",
        error: null,
        size: 0,
        version: 0,
        mode: t.mode === "render" || t.mode === "raw" ? t.mode : defaultMode(kind),
        wrap: t.wrap === true,
        modified: null,
        editing: false,
        dirty: false,
      });
    }
    const activeId = tabs.some((t) => t.id === data.active) ? data.active : (tabs.at(-1)?.id ?? null);
    this.snap = { tabs, activeId };
    for (const l of this.listeners) l();
    if (activeId) void this.load(activeId);
  }

  /** Buka (atau aktifkan) berkas. Berkas yang sama tidak digandakan. */
  open(root: string, path: string): string {
    const rel = path.replace(/\\/g, "/").replace(/^\.?\//, "");
    const id = docId(root, rel);
    if (this.snap.tabs.some((t) => t.id === id)) {
      this.activate(id);
      return id;
    }
    const kind = classify(rel, null);
    const tab: DocTab = {
      id,
      root,
      path: rel,
      name: fileName(rel),
      kind,
      status: "idle",
      error: null,
      size: 0,
      version: 0,
      mode: defaultMode(kind),
      wrap: false,
      modified: null,
      editing: false,
      dirty: false,
    };
    let tabs = [...this.snap.tabs, tab];
    if (tabs.length > this.maxTabs) {
      // buang tab tertua yang bukan aktif
      const drop = tabs.find((t) => t.id !== this.snap.activeId && t.id !== id);
      if (drop) {
        tabs = tabs.filter((t) => t.id !== drop.id);
        this.cache.delete(drop.id);
      }
    }
    this.update({ tabs, activeId: id });
    void this.load(id);
    return id;
  }

  activate(id: string) {
    const tab = this.snap.tabs.find((t) => t.id === id);
    if (!tab) return;
    if (this.snap.activeId !== id) this.update({ activeId: id });
    if (tab.status === "idle" || (tab.status === "ready" && !this.cache.has(id))) void this.load(id);
    else this.touch(id);
  }

  close(id: string) {
    const i = this.snap.tabs.findIndex((t) => t.id === id);
    if (i === -1) return;
    const tabs = this.snap.tabs.filter((t) => t.id !== id);
    this.cache.delete(id);
    this.seq.delete(id);
    let activeId = this.snap.activeId;
    if (activeId === id) activeId = tabs[Math.min(i, tabs.length - 1)]?.id ?? null;
    this.update({ tabs, activeId });
    const active = tabs.find((t) => t.id === activeId);
    if (active && (active.status === "idle" || !this.cache.has(active.id))) void this.load(active.id);
  }

  reload(id: string) {
    void this.load(id);
  }

  setMode(id: string, mode: DocMode) {
    const t = this.snap.tabs.find((x) => x.id === id);
    if (!t || t.mode === mode) return;
    if (!hasRawMode(t.kind)) return; // hanya jenis dengan dua tampilan yang bisa berganti mode
    this.patchTab(id, (x) => ({ ...x, mode }));
  }

  toggleWrap(id: string) {
    this.patchTab(id, (x) => ({ ...x, wrap: !x.wrap }));
  }

  /** Buka berkas dan tinjau `patch` (diff unified dari engine) di tab itu. */
  openDiff(root: string, path: string, patch: string): string {
    const id = this.open(root, path);
    this.setDiff(id, patch);
    return id;
  }

  /** Pasang/lepas diff yang ditinjau; null kembali ke tampilan berkas biasa. */
  setDiff(id: string, patch: string | null) {
    this.patchTab(id, (t) => ({ ...t, diff: patch && patch.trim() ? patch : null }));
  }

  /** Apakah tab ini bisa diedit sekarang? null = bisa. */
  editBlock(id: string): EditBlock | null {
    const tab = this.snap.tabs.find((t) => t.id === id);
    const bytes = this.cache.get(id);
    if (!this.options.source.write) return "noWrite";
    if (tab?.status !== "ready" || !bytes) return "notLoaded";
    if (!isTextKind(tab.kind)) return "kind";
    const d = decodeText(bytes);
    if (d.truncated) return "truncated";
    if (d.encoding !== "utf-8") return "encoding";
    if (bytes.includes(0)) return "binary";
    if (d.text.includes("\uFFFD")) return "encoding";
    return null;
  }

  /** Mulai mengedit (beralih ke tampilan mentah bila perlu). Mengembalikan alasan penolakan atau null. */
  beginEdit(id: string): EditBlock | null {
    const block = this.editBlock(id);
    if (block) return block;
    const tab = this.snap.tabs.find((t) => t.id === id);
    if (!tab) return "notLoaded";
    this.patchTab(id, (t) => ({ ...t, editing: true, mode: hasRawMode(t.kind) ? "raw" : t.mode }));
    return null;
  }

  markDirty(id: string, dirty: boolean) {
    const t = this.snap.tabs.find((x) => x.id === id);
    if (t && t.dirty !== dirty) this.patchTab(id, (x) => ({ ...x, dirty }));
  }

  /** Selesai mengedit tanpa menyimpan (pemanggil sudah mengonfirmasi). Isi di editor dimuat ulang dari cache. */
  endEdit(id: string) {
    this.patchTab(id, (x) => ({ ...x, editing: false, dirty: false, version: x.version + 1 }));
  }

  anyDirty(): boolean {
    return this.snap.tabs.some((t) => t.dirty);
  }

  /** Simpan teks hasil edit. `force` menimpa walau berkas berubah di disk. */
  async save(id: string, text: string, force = false): Promise<SaveResult> {
    const tab = this.snap.tabs.find((t) => t.id === id);
    const write = this.options.source.write;
    if (!tab || !write) return { error: "penyimpanan tidak tersedia" };
    const old = this.cache.get(id);
    const bom = old ? decodeText(old).bom : false;
    try {
      // BOM UTF-8 asli dipertahankan: host menulis string apa adanya, jadi BOM ikut sebagai karakter U+FEFF.
      const out = bom ? `﻿${text}` : text;
      const modified = await write.call(this.options.source, tab.root, tab.path, out, force ? null : tab.modified);
      const bytes = new TextEncoder().encode(out);
      this.remember(id, bytes);
      this.patchTab(id, (t) => ({ ...t, size: bytes.length, modified, dirty: false, error: null }));
      return "saved";
    } catch (e) {
      const msg = errMsg(e);
      return msg.startsWith("KONFLIK") ? "conflict" : { error: msg };
    }
  }

  /** Byte isi tab (null bila belum dimuat/sudah dibuang dari cache). */
  content(id: string): Uint8Array | null {
    return this.cache.get(id) ?? null;
  }

  private touch(id: string) {
    const v = this.cache.get(id);
    if (!v) return;
    this.cache.delete(id);
    this.cache.set(id, v);
  }

  private remember(id: string, bytes: Uint8Array) {
    this.cache.delete(id);
    this.cache.set(id, bytes);
    while (this.cache.size > this.maxCached) {
      const oldest = [...this.cache.keys()].find((k) => k !== this.snap.activeId);
      if (!oldest) break;
      this.cache.delete(oldest);
      // tab yang isinya dibuang akan dimuat ulang saat diaktifkan
      this.snap = { ...this.snap, tabs: this.snap.tabs.map((t) => (t.id === oldest ? { ...t, status: "idle" } : t)) };
    }
  }

  private async load(id: string): Promise<void> {
    const tab = this.snap.tabs.find((t) => t.id === id);
    if (!tab) return;
    const n = (this.seq.get(id) ?? 0) + 1;
    this.seq.set(id, n);
    this.patchTab(id, (t) => ({ ...t, status: "loading", error: null }));
    try {
      const bytes = await this.options.source.read(tab.root, tab.path);
      if (this.seq.get(id) !== n || !this.snap.tabs.some((t) => t.id === id)) return;
      const stat = await this.options.source.stat?.(tab.root, tab.path).catch(() => null);
      if (this.seq.get(id) !== n || !this.snap.tabs.some((t) => t.id === id)) return;
      this.remember(id, bytes);
      this.patchTab(id, (t) => ({
        ...t,
        status: "ready",
        error: null,
        size: bytes.length,
        kind: classify(t.path, bytes),
        version: t.version + 1,
        modified: stat?.modified ?? null,
        dirty: false,
      }));
    } catch (e) {
      if (this.seq.get(id) !== n || !this.snap.tabs.some((t) => t.id === id)) return;
      this.cache.delete(id);
      this.patchTab(id, (t) => ({ ...t, status: "error", error: errMsg(e) }));
    }
  }
}

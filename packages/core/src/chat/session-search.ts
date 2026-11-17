// Pencarian global lintas sesi (Ctrl+Shift+F): judul sesi + teks pesan pengguna/asisten di semua project dan Chat.
// Pesan dimuat dari engine per sesi (konkurensi terbatas) dan di-cache per sesi selama `time.updated` sama, jadi
// pencarian berikutnya instan dan sesi yang berubah dimuat ulang. Tidak ada indeks di disk.
import type { EngineApi, SessionInfo } from "../engine/api";
import { sessionTitle } from "../engine/projects";
import { messageText } from "./reducer";
import type { ChatMessage } from "./types";

export type SearchableSession = { key: string; session: SessionInfo };

export type SessionHit = {
  messageId: string;
  role: "user" | "assistant";
  /** Potongan teks di sekitar kecocokan; `match` = [awal, akhir) di dalam `snippet`. */
  snippet: string;
  match: [number, number];
};

export type SessionResult = {
  key: string;
  session: SessionInfo;
  titleMatch: boolean;
  hits: SessionHit[];
  /** Jumlah kecocokan di pesan (bisa lebih banyak dari `hits` yang ditampilkan). */
  total: number;
};

export type SearchProgress = { done: number; total: number; failed: number };

const MAX_HITS = 3;
const CONTEXT = 48;

/** Potongan di sekitar kecocokan pertama `q` (huruf kecil) mulai `from`; spasi dirapatkan. */
export function snippetAt(text: string, at: number, len: number): { snippet: string; match: [number, number] } {
  const start = Math.max(0, at - CONTEXT);
  const end = Math.min(text.length, at + len + CONTEXT);
  const pre = (start > 0 ? "…" : "") + text.slice(start, at).replace(/\s+/g, " ").trimStart();
  const hit = text.slice(at, at + len);
  const post =
    text
      .slice(at + len, end)
      .replace(/\s+/g, " ")
      .trimEnd() + (end < text.length ? "…" : "");
  return { snippet: pre + hit + post, match: [pre.length, pre.length + hit.length] };
}

/** Cari `query` di judul dan pesan satu sesi. null bila tidak ada kecocokan. */
export function searchSession(item: SearchableSession, messages: ChatMessage[], query: string): SessionResult | null {
  const q = query.trim().toLowerCase();
  if (!q) return null;
  // judul bawaan engine ("New session - …") bukan judul sungguhan, jadi tidak ikut dicocokkan
  const titleMatch = (sessionTitle(item.session) ?? "").toLowerCase().includes(q);
  const hits: SessionHit[] = [];
  let total = 0;
  for (const m of messages) {
    const text = messageText(m);
    const lower = text.toLowerCase();
    for (let i = lower.indexOf(q); i !== -1; i = lower.indexOf(q, i + q.length)) {
      total++;
      if (hits.length < MAX_HITS && !hits.some((h) => h.messageId === m.info.id)) {
        hits.push({ messageId: m.info.id, role: m.info.role, ...snippetAt(text, i, q.length) });
      }
    }
  }
  if (!titleMatch && total === 0) return null;
  return { key: item.key, session: item.session, titleMatch, hits, total };
}

/** Urutan hasil: judul cocok dulu, lalu jumlah kecocokan, lalu sesi terbaru. */
export function rankResults(results: SessionResult[]): SessionResult[] {
  return [...results].sort(
    (a, b) =>
      Number(b.titleMatch) - Number(a.titleMatch) ||
      b.total - a.total ||
      b.session.time.updated - a.session.time.updated,
  );
}

type CacheEntry = { updated: number; messages: ChatMessage[] };

export class SessionSearch {
  private cache = new Map<string, CacheEntry>();
  private seq = 0;

  constructor(
    private readonly api: () => Pick<EngineApi, "listMessages"> | null,
    private readonly concurrency = 4,
  ) {}

  /** Batalkan pencarian yang sedang berjalan (hasil parsialnya tidak dilaporkan lagi). */
  cancel() {
    this.seq++;
  }

  /**
   * Cari di semua sesi. `onUpdate` dipanggil setiap ada sesi selesai dimuat (hasil bertambah bertahap).
   * Mengembalikan hasil akhir, atau null bila dibatalkan oleh pencarian baru.
   */
  async search(
    sessions: SearchableSession[],
    query: string,
    onUpdate?: (results: SessionResult[], progress: SearchProgress) => void,
  ): Promise<{ results: SessionResult[]; progress: SearchProgress } | null> {
    const my = ++this.seq;
    const api = this.api();
    const results: SessionResult[] = [];
    const progress: SearchProgress = { done: 0, total: sessions.length, failed: 0 };
    if (!query.trim()) return { results, progress };
    let next = 0;
    const worker = async () => {
      while (next < sessions.length) {
        const item = sessions[next++] as SearchableSession;
        let messages: ChatMessage[] = [];
        const cached = this.cache.get(item.session.id);
        if (cached && cached.updated === item.session.time.updated) messages = cached.messages;
        else if (api) {
          try {
            messages = await api.listMessages(item.session.directory, item.session.id);
            this.cache.set(item.session.id, { updated: item.session.time.updated, messages });
          } catch {
            progress.failed++;
          }
        }
        if (my !== this.seq) return;
        const r = searchSession(item, messages, query);
        if (r) results.push(r);
        progress.done++;
        onUpdate?.(rankResults(results), { ...progress });
      }
    };
    await Promise.all(Array.from({ length: Math.min(this.concurrency, sessions.length) }, worker));
    if (my !== this.seq) return null;
    return { results: rankResults(results), progress };
  }
}

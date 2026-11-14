// Indikator sesi dan pelacak perhatian: dari state chat tiap sesi menurunkan status tampilan (idle, bekerja, butuh aksi, selesai,
// gagal, batas penyedia) dan mendeteksi PERALIHAN yang layak diberitahukan (selesai/gagal/butuh aksi). Murni terhadap state,
// tanpa efek samping: pengiriman toast/notifikasi/suara dilakukan oleh pemanggil.
import type { SessionChat } from "./store";

export type SessionIndicator = "idle" | "busy" | "action" | "done" | "failed" | "limit";
export type AttentionEvent =
  | { type: "done"; sessionId: string }
  | { type: "failed" | "limit"; sessionId: string; message: string }
  | { type: "action"; sessionId: string; kind: "permission" | "question"; detail: string };

const LIMIT_RE =
  /rate.?limit|too many requests|\b429\b|quota|insufficient|credit|billing|usage limit|limit (reached|exceeded)|overloaded|context (length|window)|maximum context|token limit/i;

/** Apakah pesan galat berasal dari batas penyedia (rate limit, kuota, kredit, konteks penuh)? */
export const isLimitError = (message: string): boolean => LIMIT_RE.test(message);

const busyOf = (s: SessionChat) => s.status.type !== "idle" || s.sending;

/** Kegagalan giliran terakhir: galat pada pesan asisten terakhir (bukan dibatalkan pengguna) atau galat tingkat sesi. */
export function failureOf(s: SessionChat): { kind: "failed" | "limit"; message: string } | null {
  let message = "";
  const last = s.messages.at(-1);
  if (last?.info.role === "assistant" && last.info.error && last.info.error.name !== "MessageAbortedError") {
    message = last.info.error.message || last.info.error.name;
  } else if (s.error) {
    message = s.error;
  }
  if (!message) return null;
  return { kind: isLimitError(message) ? "limit" : "failed", message };
}

export type AttentionState = { unseen: Record<string, "done"> };
/** Jeda mengendap sebelum akhir giliran dinilai (event galat/pesan bisa tiba sedikit setelah status idle). */
export const SETTLE_MS = 300;

export type ObserveContext = {
  /** Waktu sekarang (ms); bawaan Date.now(). Untuk tes. */
  now?: number;
  /** Sesi yang sedang dilihat pengguna (terpilih), atau null. */
  viewing: string | null;
  /** Jendela aplikasi sedang fokus. */
  focused: boolean;
};

type Prev = { busy: boolean; waiting: number };

export class AttentionTracker {
  private snap: AttentionState = { unseen: {} };
  private prev = new Map<string, Prev>();
  private listeners = new Set<() => void>();
  /** Sesi yang baru berhenti bekerja dan menunggu mengendap (id → waktu berhenti). */
  private ending = new Map<string, number>();

  /** Ada akhir giliran yang masih menunggu penilaian (pemanggil perlu memanggil observe lagi sesaat kemudian). */
  hasPending(): boolean {
    return this.ending.size > 0;
  }

  state(): AttentionState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private update(unseen: AttentionState["unseen"]) {
    this.snap = { unseen };
    for (const l of this.listeners) l();
  }

  /** Tandai sesi sudah dilihat (menghapus penanda "selesai"). */
  markSeen(sessionId: string): void {
    if (!this.snap.unseen[sessionId]) return;
    const { [sessionId]: _drop, ...rest } = this.snap.unseen;
    this.update(rest);
  }

  /** Status tampilan satu sesi. */
  indicator(sessionId: string, s: SessionChat | undefined): SessionIndicator {
    if (!s) return this.snap.unseen[sessionId] ? "done" : "idle";
    if (s.permissions.length + s.questions.length > 0) return "action";
    if (busyOf(s)) return "busy";
    const f = failureOf(s);
    if (f) return f.kind;
    return this.snap.unseen[sessionId] ? "done" : "idle";
  }

  /**
   * Bandingkan state sekarang dengan pengamatan sebelumnya dan kembalikan peralihan baru. Sesi yang baru pertama kali terlihat
   * hanya dicatat (riwayat lama tidak memicu pemberitahuan).
   */
  observe(sessions: Record<string, SessionChat>, ctx: ObserveContext): AttentionEvent[] {
    const events: AttentionEvent[] = [];
    const at = ctx.now ?? Date.now();
    let unseen = this.snap.unseen;
    for (const [id, s] of Object.entries(sessions)) {
      const now: Prev = { busy: busyOf(s), waiting: s.permissions.length + s.questions.length };
      const before = this.prev.get(id);
      this.prev.set(id, now);
      if (!before) continue; // pertama kali terlihat: hanya dicatat (tanpa peristiwa)
      if (now.waiting > before.waiting) {
        const perm = s.permissions.at(-1);
        const q = s.questions.at(-1);
        events.push(
          perm &&
            s.permissions.length > 0 &&
            now.waiting - before.waiting >= 1 &&
            (!q || s.permissions.length >= s.questions.length)
            ? {
                type: "action",
                sessionId: id,
                kind: "permission",
                detail: `${perm.permission}${perm.patterns.length ? `: ${perm.patterns.join(", ")}` : ""}`,
              }
            : { type: "action", sessionId: id, kind: "question", detail: q?.questions[0]?.question ?? "" },
        );
      }
      if (before.busy && !now.busy) this.ending.set(id, at);
      if (now.busy) this.ending.delete(id);
      if (!before.busy && now.busy && unseen[id]) {
        const { [id]: _drop, ...rest } = unseen; // giliran baru dimulai: penanda lama tidak relevan
        unseen = rest;
      }
    }
    // akhir giliran yang sudah mengendap: nilai (selesai / gagal / batas) dengan state terbaru
    for (const [id, since] of [...this.ending]) {
      const s = sessions[id];
      if (!s || busyOf(s)) {
        this.ending.delete(id);
        continue;
      }
      if (at - since < SETTLE_MS) continue;
      this.ending.delete(id);
      const f = failureOf(s);
      if (f) events.push({ type: f.kind, sessionId: id, message: f.message });
      else {
        events.push({ type: "done", sessionId: id });
        if (!(ctx.viewing === id && ctx.focused)) unseen = { ...unseen, [id]: "done" };
      }
    }
    // sesi yang sedang dilihat dan jendela fokus: sudah "dilihat"
    if (ctx.viewing && ctx.focused && unseen[ctx.viewing]) {
      const { [ctx.viewing]: _drop, ...rest } = unseen;
      unseen = rest;
    }
    if (unseen !== this.snap.unseen) this.update(unseen);
    return events;
  }

  /** Lupakan sesi (mis. dihapus). */
  forget(sessionId: string): void {
    this.prev.delete(sessionId);
    this.markSeen(sessionId);
  }
}

// Rekam system prompt yang benar-benar dikirim ke model, untuk panel Trajectory di aplikasi.
// Engine tidak menyimpan system prompt di riwayat pesan; hook `experimental.chat.system.transform` adalah satu-satunya
// tempat isinya terlihat. Satu berkas JSONL per sesi: `<dir>/<sessionID>.jsonl`, baris `{ t, system: string[] }`.
// Hanya ditulis bila isinya berubah dari rekaman terakhir sesi itu. Secret disamarkan sebelum menyentuh disk.
import { appendFileSync, existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { redactSecrets } from "./secrets";

/** Batas ukuran berkas per sesi; lewat dari ini rekaman baru dilewati (riwayat lama tetap). */
export const MAX_TRACE_BYTES = 8 * 1024 * 1024;
const SAFE_ID = /^[A-Za-z0-9_-]{1,128}$/;

export type SystemSnapshot = { t: number; system: string[] };

function lastLine(file: string): string | null {
  if (!existsSync(file)) return null;
  const lines = readFileSync(file, "utf8")
    .split("\n")
    .filter((l) => l.trim());
  return lines.at(-1) ?? null;
}

export function createSystemTrace(dir: string, now: () => number = Date.now) {
  /** Isi (JSON array tersamarkan) rekaman terakhir per sesi, agar tidak menulis duplikat. */
  const last = new Map<string, string>();
  return {
    record(sessionID: string | undefined, system: string[]): void {
      if (!sessionID || !SAFE_ID.test(sessionID)) return;
      try {
        const file = join(dir, `${sessionID}.jsonl`);
        const body = JSON.stringify(system.map(redactSecrets));
        if (!last.has(sessionID)) {
          // Proses engine baru: lanjutkan dari rekaman terakhir di disk supaya restart tidak menggandakan isi.
          const prev = lastLine(file);
          if (prev) {
            try {
              last.set(sessionID, JSON.stringify((JSON.parse(prev) as SystemSnapshot).system));
            } catch {
              // baris rusak: anggap belum ada rekaman
            }
          }
        }
        if (last.get(sessionID) === body) return;
        mkdirSync(dir, { recursive: true });
        if (existsSync(file) && statSync(file).size > MAX_TRACE_BYTES) return;
        appendFileSync(file, `{"t":${now()},"system":${body}}\n`);
        last.set(sessionID, body);
      } catch {
        // Rekaman hanya untuk diagnosis: galat disk tidak boleh menggagalkan giliran chat.
      }
    },
  };
}

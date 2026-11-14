// Satu giliran asisten sering terpecah engine menjadi beberapa pesan asisten berurutan (mis. pesan pertama hanya berisi tool
// call, pesan berikutnya berisi jawaban). Untuk tampilan, pesan asisten yang berurutan (tanpa pesan pengguna di antaranya)
// digabung menjadi satu giliran: satu blok proses, satu jawaban, satu baris info/aksi.
import type { ChatMessage, MessageError } from "./types";

export type TurnMessage = ChatMessage & {
  /** Id semua pesan engine yang digabung (urut); tautan/sematan lama ke salah satunya tetap bisa ditemukan. */
  sourceIds?: string[];
};

export function mergeAssistantTurns(messages: ChatMessage[]): TurnMessage[] {
  const out: TurnMessage[] = [];
  for (const m of messages) {
    const prev = out.at(-1);
    if (m.info.role !== "assistant" || !prev || prev.info.role !== "assistant") {
      out.push({ ...m, sourceIds: [m.info.id] });
      continue;
    }
    const first = prev;
    const error: MessageError | undefined = m.info.error ?? prev.info.error;
    const cost = (prev.info.cost ?? 0) + (m.info.cost ?? 0);
    out[out.length - 1] = {
      // identitas, model, dan status selesai mengikuti pesan TERAKHIR; waktu mulai dari pesan pertama
      info: {
        ...m.info,
        time: {
          created: first.info.time.created,
          ...(m.info.time.completed !== undefined ? { completed: m.info.time.completed } : {}),
        },
        ...(error ? { error } : {}),
        ...(m.info.tokens ? { tokens: m.info.tokens } : {}),
        ...(cost > 0 ? { cost } : {}),
      },
      parts: [...prev.parts, ...m.parts],
      sourceIds: [...(prev.sourceIds ?? [prev.info.id]), m.info.id],
    };
  }
  return out;
}

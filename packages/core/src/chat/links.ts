// Tautan ke satu pesan: `zandor://chat/{key}/{sessionId}/{messageId}`. Dibuat oleh aksi "Salin tautan pesan" dan dibuka dengan
// menempelkannya di palet perintah (membuka sesi dan menggulir ke pesan). Bukan tautan sistem operasi: hanya dikenali aplikasi.
const PREFIX = "zandor://chat/";

export type MessageLink = { key: string; sessionId: string; messageId: string };

export function messageLink(l: MessageLink): string {
  return `${PREFIX}${encodeURIComponent(l.key)}/${encodeURIComponent(l.sessionId)}/${encodeURIComponent(l.messageId)}`;
}

/** Urai tautan pesan dari teks (spasi di tepi diabaikan); null bila bukan tautan yang valid. */
export function parseMessageLink(text: string): MessageLink | null {
  const t = text.trim();
  if (!t.startsWith(PREFIX)) return null;
  const parts = t.slice(PREFIX.length).split("/");
  if (parts.length !== 3) return null;
  try {
    const [key, sessionId, messageId] = parts.map((p) => decodeURIComponent(p));
    return key && sessionId && messageId ? { key, sessionId, messageId } : null;
  } catch {
    return null;
  }
}

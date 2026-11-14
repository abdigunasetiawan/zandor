// Meminta aplikasi menggulir ke satu pesan (dari tautan pesan atau daftar pesan disematkan). Permintaan menunggu sampai
// elemen pesan ada (sesi baru dibuka/riwayat dimuat), lalu diambil oleh Transcript.
let pending: string | null = null;
const listeners = new Set<() => void>();

export function requestMessageFocus(messageId: string) {
  pending = messageId;
  for (const l of listeners) l();
}

export const peekMessageFocus = () => pending;
export const clearMessageFocus = () => {
  pending = null;
};
export function onMessageFocus(l: () => void): () => void {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

export const messageDomId = (messageId: string) => `msg-${messageId}`;

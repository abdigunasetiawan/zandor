// Jembatan kecil untuk membuka URL di Browser Panel 4 dari bagian lain aplikasi (mis. "Buka di browser" di pratinjau HTML).
// Bila panel belum terpasang, URL menunggu dan dipakai saat webview dibuat; bila sudah terpasang, pendengar menavigasinya.

export const BROWSER_OPEN_EVENT = "zandor:browser-open";
let pending: string | null = null;

export function requestBrowserOpen(url: string): void {
  pending = url;
  window.dispatchEvent(new CustomEvent(BROWSER_OPEN_EVENT, { detail: url }));
}

/** Ambil (dan hapus) URL yang menunggu. */
export function takePendingBrowserUrl(): string | null {
  const u = pending;
  pending = null;
  return u;
}

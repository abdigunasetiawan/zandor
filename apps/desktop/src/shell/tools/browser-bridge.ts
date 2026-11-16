// Jembatan kecil untuk membuka URL di Browser Panel 4 dari bagian lain aplikasi (mis. "Buka di browser" di pratinjau HTML,
// tautan di respons model). Bila panel belum terpasang, URL menunggu dan dipakai saat webview dibuat; bila sudah terpasang,
// pendengar menavigasinya.
import type { PanelBridge } from "../services";

export const BROWSER_OPEN_EVENT = "zandor:browser-open";
/**
 * Animasi tampil/sembunyi panel (ShellLayout). `detail.active` true saat mulai, false saat selesai.
 * Webview native tidak ikut transisi CSS, jadi BrowserPanel menyinkronkan bounds tiap frame selama aktif.
 */
export const PANEL_ANIM_EVENT = "zandor:panel-anim";
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

/** Tampilkan alat Browser di Panel 4 tanpa menutupnya. `openTool` men-toggle (klik ikon aktif = tutup), jadi bila Browser
 * sudah menjadi alat aktif cukup buka panelnya. */
export function showBrowserTool(panels: PanelBridge): void {
  const actions = panels.actions;
  if (!actions) return;
  if (panels.state().activeTool === "browser") actions.reveal("tools");
  else actions.openTool("browser");
}

/** Buka URL di Browser Panel 4 (tampilkan panelnya lalu navigasikan). */
export function openInZandorBrowser(panels: PanelBridge, url: string): void {
  showBrowserTool(panels);
  requestBrowserOpen(url);
}

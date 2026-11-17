// Build rilis/beta tidak boleh terasa seperti browser: tanpa menu klik kanan bawaan WebView2 (Inspect, Reload, Print…),
// tanpa pintasan browser (F12, Ctrl+Shift+I, F5/Ctrl+R, Ctrl+P, Ctrl+U), dan teks tidak bisa diseleksi sembarangan
// (kelas `z-app-locked` di <html>, lihat styles.css). Menu klik kanan milik Zandor (ContextMenu) tetap berjalan.

const BLOCKED_KEYS = new Set(["F12", "F5", "F7", "BrowserBack", "BrowserForward", "BrowserRefresh"]);

/** true bila kombinasi tombol adalah pintasan bawaan browser yang harus diredam. */
export function isBrowserShortcut(
  e: Pick<KeyboardEvent, "key" | "ctrlKey" | "shiftKey" | "altKey" | "metaKey">,
): boolean {
  if (BLOCKED_KEYS.has(e.key)) return true;
  const mod = e.ctrlKey || e.metaKey;
  const k = e.key.toLowerCase();
  if (mod && e.shiftKey && (k === "i" || k === "j" || k === "c")) return true; // DevTools / console / inspect
  if (mod && !e.shiftKey && (k === "r" || k === "p" || k === "u" || k === "g" || k === "j" || k === "h")) return true;
  if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) return true; // navigasi mundur/maju
  return false;
}

/** Elemen yang boleh memakai menu klik kanan bawaan (salin/tempel di kolom isian). */
const editable = (t: EventTarget | null): boolean =>
  t instanceof HTMLElement && (t.isContentEditable || t.closest("input, textarea, [contenteditable='true']") !== null);

export function installBrowserGuard(doc: Document = document): void {
  doc.documentElement.classList.add("z-app-locked");
  doc.addEventListener("contextmenu", (e) => {
    // Menu kustom (Base UI) sudah memanggil preventDefault; sisanya = menu bawaan WebView2 → redam.
    if (!editable(e.target)) e.preventDefault();
  });
  doc.addEventListener(
    "keydown",
    (e) => {
      if (isBrowserShortcut(e)) e.preventDefault();
    },
    { capture: true },
  );
  // Seret tautan/gambar keluar jendela = perilaku browser.
  doc.addEventListener("dragstart", (e) => {
    const t = e.target;
    if (t instanceof HTMLAnchorElement || t instanceof HTMLImageElement) e.preventDefault();
  });
}

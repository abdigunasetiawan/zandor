// Adapter jendela Tauri. Di luar Tauri (browser/e2e) mengembalikan null: kontrol jendela tidak ditampilkan.
// Izin yang dibutuhkan ada di src-tauri/capabilities/default.json (hanya webview UI lokal).

export type WindowApi = {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  /** Dipanggil saat ukuran/state jendela berubah. Mengembalikan fungsi berhenti. */
  onResized(cb: () => void): Promise<() => void>;
};

export async function loadWindowApi(): Promise<WindowApi | null> {
  const { isTauri } = await import("@tauri-apps/api/core");
  if (!isTauri()) return null;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  return {
    minimize: () => w.minimize(),
    toggleMaximize: () => w.toggleMaximize(),
    close: () => w.close(),
    isMaximized: () => w.isMaximized(),
    onResized: (cb) => w.onResized(() => cb()),
  };
}

// Notifikasi sistem (Fase 11). Di Tauri: plugin notifikasi (toast Windows). Di luar Tauri: Web Notification bila pengguna
// mengizinkan. Izin diminta hanya lewat tindakan pengguna (mis. saat membuat tugas terjadwal), tidak pernah diam-diam.
export type NotifyHost = {
  /** Apakah notifikasi sistem didukung di lingkungan ini. */
  available: boolean;
  /** Status izin sekarang tanpa meminta. */
  granted(): Promise<boolean>;
  /** Minta izin (panggil dari tindakan pengguna). Mengembalikan apakah diberikan. */
  request(): Promise<boolean>;
  /** Kirim notifikasi; diam bila izin belum diberikan. Mengembalikan apakah terkirim. */
  notify(title: string, body: string): Promise<boolean>;
};

export function createBrowserNotifyHost(): NotifyHost {
  const supported = typeof Notification !== "undefined";
  return {
    available: supported,
    async granted() {
      return supported && Notification.permission === "granted";
    },
    async request() {
      if (!supported) return false;
      return (await Notification.requestPermission()) === "granted";
    },
    async notify(title, body) {
      if (!supported || Notification.permission !== "granted") return false;
      new Notification(title, { body });
      return true;
    },
  };
}

export async function loadNotifyHost(): Promise<NotifyHost> {
  const { isTauri } = await import("@tauri-apps/api/core");
  if (!isTauri()) return createBrowserNotifyHost();
  const n = await import("@tauri-apps/plugin-notification");
  return {
    available: true,
    granted: () => n.isPermissionGranted(),
    async request() {
      return (await n.requestPermission()) === "granted";
    },
    async notify(title, body) {
      if (!(await n.isPermissionGranted())) return false;
      n.sendNotification({ title, body });
      return true;
    },
  };
}

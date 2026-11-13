// Adapter OpenVsxHost. Di Tauri: perintah Rust (hanya https://open-vsx.org, ukuran dibatasi). Di luar Tauri tidak ada
// akses jaringan terkendali → `available=false` dan UI menjelaskannya (impor berkas tema lokal tetap bisa).
import type { OpenVsxHost, OpenVsxItem } from "@zandor/core";

export function createUnavailableOpenVsxHost(): OpenVsxHost {
  const no = (): never => {
    throw new Error("Open VSX hanya tersedia di aplikasi desktop");
  };
  return { available: false, search: async () => no(), download: async () => no() };
}

export async function loadOpenVsxHost(): Promise<OpenVsxHost> {
  const { isTauri, invoke } = await import("@tauri-apps/api/core");
  if (!isTauri()) return createUnavailableOpenVsxHost();
  return {
    available: true,
    search: (query) => invoke<OpenVsxItem[]>("openvsx_search", { query }),
    async download(url) {
      const data = await invoke<ArrayBuffer>("openvsx_download", { url });
      return new Uint8Array(data);
    },
  };
}

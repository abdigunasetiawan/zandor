// Antarmuka klien Open VSX (tema editor). Implementasi Tauri memanggil Rust (jaringan dan batas ukuran ada di sana).
import { type EditorTheme, themesFromVsix } from "./vscode-theme";

export type OpenVsxItem = {
  namespace: string;
  name: string;
  display_name: string;
  version: string;
  description: string;
  download: string;
  downloads: number;
};

export interface OpenVsxHost {
  /** false di luar aplikasi desktop (tidak ada akses jaringan terkendali). */
  available: boolean;
  search(query: string): Promise<OpenVsxItem[]>;
  download(url: string): Promise<Uint8Array>;
}

export const OPENVSX_SOURCE = (i: Pick<OpenVsxItem, "namespace" | "name" | "version">) =>
  `openvsx:${i.namespace}.${i.name}@${i.version}`;

/** Unduh paket lalu ambil temanya. Tidak ada kode ekstensi yang dijalankan. */
export async function fetchOpenVsxThemes(
  host: OpenVsxHost,
  item: OpenVsxItem,
): Promise<{ themes: EditorTheme[]; skipped: string[]; errors: string[] }> {
  const bytes = await host.download(item.download);
  return themesFromVsix(bytes, OPENVSX_SOURCE(item));
}

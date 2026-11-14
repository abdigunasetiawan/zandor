// Adapter berkas untuk Panel 3. Di Tauri: byte persis lewat perintah Rust `fs_read` (hanya di dalam project terdaftar),
// simpan-salinan lewat dialog sistem, dan buka di aplikasi lain. Di luar Tauri (dev/e2e): membaca lewat API engine dan
// mengunduh lewat Blob; "buka di aplikasi lain" tidak tersedia.
import type { EngineApi, FileSource } from "@zandor/core";

/** Aplikasi terdaftar untuk satu ekstensi (menu "Buka dengan"); `icon` = PNG base64 atau kosong. */
export type OpenApp = { name: string; path: string; icon: string; isDefault: boolean };

export type DocHost = {
  native: boolean;
  source: FileSource;
  /** Simpan salinan berkas. Mengembalikan false bila dibatalkan. `bytes` dipakai di luar Tauri. */
  saveCopy(root: string, path: string, name: string, bytes: Uint8Array | null): Promise<boolean>;
  openExternal(root: string, path: string): Promise<void>;
  /** Buat berkas kosong/folder baru di `parent` (relatif ke `root`). Hanya di aplikasi desktop; tidak pernah menimpa. */
  createEntry?(root: string, parent: string, name: string, dir: boolean): Promise<void>;
  /** Aplikasi terdaftar untuk ekstensi berkas (hanya desktop). */
  openWithList?(root: string, path: string, textLike: boolean, refresh: boolean): Promise<OpenApp[]>;
  /** Buka berkas dengan aplikasi dari daftar di atas. */
  openWith?(root: string, path: string, appPath: string, textLike: boolean): Promise<void>;
  /** Tampilkan berkas di File Explorer. */
  reveal?(root: string, path: string): Promise<void>;
  /** URL pratinjau HTML yang boleh memuat skrip dan sumber luar (hanya desktop; skema `zpreview`). */
  previewUrl?(html: string): Promise<string>;
  /** Simpan byte hasil ekspor (teks/PNG/SVG) lewat dialog Simpan sebagai (Tauri) atau unduhan (web). false = dibatalkan. */
  saveBytes(name: string, bytes: Uint8Array): Promise<boolean>;
};

function downloadBlob(name: string, bytes: Uint8Array) {
  const url = URL.createObjectURL(new Blob([bytes as BlobPart]));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export function createBrowserDocHost(getApi: () => EngineApi | null): DocHost {
  return {
    native: false,
    source: {
      async read(root, path) {
        const api = getApi();
        if (!api) throw new Error("engine belum siap");
        return api.readFile(root, path);
      },
    },
    async saveCopy(_root, _path, name, bytes) {
      if (!bytes) throw new Error("isi berkas belum dimuat");
      downloadBlob(name, bytes);
      return true;
    },
    async openExternal() {
      throw new Error("hanya tersedia di aplikasi desktop");
    },
    async saveBytes(name, bytes) {
      downloadBlob(name, bytes);
      return true;
    },
  };
}

export async function loadDocHost(getApi: () => EngineApi | null): Promise<DocHost> {
  const { isTauri, invoke } = await import("@tauri-apps/api/core");
  if (!isTauri()) return createBrowserDocHost(getApi);
  return {
    native: true,
    source: {
      async read(root, path) {
        const buf = await invoke<ArrayBuffer>("fs_read", { root, path });
        return new Uint8Array(buf);
      },
      stat: (root, path) => invoke<{ size: number; modified: number }>("fs_stat", { root, path }),
      async write(root, path, text, expectedModified) {
        try {
          return await invoke<number>("fs_write", { root, path, text, expectedModified });
        } catch (e) {
          throw new Error(String(e)); // invoke menolak dengan string; jadikan Error agar awalan KONFLIK terbaca
        }
      },
    },
    openWithList: (root, path, textLike, refresh) =>
      invoke<OpenApp[]>("fs_open_with_list", { root, path, textLike, refresh }),
    async openWith(root, path, appPath, textLike) {
      try {
        await invoke("fs_open_with", { root, path, appPath, textLike });
      } catch (e) {
        throw new Error(String(e));
      }
    },
    async reveal(root, path) {
      try {
        await invoke("fs_reveal", { root, path });
      } catch (e) {
        throw new Error(String(e));
      }
    },
    async previewUrl(html) {
      const { convertFileSrc } = await import("@tauri-apps/api/core");
      const token = await invoke<string>("doc_preview_register", { html });
      return convertFileSrc(token, "zpreview");
    },
    async createEntry(root, parent, name, dir) {
      try {
        await invoke<string>("fs_create", { root, parent, name, dir });
      } catch (e) {
        throw new Error(String(e));
      }
    },
    async saveCopy(root, path, name) {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const dest = await save({ defaultPath: name });
      if (!dest) return false;
      await invoke("fs_copy_to", { root, path, dest });
      return true;
    },
    async openExternal(root, path) {
      await invoke("fs_open_external", { root, path });
    },
    async saveBytes(name, bytes) {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const dest = await save({ defaultPath: name });
      if (!dest) return false;
      await invoke("fs_save_as", { dest, data: Array.from(bytes) });
      return true;
    },
  };
}

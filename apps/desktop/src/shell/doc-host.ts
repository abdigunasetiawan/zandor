// Adapter berkas untuk Panel 3. Di Tauri: byte persis lewat perintah Rust `fs_read` (hanya di dalam project terdaftar),
// simpan-salinan lewat dialog sistem, dan buka di aplikasi lain. Di luar Tauri (dev/e2e): membaca lewat API engine dan
// mengunduh lewat Blob; "buka di aplikasi lain" tidak tersedia.
import type { EngineApi, FileSource } from "@zandor/core";

export type DocHost = {
  native: boolean;
  source: FileSource;
  /** Simpan salinan berkas. Mengembalikan false bila dibatalkan. `bytes` dipakai di luar Tauri. */
  saveCopy(root: string, path: string, name: string, bytes: Uint8Array | null): Promise<boolean>;
  openExternal(root: string, path: string): Promise<void>;
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
  };
}

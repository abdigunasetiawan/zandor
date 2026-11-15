// Pembeda salinan dev: build dev memakai productName "Zandor Dev" (src-tauri/tauri.dev.conf.json, ADR-061), sehingga
// label "Dev" tampil di samping nama aplikasi dan tidak tertukar dengan salinan produksi.
import { useEffect, useState } from "react";

/** Nama produk berakhiran "Dev" (mis. "Zandor Dev") = salinan pengembangan. */
export const isDevProductName = (name: string): boolean => /\sdev$/i.test(name.trim());

/** true hanya di aplikasi desktop dengan produk "… Dev". Di browser (tanpa Tauri) selalu false. */
export function useIsDevBuild(): boolean {
  const [dev, setDev] = useState(false);
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const core = await import("@tauri-apps/api/core");
        if (!core.isTauri()) return;
        const { getName } = await import("@tauri-apps/api/app");
        const name = await getName();
        if (live) setDev(isDevProductName(name));
      } catch {
        // tanpa label bila nama produk tak terbaca
      }
    })();
    return () => {
      live = false;
    };
  }, []);
  return dev;
}

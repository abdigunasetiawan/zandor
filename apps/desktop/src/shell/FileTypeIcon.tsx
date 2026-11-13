// Ikon tipe file/folder: Material Icon Theme (MIT, ADR-031). Manifest (450 KB) dan SVG dimuat malas sehingga tidak
// membebani startup; sebelum siap (atau bila gagal) dipakai ikon Lucide generik agar baris tidak melompat.
import { type IconManifest, iconFileName, resolveIconName } from "@zandor/core";
import { FileIcon, FolderIcon, FolderOpenIcon } from "@zandor/ui";
import { useEffect, useState } from "react";

const svgLoaders = import.meta.glob<string>("../../node_modules/material-icon-theme/icons/*.svg", {
  query: "?url",
  import: "default",
});

let manifestPromise: Promise<IconManifest | null> | null = null;
const loadManifest = () => {
  manifestPromise ??= import("material-icon-theme/dist/material-icons.json").then(
    (m) => (m.default ?? m) as unknown as IconManifest,
    () => null,
  );
  return manifestPromise;
};

const urlCache = new Map<string, Promise<string | null>>();
const loadUrl = (manifest: IconManifest, iconName: string): Promise<string | null> => {
  const file = iconFileName(manifest, iconName);
  const loader = file ? svgLoaders[`../../node_modules/material-icon-theme/icons/${file}`] : undefined;
  if (!loader) return Promise.resolve(null);
  let p = urlCache.get(iconName);
  if (!p) {
    p = loader().then(
      (u) => u,
      () => null,
    );
    urlCache.set(iconName, p);
  }
  return p;
};

export function FileTypeIcon({
  name,
  type,
  expanded = false,
}: {
  name: string;
  type: "file" | "directory";
  expanded?: boolean;
}) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    setUrl(null);
    void loadManifest().then(async (manifest) => {
      if (!manifest) return;
      const u = await loadUrl(manifest, resolveIconName(manifest, { name, type }, expanded));
      if (alive) setUrl(u);
    });
    return () => {
      alive = false;
    };
  }, [name, type, expanded]);

  if (url) return <img src={url} alt="" aria-hidden="true" className="z-icon z-icon--sm file-icon" draggable={false} />;
  if (type === "file") return <FileIcon size="sm" />;
  return expanded ? <FolderOpenIcon size="sm" /> : <FolderIcon size="sm" />;
}

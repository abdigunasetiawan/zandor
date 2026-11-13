// Pemilihan nama ikon tipe file dari manifest Material Icon Theme (MIT, ADR-031). Murni: manifest diberikan pemanggil,
// jadi bisa diuji tanpa memuat 450 KB JSON. Urutan mengikuti perilaku tema: nama persis → ekstensi (terpanjang dulu)
// → bawaan; folder: nama folder → bawaan.

export type IconManifest = {
  file: string;
  folder: string;
  folderExpanded: string;
  fileNames: Record<string, string>;
  fileExtensions: Record<string, string>;
  folderNames: Record<string, string>;
  folderNamesExpanded: Record<string, string>;
  iconDefinitions: Record<string, { iconPath: string }>;
};

export type IconNode = { name: string; type: "file" | "directory" };

const own = (map: Record<string, string>, key: string): string | undefined =>
  Object.hasOwn(map, key) ? map[key] : undefined;

/** Nama ikon (kunci `iconDefinitions`) untuk sebuah berkas/folder. */
export function resolveIconName(manifest: IconManifest, node: IconNode, expanded = false): string {
  const name = node.name.toLowerCase();
  let found: string | undefined;
  if (node.type === "directory") {
    found = own(expanded ? manifest.folderNamesExpanded : manifest.folderNames, name);
    found ??= expanded ? manifest.folderExpanded : manifest.folder;
  } else {
    found = own(manifest.fileNames, name);
    // "a.test.ts" → coba "test.ts" lalu "ts" (ekstensi majemuk dahulu).
    for (let dot = name.indexOf("."); !found && dot !== -1; dot = name.indexOf(".", dot + 1)) {
      found = own(manifest.fileExtensions, name.slice(dot + 1));
    }
    found ??= manifest.file;
  }
  return Object.hasOwn(manifest.iconDefinitions, found)
    ? found
    : node.type === "directory"
      ? manifest.folder
      : manifest.file;
}

/** Nama berkas SVG (tanpa folder) untuk sebuah ikon, mis. "typescript" → "typescript.svg". */
export function iconFileName(manifest: IconManifest, iconName: string): string | null {
  const def = manifest.iconDefinitions[iconName];
  return def ? (def.iconPath.split("/").pop() ?? null) : null;
}

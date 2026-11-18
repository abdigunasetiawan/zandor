// Ekspor ikon tipe berkas Material Icon Theme (MIT, ADR-031) untuk apps/desktop-gpui: manifest (pemetaan nama/ekstensi)
// + isi SVG per ikon yang dirujuk, dalam satu JSON yang di-embed ke binary.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
const pkg = "apps/desktop/node_modules/material-icon-theme";
const m = JSON.parse(readFileSync(join(pkg, "dist/material-icons.json"), "utf8"));
const files = new Set(readdirSync(join(pkg, "icons")));
const svgs: Record<string, string> = {};
const defs: Record<string, string> = {};
for (const [name, def] of Object.entries(m.iconDefinitions as Record<string, { iconPath: string }>)) {
  const file = def.iconPath.split("/").pop() ?? "";
  if (!files.has(file)) continue;
  defs[name] = file;
  svgs[file] ??= readFileSync(join(pkg, "icons", file), "utf8");
}
const out = {
  file: m.file,
  folder: m.folder,
  folderExpanded: m.folderExpanded,
  fileNames: m.fileNames,
  fileExtensions: m.fileExtensions,
  folderNames: m.folderNames,
  folderNamesExpanded: m.folderNamesExpanded,
  defs,
  svgs,
};
await Bun.write("apps/desktop-gpui/assets/file-icons.json", JSON.stringify(out));

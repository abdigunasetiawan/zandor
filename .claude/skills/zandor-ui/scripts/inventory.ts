// Inventaris desain sistem Zandor: cari apa yang SUDAH ada sebelum membuat UI baru.
//
// Pemakaian (dari akar repo):
//   bun .claude/skills/zandor-ui/scripts/inventory.ts            → ringkasan semua
//   bun .claude/skills/zandor-ui/scripts/inventory.ts tabel chip → hanya yang cocok dengan kata kunci (huruf kecil/besar bebas)
//
// Sumber: tokens.css (token), catalog.ts (komponen), index.ts (ekspor @zandor/ui), icons.tsx (ikon), styles.css +
// packages/ui/src/styles/*.css (kelas CSS), apps/desktop/src/shell (komponen aplikasi), locales (kunci i18n).
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..", "..", "..", "..");
const terms = process.argv.slice(2).map((t) => t.toLowerCase());
const match = (s: string) => terms.length === 0 || terms.some((t) => s.toLowerCase().includes(t));
const read = (p: string) => (existsSync(join(root, p)) ? readFileSync(join(root, p), "utf8") : "");

const walk = (dir: string, ext: RegExp): string[] =>
  existsSync(dir)
    ? readdirSync(dir).flatMap((n) => {
        const p = join(dir, n);
        return statSync(p).isDirectory() ? walk(p, ext) : ext.test(n) ? [p] : [];
      })
    : [];

function section(title: string, items: string[], hint = "") {
  const hits = items.filter(match);
  console.log(
    `\n## ${title} (${hits.length}${terms.length ? ` cocok dari ${items.length}` : ""})${hint ? ` — ${hint}` : ""}`,
  );
  if (hits.length === 0) console.log("  (tidak ada)");
  else console.log(`  ${hits.join("\n  ")}`);
}

// token: nama + nilai bawaan (blok :root pertama saja agar ringkas)
const tokensCss = read("packages/ui/src/styles/tokens.css");
const rootBlock = tokensCss.split(":root[data-theme")[0] ?? tokensCss;
const tokens = [...rootBlock.matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((m) => `${m[1]}: ${m[2]?.trim()}`);
section("Token desain (--z-*, --ui-scale)", tokens, "HANYA ini yang boleh dipakai untuk warna/ukuran");

// katalog komponen
const catalog = read("packages/ui/src/catalog.ts");
const list = (name: string) => {
  const m = new RegExp(`export const ${name} = \\[([\\s\\S]*?)\\] as const`).exec(catalog);
  return m ? [...(m[1] ?? "").matchAll(/"([\w]+)"/g)].map((x) => x[1] as string) : [];
};
section("Atoms", list("ATOMS"));
section("Molecules", list("MOLECULES"));
section("Infrastruktur (provider/pembungkus)", list("INFRASTRUCTURE"));

// ekspor @zandor/ui (hook, util, tipe)
const indexTs = read("packages/ui/src/index.ts");
const exported = [...indexTs.matchAll(/export \{([^}]+)\}/g)].flatMap((m) =>
  (m[1] ?? "")
    .split(",")
    .map(
      (x) =>
        x
          .trim()
          .split(/\s+as\s+/)
          .pop() as string,
    )
    .filter((x) => x && !x.startsWith("type ")),
);
section("Ekspor lain @zandor/ui (hook/util)", [...new Set(exported)].sort(), 'impor dari "@zandor/ui"');

// ikon
const icons = [...read("packages/ui/src/icons.tsx").matchAll(/export const (\w+Icon) = icon\((\w+),/g)].map(
  (m) => `${m[1]} (lucide ${m[2]})`,
);
section(
  "Ikon UI (lucide, packages/ui/src/icons.tsx)",
  icons,
  "tambah ikon baru DI SANA, jangan impor lucide langsung di komponen",
);

// kelas CSS
const cssFiles = [...walk(join(root, "packages/ui/src/styles"), /\.css$/), join(root, "apps/desktop/src/styles.css")];
const classes = new Set<string>();
for (const f of cssFiles)
  for (const m of readFileSync(f, "utf8").matchAll(/\.([a-zA-Z][\w-]*)/g)) classes.add(m[1] as string);
section("Kelas CSS yang sudah ada", [...classes].sort(), "cek tabrakan nama sebelum membuat kelas baru");

// komponen aplikasi
const shell = walk(join(root, "apps/desktop/src/shell"), /\.tsx$/)
  .filter((f) => !/\.test\./.test(f))
  .flatMap((f) =>
    [...readFileSync(f, "utf8").matchAll(/export (?:default )?function (\w+)/g)].map(
      (m) => `${m[1]}  (${f.slice(root.length + 1).replace(/\\/g, "/")})`,
    ),
  );
section("Komponen aplikasi (apps/desktop/src/shell)", shell, "pakai ulang sebelum menulis yang mirip");

// kunci i18n (hanya saat ada kata kunci agar keluaran tidak membludak)
if (terms.length) {
  const keys = [...read("packages/i18n/src/locales/id.ts").matchAll(/^\s+"([\w.-]+)":\s*"([^"\n]*)"/gm)].map(
    (m) => `${m[1]} = ${m[2]}`,
  );
  section("Kunci i18n (id)", keys, "string UI baru wajib ada di id.ts DAN en.ts");
}

console.log("\nCek otomatis setelah mengubah UI: bun run design-tokens && bun run check");

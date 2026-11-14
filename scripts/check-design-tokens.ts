// CLI: pindai kode UI dan gagal bila ada warna/ukuran hard-coded atau dialog bawaan OS.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { findViolations, type Violation } from "./design-token-rules";

const root = join(import.meta.dir, "..");
const ROOTS = ["packages/ui/src", "apps/desktop/src"];
const EXT = /\.(css|tsx?|html)$/;
// model-logos.generated.ts: aset logo merek pihak ketiga (warna merek), bukan gaya UI kita.
const SKIP = /(\.test\.tsx?$|[\\/]test-utils\.tsx$|\.d\.ts$|model-logos\.generated\.ts$|demo-assets\.ts$)/;

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? walk(p) : EXT.test(name) && !SKIP.test(p) ? [p] : [];
  });
}

const files = ROOTS.flatMap((r) => walk(join(root, r)));
const violations: Violation[] = files.flatMap((f) => findViolations(relative(root, f), readFileSync(f, "utf8")));

console.log(`Dipindai ${files.length} berkas.`);
if (violations.length) {
  console.error(`\n${violations.length} pelanggaran:`);
  for (const v of violations) console.error(`  ${v.file}:${v.line}  ${v.rule}\n      ${v.text}`);
  process.exit(1);
}
console.log("OK: tidak ada warna/ukuran hard-coded dan tidak ada dialog bawaan OS.");

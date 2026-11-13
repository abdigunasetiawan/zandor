// Audit lisensi dependensi produksi JS (PRD §4.8, ADR-009, ADR-026).
// Menelusuri `dependencies` (bukan devDependencies) setiap paket workspace secara rekursif dan gagal
// bila ada lisensi yang tidak permisif atau tidak jelas, kecuali tercantum di `licenses-allow.json`
// dengan alasan tertulis.
import { existsSync, readdirSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join } from "node:path";

const root = join(import.meta.dir, "..");
const PERMISSIVE =
  /^(MIT|MIT-0|ISC|BSD-2-Clause|BSD-3-Clause|Apache-2\.0|0BSD|Unlicense|CC0-1\.0|BlueOak-1\.0\.0|Zlib|Python-2\.0|OFL-1\.1)$/;
const BANNED_NAMES = [/openbot/i]; // ADR-009

type Allow = { package: string; license: string; reason: string };
const allowFile = join(root, "licenses-allow.json");
const allow: Allow[] = existsSync(allowFile) ? JSON.parse(readFileSync(allowFile, "utf8")) : [];

function permissive(lic: string): boolean {
  const s = lic.replace(/[()]/g, "").trim();
  if (/\sAND\s/i.test(s)) return s.split(/\s+AND\s+/i).every((x) => PERMISSIVE.test(x.trim()));
  return s.split(/\s+OR\s+/i).some((x) => PERMISSIVE.test(x.trim()));
}

function license(pj: Record<string, unknown>): string {
  const l = pj.license ?? pj.licenses;
  if (!l) return "TIDAK ADA";
  if (typeof l === "string") return l;
  if (Array.isArray(l)) return l.map((x) => (typeof x === "string" ? x : (x as { type: string }).type)).join(" OR ");
  return (l as { type?: string }).type ?? JSON.stringify(l);
}

function resolve(name: string, from: string): string | null {
  let dir = from;
  for (;;) {
    const p = join(dir, "node_modules", name, "package.json");
    if (existsSync(p)) return realpathSync(p);
    const up = dirname(dir);
    if (up === dir) return null;
    dir = up;
  }
}

const workspaces = ["apps", "packages"].flatMap((d) =>
  readdirSync(join(root, d), { withFileTypes: true })
    .filter((e) => e.isDirectory() && existsSync(join(root, d, e.name, "package.json")))
    .map((e) => join(root, d, e.name)),
);

const seen = new Map<string, { license: string; via: string }>();
const missing: string[] = [];
for (const ws of workspaces) {
  const pj = JSON.parse(readFileSync(join(ws, "package.json"), "utf8"));
  const stack: [string, string, string][] = Object.keys(pj.dependencies ?? {}).map((d) => [d, ws, pj.name]);
  while (stack.length) {
    const [name, from, via] = stack.pop() as [string, string, string];
    const p = resolve(name, from);
    if (!p) {
      missing.push(`${name} (dari ${via})`);
      continue;
    }
    const dep = JSON.parse(readFileSync(p, "utf8"));
    if (dep.name?.startsWith("@zandor/")) {
      for (const d of Object.keys(dep.dependencies ?? {})) stack.push([d, dirname(p), dep.name]);
      continue;
    }
    const key = `${dep.name}@${dep.version}`;
    if (seen.has(key)) continue;
    seen.set(key, { license: license(dep), via });
    for (const d of Object.keys(dep.dependencies ?? {})) stack.push([d, dirname(p), dep.name]);
  }
}

const problems: string[] = [];
for (const [key, { license: lic, via }] of seen) {
  const name = key.slice(0, key.lastIndexOf("@"));
  if (BANNED_NAMES.some((r) => r.test(name))) problems.push(`${key}: paket terlarang (ADR-009)`);
  if (permissive(lic)) continue;
  const ok = allow.find((a) => a.package === name && a.license === lic);
  if (!ok) problems.push(`${key}: lisensi "${lic}" (via ${via})`);
}

const counts: Record<string, number> = {};
for (const { license: l } of seen.values()) counts[l] = (counts[l] ?? 0) + 1;
console.log(`Paket produksi diaudit: ${seen.size}`);
console.log(
  Object.entries(counts)
    .sort((a, b) => b[1] - a[1])
    .map(([l, n]) => `  ${l}: ${n}`)
    .join("\n"),
);
if (missing.length) console.warn(`Tidak ter-resolve (opsional/platform lain?): ${missing.join(", ")}`);
if (problems.length) {
  console.error(`\nLisensi bermasalah (${problems.length}):\n  ${problems.join("\n  ")}`);
  process.exit(1);
}
console.log("OK: semua lisensi permisif atau punya pengecualian tertulis.");

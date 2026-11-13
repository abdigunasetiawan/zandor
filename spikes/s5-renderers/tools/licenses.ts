// Spike S5 — audit lisensi seluruh pohon dependensi produksi per kandidat renderer.
// Menandai lisensi yang tidak permisif atau tidak jelas (ADR-009 / PRD §4.8).
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";

const root = join(import.meta.dir, "..");
const PERMISSIVE = /^(MIT|ISC|BSD-2-Clause|BSD-3-Clause|Apache-2\.0|0BSD|Unlicense|CC0-1\.0|BlueOak-1\.0\.0|Python-2\.0|Zlib|WTFPL)$/;
const top = Object.keys(JSON.parse(readFileSync(join(root, "package.json"), "utf8")).dependencies);

function resolvePkg(name: string, from: string): string | null {
  let dir = from;
  while (true) {
    const p = join(dir, "node_modules", name, "package.json");
    if (existsSync(p)) return p;
    const up = dirname(dir);
    if (up === dir) return null;
    dir = up;
  }
}

function normLicense(pj: any): string {
  const l = pj.license ?? pj.licenses;
  if (!l) return "TIDAK ADA";
  if (typeof l === "string") return l;
  if (Array.isArray(l)) return l.map((x: any) => x.type ?? x).join(" OR ");
  return l.type ?? JSON.stringify(l);
}

/** Lisensi SPDX "A OR B" lolos bila salah satu opsi permisif. */
function permissive(lic: string) {
  const opts = lic.replace(/[()]/g, "").split(/\s+OR\s+/i).map((s) => s.trim());
  if (/\sAND\s/i.test(lic)) return lic.replace(/[()]/g, "").split(/\s+AND\s+/i).every((s) => PERMISSIVE.test(s.trim()));
  return opts.some((o) => PERMISSIVE.test(o));
}

const report: Record<string, any> = {};
for (const name of top) {
  const seen = new Map<string, { version: string; license: string; hasLicenseFile: boolean; repo: string }>();
  const stack: [string, string][] = [[name, root]];
  while (stack.length) {
    const [n, from] = stack.pop()!;
    const p = resolvePkg(n, from);
    if (!p) continue;
    const pj = JSON.parse(readFileSync(p, "utf8"));
    const key = `${n}@${pj.version}`;
    if (seen.has(key)) continue;
    const dir = dirname(p);
    seen.set(key, {
      version: pj.version,
      license: normLicense(pj),
      hasLicenseFile: readdirSync(dir).some((f) => /^(licen[sc]e|copying)/i.test(f)),
      repo: typeof pj.repository === "string" ? pj.repository : pj.repository?.url ?? "",
    });
    for (const d of Object.keys(pj.dependencies ?? {})) stack.push([d, dir]);
  }
  const flagged = [...seen].filter(([, v]) => !permissive(v.license)).map(([k, v]) => `${k} (${v.license})`);
  const dual = [...seen].filter(([, v]) => /\sOR\s/i.test(v.license)).map(([k, v]) => `${k} (${v.license})`);
  const noFile = [...seen].filter(([, v]) => !v.hasLicenseFile).map(([k]) => k);
  const self = seen.get(`${name}@${[...seen.keys()].find((k) => k.startsWith(name + "@"))!.slice(name.length + 1)}`)!;
  report[name] = {
    license: self.license,
    repo: self.repo || "(tidak ada)",
    deps: seen.size - 1,
    licenses: [...new Set([...seen.values()].map((v) => v.license))].sort(),
    flagged,
    dualLicensePickPermissive: dual,
    packagesWithoutLicenseFile: noFile,
  };
}
console.log(JSON.stringify(report, null, 2));

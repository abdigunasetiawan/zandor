// Siapkan artefak engine untuk build/dev: (1) unduh sidecar `opencode` yang di-pin (engine.json) dan verifikasi
// sha256; (2) bundel plugin Zandor (packages/plugin) menjadi satu berkas resource Tauri.
// Pemakaian: bun scripts/prepare-engine.ts [--fetch] [--plugin]   (tanpa flag = keduanya)
import { createHash } from "node:crypto";
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const tauri = join(root, "apps", "desktop", "src-tauri");
const args = new Set(process.argv.slice(2));
const both = !args.has("--fetch") && !args.has("--plugin");

const sha256 = (file: string) => createHash("sha256").update(readFileSync(file)).digest("hex");

async function hostTriple(): Promise<string> {
  const out = await Bun.$`rustc -vV`.text();
  const m = out.match(/^host: (.+)$/m);
  if (!m?.[1]) throw new Error("tidak bisa menentukan target rustc");
  return m[1].trim();
}

async function fetchEngine() {
  const pin = JSON.parse(readFileSync(join(root, "engine.json"), "utf8")) as {
    version: string;
    targets: Record<string, { package: string; binary: string; sha256: string }>;
  };
  const triple = await hostTriple();
  const target = pin.targets[triple];
  if (!target) throw new Error(`engine.json belum mendefinisikan target ${triple} (ADR-027: Windows dulu)`);
  const exe = triple.includes("windows") ? ".exe" : "";
  const dest = join(tauri, "binaries", `opencode-${triple}${exe}`);

  if (existsSync(dest) && sha256(dest) === target.sha256) {
    console.log(`engine ${pin.version}: sudah ada dan hash cocok (${dest})`);
    return;
  }
  const work = mkdtempSync(join(tmpdir(), "zandor-engine-"));
  try {
    console.log(`mengunduh ${target.package}@${pin.version} ...`);
    await Bun.$`npm pack ${`${target.package}@${pin.version}`} --silent`.cwd(work).quiet();
    const tgz = (await Bun.$`ls`.cwd(work).text()).split(/\r?\n/).find((f) => f.endsWith(".tgz"));
    if (!tgz) throw new Error("npm pack tidak menghasilkan .tgz");
    await Bun.$`tar -xzf ${tgz}`.cwd(work).quiet();
    const src = join(work, "package", target.binary);
    const got = sha256(src);
    if (got !== target.sha256) {
      throw new Error(
        `sha256 binary engine TIDAK cocok.\n  diharapkan ${target.sha256}\n  didapat    ${got}\nJangan lanjut; periksa engine.json.`,
      );
    }
    mkdirSync(join(tauri, "binaries"), { recursive: true });
    copyFileSync(src, dest);
    if (!exe) chmodSync(dest, 0o755);
    const ver = (await Bun.$`${dest} --version`.text()).trim();
    if (ver !== pin.version) throw new Error(`versi binary ${ver} ≠ pin ${pin.version}`);
    console.log(`engine ${ver} siap: ${dest} (${(statSync(dest).size / 1048576).toFixed(0)} MB, sha256 terverifikasi)`);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

async function buildPlugin() {
  const out = join(tauri, "resources");
  mkdirSync(out, { recursive: true });
  const r = await Bun.build({
    entrypoints: [join(root, "packages", "plugin", "src", "index.ts")],
    outdir: out,
    naming: "zandor-plugin.js",
    target: "bun",
    minify: true,
  });
  if (!r.success) throw new Error(`build plugin gagal:\n${r.logs.join("\n")}`);
  console.log(
    `plugin Zandor dibundel: ${join(out, "zandor-plugin.js")} (${(statSync(join(out, "zandor-plugin.js")).size / 1024).toFixed(0)} KB)`,
  );
}

if (both || args.has("--fetch")) await fetchEngine();
if (both || args.has("--plugin")) await buildPlugin();

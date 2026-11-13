#!/usr/bin/env node
// Spike S1 — unduh binary `opencode` versi yang di-pin dan letakkan sebagai sidecar Tauri.
//
// Pemakaian:  node spikes/tools/fetch-opencode.mjs [--target <rust-target-triple>]
// Default target = host. Hasil: spikes/s1-sidecar/src-tauri/binaries/opencode-<triple>[.exe]
//
// Sumber: paket npm resmi `opencode-<os>-<arch>` (lisensi MIT). `npm pack` memverifikasi
// integritas tarball terhadap hash `integrity` di registry; sha256 binary ditulis ke
// berkas .sha256 di sebelahnya agar bisa dibandingkan antar-mesin/CI.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync, chmodSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const spikes = resolve(here, "..");
const { version } = JSON.parse(readFileSync(join(here, "opencode-version.json"), "utf8"));

// Peta rust target triple -> nama paket npm opencode.
const MAP = {
  "x86_64-pc-windows-msvc": { pkg: "opencode-windows-x64", exe: ".exe" },
  "aarch64-pc-windows-msvc": { pkg: "opencode-windows-arm64", exe: ".exe" },
  "x86_64-apple-darwin": { pkg: "opencode-darwin-x64", exe: "" },
  "aarch64-apple-darwin": { pkg: "opencode-darwin-arm64", exe: "" },
  "x86_64-unknown-linux-gnu": { pkg: "opencode-linux-x64", exe: "" },
  "aarch64-unknown-linux-gnu": { pkg: "opencode-linux-arm64", exe: "" },
};

function hostTriple() {
  const out = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  return out.match(/^host: (.+)$/m)[1].trim();
}

const ti = process.argv.indexOf("--target");
const triple = ti > 0 ? process.argv[ti + 1] : hostTriple();
const entry = MAP[triple];
if (!entry) {
  console.error(`Target tidak didukung: ${triple}`);
  process.exit(1);
}

const dl = join(spikes, ".downloads", `${entry.pkg}-${version}`);
rmSync(dl, { recursive: true, force: true });
mkdirSync(dl, { recursive: true });

console.log(`npm pack ${entry.pkg}@${version} ...`);
const shell = process.platform === "win32";
execFileSync("npm", ["pack", `${entry.pkg}@${version}`, "--silent"], { cwd: dl, stdio: "inherit", shell });
const tgz = readdirSync(dl).find((f) => f.endsWith(".tgz"));
execFileSync("tar", ["-xzf", tgz], { cwd: dl, stdio: "inherit" });

const src = join(dl, "package", "bin", `opencode${entry.exe}`);
const outDir = join(spikes, "s1-sidecar", "src-tauri", "binaries");
mkdirSync(outDir, { recursive: true });
const dst = join(outDir, `opencode-${triple}${entry.exe}`);
copyFileSync(src, dst);
if (!entry.exe) chmodSync(dst, 0o755);

const sha = createHash("sha256").update(readFileSync(dst)).digest("hex");
writeFileSync(`${dst}.sha256`, `${sha}  opencode-${triple}${entry.exe}\n`);
const ver = execFileSync(dst, ["--version"], { encoding: "utf8" }).trim();
if (ver !== version) {
  console.error(`Versi tidak cocok: diharapkan ${version}, didapat ${ver}`);
  process.exit(1);
}
console.log(`OK ${dst}\n   versi ${ver}\n   sha256 ${sha}`);

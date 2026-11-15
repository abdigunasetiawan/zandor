// Pasang salinan "stabil" Zandor di luar repo, untuk dipakai mengembangkan Zandor itu sendiri (ADR-061).
//   bun scripts/install-stable.ts [--profile release|debug] [--dest <folder>] [--no-build]
// Salinan ini berjalan dari folder sendiri (bukan target/), jadi build baru tidak terhalang file exe yang terkunci.
// Ia memakai identifier produksi (id.web.zandor.app); build dev memakai id.web.zandor.dev (data dan single-instance terpisah).
import { spawn, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { join, relative, resolve } from "node:path";

const args = process.argv.slice(2);
const opt = (name: string, fallback: string) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? (args[i + 1] as string) : fallback;
};
const profile = opt("--profile", "release");
if (profile !== "release" && profile !== "debug") {
  console.error("--profile harus release atau debug");
  process.exit(2);
}
const repo = resolve(import.meta.dirname, "..");
const dest = resolve(opt("--dest", join(process.env.LOCALAPPDATA ?? process.env.HOME ?? ".", "ZandorStable")));
if (!relative(repo, dest).startsWith("..")) {
  console.error(`Tujuan harus di luar repo (bukan ${dest}); target/ dibersihkan dan dikunci saat build.`);
  process.exit(2);
}

if (!args.includes("--no-build")) {
  console.log(`Membangun (${profile}) dengan identifier produksi…`);
  const build = spawnSync("bun", ["x", "tauri", "build", ...(profile === "debug" ? ["--debug"] : []), "--no-bundle"], {
    cwd: join(repo, "apps", "desktop"),
    stdio: "inherit",
    shell: true,
  });
  if (build.status !== 0) process.exit(build.status ?? 1);
}

const out = join(repo, "target", profile);
const files = ["zandor-desktop.exe", "opencode.exe"];
for (const f of files) {
  if (!existsSync(join(out, f))) {
    console.error(`Tidak ada ${join(out, f)}; jalankan tanpa --no-build.`);
    process.exit(1);
  }
}
const destExe = join(dest, "zandor-desktop.exe");
// Tutup HANYA salinan stabil di dest (bukan zandor-desktop.exe lain, mis. build dev di target/debug).
const psPath = destExe.replace(/'/g, "''");
spawnSync(
  "powershell",
  [
    "-NoProfile",
    "-Command",
    `Get-CimInstance Win32_Process -Filter "Name='zandor-desktop.exe'" | Where-Object { $_.ExecutablePath -eq '${psPath}' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }`,
  ],
  { stdio: "ignore" },
);
await new Promise((r) => setTimeout(r, 1500));
// Cadangan versi sebelumnya (satu generasi) supaya bisa kembali.
if (existsSync(destExe)) {
  const bak = `${dest}.bak`;
  try {
    rmSync(bak, { recursive: true, force: true });
    cpSync(dest, bak, { recursive: true });
    console.log(`Cadangan versi lama: ${bak}`);
  } catch (e) {
    console.error(`Gagal membuat cadangan (${(e as Error).message}); pemasangan dibatalkan.`);
    process.exit(1);
  }
}
mkdirSync(dest, { recursive: true });
try {
  for (const f of files) cpSync(join(out, f), join(dest, f));
  rmSync(join(dest, "resources"), { recursive: true, force: true });
  cpSync(join(out, "resources"), join(dest, "resources"), { recursive: true });
} catch (e) {
  console.error(
    `Gagal menyalin ke ${dest}: ${(e as Error).message}\nBila Zandor stabil sedang terbuka, tutup dulu lalu ulangi.`,
  );
  process.exit(1);
}
console.log(`Terpasang di ${destExe}`);
if (!args.includes("--no-launch")) {
  spawn(destExe, [], { detached: true, stdio: "ignore" }).unref();
  console.log("Zandor stabil dibuka.");
}
console.log("Data: %APPDATA%id.web.zandor.app (sama dengan Zandor yang Anda pakai sehari-hari).");

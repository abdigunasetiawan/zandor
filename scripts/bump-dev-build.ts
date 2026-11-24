// Naikkan nomor build development (apps/desktop-gpui/dev-build.txt) dan cetak label versi
// yang persis sama dengan yang tampil di Sidebar Zandor Dev.
//
// Pakai: bun scripts/bump-dev-build.ts
// Wajib dijalankan setiap selesai mengerjakan fitur/perbaikan pada masa development,
// lalu cantumkan keluarannya di akhir laporan agar bisa dicocokkan di Sidebar.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const FILE = join(ROOT, "apps", "desktop-gpui", "dev-build.txt");

let prev = 0;
if (existsSync(FILE)) {
  const m = readFileSync(FILE, "utf8").trim().match(/(\d+)$/);
  if (m) prev = Number.parseInt(m[1], 10);
}
const next = `dev.${prev + 1}`;
writeFileSync(FILE, `${next}\n`, "utf8");

const git = Bun.spawnSync(["git", "rev-list", "--count", "HEAD"], { cwd: ROOT, stdout: "pipe" });
const count = git.exitCode === 0 ? git.stdout.toString().trim() : "";
const ver = count ? `0.0.${count}` : "0.1.0";
console.log(`v${ver} · ${next}`);

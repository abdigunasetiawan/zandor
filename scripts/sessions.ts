// Daftar dan bersihkan sesi di database engine Zandor (hanya baca kecuali ada --yes). JALANKAN DI TERMINAL ANDA SENDIRI.
//   bun scripts/sessions.ts list                         daftar semua sesi (hanya baca)
//   bun scripts/sessions.ts purge --smoke                dry-run: sisa smoke test (folder zandor-smoke-* yang sudah tidak ada)
//   bun scripts/sessions.ts purge --ids ses_a,ses_b      dry-run: sesi tertentu (id boleh awalan unik dari kolom `list`)
//   bun scripts/sessions.ts purge --all                  dry-run: SEMUA sesi
//   tambahkan --yes untuk benar-benar menghapus. Opsi: --app-id id.web.zandor.app (bawaan), --exe <opencode.exe>.
// Penghapusan lewat API engine sementara (data turunan ikut bersih); cadangan database dibuat dulu di %TEMP%.
// Sebaiknya tutup Zandor lebih dulu. Sesi demo di sidebar bukan sesi engine: matikan lewat Pengaturan → Chat.
import { Database } from "bun:sqlite";
import { spawn, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const cmd = args[0];
const opt = (name: string, fallback = "") => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? (args[i + 1] as string) : fallback;
};
const appId = opt("--app-id", "id.web.zandor.app");
const root = join(process.env.APPDATA ?? "", appId, "engine");
const dbDir = join(root, "data", "opencode");
const dbFile = join(dbDir, "opencode.db");
if (!existsSync(dbFile)) {
  console.error(`Database tidak ditemukan: ${dbFile}`);
  process.exit(1);
}

type Row = { id: string; title: string; directory: string; time_updated: number; msgs: number };
const db = new Database(dbFile, { readonly: true });
const rows = db
  .query(
    "SELECT s.id, s.title, s.directory, s.time_updated, (SELECT count(*) FROM message m WHERE m.session_id = s.id) AS msgs FROM session s ORDER BY s.time_updated DESC",
  )
  .all() as Row[];
db.close();

const smoke = (r: Row) => /[\\/]zandor-smoke-/.test(r.directory) && !existsSync(r.directory);
const line = (r: Row) =>
  [
    r.id.slice(0, 18),
    new Date(r.time_updated).toISOString().slice(0, 16),
    `${r.msgs} pesan`,
    existsSync(r.directory) ? "folder ada " : "FOLDER MATI",
    JSON.stringify(r.title.slice(0, 40)),
    r.directory.slice(-48),
  ].join(" | ");

if (cmd === "list") {
  console.log(`${rows.length} sesi di ${dbFile}`);
  console.log(`(sisa smoke: ${rows.filter(smoke).length})`);
  for (const r of rows) console.log(line(r));
  process.exit(0);
}

if (cmd !== "purge") {
  console.error("Perintah: list | purge [--smoke] [--ids a,b] [--yes]");
  process.exit(2);
}

const ids = opt("--ids")
  .split(/[,\s]+/)
  .filter(Boolean);
const targets = new Map<string, Row>();
if (args.includes("--smoke")) for (const r of rows.filter(smoke)) targets.set(r.id, r);
if (args.includes("--all")) for (const r of rows) targets.set(r.id, r);
for (const p of ids) {
  const hit = rows.filter((r) => r.id.startsWith(p));
  if (hit.length !== 1) {
    console.error(`id "${p}" ${hit.length === 0 ? "tidak ditemukan" : "tidak unik"}; tidak ada yang dihapus.`);
    process.exit(1);
  }
  targets.set((hit[0] as Row).id, hit[0] as Row);
}
if (targets.size === 0) {
  console.log("Tidak ada target. Gunakan --smoke dan/atau --ids.");
  process.exit(0);
}
console.log(`${targets.size} sesi akan dihapus:`);
for (const r of targets.values()) console.log(`  ${line(r)}`);
if (!args.includes("--yes")) {
  console.log("\nDry-run. Tambahkan --yes untuk menghapus.");
  process.exit(0);
}

const backup = join(process.env.TEMP ?? ".", `zandor-db-backup-${Date.now()}`);
mkdirSync(backup, { recursive: true });
for (const f of ["opencode.db", "opencode.db-wal", "opencode.db-shm"])
  if (existsSync(join(dbDir, f))) copyFileSync(join(dbDir, f), join(backup, f));
console.log(`Cadangan database: ${backup}`);

const exe = opt("--exe", join(process.env.LOCALAPPDATA ?? "", "Zandor", "opencode.exe"));
if (!existsSync(exe)) {
  console.error(`opencode.exe tidak ditemukan: ${exe} (gunakan --exe).`);
  process.exit(1);
}
const password = crypto.randomUUID();
const env = { ...process.env, OPENCODE_SERVER_PASSWORD: password, OPENCODE_DISABLE_AUTOUPDATE: "1" } as Record<
  string,
  string
>;
for (const k of ["DATA", "CONFIG", "CACHE", "STATE"]) env[`XDG_${k}_HOME`] = join(root, k.toLowerCase());
const child = spawn(exe, ["serve", "--hostname", "127.0.0.1", "--port", "0"], {
  cwd: root,
  env,
  stdio: ["ignore", "pipe", "pipe"],
});
let url = "";
const onData = (b: Buffer) => {
  const m = /listening on (http:\/\/[^\s]+)/.exec(b.toString());
  if (m && !url) url = m[1] as string;
};
child.stdout.on("data", onData);
child.stderr.on("data", onData);
try {
  for (let i = 0; i < 60 && !url; i++) await new Promise((r) => setTimeout(r, 500));
  if (!url) throw new Error("engine sementara tidak siap");
  const auth = `Basic ${Buffer.from(`opencode:${password}`).toString("base64")}`;
  let ok = 0;
  let fail = 0;
  for (const t of targets.values()) {
    const r = await fetch(`${url.replace(/\/$/, "")}/session/${t.id}?directory=${encodeURIComponent(t.directory)}`, {
      method: "DELETE",
      headers: { authorization: auth },
    });
    if (r.ok) ok++;
    else {
      fail++;
      console.log(`gagal ${t.id}: HTTP ${r.status}`);
    }
  }
  console.log(`Terhapus ${ok}, gagal ${fail}. Buka ulang Zandor untuk menyegarkan sidebar.`);
} finally {
  spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
}

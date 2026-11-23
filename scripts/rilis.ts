// Rilis Zandor (GPUI) ke GitHub Releases, saluran beta (ADR-084).
//
//   bun scripts/rilis.ts --keygen                  buat pasangan kunci penanda tangan (sekali saja)
//   bun scripts/rilis.ts 0.1.0 --notes "Catatan"   build, buat installer, tanda tangani, unggah rilis v0.1.0
//   bun scripts/rilis.ts 0.1.0 --dry-run           sama, tanpa mengunggah (hasil di dist/rilis/)
//   tambahan --fast: profil release (lebih cepat dibangun) alih-alih dist (fat LTO); hanya untuk uji.
//
// Kunci privat disimpan di %USERPROFILE%\.zandor\update-signing-key.pem (TIDAK di repo). Cadangkan berkas itu:
// tanpa kunci yang sama, Zandor yang sudah terpasang tidak akan menerima pembaruan berikutnya.
// Butuh: Rust (cargo), NSIS (makensis), dan GitHub CLI (gh) yang sudah login.
import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync, copyFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const APP = join(ROOT, "apps", "desktop-gpui");
const KEY_DIR = join(homedir(), ".zandor");
const KEY_FILE = join(KEY_DIR, "update-signing-key.pem");
const PUB_FILE = join(APP, "src", "core", "update_key.pub");
const REPO = "abdigunasetiawan/zandor";
const OUT = join(ROOT, "dist", "rilis");

const args = process.argv.slice(2);
const flag = (n: string) => args.includes(n);
const opt = (n: string) => {
  const i = args.indexOf(n);
  return i >= 0 ? args[i + 1] : undefined;
};

function die(msg: string): never {
  console.error(`✗ ${msg}`);
  process.exit(1);
}

function run(cmd: string[], opts: { cwd?: string; env?: Record<string, string>; quiet?: boolean } = {}): string {
  const p = Bun.spawnSync(cmd, {
    cwd: opts.cwd ?? ROOT,
    env: { ...process.env, ...opts.env },
    stdout: opts.quiet ? "pipe" : "inherit",
    stderr: opts.quiet ? "pipe" : "inherit",
  });
  if (p.exitCode !== 0) {
    die(`${cmd.join(" ")} gagal (kode ${p.exitCode})${opts.quiet ? `\n${p.stderr.toString()}` : ""}`);
  }
  return opts.quiet ? p.stdout.toString().trim() : "";
}

function keygen() {
  if (existsSync(KEY_FILE) && !flag("--force")) {
    die(`kunci sudah ada di ${KEY_FILE}. Mengganti kunci memutus pembaruan untuk instalasi yang ada; pakai --force bila memang perlu.`);
  }
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  mkdirSync(KEY_DIR, { recursive: true });
  writeFileSync(KEY_FILE, privateKey.export({ type: "pkcs8", format: "pem" }), { mode: 0o600 });
  const raw = (publicKey.export({ type: "spki", format: "der" }) as Buffer).subarray(-32);
  writeFileSync(PUB_FILE, `${raw.toString("hex")}\n`);
  console.log(`✓ kunci privat: ${KEY_FILE}  (CADANGKAN, jangan di-commit)`);
  console.log(`✓ kunci publik: ${PUB_FILE}  (di-commit; tertanam di aplikasi)`);
}

const signedMessage = (version: string, sha256: string, size: number) => `zandor-update\n${version}\n${sha256.toLowerCase()}\n${size}`;

function release(version: string) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) die(`versi harus berbentuk X.Y.Z, bukan "${version}"`);
  const dry = flag("--dry-run");
  if (flag("--fast") && !dry) die("--fast hanya untuk --dry-run; rilis resmi memakai profil dist.");
  const tag = `v${version}`;
  if (!existsSync(KEY_FILE)) die(`kunci privat tidak ada (${KEY_FILE}). Jalankan --keygen dulu atau pulihkan cadangannya.`);
  const key = createPrivateKey(readFileSync(KEY_FILE));
  const pub = readFileSync(PUB_FILE, "utf8").trim();
  const derived = (createPublicKey(key).export({ type: "spki", format: "der" }) as Buffer).subarray(-32).toString("hex");
  if (derived !== pub) die(`kunci privat tidak cocok dengan ${PUB_FILE}; instalasi yang ada akan menolak rilis ini.`);

  const dirty = run(["git", "status", "--porcelain"], { quiet: true });
  if (dirty && !dry) die("working tree belum bersih; commit dulu supaya rilis bisa dilacak ke satu commit.");
  const head = run(["git", "rev-parse", "HEAD"], { quiet: true });
  if (!dry) {
    if (run(["git", "tag", "--list", tag], { quiet: true })) die(`tag ${tag} sudah ada`);
    if (!run(["git", "branch", "-r", "--contains", head], { quiet: true })) die("commit HEAD belum ada di GitHub; push branch dulu.");
  }

  const makensis = [process.env.MAKENSIS, "C:/Program Files (x86)/NSIS/makensis.exe", "C:/Program Files/NSIS/makensis.exe"].find(
    (p) => p && existsSync(p),
  );
  if (!makensis) die("makensis (NSIS) tidak ditemukan; pasang NSIS atau set MAKENSIS.");

  // Profil `dist` (fat LTO, strip) untuk rilis; --fast memakai profil release (thin LTO) untuk uji installer.
  const profile = flag("--fast") ? "release" : "dist";
  console.log(`• build ${version} (profil ${profile})`);
  run(["cargo", "build", "--profile", profile, "--offline"], { cwd: APP, env: { ZANDOR_RELEASE_VERSION: version, CARGO_INCREMENTAL: "0" } });

  console.log("• menyusun berkas");
  const stage = join(OUT, "stage");
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(join(stage, "resources"), { recursive: true });
  const tauri = join(ROOT, "apps", "desktop", "src-tauri");
  const files: [string, string][] = [
    [join(APP, "target", profile, "zandor-gpui.exe"), join(stage, "Zandor.exe")],
    [join(tauri, "binaries", "opencode-x86_64-pc-windows-msvc.exe"), join(stage, "opencode.exe")],
    [join(tauri, "resources", "zandor-plugin.js"), join(stage, "resources", "zandor-plugin.js")],
  ];
  for (const [from, to] of files) {
    if (!existsSync(from)) die(`berkas tidak ada: ${from} (jalankan bun run prepare:engine untuk engine)`);
    copyFileSync(from, to);
  }

  console.log("• membuat installer");
  const installer = join(OUT, `Zandor-Setup-${version}.exe`);
  run([makensis, "/V2", `/DVERSION=${version}`, `/DSTAGE=${stage}`, `/DOUTFILE=${installer}`, join(APP, "installer", "zandor.nsi")]);

  const bytes = readFileSync(installer);
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  const size = statSync(installer).size;
  const signature = sign(null, Buffer.from(signedMessage(version, sha256, size)), key).toString("hex");
  const notesFile = opt("--notes-file");
  const notes = notesFile ? readFileSync(notesFile, "utf8").trim() : (opt("--notes") ?? "");
  const manifest = {
    version,
    notes,
    pub_date: new Date().toISOString(),
    url: `https://github.com/${REPO}/releases/download/${tag}/Zandor-Setup-${version}.exe`,
    sha256,
    size,
    signature,
  };
  const manifestPath = join(OUT, "latest.json");
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`✓ ${installer} (${(size / 1048576).toFixed(1)} MB, sha256 ${sha256.slice(0, 12)}…)`);
  console.log(`✓ ${manifestPath}`);

  if (dry) {
    console.log("• --dry-run: tidak diunggah.");
    return;
  }
  console.log(`• mengunggah rilis ${tag}`);
  run([
    "gh", "release", "create", tag, installer, manifestPath,
    "--repo", REPO, "--target", head, "--title", `Zandor ${version} (beta)`, "--notes", notes || `Zandor ${version} (beta).`,
  ]);
  console.log(`✓ https://github.com/${REPO}/releases/tag/${tag}`);
}

if (flag("--keygen")) keygen();
else {
  const version = args.find((a) => !a.startsWith("--") && args[args.indexOf(a) - 1] !== "--notes" && args[args.indexOf(a) - 1] !== "--notes-file");
  if (!version) die("pakai: bun scripts/rilis.ts <X.Y.Z> [--notes teks | --notes-file berkas] [--dry-run]  |  --keygen");
  release(version);
}

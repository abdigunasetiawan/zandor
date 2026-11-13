// Smoke test NATIVE: menjalankan aplikasi Tauri sungguhan, menghubungkan Playwright ke WebView2 lewat CDP,
// lalu menguji kontrol jendela kustom (maksimalkan/pulihkan/minimalkan/tutup) terhadap jendela Windows asli.
//
// Pemakaian (Windows): bun e2e-native/smoke.ts <path zandor-desktop.exe>
// Build dengan UI tertanam: `bun run tauri build --debug --no-bundle` (cargo build biasa memuat devUrl).
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "@playwright/test";

const exe = process.argv[2];
if (!exe) {
  console.error("pemakaian: bun e2e-native/smoke.ts <path zandor-desktop.exe>");
  process.exit(2);
}
const exePath: string = exe;
const PORT = 9333;

/** Status jendela Windows asli milik proses (IsZoomed/IsIconic/IsWindowVisible + ada title bar sistem?). */
function winState(pid: number): { found: boolean; zoomed: boolean; iconic: boolean; caption: boolean } {
  const ps = `
Add-Type 'using System;using System.Runtime.InteropServices;public class W{
[DllImport("user32.dll")]public static extern bool IsZoomed(IntPtr h);
[DllImport("user32.dll")]public static extern bool IsIconic(IntPtr h);
[DllImport("user32.dll")]public static extern int GetWindowLong(IntPtr h,int i);}';
$p = Get-Process -Id ${pid} -EA SilentlyContinue;
if (-not $p -or $p.MainWindowHandle -eq 0) { '{"found":false}'; exit }
$h = $p.MainWindowHandle; $st = [W]::GetWindowLong($h,-16);
@{found=$true; zoomed=[W]::IsZoomed($h); iconic=[W]::IsIconic($h); caption=[bool]($st -band 0xC00000)} | ConvertTo-Json -Compress`;
  const r = spawnSync("powershell", ["-NoProfile", "-Command", ps], { encoding: "utf8" });
  const parsed = JSON.parse(r.stdout.trim() || '{"found":false}');
  return { found: parsed.found, zoomed: !!parsed.zoomed, iconic: !!parsed.iconic, caption: !!parsed.caption };
}

async function waitFor<T>(what: string, fn: () => T | Promise<T>, ok: (v: T) => boolean, ms = 10000): Promise<T> {
  const t0 = Date.now();
  let last: T | undefined;
  while (Date.now() - t0 < ms) {
    last = await fn();
    if (ok(last)) return last;
    await new Promise((r) => setTimeout(r, 150));
  }
  throw new Error(`timeout menunggu: ${what}; terakhir: ${JSON.stringify(last)}`);
}

const results: Array<[string, boolean, string]> = [];
const check = (name: string, pass: boolean, detail = "") => {
  results.push([name, pass, detail]);
  console.log(`${pass ? "OK   " : "GAGAL"} ${name}${detail ? ` — ${detail}` : ""}`);
};

let app: ChildProcess | undefined;

/** Jalankan aplikasi, sambungkan Playwright ke WebView2, kembalikan halaman utamanya. */
async function launch() {
  const child = spawn(exePath, [], {
    stdio: "ignore",
    env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` },
  });
  app = child;
  const pid = child.pid as number;

  const browser = await waitFor(
    "CDP WebView2",
    async () => {
      try {
        return await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
      } catch {
        return null;
      }
    },
    (b) => b !== null,
    30000,
  );
  const page = await waitFor(
    "halaman aplikasi",
    () =>
      browser
        ?.contexts()[0]
        ?.pages()
        .find((p) => !p.url().startsWith("devtools")),
    (p) => !!p,
  );
  if (!page) throw new Error("halaman tidak ditemukan");
  await page.waitForSelector(".titlebar", { timeout: 15000 });
  // Bahasa deterministik (WebView CI biasanya en-US): paksa Indonesia, lalu muat ulang.
  await page.evaluate(() => localStorage.setItem("zandor.settings", JSON.stringify({ language: "id" })));
  await page.reload();
  await page.waitForSelector(".titlebar", { timeout: 15000 });
  return { browser, page, pid };
}

try {
  const { browser, page, pid } = await launch();

  // 1) IPC Tauri tersedia dan kontrol jendela tampil (di browser biasa tidak).
  const hasIpc = await page.evaluate(() => "__TAURI_INTERNALS__" in window);
  check("IPC Tauri tersedia di webview UI", hasIpc);
  const maximize = page.getByRole("button", { name: /^(Maksimalkan|Maximize)$/ });
  await maximize.waitFor({ timeout: 10000 });
  check("kontrol jendela kustom tampil", true);

  // 1b) Engine sungguhan: supervisor menyalakan opencode, UI tersambung, sesi bisa dibuat (Fase 4).
  const status = page.locator(".sidebar__status");
  await waitFor(
    "engine siap",
    async () => `${await status.textContent().catch(() => "")} | ${await status.getAttribute("title").catch(() => "")}`,
    (t) => /Engine siap/.test(t),
    40000,
  );
  check("engine opencode siap (supervisor + plugin guard)", true, (await status.textContent())?.trim());
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await page.getByText("Sesi tanpa judul").first().waitFor({ timeout: 15000 });
  check("buat sesi tanpa project lewat engine asli", true);

  // 1c) Chat nyata (Fase 5). Hanya bila mesin ini punya provider di providers.json; di CI tidak ada → dilewati.
  const modelPicker = page.getByRole("combobox", { name: "Model" });
  const hasProvider = await modelPicker
    .waitFor({ timeout: 15000 })
    .then(() => true)
    .catch(() => false);
  if (!hasProvider) {
    console.log("      (info) tidak ada provider terkonfigurasi; pengujian chat nyata dilewati");
  } else {
    const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
    await box.fill("Balas hanya dengan satu kata: pong");
    await box.press("Enter");
    await page
      .getByRole("article", { name: "Asisten" })
      .filter({ hasText: /pong/i })
      .first()
      .waitFor({ timeout: 90000 });
    check("chat nyata: prompt → jawaban streaming dari model", true);

    await box.fill("Buat file zandor-smoke.txt berisi kata halo (gunakan tool tulis file).");
    await box.press("Enter");
    const card = page.getByRole("alertdialog", { name: /Izin diperlukan/ });
    await card.waitFor({ timeout: 90000 });
    check("tool call tulis file meminta izin (kartu izin tampil)", true, (await card.textContent())?.slice(0, 60));
    await card.getByRole("button", { name: "Izinkan sekali" }).click();
    await page.locator(".fold--tool[data-status='completed']").first().waitFor({ timeout: 90000 });
    check("setelah Izinkan sekali, tool call selesai dijalankan engine", true);
    await page.getByRole("button", { name: "Kirim" }).waitFor({ timeout: 60000 });
  }

  // 2) Jendela asli tanpa title bar sistem (frameless) — bit WS_CAPTION bisa tetap menyala pada jendela
  //    tanpa dekorasi bergaya bayangan, jadi hanya dilaporkan, bukan syarat lulus.
  const s0 = winState(pid);
  check("jendela asli ditemukan", s0.found);
  console.log(`      (info) WS_CAPTION=${s0.caption}`);

  // 3) Maksimalkan → pulihkan.
  await maximize.click();
  const z = await waitFor(
    "jendela termaksimalkan",
    () => winState(pid),
    (s) => s.zoomed,
  );
  check("klik Maksimalkan → jendela asli termaksimalkan", z.zoomed);
  const restore = page.getByRole("button", { name: /^(Pulihkan|Restore)$/ });
  await restore.waitFor({ timeout: 5000 });
  check("ikon berubah menjadi Pulihkan", true);
  await restore.click();
  const u = await waitFor(
    "jendela dipulihkan",
    () => winState(pid),
    (s) => !s.zoomed,
  );
  check("klik Pulihkan → jendela kembali normal", !u.zoomed);

  // 4) Minimalkan.
  await page.getByRole("button", { name: /^(Minimalkan|Minimize)$/ }).click();
  const m = await waitFor(
    "jendela terminimalkan",
    () => winState(pid),
    (s) => s.iconic,
  );
  check("klik Minimalkan → jendela asli terminimalkan", m.iconic);

  // 5) Tutup lewat tombol → proses keluar bersih.
  // (Jendela terminimalkan: pulihkan dulu lewat CDP agar tombol bisa diklik.)
  spawnSync("powershell", [
    "-NoProfile",
    "-Command",
    `Add-Type 'using System;using System.Runtime.InteropServices;public class R{[DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int c);}'; $p=Get-Process -Id ${pid}; [R]::ShowWindow($p.MainWindowHandle,9)|Out-Null`,
  ]);
  await waitFor(
    "jendela dipulihkan dari minimize",
    () => winState(pid),
    (s) => !s.iconic,
  );
  await page.getByRole("button", { name: /^(Tutup jendela|Close window)$/ }).click();
  const exited = await waitFor(
    "proses keluar",
    () => app?.exitCode !== null || app?.killed,
    (v) => !!v,
    8000,
  ).then(
    () => true,
    () => false,
  );
  check("klik Tutup → aplikasi keluar", exited);
  await browser?.close().catch(() => {});

  // 6) Persistensi: jalankan ulang aplikasi → sesi yang dibuat tadi masih ada; lalu bersihkan semuanya.
  // (Judul sesi bisa berubah karena engine memberi judul otomatis setelah chat, jadi dicari lewat barisnya.)
  const second = await launch();
  const rows = second.page.locator(".tree__children .tree__session");
  await rows.first().waitFor({ timeout: 40000 });
  check("restart aplikasi → sesi tetap ada (persisten di engine)", true, `${await rows.count()} sesi`);
  while ((await rows.count()) > 0) {
    await second.page.locator(".tree__children .tree__row button[aria-haspopup='menu']").first().click({ force: true });
    await second.page.getByRole("menuitem", { name: "Hapus sesi" }).click();
    await second.page.getByRole("alertdialog").getByRole("button", { name: "Hapus sesi" }).click();
    await waitFor(
      "satu sesi uji terhapus",
      () => rows.count(),
      (n) => n >= 0,
      5000,
    );
    await new Promise((r) => setTimeout(r, 400));
  }
  check("sesi uji dihapus (tidak meninggalkan sampah)", true);
  // berkas yang ditulis agent saat uji izin (best effort)
  const junk = join(process.env.APPDATA ?? "", "id.web.zandor.app", "chats", "zandor-smoke.txt");
  try {
    rmSync(junk, { force: true });
  } catch {
    // diabaikan
  }
  await second.page.getByRole("button", { name: /^(Tutup jendela|Close window)$/ }).click();
  await waitFor(
    "proses keluar (kedua)",
    () => app?.exitCode !== null || app?.killed,
    (v) => !!v,
    8000,
  );
  await second.browser?.close().catch(() => {});
} catch (e) {
  check("smoke test selesai tanpa galat", false, String(e));
} finally {
  if (app && app.exitCode === null) app.kill();
  // pastikan tidak ada sisa proses engine/webview milik aplikasi
  spawnSync("taskkill", ["/F", "/IM", "zandor-desktop.exe"], { stdio: "ignore" });
}

const failed = results.filter(([, pass]) => !pass);
console.log(`\n${results.length - failed.length}/${results.length} lulus`);
process.exit(failed.length ? 1 : 0);

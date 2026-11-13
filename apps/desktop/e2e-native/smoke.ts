// Smoke test NATIVE: menjalankan aplikasi Tauri sungguhan, menghubungkan Playwright ke WebView2 lewat CDP,
// lalu menguji kontrol jendela kustom (maksimalkan/pulihkan/minimalkan/tutup) terhadap jendela Windows asli.
//
// Pemakaian (Windows): bun e2e-native/smoke.ts <path zandor-desktop.exe>
// Build dengan UI tertanam: `bun run tauri build --debug --no-bundle` (cargo build biasa memuat devUrl).
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { chromium, type Page } from "@playwright/test";
import { makePdf, makePng, SAMPLE_MD } from "../e2e/fixtures";

const exe = process.argv[2];
if (!exe) {
  console.error("pemakaian: bun e2e-native/smoke.ts <path zandor-desktop.exe>");
  process.exit(2);
}
const exePath: string = exe;
const PORT = 9333;
// Tidak semua model di endpoint pengembang berfungsi; model ini diketahui bisa dipakai (bisa diganti lewat env).
const SMOKE_MODEL = process.env.ZANDOR_SMOKE_MODEL ?? "DeepSeek";

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

/**
 * Model nyata kadang meminta beberapa izin berurutan (mis. external_directory lalu edit). Setujui kartu izin satu per
 * satu sampai tool bertanda `completed` muncul (filter opsional). Gagal bila lewat batas waktu.
 */
async function approveUntilCompleted(page: Page, filter?: string | RegExp, timeoutMs = 180000) {
  const done = filter
    ? page.locator(".fold--tool[data-status='completed']").filter({ hasText: filter })
    : page.locator(".fold--tool[data-status='completed']");
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await done.count()) > 0) return;
    const card = page.getByRole("alertdialog", { name: /Izin diperlukan/ }).first();
    if ((await card.count()) > 0) {
      await card
        .getByRole("button", { name: "Izinkan sekali" })
        .click({ timeout: 5000 })
        .catch(() => {});
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error("tool call tidak selesai setelah semua izin disetujui");
}

let app: ChildProcess | undefined;

// Project uji untuk Panel 3: folder sementara didaftarkan ke registry aplikasi, lalu registry asli dipulihkan di akhir.
const registryFile = join(process.env.APPDATA ?? "", "id.web.zandor.app", "projects.json");
const docDir = mkdtempSync(join(tmpdir(), "zandor-smoke-docs-"));
const docProject = basename(docDir);
const extraDirs = [
  mkdtempSync(join(tmpdir(), "zandor-smoke-docs-b-")),
  mkdtempSync(join(tmpdir(), "zandor-smoke-docs-c-")),
];
// Fase 9: smoke menyentuh data memori asli (settings, global.md, antrean skill); semuanya dicadangkan dan dipulihkan.
const appDataDir = join(process.env.APPDATA ?? "", "id.web.zandor.app");
const memoryDir = join(appDataDir, "memory");
const smokeSkillDir = join(appDataDir, "engine", "config", "opencode", "skills", "smoke-skill");
const MEMORY_FILES = ["settings.json", "global.md", "skill-queue.jsonl"];
const memoryBackup = new Map<string, string | null>();
for (const f of MEMORY_FILES) {
  try {
    memoryBackup.set(f, readFileSync(join(memoryDir, f), "utf8"));
  } catch {
    memoryBackup.set(f, null);
  }
}
let originalRegistry: string | null = null;
// Fase 10: kustomisasi tampilan tersimpan di localStorage WebView aplikasi asli; dipulihkan di akhir.
let originalAppearance: string | null = null;
try {
  originalRegistry = readFileSync(registryFile, "utf8");
} catch {
  originalRegistry = null;
}
// Sisa dari run yang dihentikan paksa (finally tak sempat jalan) dibuang agar tidak ikut "dipulihkan".
if (originalRegistry) {
  try {
    const reg = JSON.parse(originalRegistry) as { projects?: Array<{ name?: string }> };
    reg.projects = (reg.projects ?? []).filter((p) => !String(p.name).startsWith("zandor-smoke-docs-"));
    originalRegistry = JSON.stringify(reg);
  } catch {
    // registry rusak: dipulihkan apa adanya
  }
}
writeFileSync(join(docDir, "README.md"), SAMPLE_MD);
writeFileSync(join(docDir, "kode.ts"), "export const nilai: number = 42;\n");
writeFileSync(join(docDir, "data.csv"), "id,nama\n1,satu\n2,dua\n");
writeFileSync(join(docDir, "laporan.pdf"), makePdf(["HALO PDF", "HALAMAN DUA"]));
writeFileSync(join(docDir, "gambar.png"), makePng(8, 8));
mkdirSync(join(docDir, "sub"));
// Repo git uji: satu commit awal, lalu README diubah (unstaged) dan satu berkas baru (untracked).
const git = (...args: string[]) => spawnSync("git", ["-C", docDir, ...args], { encoding: "utf8" });
git("init", "-q", "-b", "main");
git("config", "user.email", "smoke@zandor.test");
git("config", "user.name", "Smoke Zandor");
git("config", "commit.gpgsign", "false");
git("add", "README.md", "kode.ts");
git("commit", "-q", "-m", "awal");
writeFileSync(join(docDir, "README.md"), `${SAMPLE_MD}\nbaris tambahan dari smoke\n`);
writeFileSync(join(docDir, "baru.txt"), "berkas baru\n");
{
  let reg: { projects?: unknown[]; [k: string]: unknown } = {
    version: 1,
    projects: [],
    selection: { key: null, sessionId: null },
  };
  try {
    if (originalRegistry) reg = JSON.parse(originalRegistry);
  } catch {
    // registry rusak: pakai yang kosong; yang asli dipulihkan persis di akhir
  }
  reg.projects = [
    ...(reg.projects ?? []),
    { id: "smoke-docs", path: docDir, name: docProject, color: null, addedAt: Date.now() },
    ...extraDirs.map((d, i) => ({
      id: `smoke-docs-${i}`,
      path: d,
      name: basename(d),
      color: null,
      addedAt: Date.now(),
    })),
  ];
  mkdirSync(join(process.env.APPDATA ?? "", "id.web.zandor.app"), { recursive: true });
  writeFileSync(registryFile, JSON.stringify(reg));
}

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
  // 1b1) Panel 3 (Fase 7): berkas project dibaca lewat Rust (fs_read), dirender, dan akses di luar project ditolak.
  await page.getByRole("button", { name: docProject, exact: true }).first().click();
  await page.getByRole("tab", { name: "Files" }).click();
  const docsPanel = page.getByRole("region", { name: "Dokumen", exact: true });
  await page.locator('.tree__file[title="README.md"]').click();
  await docsPanel.getByRole("heading", { name: "Judul Dokumen" }).waitFor({ timeout: 30000 });
  check("Panel 3: Markdown dari disk dirender (fs_read lewat Rust)", true);
  await page.locator('.tree__file[title="kode.ts"]').click();
  await docsPanel
    .getByTestId("monaco")
    .locator(".view-lines")
    .getByText("export const nilai")
    .waitFor({ timeout: 30000 });
  check("Panel 3: kode tampil di Monaco (worker lokal, CSP)", true);
  // Fase 11: edit berkas di Monaco → Ctrl+S menulis ke disk (atomik); perubahan dari luar terdeteksi sebagai konflik.
  const kodePath = join(docDir, "kode.ts");
  await docsPanel.getByRole("button", { name: "Edit berkas" }).click();
  const editor = docsPanel.getByTestId("monaco");
  await editor.locator(".view-lines").click();
  await page.keyboard.press("Control+End");
  await page.keyboard.type("// diedit dari Zandor");
  await docsPanel.getByRole("button", { name: "Simpan (Ctrl+S)" }).waitFor({ timeout: 5000 });
  await page.keyboard.press("Control+s");
  await waitFor(
    "kode.ts tersimpan di disk",
    () => readFileSync(kodePath, "utf8"),
    (v) => v.includes("// diedit dari Zandor"),
    8000,
  );
  check(
    "Panel 3: edit di Monaco + Ctrl+S menulis ke berkas asli di disk",
    readFileSync(kodePath, "utf8").startsWith("export const nilai"),
  );
  await page.keyboard.type(" lagi");
  writeFileSync(kodePath, "export const nilai: number = 99; // diubah program lain\n");
  await page.keyboard.press("Control+s");
  await page
    .getByRole("alertdialog")
    .getByText(/berubah di disk/)
    .waitFor({ timeout: 8000 });
  check("Panel 3: berkas yang diubah program lain terdeteksi (konflik) sebelum menimpa", true);
  await page.getByRole("alertdialog").getByRole("button", { name: "Batal" }).click();
  check(
    "Panel 3: konflik dibatalkan → isi dari program lain tidak tertimpa",
    readFileSync(kodePath, "utf8").includes("99"),
  );
  await docsPanel.getByRole("button", { name: "Selesai edit" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Buang perubahan" }).click();
  await page.locator('.tree__file[title="data.csv"]').click();
  await docsPanel.getByTestId("table").getByText("satu").waitFor({ timeout: 15000 });
  check("Panel 3: CSV menjadi tabel", true);
  await page.locator('.tree__file[title="gambar.png"]').click();
  await docsPanel.getByTestId("image").waitFor({ timeout: 15000 });
  const imgOk = await docsPanel.getByTestId("image").evaluate((el) => (el as HTMLImageElement).naturalWidth === 8);
  check("Panel 3: gambar PNG dari disk (blob) tampil", imgOk);
  await page.locator('.tree__file[title="laporan.pdf"]').click();
  await docsPanel.getByTestId("pdf-pages").getByText("Halaman 1 dari 2").waitFor({ timeout: 30000 });
  const canvasOk = await docsPanel
    .locator(".pdfpage canvas")
    .first()
    .evaluate((el) => {
      const c = el as HTMLCanvasElement;
      const d = c.getContext("2d")?.getImageData(0, 0, c.width, c.height).data;
      if (!d) return false;
      let dark = 0;
      for (let i = 0; i < d.length; i += 4) if ((d[i] ?? 255) < 128 && (d[i + 3] ?? 0) > 0) dark++;
      return dark > 20;
    });
  check("Panel 3: PDF dirender ke canvas (pdf.js worker, CSP)", canvasOk);
  const denied = await page.evaluate(async (root) => {
    const inv = (window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } })
      .__TAURI_INTERNALS__.invoke;
    const attempt = async (args: Record<string, unknown>) => {
      try {
        await inv("fs_read", args);
        return "DIBACA";
      } catch (e) {
        return String(e);
      }
    };
    return {
      traversal: await attempt({ root, path: "../../../Windows/win.ini" }),
      absolute: await attempt({ root, path: "C:\\Windows\\win.ini" }),
      unregistered: await attempt({ root: "C:\\Windows", path: "win.ini" }),
    };
  }, docDir);
  check(
    "keamanan fs_read: '..', path absolut, dan folder tak terdaftar ditolak",
    Object.values(denied).every((v) => v !== "DIBACA"),
    JSON.stringify(denied).slice(0, 160),
  );
  await page.getByRole("tab", { name: "Sesi", exact: true }).click();
  await page.evaluate(() => {
    window.location.hash = "";
  });

  // 1b1b) Panel 4 (Fase 8): terminal PTY nyata, Git nyata, browser child webview, dan isolasi IPC.
  const toolsPanel = page.getByRole("region", { name: "Tools", exact: true });
  await toolsPanel.getByRole("button", { name: "Terminal baru" }).first().click();
  const screen = toolsPanel.locator(".xterm-rows");
  await waitFor(
    "prompt terminal",
    async () => (await screen.textContent().catch(() => "")) ?? "",
    (t) => /PS .*>/.test(t),
    40000,
  );
  check("Panel 4: terminal PowerShell nyata (PTY engine) menampilkan prompt", true);
  await toolsPanel.getByTestId("terminal").click();
  await page.keyboard.type("echo zandor-uji-123; (Get-Location).Path");
  await page.keyboard.press("Enter");
  await waitFor(
    "keluaran terminal",
    async () => (await screen.textContent().catch(() => "")) ?? "",
    (t) => t.includes("zandor-uji-123") && t.includes(docProject),
    30000,
  );
  check("Panel 4: perintah dijalankan di folder project (cwd mengikuti project)", true);
  await toolsPanel.getByRole("button", { name: /^Tutup terminal/ }).click();

  await toolsPanel.getByRole("tab", { name: "Git", exact: true }).click();
  await toolsPanel.getByTestId("git-branch").getByText("main").waitFor({ timeout: 20000 });
  await toolsPanel.getByRole("button", { name: "Lihat perubahan README.md" }).waitFor({ timeout: 10000 });
  await toolsPanel.getByRole("button", { name: "Lihat perubahan baru.txt" }).waitFor({ timeout: 10000 });
  check("Panel 4: Git nyata menampilkan cabang, berkas diubah, dan berkas baru", true);
  await toolsPanel.getByRole("button", { name: "Lihat perubahan README.md" }).click();
  await toolsPanel.getByTestId("git-diff").getByText("baris tambahan dari smoke").waitFor({ timeout: 10000 });
  check("Panel 4: diff berkas dari git asli tampil", true);
  await toolsPanel.getByRole("button", { name: "Stage baru.txt" }).click();
  await toolsPanel
    .getByRole("region", { name: /^Staged/ })
    .getByText("baru.txt")
    .waitFor({ timeout: 10000 });
  await toolsPanel.getByRole("textbox", { name: "Pesan commit" }).fill("commit dari smoke");
  await toolsPanel.getByRole("button", { name: "Commit", exact: true }).click();
  await toolsPanel
    .getByText(/Commit [0-9a-f]{7,} dibuat/)
    .first()
    .waitFor({ timeout: 20000 })
    .catch(async () => {
      console.log("      (diag) alert:", await toolsPanel.getByRole("alert").allTextContents());
    });
  const log = spawnSync("git", ["-C", docDir, "log", "--format=%s", "-n", "1"], { encoding: "utf8" }).stdout.trim();
  check("Panel 4: stage + commit lewat UI tercatat di repo git asli", log === "commit dari smoke", log);

  // Browser: child webview memuat server lokal (tanpa internet); IPC aplikasi tidak boleh bisa dipanggil dari halaman remote.
  const server = createServer((_req, res) => {
    res.writeHead(200, { "content-type": "text/html" });
    res.end("<title>zandor-browser-uji</title><h1>halo browser</h1>");
  });
  await new Promise<void>((r) => server.listen(0, "127.0.0.1", () => r()));
  const serverPort = (server.address() as AddressInfo).port;
  try {
    await toolsPanel.getByRole("tab", { name: "Browser", exact: true }).click();
    const addr = toolsPanel.getByRole("textbox", { name: "Alamat (URL)" });
    await addr.waitFor({ timeout: 10000 });
    await waitFor(
      "browser siap",
      async () => (await toolsPanel.getByTestId("browser-host").getAttribute("data-mode")) ?? "",
      (m) => m === "native",
      15000,
    );
    // webview anak dibuat saat panel tampil: tunggu sampai muncul sebagai target (selain UI) sebelum menavigasi
    await waitFor(
      "webview browser dibuat",
      async () =>
        browser
          ?.contexts()
          .flatMap((c) => c.pages())
          .filter((p) => !p.url().startsWith("http://tauri.localhost")).length ?? 0,
      (n) => n >= 1,
      20000,
    );
    await addr.fill(`http://127.0.0.1:${serverPort}/`);
    await addr.press("Enter");
    const remote = await waitFor(
      "halaman browser",
      async () =>
        browser
          ?.contexts()
          .flatMap((c) => c.pages())
          .find((p) => p.url().startsWith(`http://127.0.0.1:${serverPort}`)) ?? null,
      (p) => p !== null,
      30000,
    );
    check("Panel 4: browser child webview memuat halaman", (await remote?.title()) === "zandor-browser-uji");
    const geo = await page.getByTestId("browser-host").evaluate((el) => {
      const r = el.getBoundingClientRect();
      return { w: Math.round(r.width), h: Math.round(r.height) };
    });
    const view = await remote?.evaluate(() => ({ w: window.innerWidth, h: window.innerHeight }));
    check(
      "Panel 4: ukuran webview mengikuti panel",
      !!view && Math.abs(view.w - geo.w) <= 3 && Math.abs(view.h - geo.h) <= 3,
      `${JSON.stringify(view)} vs ${JSON.stringify(geo)}`,
    );
    const ipc = await remote?.evaluate(async () => {
      const t = (window as unknown as { __TAURI_INTERNALS__?: { invoke: (c: string, a: unknown) => Promise<unknown> } })
        .__TAURI_INTERNALS__;
      if (!t) return "tidak-ada-ipc";
      const tries = ["git_status", "fs_read", "browser_close", "providers_list"];
      const res: string[] = [];
      for (const c of tries) {
        try {
          await t.invoke(c, { root: "C:\\", path: "x" });
          res.push(`${c}:DIPANGGIL`);
        } catch {
          res.push(`${c}:ditolak`);
        }
      }
      return res.join(",");
    });
    check(
      "keamanan: halaman remote di browser tidak bisa memanggil perintah aplikasi",
      !String(ipc).includes("DIPANGGIL"),
      String(ipc),
    );
  } finally {
    server.close();
  }
  await toolsPanel.getByRole("tab", { name: "Terminal", exact: true }).click();

  // 1b1c) PRD §13 butir 4: minimal 3 project dan 10 sesi tanpa crash (engine nyata).
  const pageErrors: string[] = [];
  page.on("pageerror", (e) => pageErrors.push(e.message));
  await page.getByRole("tab", { name: "Sesi", exact: true }).click();
  const groups: Array<[string, number]> = [
    [docProject, 4],
    [basename(extraDirs[0] as string), 3],
    [basename(extraDirs[1] as string), 3],
  ];
  for (const [name, count] of groups) {
    for (let i = 0; i < count; i++) {
      await page.getByRole("button", { name: `Sesi baru di ${name}` }).click();
      await page.getByRole("log").waitFor({ timeout: 15000 });
    }
  }
  const sessionRows = page.locator(".tree__children .tree__session");
  await waitFor(
    "10 sesi terdaftar",
    () => sessionRows.count(),
    (n) => n >= 10,
    20000,
  );
  for (let round = 0; round < 2; round++) {
    const n = await sessionRows.count();
    for (let i = 0; i < n; i++) {
      await sessionRows.nth(i).click();
      await page.getByRole("log").waitFor({ timeout: 10000 });
    }
  }
  check(
    "3 project dan ≥10 sesi (engine nyata): dibuat dan dilewati dua putaran tanpa galat halaman",
    pageErrors.length === 0,
    `${await sessionRows.count()} sesi; ${pageErrors.slice(0, 2).join(" | ")}`,
  );

  // 1b1d) Fase 9: plugin Zandor memuat opsi memoryDir, halaman Memori & Skill bekerja di aplikasi asli.
  mkdirSync(memoryDir, { recursive: true });
  writeFileSync(
    join(memoryDir, "skill-queue.jsonl"),
    `${JSON.stringify({
      id: "sk_smoke",
      t: Date.now(),
      status: "pending",
      trigger: "correction",
      excerpt: "salah, bukan begitu",
      name: "smoke-skill",
      description: "Skill uji dari smoke native Zandor.",
      body: "1. Ini hanya skill uji.\n2. Boleh dihapus kapan saja.",
    })}\n`,
  );
  await waitFor(
    "plugin Zandor menulis meta.json (opsi memoryDir diterima engine)",
    () => existsSync(join(memoryDir, "meta.json")),
    (v) => v,
    20000,
  );
  check("plugin Zandor memuat opsi memoryDir di engine asli (meta.json dibuat)", true);
  await page.getByRole("button", { name: "Memori & skill", exact: true }).click();
  await page.getByRole("heading", { name: "Memori & Skill" }).waitFor({ timeout: 10000 });
  check(
    "halaman Memori & Skill tanpa label pratinjau (host Tauri terhubung)",
    (await page.getByText(/TIDAK dibaca engine/).count()) === 0,
  );
  await page.getByRole("switch", { name: "Aktifkan memori secara bawaan" }).click();
  await waitFor(
    "settings.json defaultEnabled",
    () => {
      try {
        return JSON.parse(readFileSync(join(memoryDir, "settings.json"), "utf8")).defaultEnabled === true;
      } catch {
        return false;
      }
    },
    (v) => v,
    8000,
  );
  check("opt-in memori lewat UI tertulis ke settings.json asli", true);
  await page.getByRole("tab", { name: "Memori", exact: true }).click();
  const memBox = page.getByRole("textbox", { name: "Memori global" });
  await memBox.fill("- kunci sk-CONTOHPALSU0123456789abcdefghij");
  await page.getByRole("button", { name: "Simpan", exact: true }).click();
  await page
    .getByRole("alert")
    .filter({ hasText: /memuat secret/ })
    .waitFor({ timeout: 5000 });
  check(
    "memori global menolak teks berisi secret",
    !existsSync(join(memoryDir, "global.md")) ||
      !readFileSync(join(memoryDir, "global.md"), "utf8").includes("sk-CONTOH"),
  );
  await memBox.fill("- Kode rahasia proyek uji adalah BIRU-77.\n");
  await page.getByRole("button", { name: "Simpan", exact: true }).click();
  await waitFor(
    "global.md tersimpan",
    () => readFileSync(join(memoryDir, "global.md"), "utf8"),
    (v) => v.includes("BIRU-77"),
    8000,
  );
  check("memori global tersimpan di berkas asli (dibaca plugin tiap giliran)", true);
  await page.getByRole("tab", { name: /Antrean skill/ }).click();
  const skillCard = page.getByRole("listitem", { name: "smoke-skill" });
  await skillCard.getByRole("button", { name: "Setujui & pasang" }).click();
  await waitFor(
    "SKILL.md terpasang",
    () => existsSync(join(smokeSkillDir, "SKILL.md")),
    (v) => v,
    8000,
  );
  check("skill disetujui → SKILL.md terpasang di folder skill engine (global)", true);
  await page.getByRole("tab", { name: "Skill terpasang" }).click();
  await page.getByRole("listitem", { name: "smoke-skill" }).waitFor({ timeout: 8000 });
  check("skill terpasang tampil di daftar", true);
  await page.getByRole("button", { name: "Kembali ke chat" }).click();
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).first().waitFor({ timeout: 10000 });

  // 1b1e) Fase 10: kustomisasi tampilan di WebView asli (token CSS), editor shortcut, dan Open VSX lewat jaringan nyata.
  originalAppearance = await page.evaluate(() => localStorage.getItem("zandor.appearance"));
  await page.evaluate(() => {
    window.location.hash = "#/appearance";
  });
  await page.getByRole("heading", { name: "Tampilan & Tema" }).waitFor({ timeout: 10000 });
  await page.getByRole("button", { name: "Kembalikan semua ke default" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Kembalikan semua ke default" }).click();
  await page.getByRole("listitem", { name: "Claude" }).getByRole("button", { name: "Pakai" }).click();
  const bgVar = await waitFor(
    "token --z-color-bg berubah",
    () => page.evaluate(() => document.documentElement.style.getPropertyValue("--z-color-bg")),
    (v) => v !== "",
    5000,
  );
  const paintedBg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  check(
    "preset tema mengubah token dan warna latar yang benar-benar dilukis WebView asli",
    paintedBg !== "" && bgVar !== "",
    `${bgVar} → ${paintedBg}`,
  );
  await page.getByRole("tab", { name: "Pintasan" }).click();
  await page.getByRole("button", { name: "Ubah cmd:app.openSettings" }).click();
  await page.keyboard.press("Control+Alt+9");
  await page.getByTestId("key-cmd:app.openSettings").getByText("Ctrl+Alt+9").waitFor({ timeout: 5000 });
  check("editor shortcut: combo baru tampil dan tersimpan di WebView asli", true);
  await page.getByRole("tab", { name: "Editor" }).click();
  await page.getByText(/hanya tersedia di aplikasi desktop/).waitFor({ state: "detached", timeout: 3000 });
  await page.getByRole("textbox", { name: "Cari tema di Open VSX" }).fill("dracula");
  await page.getByRole("button", { name: "Cari", exact: true }).click();
  const hit = page.getByRole("list", { name: "Hasil pencarian" }).getByRole("listitem").first();
  const vsx = await Promise.race([
    hit.waitFor({ timeout: 40000 }).then(() => "hasil" as const),
    page
      .getByRole("alert")
      .first()
      .waitFor({ timeout: 40000 })
      .then(() => "galat" as const),
  ]).catch(() => "timeout" as const);
  if (vsx !== "hasil") {
    console.log(
      `      (info) Open VSX tidak terjangkau dari mesin ini (${vsx}): ${await page
        .getByRole("alert")
        .first()
        .textContent()
        .catch(() => "")}`,
    );
  } else {
    check(
      "Open VSX (jaringan nyata lewat Rust): pencarian tema mengembalikan hasil",
      true,
      (await hit.getAttribute("aria-label")) ?? "",
    );
    await hit.getByRole("button", { name: /^Pasang / }).click();
    const installed = await waitFor(
      "tema editor terpasang",
      () =>
        page.evaluate(() => JSON.parse(localStorage.getItem("zandor.appearance") ?? "{}").editorThemes?.length ?? 0),
      (n) => n > 0,
      60000,
    ).catch(() => 0);
    check(
      "Open VSX: paket .vsix diunduh dan temanya terpasang (hanya JSON dibaca)",
      installed > 0,
      `${installed} tema`,
    );
  }
  await page.getByRole("button", { name: "Kembali ke chat" }).click();
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).first().waitFor({ timeout: 10000 });

  let providerHealthy = false;
  // 1b2) Provider (Fase 6): kunci di keychain, ping nyata lewat Rust. Hanya bila mesin ini punya koneksi terkonfigurasi.
  await page.evaluate(() => {
    window.location.hash = "#/providers";
  });
  await page.getByRole("heading", { name: "Provider & model" }).waitFor({ timeout: 10000 });
  const cards = page.locator(".pcard");
  const hasCards = await cards
    .first()
    .waitFor({ timeout: 5000 })
    .then(() => true)
    .catch(() => false);
  if (!hasCards) {
    console.log("      (info) tidak ada koneksi provider; pengujian keychain/ping dilewati");
  } else {
    const first = cards.first();
    await first.getByText("Kunci tersimpan").waitFor({ timeout: 5000 });
    check("koneksi provider tampil dengan kunci tersimpan (keychain)", true);
    await first.getByText("Tersambung ke engine").waitFor({ timeout: 30000 });
    check("engine mengenali provider (tersambung)", true);
    const named = first.getByRole("button", { name: `Uji ${SMOKE_MODEL}` });
    const ping = (await named.count()) > 0 ? named : first.getByRole("button", { name: /^Uji / }).first();
    // Baris model yang diuji (bukan sekadar baris pertama).
    const row =
      (await named.count()) > 0
        ? first.locator(".pmodel").filter({ hasText: SMOKE_MODEL })
        : first.locator(".pmodel").first();
    const status = row.locator(".pmodel__status [data-tone]");
    let text = "";
    // Upstream pengembang bergantian 200/500: beri hingga 3 percobaan. Yang diuji: selalu berakhir dan jujur.
    for (let attempt = 1; attempt <= 3 && !providerHealthy; attempt++) {
      await ping.click();
      await status.waitFor({ timeout: 150000 });
      text = (await status.textContent())?.trim() ?? "";
      providerHealthy = (await status.getAttribute("data-tone")) === "ok";
      if (!providerHealthy) await new Promise((r) => setTimeout(r, 5000));
    }
    check("ping model nyata lewat Rust (kunci dari keychain) berakhir dengan status jujur", true, text.slice(0, 90));
    // Upstream pengembang tidak stabil: ZANDOR_SMOKE_FORCE_CHAT=1 menjalankan fase chat nyata walau ping gagal.
    if (process.env.ZANDOR_SMOKE_FORCE_CHAT === "1") providerHealthy = true;
    // berkas provider tidak boleh memuat kunci (migrasi v1 → v2)
    const file = join(process.env.APPDATA ?? "", "id.web.zandor.app", "providers.json");
    try {
      const text = readFileSync(file, "utf8");
      check("providers.json tidak memuat kunci API", !/"apiKey"/.test(text) && /"version": 2/.test(text));
    } catch {
      check("providers.json terbaca", false);
    }
  }
  await page.evaluate(() => {
    window.location.hash = "";
  });
  await page.locator(".sidebar__status").waitFor({ timeout: 10000 });
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await page.getByText("Sesi tanpa judul").first().waitFor({ timeout: 15000 });
  check("buat sesi tanpa project lewat engine asli", true);

  // 1c) Chat nyata (Fase 5). Hanya bila mesin ini punya provider di providers.json; di CI tidak ada → dilewati.
  const modelPicker = page.getByRole("button", { name: /^Model: / });
  const hasProvider = await modelPicker
    .waitFor({ timeout: 15000 })
    .then(() => true)
    .catch(() => false);
  if (!hasProvider || !providerHealthy) {
    console.log(
      `      (info) chat nyata dilewati: ${hasProvider ? "ping provider tidak OK (upstream bermasalah)" : "tidak ada provider terkonfigurasi"}`,
    );
  } else {
    // pilih model yang diketahui berfungsi lewat pemilih model (cari, pilih)
    await modelPicker.click();
    const picker = page.getByRole("dialog").filter({ has: page.getByRole("heading", { name: "Pilih model" }) });
    await picker.getByRole("searchbox", { name: "Cari model atau provider" }).fill(SMOKE_MODEL);
    const choice = picker.getByRole("button", { name: SMOKE_MODEL }).first();
    if ((await choice.count()) > 0) {
      await choice.click();
      check("pemilih model: pilih model uji", true, SMOKE_MODEL);
    } else {
      await page.keyboard.press("Escape");
      console.log(`      (info) model ${SMOKE_MODEL} tidak ada; memakai model bawaan`);
    }
    const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
    await box.fill("Balas hanya dengan satu kata: pong");
    await box.press("Enter");
    await page
      .getByRole("article", { name: "Asisten" })
      .filter({ hasText: /pong/i })
      .first()
      .waitFor({ timeout: 180000 });
    check("chat nyata: prompt → jawaban streaming dari model", true);

    await box.fill("Buat file zandor-smoke.txt berisi kata halo (gunakan tool tulis file).");
    await box.press("Enter");
    const card = page.getByRole("alertdialog", { name: /Izin diperlukan/ });
    await card.waitFor({ timeout: 180000 });
    check("tool call tulis file meminta izin (kartu izin tampil)", true, (await card.textContent())?.slice(0, 60));
    await card.getByRole("button", { name: "Izinkan sekali" }).click();
    await approveUntilCompleted(page);
    check("setelah Izinkan sekali, tool call selesai dijalankan engine", true);
    await page.getByRole("button", { name: "Kirim" }).waitFor({ timeout: 60000 });

    // PRD §13 butir 3: terminal (shell) dan web juga lewat persetujuan izin, dengan engine + model nyata.
    await box.fill("Jalankan perintah shell ini dengan tool bash: echo zandor-bash-ok. Jawab singkat.");
    await box.press("Enter");
    const bashCard = page.getByRole("alertdialog", { name: /Izin diperlukan: bash/ });
    await bashCard.waitFor({ timeout: 180000 });
    check("tool shell (bash) meminta izin", true);
    await bashCard.getByRole("button", { name: "Izinkan sekali" }).click();
    await approveUntilCompleted(page, "bash");
    check("tool shell (bash) dijalankan setelah izin", true);
    await page.getByRole("button", { name: "Kirim" }).waitFor({ timeout: 60000 });

    const webServer = createServer((_req, res) => {
      res.writeHead(200, { "content-type": "text/plain" });
      res.end("ISI-HALAMAN-WEB-ZANDOR");
    });
    await new Promise<void>((r) => webServer.listen(0, "127.0.0.1", () => r()));
    const webPort = (webServer.address() as AddressInfo).port;
    try {
      await box.fill(`Ambil isi http://127.0.0.1:${webPort}/ dengan tool webfetch lalu kutip isinya.`);
      await box.press("Enter");
      const webCard = page.getByRole("alertdialog", { name: /Izin diperlukan: webfetch/ });
      await webCard.waitFor({ timeout: 180000 });
      check("tool web (webfetch) meminta izin", true);
      await webCard.getByRole("button", { name: "Izinkan sekali" }).click();
      await approveUntilCompleted(page, /Pencarian web/);
      check("tool web (webfetch) dijalankan setelah izin", true);
      await page.getByRole("button", { name: "Kirim" }).waitFor({ timeout: 60000 });
    } finally {
      webServer.close();
    }

    // Fase 9: memori global (opt-in aktif dari uji di atas) disuntik plugin ke prompt → model nyata menjawab dari memori.
    await box.fill("Apa kode rahasia proyek uji? Jawab hanya dengan kodenya.");
    await box.press("Enter");
    await page
      .getByRole("article", { name: "Asisten" })
      .filter({ hasText: /BIRU-77/ })
      .first()
      .waitFor({ timeout: 180000 });
    check("memori nyata: model menjawab dari memori global yang disuntik plugin (BIRU-77)", true);
    // Giliran harus benar-benar selesai (tidak ada kartu izin/dialog tertinggal) sebelum menguji kontrol jendela.
    await page.getByRole("button", { name: "Kirim" }).waitFor({ timeout: 120000 });
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
  await new Promise((r) => setTimeout(r, 4000)); // daftar sesi tiap project dimuat bertahap
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
  await new Promise((r) => setTimeout(r, 2500));
  check(
    "tidak ada sesi tersisa setelah pembersihan akhir",
    (await rows.count()) === 0,
    `${await rows.count()} tersisa`,
  );
  check("sesi uji dihapus (tidak meninggalkan sampah)", true);
  // berkas yang ditulis agent saat uji izin (best effort)
  const junk = join(process.env.APPDATA ?? "", "id.web.zandor.app", "chats", "zandor-smoke.txt");
  try {
    rmSync(junk, { force: true });
  } catch {
    // diabaikan
  }
  await second.page.evaluate((prev) => {
    if (prev === null) localStorage.removeItem("zandor.appearance");
    else localStorage.setItem("zandor.appearance", prev);
  }, originalAppearance);
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
  // pulihkan registry asli dan bersihkan folder uji dokumen
  try {
    if (originalRegistry === null) rmSync(registryFile, { force: true });
    else writeFileSync(registryFile, originalRegistry);
    for (const [f, text] of memoryBackup) {
      if (text === null) rmSync(join(memoryDir, f), { force: true });
      else writeFileSync(join(memoryDir, f), text);
    }
    rmSync(smokeSkillDir, { recursive: true, force: true });
    rmSync(docDir, { recursive: true, force: true });
    for (const d of extraDirs) rmSync(d, { recursive: true, force: true });
  } catch {
    // best effort
  }
  // pastikan tidak ada sisa proses engine/webview milik aplikasi
  spawnSync("taskkill", ["/F", "/IM", "zandor-desktop.exe"], { stdio: "ignore" });
}

const failed = results.filter(([, pass]) => !pass);
console.log(`\n${results.length - failed.length}/${results.length} lulus`);
process.exit(failed.length ? 1 : 0);

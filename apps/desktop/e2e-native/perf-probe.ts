// Probe performa NATIVE: RAM per proses, FPS/frame time, dan long task saat idle, 1 streaming, dan 4 streaming paralel
// di 4 project berbeda (model nyata). Hanya mengukur; tidak menguji kebenaran.
// Pemakaian (Windows): bun e2e-native/perf-probe.ts <path zandor-desktop.exe>
// Build UI produksi tertanam: `bun run tauri:build:dev` (identifier dev, data prod tidak tersentuh).
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { chromium, type Page } from "@playwright/test";

const exePath = process.argv[2];
if (!exePath) {
  console.error("pemakaian: bun e2e-native/perf-probe.ts <path zandor-desktop.exe>");
  process.exit(2);
}
const PORT = 9335;
const APP_ID = process.env.ZANDOR_SMOKE_APP_ID ?? "id.web.zandor.dev";
const MODEL = process.env.ZANDOR_SMOKE_MODEL ?? "DeepSeek";
const N_PROJECTS = 4;
const MEASURE_MS = 15000;
const PROMPT =
  "Tanpa memakai tool apa pun, tulis penjelasan panjang (sekitar 900 kata) tentang algoritma sorting: " +
  "quicksort, mergesort, heapsort, timsort. Sertakan satu blok kode TypeScript untuk tiap algoritma dan satu tabel " +
  "perbandingan kompleksitas dalam Markdown.";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function waitFor<T>(what: string, fn: () => T | Promise<T>, ok: (v: T) => boolean, ms = 15000): Promise<T> {
  const t0 = Date.now();
  let last: T | undefined;
  while (Date.now() - t0 < ms) {
    last = await fn();
    if (ok(last)) return last;
    await sleep(200);
  }
  throw new Error(`timeout menunggu: ${what}; terakhir: ${JSON.stringify(last)}`);
}

// ---- Registry project uji (dicadangkan & dipulihkan) ----------------------------------------------------------
const APPDATA = join(process.env.APPDATA ?? "", APP_ID);
const registryFile = join(APPDATA, "projects.json");
const registryBefore = existsSync(registryFile) ? readFileSync(registryFile, "utf8") : null;
const longTmp = realpathSync.native(tmpdir());
const dirs = Array.from({ length: N_PROJECTS }, (_, i) => mkdtempSync(join(longTmp, `zandor-perf-${i}-`)));
for (const d of dirs) writeFileSync(join(d, "README.md"), "# Proyek uji performa\n");
{
  let reg: { projects?: unknown[]; [k: string]: unknown } = {
    version: 1,
    projects: [],
    selection: { key: null, sessionId: null },
  };
  try {
    if (registryBefore) reg = JSON.parse(registryBefore);
  } catch {}
  reg.projects = [
    ...(reg.projects ?? []),
    ...dirs.map((d, i) => ({ id: `perf-${i}`, path: d, name: basename(d), color: null, addedAt: Date.now() })),
  ];
  mkdirSync(APPDATA, { recursive: true });
  writeFileSync(registryFile, JSON.stringify(reg));
}

// ---- RAM per proses (pohon proses aplikasi) --------------------------------------------------------------------
type Mem = Record<string, { count: number; wsMB: number; privMB: number; cpuS: number }>;
function memTree(rootPid: number): Mem {
  const ps = `
$all = Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,CommandLine
$ids = New-Object System.Collections.Generic.HashSet[int]; [void]$ids.Add(${rootPid})
do { $n = $ids.Count; foreach ($p in $all) { if ($ids.Contains([int]$p.ParentProcessId)) { [void]$ids.Add([int]$p.ProcessId) } } } while ($ids.Count -ne $n)
$out = foreach ($p in $all) { if ($ids.Contains([int]$p.ProcessId)) {
  $g = Get-Process -Id $p.ProcessId -EA SilentlyContinue; if (-not $g) { continue }
  $kind = $p.Name; if ($p.CommandLine -match '--type=([a-z-]+)') { $kind = "webview:" + $Matches[1] } elseif ($p.Name -like 'msedgewebview2*') { $kind = 'webview:browser' }
  [pscustomobject]@{k=$kind; ws=$g.WorkingSet64; pv=$g.PrivateMemorySize64; cpu=$g.CPU} } }
$out | ConvertTo-Json -Compress`;
  const r = spawnSync("powershell", ["-NoProfile", "-Command", ps], { encoding: "utf8" });
  let rows: Array<{ k: string; ws: number; pv: number; cpu: number }> = [];
  try {
    const j = JSON.parse(r.stdout.trim() || "[]");
    rows = Array.isArray(j) ? j : [j];
  } catch {}
  const m: Mem = {};
  for (const x of rows) {
    const e = m[x.k] ?? { count: 0, wsMB: 0, privMB: 0, cpuS: 0 };
    e.count++;
    e.wsMB += Math.round(x.ws / 1048576);
    e.privMB += Math.round(x.pv / 1048576);
    e.cpuS += Math.round((x.cpu ?? 0) * 10) / 10;
    m[x.k] = e;
  }
  return m;
}
function printMem(label: string, m: Mem) {
  let ws = 0;
  let pv = 0;
  console.log(`\n[RAM] ${label}`);
  for (const [k, v] of Object.entries(m).sort((a, b) => b[1].privMB - a[1].privMB)) {
    ws += v.wsMB;
    pv += v.privMB;
    console.log(`  ${k.padEnd(24)} n=${v.count}  WS=${v.wsMB} MB  private=${v.privMB} MB  cpu=${v.cpuS.toFixed(1)} s`);
  }
  console.log(`  ${"TOTAL".padEnd(24)}      WS=${ws} MB  private=${pv} MB`);
}

// ---- Metrik frame di halaman ------------------------------------------------------------------------------------
async function measureFrames(page: Page, label: string, ms = MEASURE_MS) {
  const r = await page.evaluate(async (dur) => {
    const frames: number[] = [];
    const longs: number[] = [];
    const po = new PerformanceObserver((l) => {
      for (const e of l.getEntries()) longs.push(e.duration);
    });
    try {
      po.observe({ type: "longtask", buffered: false });
    } catch {}
    const t0 = performance.now();
    await new Promise<void>((res) => {
      const tick = (t: number) => {
        frames.push(t);
        if (t - t0 < dur) requestAnimationFrame(tick);
        else res();
      };
      requestAnimationFrame(tick);
    });
    po.disconnect();
    const dts = frames.slice(1).map((t, i) => t - (frames[i] as number));
    const sorted = [...dts].sort((a, b) => a - b);
    const q = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))] ?? 0;
    const median = q(0.5);
    return {
      fps: Math.round((dts.length / ((frames.at(-1)! - frames[0]!) / 1000)) * 10) / 10,
      medianMs: Math.round(median * 100) / 100,
      p95Ms: Math.round(q(0.95) * 100) / 100,
      p99Ms: Math.round(q(0.99) * 100) / 100,
      maxMs: Math.round((sorted.at(-1) ?? 0) * 10) / 10,
      jank: dts.filter((d) => d > median * 1.5).length,
      frames: dts.length,
      longTasks: longs.length,
      longTaskMs: Math.round(longs.reduce((a, b) => a + b, 0)),
      longestTask: Math.round(Math.max(0, ...longs)),
      domNodes: document.getElementsByTagName("*").length,
      heapMB: Math.round(
        ((performance as unknown as { memory?: { usedJSHeapSize: number } }).memory?.usedJSHeapSize ?? 0) / 1048576,
      ),
    };
  }, ms);
  console.log(
    `\n[FRAME] ${label}: ${r.fps} fps | median ${r.medianMs} ms, p95 ${r.p95Ms}, p99 ${r.p99Ms}, max ${r.maxMs} | ` +
      `frame lambat(>1,5x) ${r.jank}/${r.frames} | long task ${r.longTasks} (total ${r.longTaskMs} ms, terlama ${r.longestTask} ms) | ` +
      `DOM ${r.domNodes} | JS heap ${r.heapMB} MB`,
  );
  return r;
}

// ---- Helper UI (sama dengan memory-smoke) -----------------------------------------------------------------------
const box = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });
async function pickModel(page: Page) {
  const picker = page.getByRole("button", { name: /^Model: / });
  await picker.waitFor({ timeout: 15000 });
  const dlg = page.getByRole("dialog").filter({ has: page.getByRole("heading", { name: "Pilih model" }) });
  let opened = false;
  for (let i = 0; i < 4 && !opened; i++) {
    await picker.click({ timeout: 5000 }).catch(() => {});
    opened = await dlg
      .getByRole("searchbox", { name: "Cari model atau provider" })
      .fill(MODEL, { timeout: 5000 })
      .then(() => true)
      .catch(() => false);
    if (!opened) {
      await page.keyboard.press("Escape");
      await sleep(800);
    }
  }
  if (!opened) throw new Error("pemilih model tidak terbuka");
  const choice = dlg.getByRole("button", { name: MODEL }).first();
  if ((await choice.count()) === 0) throw new Error(`model "${MODEL}" tidak ada di pemilih`);
  await choice.click({ timeout: 5000 });
  for (let i = 0; i < 5 && (await dlg.count()) > 0; i++) {
    await page.keyboard.press("Escape");
    await sleep(300);
  }
  if (/antigravity|claude/i.test((await picker.textContent()) ?? "") && !process.env.ZANDOR_SMOKE_MODEL)
    throw new Error("model uji memakai Antigravity/Claude");
}
async function startStream(page: Page, project: string) {
  const t0 = Date.now();
  await page
    .getByRole("button", { name: `Sesi baru di ${project}` })
    .first()
    .evaluate((b: HTMLElement) => b.click());
  await box(page).waitFor({ timeout: 60000 });
  console.log(`  [respons] buka sesi baru di ${project}: ${Date.now() - t0} ms`);
  await pickModel(page);
  await box(page).fill(PROMPT);
  await box(page).press("Enter");
  // Tunggu token pertama dari asisten.
  await waitFor(
    "token pertama",
    async () =>
      (
        (await page
          .getByRole("article", { name: "Asisten" })
          .last()
          .textContent({ timeout: 1000 })
          .catch(() => "")) ?? ""
      ).length,
    (n) => n > 40,
    90000,
  );
}
async function allIdle(page: Page, ms = 240000) {
  await waitFor(
    "semua streaming selesai",
    async () => page.locator(".sdot--busy").count(),
    (n) => n === 0,
    ms,
  ).catch(() => console.log("  (info) indikator sibuk tidak terdeteksi/belum hilang; lanjut"));
}

// ---- Jalankan --------------------------------------------------------------------------------------------------
let app: ChildProcess | undefined;
let settingsBefore: string | null = null;
let pageRef: Page | undefined;
try {
  app = spawn(exePath, [], {
    stdio: "ignore",
    env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` },
  });
  const pid = app.pid as number;
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
  const page = (await waitFor(
    "halaman",
    () =>
      browser
        ?.contexts()[0]
        ?.pages()
        .find((p) => !p.url().startsWith("devtools")),
    (p) => !!p,
  )) as Page;
  pageRef = page;
  await page.waitForSelector(".titlebar", { timeout: 20000 });
  // Bahasa Indonesia untuk selektor; pengaturan asli dipulihkan di akhir.
  settingsBefore = await page.evaluate(() => localStorage.getItem("zandor.settings"));
  await page.evaluate((s) => {
    const o = s ? JSON.parse(s) : {};
    localStorage.setItem("zandor.settings", JSON.stringify({ ...o, language: "id" }));
  }, settingsBefore);
  await page.reload();
  await page.waitForSelector(".titlebar", { timeout: 20000 });
  await waitFor(
    "engine siap",
    async () =>
      (await page
        .locator(".sidebar__status")
        .textContent()
        .catch(() => "")) ?? "",
    (t) => /Engine siap/.test(t),
    60000,
  );
  const vp = await page.evaluate(() => ({ w: innerWidth, h: innerHeight, dpr: devicePixelRatio }));
  console.log(`viewport ${vp.w}x${vp.h} @${vp.dpr}x`);
  await sleep(4000);

  printMem("idle setelah start", memTree(pid));
  await measureFrames(page, "idle", 5000);

  // 1 streaming
  const names = dirs.map((d) => basename(d));
  await startStream(page, names[0] as string);
  await measureFrames(page, "1 streaming (terlihat)");
  printMem("saat 1 streaming", memTree(pid));
  await allIdle(page, Number(process.env.PERF_IDLE_MS ?? 240000));

  // 4 streaming paralel di 4 project (yang terakhir terlihat)
  for (const n of names) await startStream(page, n);
  await measureFrames(page, `${names.length} streaming paralel`);
  printMem(`saat ${names.length} streaming`, memTree(pid));
  await allIdle(page);
  await sleep(3000);
  printMem("setelah semua selesai", memTree(pid));
  await measureFrames(page, "idle dengan chat panjang", 5000);

  // Scroll riwayat chat panjang
  await page.evaluate(() => {
    const el = [...document.querySelectorAll<HTMLElement>("*")].find(
      (e) => e.scrollHeight > e.clientHeight + 200 && /auto|scroll/.test(getComputedStyle(e).overflowY),
    );
    if (!el) return;
    let dir = -1;
    const id = setInterval(() => {
      el.scrollTop += dir * 120;
      if (el.scrollTop <= 0 || el.scrollTop + el.clientHeight >= el.scrollHeight) dir *= -1;
    }, 16);
    setTimeout(() => clearInterval(id), 5000);
  });
  await measureFrames(page, "scroll chat panjang", 5000);
} catch (e) {
  console.error("GAGAL:", e);
  if (pageRef) {
    const out = process.env.PERF_SHOT ?? join(longTmp, "zandor-perf-fail.png");
    await pageRef.screenshot({ path: out }).catch(() => {});
    const btns = await pageRef
      .getByRole("button", { name: /Sesi baru/ })
      .evaluateAll((b) => b.map((x) => x.getAttribute("aria-label") ?? x.textContent))
      .catch(() => []);
    const busy = await pageRef
      .locator(".sdot--busy")
      .evaluateAll((b) =>
        b.map(
          (x) =>
            x.closest("[title],[aria-label]")?.getAttribute("aria-label") ?? x.parentElement?.textContent?.slice(0, 60),
        ),
      )
      .catch(() => []);
    console.error("screenshot:", out, "\ntombol sesi baru:", JSON.stringify(btns), "\nsibuk:", JSON.stringify(busy));
  }
  process.exitCode = 1;
} finally {
  if (pageRef && settingsBefore !== undefined)
    await pageRef
      .evaluate(
        (s) => (s === null ? localStorage.removeItem("zandor.settings") : localStorage.setItem("zandor.settings", s)),
        settingsBefore,
      )
      .catch(() => {});
  if (app?.pid) spawnSync("taskkill", ["/PID", String(app.pid), "/T", "/F"]);
  await sleep(1500);
  if (registryBefore === null) rmSync(registryFile, { force: true });
  else writeFileSync(registryFile, registryBefore);
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
}

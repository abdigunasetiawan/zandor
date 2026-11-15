// Smoke NATIVE khusus memori + skill creator dengan MODEL NYATA (ADR-063).
// Pemakaian (Windows): bun e2e-native/memory-smoke.ts <path zandor-desktop.exe>
// Memakai identifier dev (id.web.zandor.dev) supaya data prod tidak tersentuh. Data memori dicadangkan dan dipulihkan.
// Model nyata tidak deterministik: tiap skenario memberi instruksi eksplisit dan menunggu EFEK di berkas/UI, bukan kata persis.
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { chromium, type Page } from "@playwright/test";

const exePath = process.argv[2];
if (!exePath) {
  console.error("pemakaian: bun e2e-native/memory-smoke.ts <path zandor-desktop.exe>");
  process.exit(2);
}
const PORT = 9334;
const APP_ID = process.env.ZANDOR_SMOKE_APP_ID ?? "id.web.zandor.dev";
const MODEL = process.env.ZANDOR_SMOKE_MODEL ?? "DeepSeek";
const APPDATA = join(process.env.APPDATA ?? "", APP_ID);
const memoryDir = join(APPDATA, "memory");
const globalSkills = join(APPDATA, "engine", "config", "opencode", "skills");
const registryFile = join(APPDATA, "projects.json");
const SECRET_CODE = "ZEBRA-42";
const FAKE_KEY = "sk-abcdefghijklmnopqrstuvwxyz123456";

const launched: number[] = [];
const logs: string[] = [];
let pageRef: Page | undefined;
let app: ChildProcess | undefined;
const results: Array<[string, boolean, string]> = [];
const check = (name: string, pass: boolean, detail = "") => {
  results.push([name, pass, detail]);
  console.log(`${pass ? "OK   " : "GAGAL"} ${name}${detail ? ` — ${detail}` : ""}`);
};
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

const read = (p: string) => {
  try {
    return readFileSync(p, "utf8");
  } catch {
    return "";
  }
};
const projectNotes = () => {
  const dir = join(memoryDir, "projects");
  if (!existsSync(dir)) return [] as Array<[string, string]>;
  return readdirSync(dir).map((f) => [f, read(join(dir, f))] as [string, string]);
};
const settings = () => {
  try {
    return JSON.parse(read(join(memoryDir, "settings.json")) || "{}");
  } catch {
    return {};
  }
};
const queue = () =>
  read(join(memoryDir, "skill-queue.jsonl"))
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l) as Record<string, unknown>);

// ---- Cadangan data memori + registry ------------------------------------------------------------------------
const FILES = ["settings.json", "global.md", "skill-queue.jsonl"];
const backup = new Map<string, string | null>();
for (const f of FILES) backup.set(f, existsSync(join(memoryDir, f)) ? read(join(memoryDir, f)) : null);
const projectsBefore = new Set(existsSync(join(memoryDir, "projects")) ? readdirSync(join(memoryDir, "projects")) : []);
const skillsBefore = new Set(existsSync(globalSkills) ? readdirSync(globalSkills) : []);
const registryBefore = existsSync(registryFile) ? read(registryFile) : null;

// Path panjang (bukan 8.3 ABDIGU~1): kunci project UI dan plugin dihitung dari path yang sama.
const longTmp = realpathSync.native(tmpdir());
const dirA = mkdtempSync(join(longTmp, "zandor-memsmoke-a-"));
const dirB = mkdtempSync(join(longTmp, "zandor-memsmoke-b-"));
const nameA = basename(dirA);
const nameB = basename(dirB);
mkdirSync(APPDATA, { recursive: true });
{
  let reg: { projects?: unknown[]; [k: string]: unknown } = {
    version: 1,
    projects: [],
    selection: { key: null, sessionId: null },
  };
  try {
    if (registryBefore) reg = JSON.parse(registryBefore);
  } catch {
    // registry rusak: pakai kosong, yang asli dipulihkan persis di akhir
  }
  reg.projects = [
    ...(reg.projects ?? []),
    { id: "memsmoke-a", path: dirA, name: nameA, color: null, addedAt: Date.now() },
    { id: "memsmoke-b", path: dirB, name: nameB, color: null, addedAt: Date.now() },
  ];
  writeFileSync(registryFile, JSON.stringify(reg));
}
// Mulai dari kondisi bersih: memori dan skill creator mati bawaan.
mkdirSync(memoryDir, { recursive: true });
writeFileSync(join(memoryDir, "settings.json"), JSON.stringify({ defaultEnabled: false }));
rmSync(join(memoryDir, "skill-queue.jsonl"), { force: true });

// ---- Helper UI -----------------------------------------------------------------------------------------------
async function launch() {
  const child = spawn(exePath as string, [], {
    stdio: "ignore",
    env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}` },
  });
  app = child;
  launched.push(child.pid as number);
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
  page.on("console", (m) => {
    if (m.type() === "error" || m.type() === "warning") logs.push(`${m.type()}: ${m.text().slice(0, 240)}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${String(e).slice(0, 240)}`));
  await page.waitForSelector(".titlebar", { timeout: 15000 });
  await page.evaluate(() => localStorage.setItem("zandor.settings", JSON.stringify({ language: "id" })));
  await page.reload();
  await page.waitForSelector(".titlebar", { timeout: 15000 });
  return page;
}

/** Setujui kartu izin yang muncul sambil menunggu kondisi terpenuhi (model nyata sering meminta izin). */
async function until(page: Page, what: string, cond: () => boolean | Promise<boolean>, ms = 180000) {
  const deadline = Date.now() + ms;
  // Model gratis sering macet: bila lewat 75 dtk tanpa hasil, hentikan dan kirim ulang pesan terakhir (maks. 2 kali).
  let resent = 0;
  let stuckSince = Date.now();
  while (Date.now() < deadline) {
    if (await cond()) return;
    if (lastSent && resent < 2 && Date.now() - stuckSince > 75000) {
      const stop = page.getByRole("button", { name: "Hentikan", exact: true });
      if ((await stop.count()) > 0) await stop.click({ timeout: 3000 }).catch(() => {});
      await sleep(1000);
      console.log(`      (info) model macet >75 dtk; kirim ulang (${resent + 1}/2)`);
      await send(page, lastSent);
      resent++;
      stuckSince = Date.now();
    }
    const card = page.getByRole("alertdialog", { name: /Izin diperlukan/ }).first();
    if ((await card.count()) > 0)
      await card
        .getByRole("button", { name: "Izinkan sekali" })
        .click({ timeout: 3000 })
        .catch(() => {});
    await sleep(500);
  }
  const art = await page
    .getByRole("article")
    .allTextContents()
    .catch(() => []);
  if (art.length === 0)
    console.log(
      `      (diagnosis) layar: ${JSON.stringify(((await page.evaluate(() => document.body.innerText).catch(() => "")) as string).slice(0, 900))}`,
    );
  console.log(`      (diagnosis) konsol terakhir: ${JSON.stringify(logs.slice(-8))}`);
  console.log(`      (diagnosis) ${what}; percakapan: ${JSON.stringify(art).slice(0, 900)}`);
  throw new Error(`timeout menunggu: ${what}`);
}

const box = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });
let lastSent = "";
async function send(page: Page, text: string) {
  lastSent = text;
  // Pesan bisa hilang bila UI sedang render ulang; pastikan terkirim (kotak kosong + pesan tampil), ulangi bila tidak.
  for (let i = 0; i < 6; i++) {
    const typed = await box(page)
      .fill(text, { timeout: 8000 })
      .then(() => true)
      .catch(() => false);
    if (!typed) {
      await sleep(1500);
      continue;
    }
    await box(page).press("Enter");
    const sent = await page
      .getByText(text.slice(0, 30), { exact: false })
      .first()
      .waitFor({ timeout: 6000 })
      .then(() => true)
      .catch(() => false);
    if (
      sent &&
      (await box(page)
        .inputValue()
        .catch(() => "")) === ""
    )
      return;
    await sleep(1000);
  }
}
async function idle(page: Page) {
  await until(
    page,
    "model selesai (tombol Kirim kembali)",
    async () => (await page.getByRole("button", { name: "Kirim" }).count()) > 0,
  );
}
const lastAnswer = async (page: Page) =>
  (await page
    .getByRole("article", { name: "Asisten" })
    .last()
    .textContent({ timeout: 1500 })
    .catch(() => "")) ?? "";
async function newSession(page: Page, project: string | null) {
  await page
    .getByRole("button", { name: project ? `Sesi baru di ${project}` : "Sesi baru di Chat (tanpa project)" })
    .first()
    .click();
  await box(page).waitFor({ timeout: 15000 });
  await pickModel(page);
}
async function pickModel(page: Page) {
  const picker = page.getByRole("button", { name: /^Model: / });
  await picker.waitFor({ timeout: 15000 });
  const dlg = page.getByRole("dialog").filter({ has: page.getByRole("heading", { name: "Pilih model" }) });
  // Pemilih kadang terlambat/terlewat saat UI render ulang: buka dan isi kotak cari dengan percobaan ulang.
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
  if (!opened) throw new Error("pemilih model tidak terbuka setelah 4 percobaan");
  const choice = dlg.getByRole("button", { name: MODEL }).first();
  if ((await choice.count()) === 0) {
    const names = await dlg.getByRole("button").allTextContents();
    await page.keyboard.press("Escape");
    throw new Error(
      `model "${MODEL}" tidak ada di pemilih; tersedia: ${JSON.stringify(names.map((n) => n.trim().slice(0, 50)).slice(0, 25))}`,
    );
  }
  if ((await choice.count()) > 0) await choice.click({ timeout: 5000 }).catch(() => page.keyboard.press("Escape"));
  else await page.keyboard.press("Escape");
  // Pastikan dialog benar-benar tertutup sebelum mengetik (jika tidak, kotak pesan tertutup overlay).
  for (let i = 0; i < 5 && (await dlg.count()) > 0; i++) {
    await page.keyboard.press("Escape");
    await sleep(300);
  }
  await box(page).waitFor({ state: "visible", timeout: 10000 });
  const chosen = (await picker.textContent()) ?? "";
  // Model Antigravity/Claude hanya boleh bila diminta eksplisit lewat ZANDOR_SMOKE_MODEL.
  if (!process.env.ZANDOR_SMOKE_MODEL && /antigravity|claude/i.test(chosen))
    throw new Error(`model uji memakai Antigravity/Claude (${chosen.trim()})`);
}
const toggle = (page: Page, name: "Ingat memori" | "Skill creator") => page.getByRole("button", { name });

try {
  const page = await launch();
  pageRef = page;
  await page.locator(".sidebar__status").waitFor({ timeout: 20000 });
  await waitFor(
    "engine siap",
    async () =>
      (await page
        .locator(".sidebar__status")
        .textContent()
        .catch(() => "")) ?? "",
    (t) => /Engine siap/.test(t),
    40000,
  );
  check("engine siap dengan plugin Zandor", true);

  // ---- 1) Project A: nyalakan memori + skill creator lewat composer -------------------------------------------
  await newSession(page, nameA);
  await pickModel(page);
  check(
    "toggle memori mati bawaan di project baru",
    (await toggle(page, "Ingat memori").getAttribute("aria-pressed")) === "false",
  );
  await toggle(page, "Ingat memori").click();
  await toggle(page, "Skill creator").click();
  await waitFor(
    "settings.json",
    () => settings(),
    (v) => Object.values(v.skillCreatorProjects ?? {}).includes(true),
    8000,
  ).catch(() => {});
  const s1 = settings();
  const onProjects = Object.values(s1.projects ?? {}).filter((v) => v === true).length;
  const skillOn = Object.values(s1.skillCreatorProjects ?? {}).filter((v) => v === true).length;
  check(
    "toggle composer menulis settings.json (memori + skill creator, per project)",
    onProjects === 1 && skillOn === 1,
    JSON.stringify(s1).slice(0, 160),
  );

  // ---- 2) Model menulis memori project lewat memory_write ------------------------------------------------------
  await send(
    page,
    `Simpan fakta ini ke memori project dengan tool memory_write (scope project): kode nama proyek ini adalah ${SECRET_CODE}. Setelah itu jawab "tersimpan".`,
  );
  await until(page, `memori project memuat ${SECRET_CODE}`, () =>
    projectNotes().some(([, t]) => t.includes(SECRET_CODE)),
  );
  check("model nyata memakai memory_write: fakta masuk berkas memori project", true);
  await idle(page);
  const notes = projectNotes().filter(([f]) => !projectsBefore.has(f));
  check(
    "fakta hanya di berkas project A (tidak ke global)",
    !read(join(memoryDir, "global.md")).includes(SECRET_CODE) && notes.length === 1,
  );

  // ---- 3) Rahasia ditolak ---------------------------------------------------------------------------------------
  await send(
    page,
    `Gunakan tool memory_write (scope global) untuk menyimpan teks ini persis: kunci API saya ${FAKE_KEY}. Laporkan hasil tool.`,
  );
  await idle(page);
  check(
    "secret ditolak: kunci palsu tidak pernah masuk berkas memori",
    !read(join(memoryDir, "global.md")).includes(FAKE_KEY) && !projectNotes().some(([, t]) => t.includes(FAKE_KEY)),
  );

  // ---- 4) Lintas sesi di project yang sama (recall) -----------------------------------------------------------------
  await newSession(page, nameA);
  check(
    "sesi baru di project A mengikuti toggle project (memori aktif)",
    (await waitFor(
      "toggle memori",
      async () => toggle(page, "Ingat memori").getAttribute("aria-pressed"),
      (v) => v === "true",
      8000,
    ).catch(() => "")) === "true",
  );
  await send(page, "Apa kode nama proyek ini? Jawab hanya dengan kodenya.");
  await until(page, "jawaban memuat kode", async () => (await lastAnswer(page)).includes(SECRET_CODE), 120000);
  check("ingatan lintas sesi: sesi baru project A menjawab dari memori", true);
  await idle(page);

  // ---- 5) memory_search ------------------------------------------------------------------------------------------------
  await send(page, "Gunakan tool memory_search dengan kata kunci 'kode nama' lalu sebutkan hasil yang ditemukan.");
  await until(
    page,
    "jawaban lewat langkah tool memuat hasil pencarian memori",
    async () => {
      const t = await lastAnswer(page);
      // Langkah tool terlipat di dalam "Proses · N langkah"; hasil tool dikutip model.
      return t.includes("Proses") && t.includes(SECRET_CODE);
    },
    120000,
  );
  check("model nyata memanggil memory_search dan tool selesai", true);
  await idle(page);

  // ---- 6) Isolasi: project B (memori mati) tidak melihat memori A -----------------------------------------------
  await newSession(page, nameB);
  check("project B: memori mati bawaan", (await toggle(page, "Ingat memori").getAttribute("aria-pressed")) === "false");
  await send(page, "Apa kode nama proyek ini? Jika tidak tahu, jawab 'tidak tahu'.");
  await idle(page);
  check(
    "isolasi: project B tidak bisa menyebut kode project A",
    !(await lastAnswer(page)).includes(SECRET_CODE),
    (await lastAnswer(page)).slice(0, 80),
  );

  // ---- 7) Sesi Chat tanpa project: per sesi, tidak ikut project --------------------------------------------------
  await newSession(page, null);
  check(
    "sesi Chat: memori mati (per sesi, tidak mewarisi project A)",
    (await toggle(page, "Ingat memori").getAttribute("aria-pressed")) === "false",
  );
  await send(page, "Apa kode nama proyek ini? Jika tidak tahu, jawab 'tidak tahu'.");
  await idle(page);
  check("isolasi: sesi Chat tidak menyebut kode project A", !(await lastAnswer(page)).includes(SECRET_CODE));

  // ---- 8) Skill creator: koreksi -> kandidat -> draf model -> persetujuan -> skill terpasang ----------------------
  await newSession(page, nameA);
  await send(page, "Tulis satu kalimat tentang cuaca.");
  await idle(page);
  await send(
    page,
    "Salah, bukan begitu. Mulai sekarang selalu awali jawaban tentang cuaca dengan kata 'Cuaca:'. Jadikan ini skill: panggil tool skill_propose dengan nama cuaca-awalan.",
  );
  await until(page, "entri antrean skill muncul", () => queue().length > 0, 60000);
  check(
    "pemicu koreksi mencatat kandidat di skill-queue.jsonl",
    queue().some((e) => e.trigger === "correction"),
  );
  await until(page, "draf skill menunggu persetujuan", () => queue().some((e) => e.status === "pending"), 180000);
  const draft = queue().find((e) => e.status === "pending");
  check(
    "model nyata mengisi draf lewat skill_propose (status menunggu persetujuan)",
    !!draft,
    String(draft?.name ?? ""),
  );
  await idle(page);

  await page.getByRole("button", { name: "Menu pengguna" }).click();
  await page.getByRole("menuitem", { name: "Memori & skill" }).click();
  await page.getByRole("heading", { name: "Memori & Skill" }).waitFor({ timeout: 10000 });
  await page.getByRole("tab", { name: "Antrean skill" }).click();
  const card = page.getByRole("listitem").filter({ hasText: "Menunggu persetujuan" }).first();
  await card.waitFor({ timeout: 15000 });
  await card.getByRole("button", { name: "Setujui & pasang" }).click();
  const skillName = String(draft?.name ?? "");
  await sleep(2500);
  console.log(
    `      (diagnosis) kartu: ${JSON.stringify(
      (
        await page
          .locator(".mempage")
          .first()
          .textContent()
          .catch(() => "")
      )?.slice(0, 700),
    )}; antrean: ${JSON.stringify(queue().map((e) => [e.name, e.status]))}`,
  );
  console.log(
    `      (diagnosis) isi folder skill: ${JSON.stringify(existsSync(globalSkills) ? readdirSync(globalSkills) : null)}`,
  );
  await waitFor(
    "SKILL.md terpasang",
    () => existsSync(join(dirA, ".opencode", "skills", skillName, "SKILL.md")),
    (v) => v,
    15000,
  );
  check("approve di UI memasang SKILL.md asli di <project A>/.opencode/skills", true, skillName);

  // ---- 9) Skill baru langsung terlihat engine tanpa restart (dispose instance, ADR-063) -------------------------
  await page.evaluate(() => {
    window.location.hash = "";
  });
  await page.locator(".sidebar__status").waitFor({ timeout: 10000 });
  // Pembeda: project B tidak di-dispose; bila B normal tetapi A diam, penyebabnya dispose instance.
  await newSession(page, nameB);
  await send(page, "Balas hanya satu kata: halo");
  const bOk = await page
    .getByRole("article", { name: "Asisten" })
    .first()
    .waitFor({ timeout: 90000 })
    .then(() => true)
    .catch(() => false);
  check("project B (tidak di-dispose) tetap bisa chat setelah approve skill", bOk);
  await newSession(page, nameA);
  const skillBody = read(join(dirA, ".opencode", "skills", skillName, "SKILL.md"))
    .split("---")
    .slice(2)
    .join(" ");
  const bodyWords = [...new Set((skillBody.match(/[A-Za-z]{8,}/g) ?? []).map((w) => w.toLowerCase()))].filter(
    (w) => !skillName.includes(w) && w !== "petunjuk",
  );
  await send(page, `Panggil tool skill dengan name "${skillName}" lalu sebutkan isi petunjuknya.`);
  await until(
    page,
    "jawaban memuat isi skill baru dari engine",
    async () => {
      const t = (await lastAnswer(page)).toLowerCase();
      return t.includes("proses") && bodyWords.some((w) => t.includes(w));
    },
    180000,
  );
  check(
    "skill yang baru disetujui terbaca engine tanpa restart (instance dispose bekerja)",
    true,
    (await lastAnswer(page)).slice(0, 140),
  );
  await idle(page);
  check("sesi tetap sehat setelah dispose: model masih menjawab", (await lastAnswer(page)).length > 0);

  // ---- 10) Persistensi: restart aplikasi, memori tetap ------------------------------------------------------------
  const settingsBefore = JSON.stringify(settings());
  await page.getByRole("button", { name: /^(Tutup jendela|Close window)$/ }).click();
  await sleep(3000);
  check(
    "data memori utuh setelah aplikasi ditutup",
    JSON.stringify(settings()) === settingsBefore && projectNotes().some(([, t]) => t.includes(SECRET_CODE)),
  );
} catch (e) {
  await pageRef?.screenshot({ path: join(tmpdir(), "memsmoke-fail.png") }).catch(() => {});
  check("smoke memori selesai tanpa galat", false, String(e));
} finally {
  if (app && app.exitCode === null) app.kill();
  try {
    if (registryBefore === null) rmSync(registryFile, { force: true });
    else writeFileSync(registryFile, registryBefore);
    for (const [f, text] of backup) {
      if (text === null) rmSync(join(memoryDir, f), { force: true });
      else writeFileSync(join(memoryDir, f), text);
    }
    for (const f of existsSync(join(memoryDir, "projects")) ? readdirSync(join(memoryDir, "projects")) : [])
      if (!projectsBefore.has(f)) rmSync(join(memoryDir, "projects", f), { force: true });
    for (const d of existsSync(globalSkills) ? readdirSync(globalSkills) : [])
      if (!skillsBefore.has(d)) rmSync(join(globalSkills, d), { recursive: true, force: true });
    rmSync(dirA, { recursive: true, force: true });
    rmSync(dirB, { recursive: true, force: true });
  } catch {
    // best effort
  }
  for (const pid of launched) spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], { stdio: "ignore" });
}

const failed = results.filter(([, pass]) => !pass);
console.log(`\n${results.length - failed.length}/${results.length} lulus`);
process.exit(failed.length ? 1 : 0);

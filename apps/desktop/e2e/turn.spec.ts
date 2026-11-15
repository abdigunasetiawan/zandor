import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });

async function openProjectSession(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await expect(composer(page)).toBeVisible();
}

async function openChatSession(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(composer(page)).toBeVisible();
}

const send = async (page: Page, text: string) => {
  await composer(page).fill(text);
  await composer(page).press("Enter");
};

test("saat bekerja: hanya 'Bekerja selama…' + langkah terakhir (terlipat), logo model berkedip; berhenti saat selesai", async ({
  page,
}) => {
  await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tugas bekerja");
  await expect(page.getByText(/Bekerja selama \d+d/)).toBeVisible({ timeout: 10_000 });
  // langkah terakhir = Mencari di internet (+ kueri); proses tidak ditumpuk
  const step = page.locator(".work__step");
  await expect(step).toContainText("Mencari di internet");
  await expect(step).toContainText("cuaca jakarta");
  await expect(step).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator(".fold--tool")).toHaveCount(0);
  // logo model berkedip saat bekerja
  await expect(page.locator(".msg__meta[data-working] .z-model-logo--working")).toBeVisible();
  // bisa dibuka manual
  await step.click();
  await expect(step).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator(".fold--tool")).toHaveCount(1);
  await step.click();
  await expect(page.locator(".fold--tool")).toHaveCount(0);
  // berhenti → tidak berkedip lagi
  await page.getByRole("button", { name: "Hentikan" }).click();
  await expect(page.locator(".z-model-logo--working")).toHaveCount(0);
  await expect(page.locator(".msg__meta[data-working]")).toHaveCount(0);
});

test("setelah disajikan: ringkasan proses terlipat, jawaban, baris info (provider • model • durasi • waktu) dan aksi", async ({
  page,
}) => {
  await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tolong baca catatan");
  const msg = page.getByRole("article", { name: "Asisten" });
  await expect(msg).toContainText("Isinya halo dunia.", { timeout: 15_000 });
  const sum = msg.getByRole("button", { name: /^Proses · 1 langkah/ });
  await expect(sum).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator(".fold--tool")).toHaveCount(0);
  await sum.click();
  await expect(page.locator(".fold--tool")).toHaveCount(1);
  // baris info + aksi selalu terlihat
  const meta = msg.locator(".msg__meta");
  await expect(meta).toContainText("Mock");
  await expect(meta).toContainText("m-satu");
  await expect(meta).toContainText(/\d+d/);
  await expect(meta).not.toHaveAttribute("data-working", "");
  await expect(msg.getByRole("button", { name: "Salin pesan" })).toBeVisible();
  await expect(msg.getByRole("button", { name: /Cabangkan/ })).toBeVisible();
  await expect(msg.getByRole("button", { name: /Ulangi|Buat ulang/ })).toBeVisible();
});

test("kartu dokumen: muncul hanya di sesi TANPA project (Chat) saat agent membuat dokumen (Buka + Unduh); bukan untuk kode", async ({
  page,
}) => {
  await mockEngine(page);
  await openChatSession(page);
  await send(page, "tolong buat dokumen ringkasan");
  // engine memecah giliran jadi 2 pesan asisten (tool call lalu jawaban) → tetap SATU giliran: satu artikel, satu baris info
  await expect(page.getByRole("article", { name: "Asisten" })).toHaveCount(1, { timeout: 15_000 });
  await expect(page.locator(".msg__foot")).toHaveCount(1);
  await expect(page.getByRole("article", { name: "Asisten" }).getByRole("button", { name: "Salin pesan" })).toHaveCount(
    1,
  );
  const card = page.getByTestId("file-card");
  await expect(card).toHaveCount(1, { timeout: 15_000 });
  await expect(card).toContainText("ringkasan.md");
  await expect(card.getByRole("button", { name: /^Buka/ })).toBeVisible();
  await expect(card.getByRole("button", { name: "Unduh ringkasan.md" })).toBeVisible();
  await expect(card).not.toContainText("agen"); // tanpa teks tambahan seperti "Tidak ada teks untuk agen"

  // berkas kode bukan dokumen → tanpa kartu
  await send(page, "tolong buat kode");
  await expect(page.getByRole("article", { name: "Asisten" })).toHaveCount(2, { timeout: 15_000 });
  await expect(page.getByTestId("file-card")).toHaveCount(1);
});

test("kartu dokumen tidak tampil di sesi project (berkasnya sudah ada di pohon berkas)", async ({ page }) => {
  await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tolong buat dokumen ringkasan");
  await expect(page.getByRole("article", { name: "Asisten" })).toBeVisible({ timeout: 15_000 });
  await expect(page.getByRole("button", { name: /^Proses · 1 langkah/ })).toBeVisible();
  await expect(page.getByTestId("file-card")).toHaveCount(0);
});

const NL = String.fromCharCode(10);
const readStep = (n: number) => ({
  type: "tool",
  tool: "read",
  callID: `c_read_${n}`,
  state: {
    status: "completed",
    input: { filePath: `C:/Work/alpha/src/f${n}.ts`, offset: 10, limit: 5 },
    output: ["<content>", "10: baris sepuluh", "11: baris sebelas", "</content>"].join(NL),
    title: `f${n}.ts`,
    metadata: { display: { lineStart: 10, lineEnd: 14, totalLines: 80 } },
    time: { start: 1, end: 2 },
  },
});
const editStep = {
  type: "tool",
  tool: "edit",
  callID: "c_edit",
  state: {
    status: "completed",
    input: { filePath: "C:/Work/alpha/src/a.ts" },
    output: "Edit applied successfully.",
    title: "a.ts",
    metadata: {
      diff: [
        "Index: a.ts",
        "=".repeat(20),
        "--- a.ts",
        "+++ a.ts",
        "@@ -7,3 +7,3 @@",
        " satu",
        "-dua",
        "+DUA-BARU",
        " tiga",
      ].join(NL),
      filediff: { file: "a.ts", patch: "", additions: 1, deletions: 1 },
    },
    time: { start: 1, end: 2 },
  },
};

test("langkah tool: berkas, baris yang dibaca, +/− dan diff bernomor baris tampil seperti agent harness", async ({
  page,
}) => {
  const engine = await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tugas bertahap");
  const sid = () => engine.sessions.at(-1)?.id ?? "";
  await expect.poll(() => sid()).not.toBe("");
  await page.locator(".work__step").click();
  const read = page.locator(".fold--tool").first();
  await expect(read).toContainText("a.ts");
  await expect(read).toContainText("baris 1–3 dari 9");
  engine.addStep(sid(), editStep);
  const edit = page.locator('.fold--tool[data-kind="edit"]');
  await expect(edit).toContainText("+1");
  await expect(edit).toContainText("−1");
  await edit.locator(".fold__head").click();
  // diff: header hunk, nomor baris dari hunk (-dua lama baris 8, +DUA-BARU baris 8), tanpa header Index/=====
  await expect(edit.locator(".diff__line--hunk")).toContainText("@@ -7,3 +7,3 @@");
  await expect(edit.locator(".diff__line--del .diffb__no")).toHaveText("8");
  await expect(edit.locator(".diff__line--add .diffb__no")).toHaveText("8");
  await expect(edit).not.toContainText("Index:");
});

test("proses yang dibuka tidak menutup sendiri saat langkah baru datang (pesan engine baru bergabung ke giliran)", async ({
  page,
}) => {
  const engine = await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tugas bertahap");
  const sid = () => engine.sessions.at(-1)?.id ?? "";
  await expect.poll(() => sid()).not.toBe("");
  // bawaan tertutup
  const step = page.locator(".work__step");
  await expect(step).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator(".fold--tool")).toHaveCount(0);
  // buka proses dan kartu langkah pertama
  await step.click();
  const first = page.locator(".fold--tool").first();
  await first.locator(".fold__head").click();
  await expect(first.locator(".diffb__no").first()).toBeVisible();
  // langkah baru = pesan engine baru: giliran digabung, tampilan tidak dipasang ulang
  engine.addStep(sid(), readStep(2));
  engine.addStep(sid(), editStep);
  await expect(page.locator(".fold--tool")).toHaveCount(3);
  await expect(page.locator(".work__step")).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator(".fold--tool").first().locator(".fold__head")).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator(".fold--tool").first().locator(".diffb__no").first()).toBeVisible();
});

test("posisi gulir mengikuti langkah baru saat di dasar, tetapi tidak merebut saat pengguna membaca ke atas", async ({
  page,
}) => {
  const engine = await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tugas bertahap");
  const sid = () => engine.sessions.at(-1)?.id ?? "";
  await expect.poll(() => sid()).not.toBe("");
  await page.locator(".work__step").click();
  for (let i = 2; i < 30; i++) engine.addStep(sid(), readStep(i));
  const scroll = page.locator(".chat__scroll");
  const gap = () => scroll.evaluate((el) => el.scrollHeight - el.scrollTop - el.clientHeight);
  await expect(page.locator(".fold--tool")).toHaveCount(29, { timeout: 15_000 });
  await expect.poll(gap).toBeLessThan(50); // mengikuti ke dasar
  engine.addStep(sid(), readStep(99));
  await expect(page.locator(".fold--tool")).toHaveCount(30);
  await expect.poll(gap).toBeLessThan(50);
  // pengguna menggulir ke atas: langkah baru tidak menarik kembali ke bawah
  await scroll.evaluate((el) => {
    el.scrollTop = 0;
  });
  await expect.poll(() => scroll.evaluate((el) => el.scrollTop)).toBe(0);
  engine.addStep(sid(), readStep(100));
  await expect(page.locator(".fold--tool")).toHaveCount(31);
  await page.waitForTimeout(400);
  expect(await scroll.evaluate((el) => el.scrollTop)).toBeLessThan(5);
});

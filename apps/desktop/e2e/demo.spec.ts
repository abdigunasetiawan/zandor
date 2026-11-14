import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function open(page: Page) {
  const engine = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  return engine;
}
const demo = (page: Page, name: string) => page.getByRole("list", { name: /Demo/ }).getByRole("button", { name });

test("tiga sesi demo disematkan di sidebar, berlabel Demo, dan tanpa menu hapus/ganti nama", async ({ page }) => {
  await open(page);
  const list = page.getByRole("list", { name: /Demo/ });
  await expect(list.getByRole("button")).toHaveCount(3);
  for (const n of ["Demo: Trajectory & Tools", "Demo: Render Kaya", "Demo: Izin & Pertanyaan"])
    await expect(demo(page, n)).toBeVisible();
  await expect(list.getByRole("button", { name: /^Menu untuk/ })).toHaveCount(0);
});

test("demo trajectory: berpikir, tool call, pencarian web, memori, skill, diff, dan tool galat", async ({ page }) => {
  await open(page);
  await demo(page, "Demo: Trajectory & Tools").click();
  await expect(page.getByRole("note")).toContainText("bukan percakapan nyata");
  // proses terlipat di bawah ringkasan; buka untuk melihat semua langkah
  await page.getByRole("button", { name: /^Proses · 13 langkah/ }).click();
  await expect(page.locator(".fold--tool")).toHaveCount(12);
  for (const t of ["skill", "memory_search", "memory_write", "bash", "grep", "read", "edit", "todowrite"])
    await expect(page.locator(".fold__tool", { hasText: new RegExp(`^${t}$`) }).first()).toBeVisible();
  await expect(page.locator(".fold__tool", { hasText: /Pencarian web|Web search/ }).first()).toBeVisible();
  await expect(page.getByRole("button", { name: /Berpikir|Thinking/ }).first()).toBeVisible();
  await expect(page.locator('.fold--tool[data-status="error"]')).toHaveCount(1);
  // kartu tool bisa dibuka: edit menampilkan diff
  await page.locator(".fold--tool", { hasText: "edit" }).first().locator(".fold__head").click();
  await expect(page.locator(".diff__line--add").first()).toBeVisible();
  await expect(page.getByTestId("todo-list")).toBeVisible();
});

test("demo render kaya: tabel, kode, rumus, diagram, chart, gambar, dan kartu berkas", async ({ page }) => {
  await open(page);
  await demo(page, "Demo: Render Kaya").click();
  await expect(page.getByRole("region", { name: "tabel" }).first()).toContainText("claude-sonnet");
  expect(await page.getByTestId("code-block").count()).toBeGreaterThanOrEqual(4);
  await expect(page.locator(".katex").first()).toBeAttached();
  await expect(page.getByTestId("mermaid").locator("svg").first()).toBeAttached({ timeout: 25_000 });
  await expect
    .poll(async () => page.getByTestId("mermaid").locator("svg").count(), { timeout: 25_000 })
    .toBeGreaterThanOrEqual(4);
  // gambar: data URI dimuat (markdown) + kartu gambar (part berkas)
  await expect(page.locator("img.md__image")).toHaveCount(1);
  await expect(page.locator("figure.fcard--image img")).toHaveCount(1);
  // kartu berkas non-gambar
  await expect(page.locator('section.fcard[data-kind="application/pdf"]')).toContainText("laporan-q3.pdf");
  expect(await page.getByTestId("file-card").count()).toBeGreaterThanOrEqual(6);
});

test("demo izin & pertanyaan: dijawab lokal (tidak ke engine), bisa diatur ulang", async ({ page }) => {
  const engine = await open(page);
  await demo(page, "Demo: Izin & Pertanyaan").click();
  await expect(page.getByRole("alertdialog")).toHaveCount(3);
  await expect(page.getByTestId("question-card")).toBeVisible();

  await page.getByRole("alertdialog").first().getByRole("button", { name: "Izinkan sekali" }).click();
  await expect(page.getByRole("alertdialog")).toHaveCount(2);
  await expect(page.getByText(/Demo: izin dijawab/)).toBeVisible();

  const q = page.getByTestId("question-card");
  await q.getByRole("radio", { name: /Bertahap/ }).check();
  await q.getByRole("button", { name: "Berikutnya" }).click();
  await q.getByRole("checkbox", { name: /auth/ }).check();
  await q.getByRole("button", { name: "Berikutnya" }).click();
  await q.getByRole("textbox", { name: /3/ }).fill("fitur/sesi-berakhir");
  await q.getByRole("button", { name: "Kirim jawaban" }).click();
  await expect(page.getByTestId("question-card")).toHaveCount(0);
  await expect(page.getByText(/Demo: jawaban Anda: Bertahap/)).toBeVisible();

  await page.getByRole("button", { name: "Atur ulang demo" }).click();
  await expect(page.getByRole("alertdialog")).toHaveCount(3);
  await expect(page.getByTestId("question-card")).toBeVisible();
  expect(engine.prompts).toHaveLength(0);
});

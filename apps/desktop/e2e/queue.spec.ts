import { expect, type Page, test } from "@playwright/test";
import { type MockEngine, mockEngine } from "./mock-engine";

async function setup(page: Page): Promise<MockEngine> {
  const engine = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  return engine;
}

const box = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });
const dock = (page: Page) => page.getByRole("region", { name: /^Antrean pesan/ });
const items = (page: Page) => dock(page).getByRole("listitem");

async function queue(page: Page, text: string) {
  await box(page).fill(text);
  await box(page).press("Enter");
}

test("antrean pesan: antre saat agent sibuk, atur urutan (seret & keyboard), edit, hapus", async ({ page }) => {
  const engine = await setup(page);
  await queue(page, "tugas lambat"); // agent tetap sibuk sampai dihentikan
  await expect(page.getByRole("button", { name: "Hentikan", exact: true })).toBeVisible();

  await queue(page, "pesan A");
  await queue(page, "pesan B");
  await queue(page, "pesan C");
  await expect(dock(page)).toBeVisible();
  await expect(items(page)).toHaveCount(3);
  await expect(items(page).nth(0)).toContainText("pesan A");
  expect(engine.prompts).toHaveLength(1); // hanya "tugas lambat" yang terkirim; sisanya menunggu

  // seret C ke posisi pertama
  await items(page).nth(2).dragTo(items(page).nth(0));
  await expect(items(page).nth(0)).toContainText("pesan C");

  // keyboard: Alt+ArrowDown pada gagang item pertama menggesernya ke bawah
  await items(page)
    .nth(0)
    .getByRole("button", { name: /Geser untuk mengubah urutan/ })
    .press("Alt+ArrowDown");
  await expect(items(page).nth(1)).toContainText("pesan C");

  // edit
  await items(page).nth(0).getByRole("button", { name: "Ubah pesan" }).click();
  await dock(page).getByRole("textbox", { name: "Ubah pesan" }).fill("pesan A (diedit)");
  await dock(page).getByRole("textbox", { name: "Ubah pesan" }).press("Enter");
  await expect(items(page).nth(0)).toContainText("pesan A (diedit)");

  // hapus
  await items(page).nth(2).getByRole("button", { name: "Hapus dari antrean" }).click();
  await expect(items(page)).toHaveCount(2);
});

test("antrean: kirim langsung menghentikan giliran lalu mengirim item itu; sisanya terkirim berurutan", async ({
  page,
}) => {
  const engine = await setup(page);
  await queue(page, "tugas lambat");
  await expect(page.getByRole("button", { name: "Hentikan", exact: true })).toBeVisible();
  await queue(page, "pertama");
  await queue(page, "kedua");
  await expect(items(page)).toHaveCount(2);

  await items(page)
    .nth(1)
    .getByRole("button", { name: /^Kirim langsung/ })
    .click();
  await expect.poll(() => engine.prompts.map((p) => p.text)).toEqual(["tugas lambat", "kedua"]);
  await expect.poll(() => engine.prompts.map((p) => p.text)).toEqual(["tugas lambat", "kedua", "pertama"]);
  await expect(dock(page)).toHaveCount(0);
});

test("antrean: Stop menjeda antrean; Lanjutkan mengirim", async ({ page }) => {
  const engine = await setup(page);
  await queue(page, "tugas lambat");
  await expect(page.getByRole("button", { name: "Hentikan", exact: true })).toBeVisible();
  await queue(page, "menunggu");
  await page.getByRole("button", { name: "Hentikan", exact: true }).click();
  await expect(dock(page).getByText("Antrean dijeda.")).toBeVisible();
  expect(engine.prompts.map((p) => p.text)).toEqual(["tugas lambat"]);
  await dock(page).getByRole("button", { name: "Lanjutkan" }).click();
  await expect.poll(() => engine.prompts.map((p) => p.text)).toEqual(["tugas lambat", "menunggu"]);
});

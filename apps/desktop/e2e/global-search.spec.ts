// Pencarian global lintas sesi (Ctrl+Shift+F): judul + isi pesan dari engine, klik hasil membuka sesinya.
import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const send = async (page: Page, text: string) => {
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill(text);
  await box.press("Enter");
};

test("cari di semua sesi: isi pesan dari sesi lain ditemukan, disorot, dan bisa dibuka", async ({ page }) => {
  const engine = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");

  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await send(page, "tolong cek kode zebra unik");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo", { timeout: 15_000 });
  const first = engine.sessions[0]?.id ?? "";

  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await send(page, "pertanyaan lain");
  await expect.poll(() => engine.sessions.length).toBe(2);

  await page.keyboard.press("Control+Shift+F");
  const dialog = page.getByRole("dialog", { name: "Cari di semua sesi" });
  await expect(dialog).toBeVisible();
  const field = dialog.getByRole("searchbox");
  await expect(field).toBeFocused();
  await field.fill("ZEBRA");
  const results = dialog.getByRole("list", { name: "Hasil pencarian" });
  await expect(results.locator("mark").first()).toHaveText("zebra", { timeout: 15_000 });
  await expect(dialog.getByRole("status")).toContainText("1 sesi cocok");
  await expect(results.locator(".gsearch__hit")).toHaveCount(1);

  await results.locator(".gsearch__hit").first().click();
  await expect(dialog).toBeHidden();
  await expect(page.getByRole("article", { name: "Anda" }).filter({ hasText: "zebra unik" })).toBeVisible();
  expect(first).not.toBe("");

  // tanpa hasil → status jujur; juga dari palet perintah
  await page.keyboard.press("Control+k");
  await page.getByRole("combobox").fill("cari di semua");
  await page.keyboard.press("Enter");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("searchbox").fill("tidak-ada-di-mana-pun");
  await expect(dialog.getByRole("status")).toContainText("Tidak ada hasil", { timeout: 15_000 });
});

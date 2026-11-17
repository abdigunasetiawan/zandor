import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("tur pengenalan muncul tanpa project, menyimpan nama, dan tidak muncul lagi setelah dilewati", async ({
  page,
}) => {
  await mockEngine(page, { onboarding: true });
  await page.goto("/");
  const dialog = page.getByRole("dialog", { name: "Selamat datang di Zandor" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Nama tampilan").fill("Rina");
  await dialog.getByRole("button", { name: "Berikutnya" }).click();
  await expect(page.getByRole("dialog", { name: "Model AI" }).getByRole("status")).toContainText("1 model siap");
  await page.getByRole("button", { name: "Berikutnya" }).click();
  await expect(page.getByRole("dialog", { name: "Tempat kerja" })).toBeVisible();
  await page.getByRole("button", { name: "Berikutnya" }).click();
  const keys = page.getByRole("dialog", { name: "Pintasan penting" });
  await expect(keys).toContainText("Cari di semua sesi");
  await keys.getByRole("button", { name: "Selesai" }).click();
  await expect(keys).toBeHidden();

  await page.reload();
  await expect(page.getByRole("dialog", { name: "Selamat datang di Zandor" })).toBeHidden();
  await expect(page.getByText("Rina").first()).toBeVisible();

  // bisa dibuka lagi dari palet
  await page.keyboard.press("Control+K");
  await page.keyboard.type("Tur pengenalan");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog", { name: "Selamat datang di Zandor" })).toBeVisible();
});

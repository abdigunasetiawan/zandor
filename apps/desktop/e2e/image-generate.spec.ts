import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("Generate gambar dari palet: dialog jujur di luar aplikasi desktop (tanpa hasil palsu)", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  // Ctrl+K diulang sampai palet terbuka (pintasan baru terpasang setelah aplikasi selesai dimuat).
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await expect(async () => {
    await page.keyboard.press("Control+K");
    await expect(palette).toBeVisible({ timeout: 1000 });
  }).toPass();
  await palette.getByRole("combobox").fill("Generate gambar");
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Generate gambar" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Deskripsi gambar").fill("kucing oranye di bulan");
  await expect(dialog.getByRole("status")).toHaveText("Generate gambar hanya tersedia di aplikasi desktop.");
  await expect(dialog.getByRole("button", { name: "Generate", exact: true })).toBeDisabled();
  await dialog.getByRole("button", { name: "Batal" }).click();
  await expect(dialog).toBeHidden();
});

test("pengaturan kontrol agent ada di Pengaturan → Umum", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/#/settings");
  await expect(page.getByRole("switch", { name: "Agent boleh mengoperasikan Zandor" })).toBeChecked();
});

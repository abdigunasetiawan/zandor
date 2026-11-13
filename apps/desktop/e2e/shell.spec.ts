import { expect, test } from "@playwright/test";

test("kerangka aplikasi tampil jujur dengan versi build", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Zandor");
  await expect(page.getByTestId("version")).toHaveText(/^(Versi|Version) 0\.0\.\d+$/);
  await expect(page.getByText(/Fitur belum tersedia|No features yet/)).toBeVisible();
});

test("ganti bahasa ID ↔ EN dan tersimpan setelah reload", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("combobox", { name: "Bahasa" }).click();
  await page.getByRole("option", { name: "English" }).click();
  await expect(page.getByRole("heading", { level: 2 })).toHaveText("Application shell");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await page.reload();
  await expect(page.getByRole("heading", { level: 2 })).toHaveText("Application shell");
  await page.getByRole("combobox", { name: "Language" }).click();
  await page.getByRole("option", { name: "Indonesia" }).click();
  await expect(page.getByRole("heading", { level: 2 })).toHaveText("Kerangka aplikasi");
});

test("tautan ke halaman design system", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Design system" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Design system Zandor" })).toBeVisible();
});

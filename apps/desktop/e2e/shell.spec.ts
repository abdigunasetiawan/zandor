import { expect, test } from "@playwright/test";

test("kerangka aplikasi tampil jujur dengan versi build", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Zandor");
  await expect(page.getByTestId("version")).toHaveText(/^(Versi|Version) 0\.0\.\d+$/);
  await expect(page.getByText(/Fitur belum tersedia|No features yet/)).toBeVisible();
});

test("ganti bahasa ID ↔ EN dan tersimpan setelah reload", async ({ page }) => {
  await page.goto("/");
  const lang = page.getByRole("combobox");
  await lang.selectOption("en");
  await expect(page.getByRole("heading", { level: 2 })).toHaveText("Application shell");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await page.reload();
  await expect(page.getByRole("heading", { level: 2 })).toHaveText("Application shell");
  await page.getByRole("combobox").selectOption("id");
  await expect(page.getByRole("heading", { level: 2 })).toHaveText("Kerangka aplikasi");
});

import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });

async function open(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(composer(page)).toBeVisible();
}

test("izin dan pertanyaan berada di dok atas composer, bukan di dalam isi respons", async ({ page }) => {
  await mockEngine(page);
  await open(page);

  await composer(page).fill("tolong minta izin");
  await composer(page).press("Enter");
  const perm = page.getByRole("alertdialog");
  await expect(perm).toBeVisible({ timeout: 15_000 });
  // di dalam dok (region "Menunggu Anda"), di luar transkrip (log), dan tepat di atas composer
  await expect(page.getByRole("region", { name: /Menunggu Anda/ }).getByRole("alertdialog")).toBeVisible();
  await expect(page.getByRole("log").getByRole("alertdialog")).toHaveCount(0);
  const pBox = await perm.boundingBox();
  const cBox = await composer(page).boundingBox();
  if (!pBox || !cBox) throw new Error("ukuran tidak terbaca");
  expect(pBox.y + pBox.height).toBeLessThanOrEqual(cBox.y);

  await perm.getByRole("button", { name: "Izinkan sekali" }).click();
  await expect(page.getByRole("alertdialog")).toHaveCount(0);

  await expect(page.getByRole("log")).toHaveAttribute("aria-busy", "false"); // giliran izin selesai dulu
  await composer(page).fill("tolong ajukan pertanyaan");
  await composer(page).press("Enter");
  const q = page.getByTestId("question-card");
  await expect(q).toBeVisible({ timeout: 15_000 });
  await expect(page.getByRole("region", { name: /Menunggu Anda/ }).getByTestId("question-card")).toBeVisible();
  await expect(page.getByRole("log").getByTestId("question-card")).toHaveCount(0);
});

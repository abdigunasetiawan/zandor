import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function openThreeTabs(page: Page) {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  for (let i = 0; i < 3; i++) await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await expect(tabs(page)).toHaveCount(3);
}

const bar = (page: Page) => page.getByRole("navigation", { name: "Sesi terbuka" });
const tabs = (page: Page) => bar(page).locator(".stabs__tab");
const menu = async (page: Page, index: number, item: string | RegExp) => {
  await tabs(page).nth(index).locator(".stabs__label").click({ button: "right" });
  await page.getByRole("menuitem", { name: item, exact: true }).click();
};

test("klik kanan tab: tutup tab, tutup tab lainnya, tutup kanan, tutup kiri, tutup semua", async ({ page }) => {
  await openThreeTabs(page);

  // item menu lengkap; yang tidak berlaku nonaktif (tab pertama: tidak ada tab di kiri)
  await tabs(page).nth(0).locator(".stabs__label").click({ button: "right" });
  for (const name of ["Tutup tab", "Tutup tab lainnya", "Tutup tab di kanan", "Tutup tab di kiri", "Tutup semua tab"])
    await expect(page.getByRole("menuitem", { name, exact: true })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Tutup tab di kiri" })).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Escape");

  // tutup tab (satu)
  await menu(page, 1, "Tutup tab");
  await expect(tabs(page)).toHaveCount(2);

  // buka lagi sampai 3, lalu tutup di kanan dari tab pertama → tersisa 1
  await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await expect(tabs(page)).toHaveCount(3);
  await menu(page, 0, "Tutup tab di kanan");
  await expect(tabs(page)).toHaveCount(1);

  // tutup di kiri dari tab terakhir
  await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await expect(tabs(page)).toHaveCount(3);
  await menu(page, 2, "Tutup tab di kiri");
  await expect(tabs(page)).toHaveCount(1);

  // tutup lainnya
  await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await expect(tabs(page)).toHaveCount(3);
  await menu(page, 1, "Tutup tab lainnya");
  await expect(tabs(page)).toHaveCount(1);

  // tutup semua → bilah tab hilang
  await menu(page, 0, "Tutup semua tab");
  await expect(bar(page)).toHaveCount(0);
});

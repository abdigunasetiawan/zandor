import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("tanpa tab terbuka: Panel 2 menampilkan halaman sesi baru (sapaan + composer); mengirim pesan membuat sesi", async ({
  page,
}) => {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  // tidak ada tab, tidak ada bilah tab; tampil sapaan dan composer seperti sesi baru
  await expect(page.getByRole("navigation", { name: "Sesi terbuka" })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: /^Selamat (pagi|siang|sore|malam)/ })).toBeVisible();
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await expect(box).toBeVisible();
  await expect(page.getByRole("button", { name: "Kirim" })).toBeDisabled();

  // pesan pertama membuat sesi (di Chat karena belum ada project) dan langsung dijawab
  await box.fill("halo");
  await box.press("Enter");
  await expect(page.getByRole("article", { name: "Anda" })).toContainText("halo", { timeout: 15_000 });
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?", {
    timeout: 15_000,
  });
  await expect(page.getByRole("navigation", { name: "Sesi terbuka" }).locator(".stabs__tab")).toHaveCount(1);

  // menutup semua tab → kembali ke halaman bawaan
  await page
    .getByRole("button", { name: /^Tutup tab/ })
    .first()
    .click();
  await expect(page.getByRole("navigation", { name: "Sesi terbuka" })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: /^Selamat/ })).toBeVisible();
});

test("bilah tab sesi tidak menampilkan scrollbar", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  for (let i = 0; i < 6; i++) await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  const bar = page.getByRole("navigation", { name: "Sesi terbuka" });
  await expect(bar.locator(".stabs__tab")).toHaveCount(6);
  expect(await bar.evaluate((el) => getComputedStyle(el).scrollbarWidth)).toBe("none");
});

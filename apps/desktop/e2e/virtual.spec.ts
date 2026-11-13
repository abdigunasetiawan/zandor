import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("sesi panjang (600 pesan) divirtualisasi: DOM kecil, scroll ke atas dan ke bawah tetap benar", async ({
  page,
}) => {
  await mockEngine(page, { preload: 300 });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/panjang");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Sesi baru di panjang" }).click();

  const log = page.getByRole("log");
  // awalnya menempel di bawah: pesan terakhir terlihat, pesan pertama tidak ada di DOM
  await expect(log.getByText("jawaban 299", { exact: true })).toBeVisible({ timeout: 15_000 });
  await expect(log.getByText("pertanyaan 0", { exact: true })).toHaveCount(0);
  const rendered = await page.locator(".vrow").count();
  expect(rendered).toBeGreaterThan(0);
  expect(rendered).toBeLessThan(80); // bukan 600

  // ke atas: pesan pertama muncul, yang terakhir keluar dari DOM
  await log.evaluate((el) => {
    el.scrollTop = 0;
  });
  await expect(log.getByText("pertanyaan 0", { exact: true })).toBeVisible();
  await expect(log.getByText("jawaban 299", { exact: true })).toHaveCount(0);

  // kembali ke bawah
  await log.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });
  await expect(log.getByText("jawaban 299", { exact: true })).toBeVisible();
  expect(await page.locator(".vrow").count()).toBeLessThan(80);
});

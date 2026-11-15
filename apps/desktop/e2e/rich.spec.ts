import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function setup(page: Page) {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill("tampilkan kaya");
  await box.press("Enter");
  return page.getByRole("article", { name: "Asisten" }).last();
}

test("respons kaya: tabel, kode berwarna + salin, Mermaid, KaTeX, daftar tugas", async ({ page }) => {
  const msg = await setup(page);
  await expect(msg.getByRole("heading", { name: "Ringkasan" })).toBeVisible({ timeout: 15_000 });
  await expect(msg.locator("strong", { hasText: "tebal" })).toBeVisible();

  // tabel: dalam area yang bisa digulir, header dan sel benar
  const table = msg.getByRole("region", { name: "tabel" }).locator("table");
  await expect(table.locator("thead th")).toHaveText(["Nama", "Nilai"]);
  await expect(table.locator("tbody tr").nth(1).locator("td")).toHaveText(["Jeruk", "10"]);

  // kode: label bahasa, pewarnaan sintaks, tombol salin menyalin teks mentah
  const code = msg.getByTestId("code-block").first();
  await expect(code.locator(".cblock__lang")).toHaveText("ts");
  await expect(code.locator(".hljs-keyword").first()).toBeVisible();
  await expect(code.locator(".hljs-comment")).toContainText("komentar");
  const color = await code
    .locator(".hljs-keyword")
    .first()
    .evaluate((el) => getComputedStyle(el).color);
  const plain = await code.locator("code").evaluate((el) => getComputedStyle(el).color);
  expect(color).not.toBe(plain);
  await code.getByRole("button", { name: "Salin kode" }).click();
  // Windows menormalkan akhir baris di clipboard menjadi CRLF
  expect((await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/g, "\n")).toBe(
    "const a: number = 1;\nfunction hitung(x: number) { return x * 2; } // komentar",
  );

  // Mermaid dirender menjadi SVG setelah jawaban selesai
  await expect(msg.getByTestId("mermaid").locator("svg")).toBeVisible({ timeout: 20_000 });
  // KaTeX
  await expect(msg.locator(".katex").first()).toBeVisible();
  // daftar tugas
  await expect(msg.locator('input[type="checkbox"]')).toHaveCount(2);
  await expect(msg.locator('input[type="checkbox"]').first()).toBeChecked();
});

test("respons kaya: konten berbahaya tidak aktif (skrip, tautan javascript:, gambar luar)", async ({ page }) => {
  const dialogs: string[] = [];
  page.on("dialog", (d) => {
    dialogs.push(d.message());
    void d.dismiss();
  });
  const requests: string[] = [];
  page.on("request", (r) => requests.push(r.url()));
  const msg = await setup(page);
  await expect(msg.getByRole("heading", { name: "Ringkasan" })).toBeVisible({ timeout: 15_000 });
  // tautan web = pemicu menu (browser Zandor / eksternal / salin), bukan navigasi langsung
  const safe = msg.locator('a[href="https://example.com"]');
  await expect(safe).toHaveText("aman");
  await expect(msg.locator("a", { hasText: "jahat" })).toHaveCount(0);
  const before = page.url();
  await safe.click();
  await expect(page.getByRole("menuitem", { name: "Buka di browser eksternal" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Buka di browser Zandor/ })).toBeVisible();
  await page.keyboard.press("Escape");
  expect(page.url()).toBe(before);
  await expect(msg.locator("script")).toHaveCount(0);
  await expect(msg.locator("img")).toHaveCount(0);
  await expect(msg.getByText("gambar luar")).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { __pwned?: number }).__pwned)).toBeUndefined();
  expect(requests.filter((u) => u.includes("evil.example"))).toEqual([]);
  expect(dialogs).toEqual([]);
});

test("blok kode panjang diciutkan dan bisa diperluas", async ({ page }) => {
  test.setTimeout(60_000); // Mermaid dan KaTeX dimuat malas; mesin yang sibuk bisa lambat
  const msg = await setup(page);
  await expect(msg.getByRole("heading", { name: "Ringkasan" })).toBeVisible({ timeout: 15_000 });
  // tunggu tata letak stabil (diagram selesai dirender) sebelum berinteraksi
  await expect(msg.getByTestId("mermaid").locator("svg")).toBeVisible({ timeout: 30_000 });
  const long = msg.getByTestId("code-block").last();
  await expect(long.locator("pre")).toHaveAttribute("data-collapsed", "");
  await long.getByRole("button", { name: /Tampilkan semua \(40 baris\)/ }).click();
  await expect(long.locator("pre")).not.toHaveAttribute("data-collapsed", "");
  await long.getByRole("button", { name: "Ciutkan" }).click();
  await expect(long.locator("pre")).toHaveAttribute("data-collapsed", "");
});

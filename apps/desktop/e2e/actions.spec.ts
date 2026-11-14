import { readFileSync } from "node:fs";
import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });

async function openSession(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(composer(page)).toBeVisible();
}

async function reply(page: Page, text = "halo") {
  await composer(page).fill(text);
  await composer(page).press("Enter");
  const msg = page.getByRole("article", { name: "Asisten" }).last();
  await expect(msg).toContainText("Halo, ada yang bisa dibantu?", { timeout: 15_000 });
  await expect(msg.getByRole("button", { name: "Salin pesan" })).toBeVisible();
  return msg;
}

test("baris aksi pesan asisten: tautan, salin, simpan sebagai, catatan, sematkan, cabangkan, ulangi", async ({
  page,
}) => {
  await mockEngine(page);
  await openSession(page);
  const msg = await reply(page);
  for (const name of [
    "Salin tautan pesan",
    "Salin pesan",
    "Simpan sebagai",
    "Simpan ke catatan",
    "Sematkan pesan",
    "Cabangkan dari sini",
    "Ulangi jawaban",
  ])
    await expect(msg.getByRole("button", { name, exact: true })).toBeVisible();
});

test("simpan sebagai: teks (Markdown), gambar (PNG), dan SVG", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  const msg = await reply(page);
  const choose = async (label: string) => {
    await msg.getByRole("button", { name: "Simpan sebagai" }).click();
    const download = page.waitForEvent("download");
    await page.getByRole("button", { name: label }).click();
    return download;
  };

  const md = await choose("Teks (Markdown)");
  expect(md.suggestedFilename()).toMatch(/^respons-\d{8}-\d{6}\.md$/);
  expect(readFileSync((await md.path()) as string, "utf8")).toContain("Halo, ada yang bisa dibantu?");

  const png = await choose("Gambar (PNG)");
  expect(png.suggestedFilename()).toMatch(/\.png$/);
  const pngBytes = readFileSync((await png.path()) as string);
  expect(pngBytes.subarray(1, 4).toString("latin1")).toBe("PNG"); // tanda tangan PNG
  expect(pngBytes.length).toBeGreaterThan(500);

  const svg = await choose("SVG");
  expect(svg.suggestedFilename()).toMatch(/\.svg$/);
  expect(readFileSync((await svg.path()) as string, "utf8")).toContain("<svg");
});

test("simpan ke catatan: global dan sesi, dengan konfirmasi", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  const msg = await reply(page);
  await msg.getByRole("button", { name: "Simpan ke catatan" }).click();
  await page.getByRole("menuitem", { name: "Catatan global" }).click();
  await expect(page.getByText("Disimpan ke Catatan global")).toBeVisible();
  await msg.getByRole("button", { name: "Simpan ke catatan" }).click();
  await page.getByRole("menuitem", { name: "Catatan sesi" }).click();
  await expect(page.getByText("Disimpan ke Catatan sesi")).toBeVisible();
  // sesi di luar project: tidak ada opsi "Catatan project"
  await msg.getByRole("button", { name: "Simpan ke catatan" }).click();
  await expect(page.getByRole("menuitem", { name: "Catatan project" })).toHaveCount(0);
});

test("sematkan pesan: tampil di bar sematan, bertahan setelah muat ulang, bisa dilepas", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  const msg = await reply(page);
  const pin = msg.getByRole("button", { name: "Sematkan pesan" });
  await pin.click();
  await expect(msg.getByRole("button", { name: "Lepas sematan pesan" })).toHaveAttribute("aria-pressed", "true");
  const bar = page.locator(".pins");
  await expect(bar).toContainText("Pesan disematkan (1)");
  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(page.locator(".pins")).toContainText("Pesan disematkan (1)");
  await page.locator(".pins summary").click();
  await page.locator(".pins").getByRole("button", { name: "Lepas sematan pesan" }).click();
  await expect(page.locator(".pins")).toHaveCount(0);
});

test("tautan pesan: disalin lalu dibuka dari palet perintah (pindah sesi dan sorot pesan)", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await mockEngine(page);
  await openSession(page);
  const msg = await reply(page);
  await msg.getByRole("button", { name: "Salin tautan pesan" }).click();
  const link = await page.evaluate(() => navigator.clipboard.readText());
  expect(link).toMatch(/^zandor:\/\/chat\/chat\/[^/]+\/[^/]+$/);

  // pindah ke sesi lain, lalu buka tautan lewat palet
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(page.getByRole("article", { name: "Asisten" })).toHaveCount(0);
  await page.keyboard.press("Control+k");
  await page.getByRole("combobox", { name: /Command palette|palet/i }).fill(link);
  await page.getByRole("option", { name: "Buka pesan dari tautan" }).click();
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?");
});

import { expect, type Page, test } from "@playwright/test";
import { type MockOptions, mockEngine } from "./mock-engine";

async function openSession(page: Page, opts?: MockOptions) {
  const engine = await mockEngine(page, opts);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.getByRole("log")).toBeVisible();
  return engine;
}

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });
const send = async (page: Page, text: string) => {
  await composer(page).fill(text);
  await composer(page).press("Enter");
};
const assistant = (page: Page) => page.getByRole("article", { name: "Asisten" });

async function chooseOption(page: Page, label: string, option: string) {
  await page.getByRole("combobox", { name: label }).click();
  await page.getByRole("option", { name: option }).click();
}

test("Salin pesan menaruh jawaban di clipboard", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openSession(page);
  await send(page, "halo");
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?", { timeout: 10_000 });
  await assistant(page).getByRole("button", { name: "Salin pesan" }).click();
  await expect(page.getByText("Tersalin")).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("Halo, ada yang bisa dibantu?");
});

test("mode izin 'Setujui edit otomatis' menjawab izin edit tanpa kartu", async ({ page }) => {
  const engine = await openSession(page);
  await chooseOption(page, "Mode izin", "Setujui edit otomatis");
  await send(page, "ubah file, butuh izin");
  await expect.poll(() => engine.permissionReplies.length, { timeout: 10_000 }).toBe(1);
  expect(engine.permissionReplies[0]?.reply).toBe("once");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect(assistant(page)).toContainText("Sudah diubah.", { timeout: 10_000 });
});

test("varian reasoning dikirim bersama prompt", async ({ page }) => {
  const engine = await openSession(page, { models: [{ id: "m-satu", name: "Model Satu", variants: ["low", "high"] }] });
  await chooseOption(page, "Reasoning", "high");
  await send(page, "halo");
  await expect.poll(() => engine.variants.length).toBe(1);
  expect(engine.variants[0]).toBe("high");
});

test("lampiran: pilih berkas → chip → terkirim sebagai file part; bisa dihapus", async ({ page }) => {
  const engine = await openSession(page);
  const chooser = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Lampirkan file" }).click();
  await (await chooser).setFiles([{ name: "catatan.txt", mimeType: "text/plain", buffer: Buffer.from("halo") }]);
  await expect(page.locator(".composer .chip")).toContainText("catatan.txt");
  await page.getByRole("button", { name: "Hapus lampiran catatan.txt" }).click();
  await expect(page.locator(".composer .chip")).toHaveCount(0);

  const again = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Lampirkan file" }).click();
  await (await again).setFiles([{ name: "catatan.txt", mimeType: "text/plain", buffer: Buffer.from("halo") }]);
  await send(page, "tolong lihat ini");
  await expect.poll(() => engine.files.length).toBe(1);
  expect(engine.files[0]).toMatchObject({ mime: "text/plain", filename: "catatan.txt" });
  expect(engine.files[0]?.url.startsWith("data:text/plain")).toBe(true);
});

test("revert menyembunyikan pesan sesudahnya; Batalkan revert memulihkannya", async ({ page }) => {
  const engine = await openSession(page);
  await send(page, "halo");
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?", { timeout: 10_000 });
  await page.getByRole("article", { name: "Anda" }).getByRole("button", { name: "Kembalikan ke sebelum ini" }).click();
  await expect(page.getByText(/Sesi dikembalikan/)).toBeVisible();
  await expect(page.getByRole("article", { name: "Anda" })).toHaveCount(0);
  expect(engine.reverts).toHaveLength(1);
  await page.getByRole("button", { name: "Batalkan revert" }).click();
  await expect(page.getByRole("article", { name: "Anda" })).toBeVisible();
  expect(engine.unreverts).toBe(1);
});

test("Ulangi jawaban: revert lalu kirim ulang pesan yang sama", async ({ page }) => {
  const engine = await openSession(page);
  await send(page, "halo");
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?", { timeout: 10_000 });
  await assistant(page).getByRole("button", { name: "Ulangi jawaban" }).click();
  await expect.poll(() => engine.prompts.length, { timeout: 10_000 }).toBe(2);
  expect(engine.prompts[1]?.text).toBe("halo");
  expect(engine.reverts).toHaveLength(1);
  // setelah pembersihan hanya ada satu pasang pesan
  await expect(page.getByRole("article", { name: "Anda" })).toHaveCount(1, { timeout: 10_000 });
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?");
});

test("Cabangkan dari sini membuat sesi baru dan membukanya sebagai tab", async ({ page }) => {
  const engine = await openSession(page);
  await send(page, "halo");
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?", { timeout: 10_000 });
  await assistant(page).getByRole("button", { name: "Cabangkan dari sini" }).click();
  await expect.poll(() => engine.forks.length).toBe(1);
  await expect(page.getByRole("navigation", { name: "Sesi terbuka" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Cabang percakapan" })).toBeVisible();
});

test("tab sesi: beralih dan menutup", async ({ page }) => {
  await openSession(page);
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  const tabs = page.getByRole("navigation", { name: "Sesi terbuka" });
  await expect(tabs).toBeVisible();
  await expect(tabs.locator(".stabs__tab")).toHaveCount(2);
  await tabs.locator(".stabs__label").first().click();
  await expect(tabs.locator(".stabs__tab").first()).toHaveAttribute("data-active", "");
  await tabs
    .getByRole("button", { name: /^Tutup tab/ })
    .first()
    .click();
  await expect(tabs).toHaveCount(0); // tinggal satu tab → bilah tab disembunyikan
});

test("pencarian web tampil sebagai kartu khusus; diff edit bisa dibuka", async ({ page }) => {
  await openSession(page);
  await send(page, "tolong cari web cuaca");
  const card = page.locator(".fold--tool").first();
  await expect(card).toContainText("Pencarian web", { timeout: 10_000 });
  await expect(card).toContainText("cuaca jakarta");
  await send(page, "tolong buat diff");
  const edit = page.locator(".fold--tool").filter({ hasText: "edit" });
  await edit.getByRole("button").first().click();
  await expect(edit.locator(".diff__line--add")).toContainText("+baru");
  await expect(edit.locator(".diff__line--del")).toContainText("-lama");
});

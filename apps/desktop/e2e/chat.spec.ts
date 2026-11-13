import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function openSession(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.getByRole("log")).toBeVisible();
}

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });

test("kirim pesan dan terima jawaban streaming ber-Markdown; pilihan model ikut terkirim", async ({ page }) => {
  const engine = await mockEngine(page, {
    models: [
      { id: "m-satu", name: "Model Satu" },
      { id: "m-dua", name: "Model Dua" },
    ],
  });
  await openSession(page);
  await expect(page.getByRole("button", { name: /^Model: / })).toBeVisible();

  await composer(page).fill("halo");
  await page.getByRole("button", { name: "Kirim" }).click();
  await expect(page.getByRole("article", { name: "Anda" })).toContainText("halo");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?", {
    timeout: 10_000,
  });
  expect(engine.prompts[0]).toMatchObject({ text: "halo", model: { providerID: "mock", modelID: "m-satu" } });
  await expect(composer(page)).toHaveValue("");
  // selesai → tombol kembali "Kirim", meta model tampil
  await expect(page.getByRole("button", { name: "Kirim" })).toBeVisible();
  await expect(page.locator(".msg__meta")).toContainText("m-satu");
});

test("Enter mengirim, Shift+Enter baris baru", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  const box = composer(page);
  await box.fill("baris satu");
  await box.press("Shift+Enter");
  await box.type("baris dua");
  await expect(box).toHaveValue("baris satu\nbaris dua");
  expect(engine.prompts).toHaveLength(0);
  await box.press("Enter");
  await expect.poll(() => engine.prompts.length).toBe(1);
  expect(engine.prompts[0]?.text).toBe("baris satu\nbaris dua");
});

test("tool call tampil sebagai kartu yang bisa dibuka", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  await composer(page).fill("tolong baca catatan");
  await composer(page).press("Enter");
  const card = page.locator(".fold--tool");
  await expect(card).toContainText("read", { timeout: 10_000 });
  await expect(card).toContainText("Selesai");
  await card.locator(".fold__head").click();
  await expect(card).toContainText("halo dunia");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Isinya halo dunia.");
});

test("kartu izin: Izinkan sekali dikirim ke engine lalu kartu hilang dan jawaban lanjut", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  await composer(page).fill("ubah file, butuh izin");
  await composer(page).press("Enter");
  const card = page.getByRole("alertdialog", { name: /Izin diperlukan: edit/ });
  await expect(card).toBeVisible({ timeout: 10_000 });
  await expect(card).toContainText("catatan.txt");
  await card.getByRole("button", { name: "Izinkan sekali" }).click();
  await expect(card).toHaveCount(0);
  expect(engine.permissionReplies[0]?.reply).toBe("once");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Sudah diubah.", { timeout: 10_000 });
});

test("kartu izin: Tolak", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  await composer(page).fill("hapus, butuh izin");
  await composer(page).press("Enter");
  const card = page.getByRole("alertdialog", { name: /Izin diperlukan/ });
  await card.getByRole("button", { name: "Tolak" }).click();
  await expect(card).toHaveCount(0);
  expect(engine.permissionReplies[0]?.reply).toBe("reject");
});

test("Hentikan membatalkan giliran yang sedang berjalan", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  await composer(page).fill("tugas lambat");
  await composer(page).press("Enter");
  const stop = page.getByRole("button", { name: "Hentikan" });
  await expect(stop).toBeVisible({ timeout: 10_000 });
  await stop.click();
  await expect.poll(() => engine.aborted.length).toBe(1);
  await expect(page.getByRole("button", { name: "Kirim" })).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("Dihentikan")).toBeVisible();
});

test("tanpa provider: composer memberi tahu dan Kirim nonaktif", async ({ page }) => {
  await mockEngine(page, { models: [] });
  await openSession(page);
  await expect(page.getByText(/Belum ada model/)).toBeVisible();
  await composer(page).fill("halo");
  await expect(page.getByRole("button", { name: "Kirim" })).toBeDisabled();
});

test("riwayat dimuat ulang setelah muat ulang halaman", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  await composer(page).fill("halo");
  await composer(page).press("Enter");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?", {
    timeout: 10_000,
  });
  await page.reload();
  await expect(page.getByRole("article", { name: "Anda" })).toContainText("halo", { timeout: 10_000 });
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?");
});

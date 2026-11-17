import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const MODELS = [
  { id: "m-satu", name: "Model Satu" },
  { id: "m-dua", name: "Model Dua" },
];

test("Pengaturan › Provider dan model: sembunyikan model, pilih/batalkan semua, model bawaan chat", async ({
  page,
}) => {
  await mockEngine(page, { models: MODELS });
  await page.goto("/#/settings");
  await page.getByRole("button", { name: "Provider dan model" }).first().click();

  // model bawaan chat
  // daftar model dikelompokkan per provider + kolom cari (Select standar, grup tunggal terbuka)
  const chatModel = page.getByRole("combobox", { name: "Model chat bawaan" });
  await chatModel.click();
  await expect(page.getByRole("button", { name: /Mock/, expanded: true })).toBeVisible();
  await page.getByRole("combobox", { name: "Cari" }).fill("dua");
  await page.getByRole("option", { name: "Model Dua" }).click();
  await expect(chatModel).toHaveText(/Mock · Model Dua/);

  // kelola model provider
  await page.getByRole("listitem", { name: "Mock" }).getByRole("button", { name: "Kelola model" }).click();
  await expect(page.getByText("2 dari 2 tampil di pemilih model")).toBeVisible();
  await page.getByRole("button", { name: "Batalkan semua" }).click();
  await expect(page.getByText("0 dari 2 tampil di pemilih model")).toBeVisible();
  await page.getByRole("button", { name: "Pilih semua" }).click();
  await page.getByRole("switch", { name: "Tampilkan Model Satu di pemilih model" }).click();
  await expect(page.getByText("1 dari 2 tampil di pemilih model")).toBeVisible();

  // kelompokkan menurut kemampuan
  await page.getByRole("combobox", { name: "Kelompokkan" }).click();
  await page.getByRole("option", { name: "Kemampuan" }).click();
  await expect(page.getByRole("heading", { name: /Lainnya/ })).toBeVisible();
});

test("tambah provider kustom lengkap: id, protokol, base URL", async ({ page }) => {
  await mockEngine(page, { models: MODELS });
  await page.goto("/#/settings");
  await page.getByRole("button", { name: "Provider dan model" }).first().click();
  await page.getByRole("button", { name: "Tambah koneksi" }).click();
  const dialog = page.getByRole("region", { name: /koneksi/ });
  await dialog.getByLabel("Nama", { exact: true }).fill("Gateway Saya");
  await dialog.getByLabel("ID provider").fill("gateway-saya");
  await dialog.getByRole("combobox", { name: "Protokol API" }).click();
  await page.getByRole("option", { name: "Anthropic (Messages)" }).click();
  await dialog.getByLabel(/Base URL/).fill("http://gw.test/v1");
  await dialog.getByRole("textbox", { name: "ID model" }).fill("gw-1");
  await dialog.getByRole("button", { name: "Tambah", exact: true }).click();
  await dialog.getByRole("button", { name: "Simpan" }).click();
  await expect(page.getByRole("listitem", { name: "Gateway Saya" })).toBeVisible();
});

test("model kecil: di build web nonaktif dengan alasan jujur (hanya desktop)", async ({ page }) => {
  await mockEngine(page, { models: MODELS });
  await page.goto("/#/settings/models");
  const small = page.getByRole("combobox", { name: "Model kecil" });
  await expect(small).toBeDisabled();
  await expect(small).toContainText("Otomatis (engine memilih)");
  await expect(page.getByText("Hanya bisa diatur di aplikasi desktop")).toBeVisible();
});

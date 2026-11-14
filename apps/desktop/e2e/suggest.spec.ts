import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("saran prompt: dari isi project nyata (README hilang, ada manifes) dan berkas yang dibuka; klik mengisi kotak pesan", async ({
  page,
}) => {
  const engine = await mockEngine(page);
  engine.project.set("package.json", { text: '{"name":"x"}' });
  engine.project.set("app.ts", { text: "export const a = 1;\n" });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();

  const group = page.getByRole("list", { name: "Saran" });
  await expect(group.getByRole("button", { name: "Buat README" })).toBeVisible();
  await expect(group.getByRole("button", { name: "Jalankan tes" })).toBeVisible();
  await expect(group.getByRole("button", { name: "Jelaskan struktur project" })).toBeVisible();

  // berkas yang dibuka mengubah saran
  await page.getByRole("tab", { name: "Files" }).click();
  await page.locator('.tree__file[title="app.ts"]').click();
  await expect(group.getByRole("button", { name: "Jelaskan app.ts" })).toBeVisible();
  await expect(group.getByRole("button", { name: "Tulis tes untuk app.ts" })).toBeVisible();

  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await group.getByRole("button", { name: "Jelaskan app.ts" }).click();
  await expect(box).toHaveValue("Jelaskan apa yang dilakukan berkas app.ts dan bagian-bagian pentingnya.");
  // setelah ada teks, saran hilang (tidak menutupi pengetikan)
  await expect(group).toHaveCount(0);
});

test("saran prompt: sesi tanpa project mendapat saran umum; hilang setelah percakapan dimulai", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  const group = page.getByRole("list", { name: "Saran" });
  await expect(group.getByRole("button", { name: "Rencana minggu ini" })).toBeVisible();
  await expect(group.getByRole("button", { name: "Jelaskan struktur project" })).toHaveCount(0);

  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill("halo");
  await box.press("Enter");
  await expect(page.getByRole("article", { name: "Asisten" }).last()).toContainText("Halo", { timeout: 15_000 });
  await expect(group).toHaveCount(0);
});

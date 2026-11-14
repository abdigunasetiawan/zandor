import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function setup(page: Page, limit?: { context: number; output?: number }) {
  await mockEngine(page, { models: [{ id: "m-satu", name: "Model Satu", ...(limit ? { limit } : {}) }] });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
}

const send = async (page: Page, text: string) => {
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill(text);
  await box.press("Enter");
  await expect(page.getByRole("article", { name: "Asisten" }).last()).toContainText("Halo", { timeout: 15_000 });
};

test("info composer: model, konteks terpakai vs batas, token; rincian dibuka dan ditutup", async ({ page }) => {
  await setup(page, { context: 4000, output: 1000 });
  const info = page.getByTestId("composer-info");
  await expect(info).toContainText("Model Satu");
  await expect(info.getByTestId("cinfo-context")).toContainText("belum ada pemakaian");

  await send(page, "halo");
  await expect(info.getByTestId("cinfo-context")).toContainText("1,2k / 4k (30%)");
  await expect(info.getByTestId("cinfo-tokens")).toContainText("↑1,2k ↓80");
  await expect(info.getByRole("status")).toHaveCount(0); // 30% → tanpa peringatan

  await info.getByRole("button", { name: "Info pemakaian dan konteks" }).click();
  const detail = info.getByRole("list", { name: "Rincian pemakaian" }).or(info.locator("dl"));
  await expect(detail).toContainText("Konteks tersisa");
  await expect(detail).toContainText("2,8k");
  await expect(detail).toContainText("Batas keluaran");
  await expect(detail).toContainText("1k");
  await expect(detail).toContainText("tidak diekspos engine");
  await info.getByRole("button", { name: "Info pemakaian dan konteks" }).click();
  await expect(info.locator("dl")).toHaveCount(0);
});

test("info composer: peringatan saat konteks hampir penuh (≥ 80% dan ≥ 95%)", async ({ page }) => {
  await setup(page, { context: 1500 }); // 1200/1500 = 80%
  await send(page, "halo");
  const info = page.getByTestId("composer-info");
  await expect(info).toHaveAttribute("data-level", "warn");
  await expect(info.getByRole("status")).toContainText("hampir penuh");
});

test("info composer: batas model tidak diketahui → tidak ada persentase palsu", async ({ page }) => {
  await setup(page);
  await send(page, "halo");
  const info = page.getByTestId("composer-info");
  await expect(info.getByTestId("cinfo-context")).toContainText("batas model tidak diketahui");
  await expect(info.locator("progress")).toHaveCount(0);
  await expect(info).toHaveAttribute("data-level", "ok");
});

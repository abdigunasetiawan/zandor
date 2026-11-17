import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function setup(page: Page, limit?: { context: number; output?: number }) {
  await mockEngine(page, { models: [{ id: "m-satu", name: "Model Satu", ...(limit ? { limit } : {}) }] });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
}

const send = async (page: Page, text: string) => {
  // composer baru bisa mengirim setelah daftar model dimuat; di bawah beban ini bisa lebih lambat dari klik
  await expect(page.getByRole("button", { name: /^Model: / })).toBeVisible();
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill(text);
  await box.press("Enter");
  await expect(page.getByRole("article", { name: "Anda" }).last()).toContainText(text);
  await expect(page.getByRole("article", { name: "Asisten" }).last()).toContainText("Halo", { timeout: 15_000 });
};

const ring = (page: Page) =>
  page.getByTestId("composer-info").getByRole("button", { name: /^Info pemakaian dan konteks/ });
const popup = (page: Page) => page.getByRole("dialog");

test("info composer: cincin konteks; popup berisi model, konteks vs batas, token, rincian; bisa dibuka dan ditutup", async ({
  page,
}) => {
  await setup(page, { context: 4000, output: 1000 });
  await ring(page).click();
  await expect(popup(page)).toContainText("Model Satu");
  await expect(popup(page).getByTestId("cinfo-context")).toContainText("belum ada pemakaian");
  await page.keyboard.press("Escape");
  await expect(popup(page)).toHaveCount(0);

  await send(page, "halo");
  await expect(ring(page)).toHaveAccessibleName(/30%/);
  await ring(page).click();
  const detail = popup(page);
  await expect(detail.getByTestId("cinfo-context")).toContainText("1,2k / 4k (30%)");
  await expect(detail.getByTestId("cinfo-tokens")).toContainText("↑1,2k ↓80");
  await expect(detail.getByRole("status")).toHaveCount(0); // 30% → tanpa peringatan
  await expect(detail).toContainText("Konteks tersisa");
  await expect(detail).toContainText("2,8k");
  await expect(detail).toContainText("Batas keluaran");
  await expect(detail).toContainText("1k");
  await expect(detail).toContainText("tidak diekspos engine");
  await page.keyboard.press("Escape");
  await expect(popup(page)).toHaveCount(0);
});

test("info composer: peringatan saat konteks hampir penuh (≥ 80% dan ≥ 95%)", async ({ page }) => {
  await setup(page, { context: 1500 }); // 1200/1500 = 80%
  await send(page, "halo");
  await expect(page.getByTestId("composer-info")).toHaveAttribute("data-level", "warn");
  await ring(page).click();
  await expect(popup(page).getByRole("status")).toContainText("hampir penuh");
});

test("info composer: batas model tidak diketahui → tidak ada persentase palsu", async ({ page }) => {
  await setup(page);
  await send(page, "halo");
  await expect(page.getByTestId("composer-info")).toHaveAttribute("data-level", "ok");
  await expect(ring(page)).not.toHaveAccessibleName(/%/);
  await ring(page).click();
  await expect(popup(page).getByTestId("cinfo-context")).toContainText("batas model tidak diketahui");
  await expect(popup(page).locator("progress")).toHaveCount(0);
});

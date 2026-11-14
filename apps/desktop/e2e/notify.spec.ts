import { expect, type Page, test } from "@playwright/test";
import { type MockEngine, mockEngine } from "./mock-engine";

async function setup(page: Page): Promise<MockEngine> {
  const engine = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(page.getByRole("button", { name: /^Model:/ })).toBeVisible(); // model dimuat → pesan bisa dikirim
  return engine;
}
const send = async (page: Page, text: string) => {
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill(text);
  await box.press("Enter");
};
/** Pindah ke sesi demo supaya sesi nyata tidak sedang dilihat. */
const lookAway = (page: Page) => page.getByRole("button", { name: "Demo: Trajectory & Tools" }).click();
const row = (page: Page) => page.locator(".sidebar__section--chats .tree__session").first();
const toasts = (page: Page) => page.getByRole("region", { name: "Notifikasi" });
const sid = (engine: MockEngine) => engine.sessions[0]?.id ?? "";

test("agent bekerja: titik berubah menjadi indikator kerja (bukan spinner biasa)", async ({ page }) => {
  await setup(page);
  await send(page, "tolong bekerja");
  await expect(row(page).getByRole("status", { name: "Agent bekerja" })).toBeVisible({ timeout: 15_000 });
  await expect(row(page).locator(".z-working--ripple i")).toHaveCount(3);
});

test("selesai saat tidak dilihat: toast + titik 'selesai'; membukanya menghapus penanda", async ({ page }) => {
  const engine = await setup(page);
  await send(page, "tolong bekerja");
  await expect(row(page).getByRole("status", { name: "Agent bekerja" })).toBeVisible({ timeout: 15_000 });
  await lookAway(page);
  engine.finishTurn(sid(engine));
  await expect(toasts(page).getByText("Sesi selesai")).toBeVisible({ timeout: 15_000 });
  await expect(row(page).getByRole("img", { name: "Selesai (belum dilihat)" })).toBeVisible();
  await toasts(page).getByRole("button", { name: "Buka sesi" }).click();
  await expect(row(page).getByRole("img", { name: "Siap" })).toBeVisible(); // sudah dilihat → idle
});

test("gagal karena batas penyedia: toast + titik 'batas'; galat biasa: titik 'gagal'", async ({ page }) => {
  const engine = await setup(page);
  await send(page, "tolong bekerja");
  await expect(row(page).getByRole("status", { name: "Agent bekerja" })).toBeVisible({ timeout: 15_000 });
  await lookAway(page);
  engine.finishTurn(sid(engine), { name: "APIError", message: "Rate limit exceeded, coba lagi nanti" });
  await expect(toasts(page).getByText("Batas penyedia tercapai")).toBeVisible({ timeout: 15_000 });
  await expect(row(page).getByRole("img", { name: "Batas penyedia tercapai" })).toBeVisible();
});

test("izin menunggu saat tidak dilihat: titik 'butuh tindakan' + toast", async ({ page }) => {
  await setup(page);
  await send(page, "tolong minta izin");
  await expect(page.getByRole("alertdialog")).toBeVisible({ timeout: 15_000 });
  await lookAway(page);
  await expect(row(page).getByRole("img", { name: "Butuh tindakan Anda" })).toBeVisible();
});

test("pengaturan notifikasi: toast bisa dimatikan; tidak ada toast saat sesi selesai", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/#/settings");
  await page
    .getByRole("navigation", { name: "Kategori pengaturan" })
    .getByRole("button", { name: "Notifikasi dan suara" })
    .click();
  await page.locator('[data-setting="notifyToast"]').getByRole("switch").click();
  await page.goto("/#/");
  const engine = await mockEngine(page);
  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(page.getByRole("button", { name: /^Model:/ })).toBeVisible();
  await send(page, "tolong bekerja");
  await expect(row(page).getByRole("status", { name: "Agent bekerja" })).toBeVisible({ timeout: 15_000 });
  await lookAway(page);
  engine.finishTurn(sid(engine));
  await expect(row(page).getByRole("img", { name: "Selesai (belum dilihat)" })).toBeVisible({ timeout: 15_000 });
  await expect(toasts(page).getByText("Sesi selesai")).toHaveCount(0);
});

test("halaman Pengaturan: kategori, pencarian, suara kustom, dan data", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/#/settings");
  await expect(page.getByRole("heading", { level: 1, name: "Pengaturan" })).toBeVisible();
  for (const name of [
    "Umum",
    "Tampilan",
    "Chat",
    "Notifikasi dan suara",
    "Alat",
    "Tata letak",
    "Lainnya",
    "Data dan cadangan",
  ])
    await expect(
      page.getByRole("navigation", { name: "Kategori pengaturan" }).getByRole("button", { name }),
    ).toBeVisible();
  await page.getByRole("searchbox", { name: "Cari pengaturan…" }).fill("suara");
  await expect(page.locator('[data-setting="soundDone"]')).toBeVisible();
  await expect(page.locator('[data-setting="theme"]')).toHaveCount(0);
  await expect(page.locator('[data-setting="soundCustom.done"]')).toBeVisible();
  await page.getByRole("searchbox", { name: "Cari pengaturan…" }).fill("");
  await page
    .getByRole("navigation", { name: "Kategori pengaturan" })
    .getByRole("button", { name: "Data dan cadangan" })
    .click();
  await expect(page.getByRole("button", { name: "Ekspor" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Atur ulang semua" })).toBeVisible();
});

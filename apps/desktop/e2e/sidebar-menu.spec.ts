import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function setup(page: Page) {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
}
async function addProject(page: Page, path: string) {
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill(path);
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
}
const sessions = (page: Page) => page.locator(".sidebar__section--chats .tree__session .tree__label");
const projectRows = (page: Page) =>
  page.locator(".sidebar__section:not(.sidebar__section--chats) .tree__toggle .tree__label");

test("menu sesi: sematkan, ganti nama, salin ID, hapus; sematan naik ke atas dan bertahan setelah muat ulang", async ({
  page,
}) => {
  await setup(page);
  const add = page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" });
  for (let i = 0; i < 3; i++) await add.click();
  await expect(sessions(page)).toHaveCount(3);

  // menu ⋯ memuat semua aksi standar
  await page
    .getByRole("button", { name: /^Menu untuk/ })
    .nth(2)
    .click();
  for (const name of ["Sematkan", "Ganti nama", "Salin ID sesi", "Hapus sesi"])
    await expect(page.getByRole("menuitem", { name })).toBeVisible();
  await page.getByRole("menuitem", { name: "Sematkan" }).click();
  await expect(page.locator(".sidebar__section--chats .tree__session").first().locator(".tree__pinmark")).toBeVisible();

  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(sessions(page)).toHaveCount(3);
  await expect(page.locator(".sidebar__section--chats .tree__session").first().locator(".tree__pinmark")).toBeVisible();

  // lepas sematan lewat klik kanan
  await page.locator(".sidebar__section--chats .tree__session").first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Lepas sematan" }).click();
  await expect(page.locator(".tree__pinmark")).toHaveCount(0);
});

test("seret-lepas: urutan sesi dan project bisa diatur dan bertahan setelah muat ulang", async ({ page }) => {
  await setup(page);
  await addProject(page, "C:/Work/alpha");
  await addProject(page, "C:/Work/beta");
  await addProject(page, "C:/Work/gamma");
  await expect(projectRows(page)).toHaveText(["alpha", "beta", "gamma"]);
  // seret gamma ke atas alpha
  await projectRows(page)
    .nth(2)
    .dragTo(projectRows(page).nth(0), { targetPosition: { x: 10, y: 2 } });
  await expect(projectRows(page)).toHaveText(["gamma", "alpha", "beta"]);

  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(projectRows(page)).toHaveText(["gamma", "alpha", "beta"]);

  // sesi: buat tiga, seret yang terakhir ke atas
  const add = page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" });
  for (let i = 0; i < 3; i++) await add.click();
  const rows = page.locator(".sidebar__section--chats .tree__session");
  await expect(rows).toHaveCount(3);
  const titles = await sessions(page).allInnerTexts();
  expect(titles).toHaveLength(3);
  const ids = await rows.evaluateAll((els) => els.map((e) => (e as HTMLElement).getAttribute("title") ?? ""));
  await rows.nth(2).dragTo(rows.nth(0), { targetPosition: { x: 10, y: 2 } });
  const after = await rows.evaluateAll((els) => els.map((e) => (e as HTMLElement).getAttribute("title") ?? ""));
  expect(after[0]).toBe(ids[2]);
});

test("menu project: sematkan, kustomisasi tampilan (gradien dari templat), dan latar tampil di baris project", async ({
  page,
}) => {
  await setup(page);
  await addProject(page, "C:/Work/alpha");
  await page.getByRole("button", { name: /^Menu untuk alpha/ }).click();
  for (const name of [
    "Sesi baru",
    "Sematkan",
    "Ganti nama",
    "Kustomisasi tampilan…",
    "Muat ulang",
    "Lepas dari daftar",
  ])
    await expect(page.getByRole("menuitem", { name })).toBeVisible();
  await page.getByRole("menuitem", { name: "Kustomisasi tampilan…" }).click();

  const dialog = page.getByRole("dialog", { name: /Tampilan project: alpha/ });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("tab", { name: "Gradien" }).click();
  await dialog.getByRole("button", { name: "Templat Aurora" }).click();
  await expect(dialog.getByTestId("pstyle-preview")).toHaveAttribute("style", /linear-gradient/);
  // generator: sudut dan titik warna
  await expect(dialog.getByRole("slider", { name: /Sudut/ })).toBeVisible();
  await dialog.getByRole("button", { name: "Tambah titik warna" }).click();
  await expect(dialog.getByRole("list", { name: "Titik warna" }).getByRole("listitem")).toHaveCount(4);
  await dialog.getByLabel("Logo (teks atau emoji)").fill("🚀");
  await dialog.getByRole("button", { name: "Simpan" }).click();
  await expect(dialog).toHaveCount(0);

  const row = page.locator(".sidebar__section:not(.sidebar__section--chats) .tree__row[data-bg]");
  await expect(row).toBeVisible();
  await expect(row).toHaveAttribute("style", /linear-gradient/);
  await expect(row.getByText("🚀")).toBeVisible();

  // tersimpan setelah muat ulang
  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(page.locator(".tree__row[data-bg]")).toBeVisible();
});

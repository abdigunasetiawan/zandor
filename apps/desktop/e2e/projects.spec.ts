import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function addProject(page: Page, path: string) {
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill(path);
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
}

test("tambah project, buat sesi, rename, cari, hapus; data bertahan setelah muat ulang", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(page.getByText("Belum ada project")).toBeVisible();

  await addProject(page, "C:\\Work\\alpha");
  await expect(page.getByRole("button", { name: "alpha", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.locator(".tree__label", { hasText: "Sesi tanpa judul" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Chat", exact: true })).toContainText("Project: alpha");

  // rename lewat menu ⋯ (tombol menu bernama sama dengan judul sesi; yang terakhir adalah tombol menu)
  await page.getByRole("button", { name: "Menu untuk Sesi tanpa judul" }).click({ force: true });
  await page.getByRole("menuitem", { name: "Ganti nama" }).click();
  await page.getByRole("textbox", { name: "Nama" }).fill("Perbaiki login");
  await page.getByRole("button", { name: "Simpan" }).click();
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" }).first()).toBeVisible();

  // pencarian
  const search = page.getByRole("searchbox", { name: "Cari sesi" });
  await search.fill("zzz");
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" })).toHaveCount(0);
  await search.fill("login");
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" }).first()).toBeVisible();
  await search.fill("");

  // muat ulang: registry (localStorage) + sesi (engine) tetap ada
  await page.reload();
  await expect(page.getByRole("button", { name: "alpha", exact: true })).toBeVisible();
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" }).first()).toBeVisible();

  // hapus sesi dengan konfirmasi
  await page.getByRole("button", { name: "Menu untuk Perbaiki login" }).click({ force: true });
  await page.getByRole("menuitem", { name: "Hapus sesi" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Hapus sesi" }).click();
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" })).toHaveCount(0);
});

test("lepas project dari daftar meminta konfirmasi dan tidak menghapus sesi", async ({ page }) => {
  const { sessions } = await mockEngine(page);
  await page.goto("/");
  await addProject(page, "C:\\Work\\beta");
  await page.getByRole("button", { name: "Sesi baru di beta" }).click();
  await expect(page.locator(".tree__label", { hasText: "Sesi tanpa judul" })).toBeVisible();
  await page.getByRole("button", { name: "beta", exact: true }).first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Lepas dari daftar" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("Folder dan sesinya di engine tidak dihapus");
  await page.getByRole("alertdialog").getByRole("button", { name: "Lepas dari daftar" }).click();
  await expect(page.getByRole("button", { name: "beta", exact: true })).toHaveCount(0);
  expect(sessions).toHaveLength(1);
});

test("sesi tanpa project dibuat di grup Chat", async ({ page }) => {
  const { sessions } = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(page.locator(".tree__label", { hasText: "Sesi tanpa judul" })).toBeVisible();
  expect(sessions[0]?.directory).toBe("C:\\data\\chats");
});

test("tab Files menampilkan pohon file project aktif, dimuat malas", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await page.getByRole("tab", { name: "Files" }).click();
  await expect(page.getByText("Pilih project untuk melihat file.")).toBeVisible();
  await page.getByRole("tab", { name: "Sesi", exact: true }).click();
  await addProject(page, "C:\\Work\\gamma");
  await page.getByRole("tab", { name: "Files" }).click();
  await expect(page.getByText("README.md")).toBeVisible();
  await expect(page.getByText("main.ts")).toHaveCount(0);
  await page.getByRole("button", { name: "src", exact: true }).click();
  await expect(page.getByText("main.ts")).toBeVisible();
  // ikon Material dimuat (img), bukan ikon cadangan Lucide
  await expect(page.locator("img.file-icon").first()).toBeVisible();
  expect(
    await page
      .locator("img.file-icon")
      .evaluateAll((els) => els.every((e) => (e as HTMLImageElement).naturalWidth > 0)),
  ).toBe(true);
});

test("engine bermasalah: galat per-grup tampil, UI tidak runtuh", async ({ page }) => {
  await mockEngine(page, { fail: true });
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Tambah project (pilih folder)" })).toBeVisible();
  await expect(page.getByText(/Gagal memuat sesi/).first()).toBeVisible();
});

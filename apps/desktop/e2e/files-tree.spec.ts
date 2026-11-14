import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function openFilesTab(page: Page, ...paths: string[]) {
  const engine = await mockEngine(page);
  engine.project.set("README.md", { text: "# Halo\n\nIsi README." });
  engine.project.set("src/main.ts", { text: "export {};\n" });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  for (const p of paths) {
    await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
    await page.getByLabel("Path folder").fill(p);
    await page.getByRole("button", { name: "Tambah", exact: true }).click();
  }
  await page.getByRole("tab", { name: "Berkas" }).click();
}

test("header Files: aksi lengkap; tampil juga tanpa project dengan ganti project", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await page.getByRole("tab", { name: "Berkas" }).click();
  await expect(page.getByRole("button", { name: "Ganti project" })).toBeVisible();
  await expect(page.getByText("Pilih project untuk melihat file.")).toBeVisible();
  for (const name of ["Muat ulang", "Buka semua folder"])
    await expect(page.getByRole("button", { name })).toBeDisabled();
});

test("buka semua / tutup semua folder, muat ulang, dan pencarian", async ({ page }) => {
  await openFilesTab(page, "C:/Work/alpha");
  await expect(page.getByText("README.md")).toBeVisible();
  await expect(page.getByText("main.ts")).toHaveCount(0);

  await page.getByRole("button", { name: "Buka semua folder" }).click();
  await expect(page.getByText("main.ts")).toBeVisible();
  await page.getByRole("button", { name: "Tutup semua folder" }).click();
  await expect(page.getByText("main.ts")).toHaveCount(0);

  await page.getByRole("button", { name: "Muat ulang" }).click();
  await expect(page.getByText("README.md")).toBeVisible();

  // pencarian membuka semua lalu menyaring; hasil berkas bisa dibuka
  await page.getByRole("searchbox", { name: "Cari file di workspace…" }).fill("main");
  await expect(page.getByRole("button", { name: /main\.ts/ })).toBeVisible();
  await expect(page.getByText("README.md")).toHaveCount(0);
  await page.getByRole("searchbox", { name: "Cari file di workspace…" }).fill("zzz");
  await expect(page.getByText("Tidak ada yang cocok.")).toBeVisible();
});

test("klik berkas di tab Files membukanya di Panel 3", async ({ page }) => {
  await openFilesTab(page, "C:/Work/alpha");
  await page.locator('.tree__file[title="README.md"]').click();
  const docs = page.getByRole("region", { name: "Dokumen", exact: true });
  await expect(docs.getByText("Isi README.")).toBeVisible();
});

test("ganti project dari header; membuat berkas dinonaktifkan jujur di luar aplikasi desktop", async ({ page }) => {
  await openFilesTab(page, "C:/Work/alpha", "C:/Work/beta");
  await page.getByRole("button", { name: "Ganti project" }).click();
  await page.getByRole("menuitemcheckbox", { name: "alpha" }).click();
  await expect(page.getByRole("button", { name: "Ganti project" })).toContainText("alpha");
  await expect(
    page.getByRole("button", { name: "Membuat berkas dan folder hanya tersedia di aplikasi desktop." }).first(),
  ).toBeDisabled();
});

test("sesi tanpa project: tab Files otomatis menampilkan folder kerja sesi itu dan berkasnya terbuka di Panel 3", async ({
  page,
}) => {
  const engine = await mockEngine(page);
  engine.project.set("hello.md", { text: "# Hello World" });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await page.getByRole("tab", { name: "Berkas" }).click();
  await expect(page.getByRole("button", { name: "Ganti project" })).toContainText(/^\s*chat-\d{8}-\d{6}-/);
  await page.locator('.tree__file[title="hello.md"]').click();
  await expect(page.getByRole("region", { name: "Dokumen", exact: true }).getByText("Hello World")).toBeVisible();
});

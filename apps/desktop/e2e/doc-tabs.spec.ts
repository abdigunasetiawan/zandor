import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function open(page: Page) {
  const engine = await mockEngine(page);
  engine.project.set("README.md", { text: "# Halo" });
  engine.project.set("src/main.ts", { text: "export {};\n" });
  engine.project.set("halaman.html", { text: "<h1>Hai</h1><img src='https://example.com/x.png'>" });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/docs");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Ya, saya percaya folder ini" }).click();
  await page.getByRole("tab", { name: "Berkas" }).click();
  await page.locator('.tree__file[title="README.md"]').click();
  await page.locator('.tree__file[title="halaman.html"]').click();
  await page.getByRole("button", { name: "src", exact: true }).click();
  await page.locator('.tree__file[title="src/main.ts"]').click();
}

const bar = (page: Page) => page.getByRole("navigation", { name: "Dokumen terbuka" });
const tabs = (page: Page) => bar(page).locator(".stabs__tab");
const menu = async (page: Page, index: number, item: string) => {
  await tabs(page).nth(index).locator(".stabs__label").click({ button: "right" });
  await page.getByRole("menuitem", { name: item, exact: true }).click();
};

test("tab dokumen: ikon file, tanpa scrollbar, dan menu klik kanan (tutup/lainnya/kanan/kiri/semua)", async ({
  page,
}) => {
  await open(page);
  await expect(tabs(page)).toHaveCount(3);
  const docs = page.getByRole("region", { name: "Dokumen", exact: true });
  const closeDocs = docs.getByRole("button", { name: "Tutup panel Dokumen" });
  await expect(closeDocs).toBeVisible();
  const docsCloseBox = await closeDocs.boundingBox();
  const docsTabsBox = await bar(page).boundingBox();
  expect(
    docsCloseBox &&
      docsTabsBox &&
      Math.abs(docsCloseBox.y + docsCloseBox.height / 2 - (docsTabsBox.y + docsTabsBox.height / 2)),
  ).toBeLessThanOrEqual(2);
  // ikon jenis file (gambar) di tiap tab
  for (let i = 0; i < 3; i++) await expect(tabs(page).nth(i).locator(".file-icon").first()).toBeVisible();
  // bilah tab tanpa scrollbar terlihat
  expect(await bar(page).evaluate((el) => getComputedStyle(el).scrollbarWidth)).toBe("none");

  // item menu lengkap; yang tidak berlaku nonaktif
  await tabs(page).nth(0).locator(".stabs__label").click({ button: "right" });
  for (const name of ["Tutup tab", "Tutup tab lainnya", "Tutup tab di kanan", "Tutup tab di kiri", "Tutup semua tab"])
    await expect(page.getByRole("menuitem", { name, exact: true })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Tutup tab di kiri" })).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Escape");

  await menu(page, 1, "Tutup tab");
  await expect(tabs(page)).toHaveCount(2);
  await menu(page, 0, "Tutup tab di kanan");
  await expect(tabs(page)).toHaveCount(1);
  await expect(tabs(page).nth(0)).toContainText("README.md");

  await page.locator('.tree__file[title="halaman.html"]').click();
  await page.locator('.tree__file[title="src/main.ts"]').click();
  await expect(tabs(page)).toHaveCount(3);
  await menu(page, 1, "Tutup tab lainnya");
  await expect(tabs(page)).toHaveCount(1);
  await expect(tabs(page).nth(0)).toContainText("halaman.html");

  await menu(page, 0, "Tutup semua tab");
  await expect(bar(page)).toHaveCount(0);
});

test("HTML: pratinjau sandbox tanpa bilah opsi; Buka di browser (live server) dan buka dengan nonaktif jujur di luar desktop", async ({
  page,
}) => {
  await open(page);
  await tabs(page).filter({ hasText: "halaman.html" }).locator(".stabs__label").click();
  const docs = page.getByRole("region", { name: "Dokumen", exact: true });
  await expect(docs.getByRole("switch")).toHaveCount(0); // saklar sumber luar dihapus
  await expect(docs.getByTestId("html-frame")).toHaveAttribute("sandbox", "");
  await expect(
    page
      .getByRole("toolbar", { name: "Alat dokumen" })
      .getByRole("button", { name: "Buka di browser (hanya di aplikasi desktop)" }),
  ).toBeDisabled(); // live server butuh aplikasi desktop
  await expect(docs.getByRole("button", { name: "Buka di aplikasi lain (hanya di aplikasi desktop)" })).toBeDisabled();
});

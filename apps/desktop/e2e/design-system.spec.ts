import { expect, type Page, test } from "@playwright/test";

const SECTION_IDS = ["tokens", "atoms", "molecules", "organisms", "panels", "menus", "modals", "pages", "shortcuts"];

async function open(page: Page, hash = "#/design-system") {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
  page.on("console", (m) => m.type() === "error" && errors.push(`console: ${m.text()}`));
  await page.goto(`/${hash}`);
  await expect(page.getByRole("heading", { level: 1, name: "Design system Zandor" })).toBeVisible();
  return errors;
}

const rootFontPx = (page: Page) =>
  page.evaluate(() => Number.parseFloat(getComputedStyle(document.documentElement).fontSize));
const bodyBg = (page: Page) => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
const noOverflow = (page: Page) => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);

test("bagian berurutan, tanpa galat konsol, dan tanpa luapan horizontal", async ({ page }) => {
  const errors = await open(page);
  const ids = await page.locator("main section.ds-section").evaluateAll((els) => els.map((e) => e.id));
  expect(ids).toEqual(SECTION_IDS);
  expect(await noOverflow(page)).toBe(true);
  expect(errors).toEqual([]);
});

test("font Plus Jakarta Sans dan JetBrains Mono termuat", async ({ page }) => {
  await open(page);
  const ok = await page.evaluate(async () => {
    await Promise.all([
      document.fonts.load('400 16px "Plus Jakarta Sans Variable"'),
      document.fonts.load('400 16px "JetBrains Mono Variable"'),
    ]);
    return [
      document.fonts.check('400 16px "Plus Jakarta Sans Variable"'),
      document.fonts.check('400 16px "JetBrains Mono Variable"'),
    ];
  });
  expect(ok).toEqual([true, true]);
});

test("tema: gelap, terang, ikut sistem; tersimpan setelah reload", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await open(page);
  await page.getByRole("combobox", { name: "Tema" }).click();
  await page.getByRole("option", { name: "Terang" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  expect(await bodyBg(page)).toBe("rgb(250, 250, 250)");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.getByRole("combobox", { name: "Tema" }).click();
  await page.getByRole("option", { name: "Gelap" }).click();
  expect(await bodyBg(page)).toBe("rgb(18, 18, 18)");
  await page.getByRole("combobox", { name: "Tema" }).click();
  await page.getByRole("option", { name: "Ikuti sistem" }).click();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.+/);
  await page.emulateMedia({ colorScheme: "light" });
  expect(await bodyBg(page)).toBe("rgb(250, 250, 250)");
});

test("zoom UI lewat Ctrl +/-/0 mengubah --ui-scale dan ukuran root, tanpa luapan", async ({ page }) => {
  await open(page);
  expect(await rootFontPx(page)).toBe(16);
  await page.keyboard.press("Control+=");
  expect(await rootFontPx(page)).toBe(18);
  for (let i = 0; i < 5; i++) await page.keyboard.press("Control+=");
  expect(await rootFontPx(page)).toBe(24); // 150% = batas atas
  expect(await noOverflow(page)).toBe(true);
  await page.keyboard.press("Control+0");
  expect(await rootFontPx(page)).toBe(16);
  await page.keyboard.press("Control+-");
  expect(await rootFontPx(page)).toBe(14);
  await page.reload();
  expect(await rootFontPx(page)).toBe(14); // tersimpan
});

test("menu: buka, pilih item, status berubah; Esc menutup", async ({ page }) => {
  await open(page);
  const demo = page.locator('[data-demo="Menu"]');
  await demo.getByRole("button", { name: "Berkas" }).click();
  await expect(page.getByRole("menu")).toBeVisible();
  await page.getByRole("menuitem", { name: /Sesi baru/ }).click();
  await expect(demo.getByRole("status")).toHaveText("Dipilih: Sesi baru");
  await demo.getByRole("button", { name: "Berkas" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("konfirmasi kustom (pengganti window.confirm): batal, Esc, lalu setuju", async ({ page }) => {
  await open(page);
  // Jika kode memakai dialog bawaan OS, Playwright melaporkannya sebagai event dialog.
  const nativeDialogs: string[] = [];
  page.on("dialog", (d) => {
    nativeDialogs.push(d.message());
    void d.dismiss();
  });
  const demo = page.locator('[data-demo="ConfirmDialog"]');
  const status = demo.getByRole("status");

  await demo.getByRole("button", { name: "Hapus sesi…" }).click();
  const dlg = page.getByRole("alertdialog", { name: "Hapus sesi?" });
  await expect(dlg).toBeVisible();
  await dlg.getByRole("button", { name: "Batal" }).click();
  await expect(status).toHaveText("Hasil: dibatalkan");

  await demo.getByRole("button", { name: "Hapus sesi…" }).click();
  await expect(dlg).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(status).toHaveText("Hasil: dibatalkan");

  await demo.getByRole("button", { name: "Hapus sesi…" }).click();
  await dlg.getByRole("button", { name: "Hapus" }).click();
  await expect(status).toHaveText("Hasil: dikonfirmasi");
  expect(nativeDialogs).toEqual([]);
});

test("dialog: fokus terkunci di dalam dan kembali ke pemicu setelah ditutup", async ({ page }) => {
  await open(page);
  const trigger = page.getByRole("button", { name: "Buka dialog" });
  await trigger.click();
  const dlg = page.getByRole("dialog", { name: "Pengaturan project" });
  await expect(dlg).toBeVisible();
  for (let i = 0; i < 6; i++) await page.keyboard.press("Tab");
  expect(await page.evaluate(() => document.activeElement?.closest('[role="dialog"]') !== null)).toBe(true);
  await page.keyboard.press("Escape");
  await expect(dlg).toHaveCount(0);
  await expect(trigger).toBeFocused();
});

test("toast muncul setelah tombol ditekan", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Sukses" }).click();
  await expect(page.getByText("Sesi dicatat di engine.")).toBeVisible();
});

test("fokus keyboard terlihat pada tombol", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "100%" }).focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  const outline = await page.evaluate(() => {
    const cs = getComputedStyle(document.activeElement as Element);
    return { style: cs.outlineStyle, width: cs.outlineWidth };
  });
  expect(outline.style).toBe("solid");
  expect(Number.parseFloat(outline.width)).toBeGreaterThanOrEqual(2);
});

test("tabel shortcut menampilkan pintasan nyata (tidak kosong)", async ({ page }) => {
  await open(page);
  const rows = page.locator("#shortcuts tbody tr");
  expect(await rows.count()).toBeGreaterThanOrEqual(8);
  await expect(page.locator("#shortcuts")).toContainText("cmd:view.toggleTheme");
  await expect(page.locator("#shortcuts")).toContainText("cmd:app.palette");
});

test("Tabs: panah memindahkan fokus, Enter memilih", async ({ page }) => {
  await open(page);
  const demo = page.locator('[data-demo="Tabs"]');
  await demo.getByRole("tab", { name: "Terminal" }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(demo.getByRole("tab", { name: "Git" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(demo.getByRole("tab", { name: "Git" })).toHaveAttribute("aria-selected", "true");
  await expect(demo.getByText("Isi git.")).toBeVisible();
});

test("rute bagian membuka halaman di bagian yang benar", async ({ page }) => {
  await open(page, "#/design-system/modals");
  await expect(page.locator("#modals")).toBeInViewport();
});

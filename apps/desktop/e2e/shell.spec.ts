import { expect, type Page, test } from "@playwright/test";

const width = (page: Page, id: string) =>
  page.evaluate((i) => document.getElementById(i)?.getBoundingClientRect().width ?? -1, id);
const rootPx = (page: Page) =>
  page.evaluate(() => Number.parseFloat(getComputedStyle(document.documentElement).fontSize));
const settled = (page: Page) => page.waitForTimeout(250); // beri waktu ResizeObserver + perencana

async function home(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
  page.on("console", (m) => m.type() === "error" && errors.push(`console: ${m.text()}`));
  await page.goto("/");
  await expect(page.getByRole("region", { name: "Chat" })).toBeVisible();
  await settled(page);
  return errors;
}

test("beranda: lima panel + rel, empty state jujur, tanpa galat konsol", async ({ page }) => {
  const errors = await home(page);
  for (const name of ["Sidebar", "Chat", "Dokumen", "Tools"]) {
    await expect(page.getByRole("region", { name, exact: true })).toBeVisible();
  }
  await expect(page.getByRole("navigation", { name: "Rel alat" })).toBeVisible();
  await expect(page.getByText("Segera hadir")).toHaveCount(3); // chat, dokumen, tab aktif di Tools
  await expect(page.getByRole("button", { name: "Tambah project (pilih folder)" })).toBeVisible();
  expect(errors).toEqual([]);
});

test("top bar: back/next nonaktif di awal; ke Pengaturan lalu kembali", async ({ page }) => {
  await home(page);
  const back = page.getByRole("button", { name: "Kembali", exact: true });
  const next = page.getByRole("button", { name: "Maju", exact: true });
  await expect(back).toBeDisabled();
  await expect(next).toBeDisabled();
  await page.getByRole("button", { name: "Pengaturan" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Pengaturan" })).toBeVisible();
  await expect(back).toBeEnabled();
  await back.click();
  await expect(page.getByRole("region", { name: "Chat" })).toBeVisible();
  await expect(next).toBeEnabled();
  await next.click();
  await expect(page.getByRole("heading", { level: 1, name: "Pengaturan" })).toBeVisible();
});

test("command palette: Ctrl+K, cari, Enter menjalankan perintah", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await home(page);
  await page.keyboard.press("Control+k");
  const input = page.getByRole("combobox", { name: "Command palette" });
  await expect(input).toBeFocused();
  await input.fill("tema");
  await expect(page.getByRole("option", { name: /Ganti tema/ })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("combobox", { name: "Command palette" })).toHaveCount(0);
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
});

test("command palette: klik omnibox, Esc menutup, perintah tanpa hasil", async ({ page }) => {
  await home(page);
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByRole("combobox", { name: "Command palette" });
  await input.fill("zzzzqq");
  await expect(page.getByText("Tidak ada hasil")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(input).toHaveCount(0);
});

test("menu Tampilan memuat perintah panel; Tentang menampilkan versi", async ({ page }) => {
  await home(page);
  await page.getByRole("button", { name: "Tampilan" }).click();
  await expect(page.getByRole("menuitem", { name: /Tampilkan\/sembunyikan Sidebar/ })).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Bantuan" }).click();
  await page.getByRole("menuitem", { name: "Tentang Zandor" }).click();
  await expect(page.getByRole("dialog", { name: "Tentang Zandor" })).toContainText(/Versi 0\.0\.\d+/);
});

test("toggle panel: Ctrl+B dan tombol; keadaan tersimpan setelah reload", async ({ page }) => {
  await home(page);
  expect(await width(page, "sidebar")).toBeGreaterThan(100);
  await page.keyboard.press("Control+b");
  await settled(page);
  expect(await width(page, "sidebar")).toBeLessThan(2);
  await expect(page.getByRole("button", { name: /^Sidebar/ })).toHaveAttribute("aria-pressed", "false");
  await page.reload();
  await settled(page);
  expect(await width(page, "sidebar")).toBeLessThan(2); // pilihan pengguna dipertahankan
  await page.getByRole("button", { name: /^Sidebar/ }).click();
  await settled(page);
  expect(await width(page, "sidebar")).toBeGreaterThan(100);
});

test("rel: klik ikon membuka tab dan panel Tools; klik ikon aktif menutupnya", async ({ page }) => {
  await home(page);
  const rail = page.getByRole("navigation", { name: "Rel alat" });
  await rail.getByRole("button", { name: "Git" }).click();
  await expect(page.getByRole("tab", { name: "Git" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByText("Git hadir di Fase 8.")).toBeVisible();
  await rail.getByRole("button", { name: "Git" }).click();
  await settled(page);
  expect(await width(page, "tools")).toBeLessThan(2);
  await rail.getByRole("button", { name: "Log aktivitas" }).click(); // membuka kembali + tab lain
  await settled(page);
  expect(await width(page, "tools")).toBeGreaterThan(100);
  await expect(page.getByRole("tab", { name: "Log aktivitas" })).toHaveAttribute("aria-selected", "true");
});

test("seret separator mengubah lebar panel dan tersimpan setelah reload", async ({ page }) => {
  await home(page);
  const before = await width(page, "sidebar");
  const box = await page.locator("#sep-1").boundingBox();
  if (!box) throw new Error("separator tidak ditemukan");
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 60, y, { steps: 8 });
  await page.mouse.up();
  await settled(page);
  const after = await width(page, "sidebar");
  expect(after).toBeGreaterThan(before + 40);
  await page.reload();
  await settled(page);
  expect(Math.abs((await width(page, "sidebar")) - after)).toBeLessThan(4);
});

test("auto-collapse: jendela sempit melipat Dokumen dulu, lalu terbuka lagi saat melebar", async ({ page }) => {
  await home(page);
  await page.setViewportSize({ width: 1000, height: 700 });
  await settled(page);
  expect(await width(page, "docs")).toBeLessThan(2);
  expect(await width(page, "sidebar")).toBeGreaterThan(100); // sidebar masih terbuka
  expect(await width(page, "chat")).toBeGreaterThanOrEqual(22 * 16 - 1); // chat tidak di bawah minimum
  await page.setViewportSize({ width: 1400, height: 700 });
  await settled(page);
  expect(await width(page, "docs")).toBeGreaterThan(100); // dibuka lagi otomatis
});

test("zoom 150%: panel terlipat otomatis berurutan, chat tetap di atas minimum, tanpa luapan", async ({ page }) => {
  await home(page);
  for (let i = 0; i < 6; i++) await page.keyboard.press("Control+=");
  await settled(page);
  expect(await rootPx(page)).toBe(24);
  expect(await width(page, "docs")).toBeLessThan(2);
  expect(await width(page, "sidebar")).toBeLessThan(2);
  expect(await width(page, "chat")).toBeGreaterThanOrEqual(22 * 24 - 1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.keyboard.press("Control+0");
  await settled(page);
  expect(await width(page, "docs")).toBeGreaterThan(100); // kembali terbuka
  expect(await width(page, "sidebar")).toBeGreaterThan(100);
});

test("Pengaturan: bahasa, tema, skala, dan ingat tata letak; tersimpan; JSON saat ini tampil", async ({ page }) => {
  await home(page);
  await page.getByRole("button", { name: "Pengaturan" }).click();
  await page.getByRole("combobox", { name: "Bahasa" }).click();
  await page.getByRole("option", { name: "English" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.getByTestId("settings-json")).toContainText('"language": "en"');

  await page.getByRole("combobox", { name: "UI scale" }).click();
  await page.getByRole("option", { name: "125%" }).click();
  expect(await rootPx(page)).toBe(20);
  await expect(page.getByTestId("settings-json")).toContainText('"uiScale": 1.25');

  const resetBtn = page.locator('[data-setting="theme"]').getByRole("button", { name: "Reset to default" });
  await expect(resetBtn).toBeDisabled(); // belum pernah diubah
  await page.getByRole("combobox", { name: "Theme" }).click();
  await page.getByRole("option", { name: "Dark" }).click();
  await expect(resetBtn).toBeEnabled();
  await resetBtn.click();
  await expect(page.getByTestId("settings-json")).toContainText('"theme": "system"');

  await page.reload();
  await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible(); // rute tersimpan di hash
  expect(await rootPx(page)).toBe(20);
});

test("tata letak tidak disimpan bila 'ingat tata letak' dimatikan", async ({ page }) => {
  await home(page);
  const defaultWidth = await width(page, "sidebar");
  await page.getByRole("button", { name: "Pengaturan" }).click();
  await page.getByRole("switch", { name: "Ingat tata letak panel" }).click();
  await page.getByRole("button", { name: "Kembali", exact: true }).click();
  await expect(page.getByRole("region", { name: "Chat" })).toBeVisible();
  await settled(page);
  const box = await page.locator("#sep-1").boundingBox();
  if (!box) throw new Error("separator tidak ditemukan");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 100, box.y + box.height / 2, { steps: 6 });
  await page.mouse.up();
  await settled(page);
  expect(await width(page, "sidebar")).toBeGreaterThan(defaultWidth + 40);
  await page.reload();
  await settled(page);
  expect(Math.abs((await width(page, "sidebar")) - defaultWidth)).toBeLessThan(6); // kembali ke bawaan
});

test("Reset tata letak mengembalikan ukuran dan membuka semua panel", async ({ page }) => {
  await home(page);
  await page.keyboard.press("Control+b");
  await page.keyboard.press("Control+j");
  await settled(page);
  await page.keyboard.press("Control+k");
  await page.getByRole("combobox", { name: "Command palette" }).fill("reset tata");
  await page.keyboard.press("Enter");
  await settled(page);
  expect(await width(page, "sidebar")).toBeGreaterThan(100);
  expect(await width(page, "tools")).toBeGreaterThan(100);
});

test("di luar Tauri tidak ada kontrol jendela palsu", async ({ page }) => {
  await home(page);
  await expect(page.getByRole("button", { name: "Minimalkan" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Tutup jendela" })).toHaveCount(0);
});

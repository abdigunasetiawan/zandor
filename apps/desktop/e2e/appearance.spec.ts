import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function setup(page: Page) {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await expect(page.getByRole("button", { name: "alpha", exact: true })).toBeVisible();
}

const openPage = async (page: Page) => {
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByRole("combobox", { name: "Command palette" });
  await input.fill("kustomisasi");
  await input.press("Enter");
  await expect(page.getByRole("heading", { name: "Tampilan & Tema" })).toBeVisible();
};

const rootVar = (page: Page, name: string) =>
  page.evaluate((n) => document.documentElement.style.getPropertyValue(n), name);

test("preset mengubah token warna sesuai mode, bertahan setelah muat ulang, dan bisa dikembalikan", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await setup(page);
  await openPage(page);
  expect(await rootVar(page, "--z-color-bg")).toBe("");

  await page.getByRole("listitem", { name: "Claude" }).getByRole("button", { name: "Pakai" }).click();
  await expect(page.getByRole("listitem", { name: "Claude" })).toContainText("Aktif");
  expect(await rootVar(page, "--z-color-bg")).toBe("#262624");
  expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe("rgb(38, 38, 36)");

  // mode terang memakai palet terang preset yang sama
  await page.emulateMedia({ colorScheme: "light" });
  await expect.poll(() => rootVar(page, "--z-color-bg")).toBe("#faf9f5");

  await page.reload();
  await expect.poll(() => rootVar(page, "--z-color-bg")).toBe("#faf9f5");

  await openPage(page);
  await page.getByRole("button", { name: "Kembalikan semua ke default" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Kembalikan semua ke default" }).click();
  await expect.poll(() => rootVar(page, "--z-color-bg")).toBe("");
});

test("tema kustom: buat dari preset, validasi warna, kontras, simpan & pakai", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await setup(page);
  await openPage(page);
  await page.getByRole("button", { name: "Buat tema" }).click();
  const editor = page.getByRole("region", { name: "Editor tema" });
  await editor.getByLabel("Nama tema").fill("Kopi Susu");
  await editor.getByLabel("Gelap bg", { exact: true }).fill("#221100");
  await editor.getByLabel("Gelap fg", { exact: true }).fill("#fff0e0");
  await expect(editor.getByTestId("contrast-Gelap")).not.toHaveAttribute("data-low", "");
  await editor.getByLabel("Gelap fg", { exact: true }).fill("#332211");
  await expect(editor.getByTestId("contrast-Gelap")).toHaveAttribute("data-low", "");
  await editor.getByLabel("Gelap fg", { exact: true }).fill("#fff0e0");
  await editor.getByLabel("Gelap accent", { exact: true }).fill("merah");
  await editor.getByRole("button", { name: "Simpan & pakai" }).click();
  await expect(editor.getByRole("alert")).toContainText("dark.accent");
  await editor.getByLabel("Gelap accent", { exact: true }).fill("#ff9966");
  await editor.getByRole("button", { name: "Simpan & pakai" }).click();
  await expect(page.getByRole("listitem", { name: "Kopi Susu" })).toContainText("Aktif");
  expect(await rootVar(page, "--z-color-bg")).toBe("#221100");
  expect(await rootVar(page, "--z-color-accent")).toBe("#ff9966");
  await page.getByRole("listitem", { name: "Kopi Susu" }).getByRole("button", { name: "Hapus Kopi Susu" }).click();
  await expect(page.getByRole("listitem", { name: "Kopi Susu" })).toHaveCount(0);
  await expect.poll(() => rootVar(page, "--z-color-bg")).toBe("");
});

test("impor tema: berkas valid dipakai, berkas jahat ditolak tanpa menyentuh CSS; ekspor menghasilkan JSON", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await setup(page);
  await openPage(page);

  await page.getByLabel("Impor tema").setInputFiles({
    name: "jahat.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify({ id: "jahat", name: "Jahat", dark: { bg: "url(javascript:alert(1))" } })),
  });
  await expect(page.getByRole("alert")).toContainText("dark.bg");
  await expect(page.getByRole("listitem", { name: "Jahat" })).toHaveCount(0);
  expect(await rootVar(page, "--z-color-bg")).toBe("");

  await page.getByLabel("Impor tema").setInputFiles({
    name: "ok.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify({ id: "senja", name: "Senja", dark: { bg: "#101820", fg: "#f2e9dc" } })),
  });
  await expect(page.getByRole("listitem", { name: "Senja" })).toBeVisible();
  await page.getByRole("listitem", { name: "Senja" }).getByRole("button", { name: "Pakai" }).click();
  expect(await rootVar(page, "--z-color-bg")).toBe("#101820");

  const download = page.waitForEvent("download");
  await page.getByRole("button", { name: "Ekspor Senja" }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("zandor-tema-senja.json");
});

test("tipografi, font, panel chat: nilai valid jadi variabel CSS, nilai jahat ditolak", async ({ page }) => {
  await setup(page);
  await openPage(page);
  await page.getByRole("tab", { name: "Tipografi" }).click();
  const size = page.getByLabel("h1 · ukuran");
  await size.fill("2.5rem");
  await size.press("Enter");
  expect(await rootVar(page, "--z-type-h1-size")).toBe("2.5rem");
  await size.fill("1px;}body{display:none");
  await size.press("Enter");
  await expect(page.getByText(/ukuran harus dalam rem/)).toBeVisible();
  expect(await rootVar(page, "--z-type-h1-size")).toBe("2.5rem");
  await size.fill("");
  await size.press("Enter");
  expect(await rootVar(page, "--z-type-h1-size")).toBe("");

  const font = page.getByLabel("Font kode");
  await font.fill("Consolas, monospace");
  await font.press("Enter");
  expect(await rootVar(page, "--z-font-code")).toBe("Consolas, monospace");

  await page.getByRole("tab", { name: "Panel chat" }).click();
  const op = page.getByLabel("Opasitas teks (0.3–1)");
  await op.fill("0.6");
  await op.press("Enter");
  expect(await rootVar(page, "--z-chat-opacity")).toBe("0.6");
  expect(
    await page
      .getByTestId("chat-preview")
      .locator(".msg__text")
      .evaluate((e) => getComputedStyle(e).opacity),
  ).toBe("0.6");
  await op.fill("9");
  await op.press("Enter");
  await expect(page.getByText("opasitas harus 0.3–1")).toBeVisible();
});

test("section project: logo dan warna tampil di sidebar", async ({ page }) => {
  await setup(page);
  await openPage(page);
  await page.getByRole("tab", { name: "Section project" }).click();
  const logo = page.getByLabel("alpha · Logo");
  await logo.fill("🚀");
  await logo.press("Enter");
  await page.getByLabel("alpha · Warna border").fill("#ff0000");
  await page.getByLabel("alpha · Warna border").press("Enter");
  await logo.fill("<b>");
  await logo.press("Enter");
  await expect(page.getByText(/karakter terlarang|maksimal/).first()).toBeVisible();
  await page.getByRole("button", { name: "Kembali ke chat" }).click();
  await expect(page.locator(".tree__logo")).toHaveText("🚀");
  const border = await page
    .locator(".tree__group[data-styled]")
    .first()
    .evaluate((e) => getComputedStyle(e).borderLeftColor);
  expect(border).toBe("rgb(255, 0, 0)");
});

test("pintasan: combo baru benar-benar menjalankan perintah, yang lama tidak lagi; bertahan setelah muat ulang", async ({
  page,
}) => {
  await setup(page);
  await openPage(page);
  await page.getByRole("tab", { name: "Pintasan" }).click();
  await page.getByRole("button", { name: "Ubah cmd:app.openSettings" }).click();
  await page.keyboard.press("Control+Alt+9");
  await expect(page.getByTestId("key-cmd:app.openSettings")).toHaveText("Ctrl+Alt+9");

  await page.getByRole("button", { name: "Kembali ke chat" }).click();
  await page.keyboard.press("Control+,");
  await expect(page).not.toHaveURL(/#\/settings/);
  await page.keyboard.press("Control+Alt+9");
  await expect(page).toHaveURL(/#\/settings/);

  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  // pintasan terdaftar setelah perintah dimuat; ulangi tekan sampai berlaku
  await expect(async () => {
    await page.keyboard.press("Control+Alt+9");
    await expect(page).toHaveURL(/#\/settings/, { timeout: 500 });
  }).toPass({ timeout: 10_000 });
});

test("tema editor: impor berkas tema VS Code dipilih dan tersimpan", async ({ page }) => {
  await setup(page);
  await openPage(page);
  await page.getByRole("tab", { name: "Editor" }).click();
  await expect(page.getByText(/hanya tersedia di aplikasi desktop/)).toBeVisible();
  await page.getByLabel("Impor tema VS Code (.json)").setInputFiles({
    name: "senja.json",
    mimeType: "application/json",
    buffer: Buffer.from(
      '{ // komentar\n "name": "Senja", "type": "dark", "colors": {"editor.background": "#101820"}, "tokenColors": [] }',
    ),
  });
  await expect(page.getByRole("listitem", { name: "Senja" })).toBeVisible();
  const stored = await page.evaluate(() => JSON.parse(localStorage.getItem("zandor.appearance") ?? "{}"));
  expect(stored.editorTheme).toBe("senja");
  expect(stored.editorThemes[0].colors["editor.background"]).toBe("#101820");
});

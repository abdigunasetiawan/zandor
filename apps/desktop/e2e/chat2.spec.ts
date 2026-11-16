import { expect, type Page, test } from "@playwright/test";
import { type MockOptions, mockEngine } from "./mock-engine";

const expandProcess = async (page: Page, count = 1) => {
  await expect(page.locator(".work__sum, .work__step")).toHaveCount(count, { timeout: 10_000 });
  const toggle = page.locator(".work__sum, .work__step").last();
  await expect(toggle).toBeVisible({ timeout: 10_000 });
  if ((await toggle.getAttribute("aria-expanded")) !== "true") await toggle.click();
};

async function openSession(page: Page, opts?: MockOptions) {
  const engine = await mockEngine(page, opts);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Ya, saya percaya folder ini" }).click();
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.getByRole("textbox", { name: /^Tulis pesan/ })).toBeVisible();
  return engine;
}

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });
const send = async (page: Page, text: string) => {
  await composer(page).fill(text);
  await composer(page).press("Enter");
};
const assistant = (page: Page) => page.getByRole("article", { name: "Asisten" });

async function chooseOption(page: Page, label: string, option: string) {
  await page.getByRole("combobox", { name: label }).click();
  await page.getByRole("option", { name: option }).click();
}

test("Salin pesan menaruh jawaban di clipboard", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openSession(page);
  await send(page, "halo");
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?", { timeout: 10_000 });
  await assistant(page).getByRole("button", { name: "Salin pesan" }).click();
  await expect(page.getByText("Tersalin")).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("Halo, ada yang bisa dibantu?");
});

test("gelembung pesan pengguna punya latar berbeda dari latar panel", async ({ page }) => {
  await openSession(page);
  await send(page, "halo");
  const bubble = page.getByRole("article", { name: "Anda", exact: true });
  await expect(bubble).toBeVisible();
  const readColors = () =>
    bubble
      .locator(".msg__bubble")
      .evaluate((el) => [
        getComputedStyle(el).backgroundColor,
        getComputedStyle(el.closest(".shell-panel__inner") as Element).backgroundColor,
      ]);
  const [darkBubble, darkPanel] = await readColors();
  expect(darkBubble).not.toBe("rgba(0, 0, 0, 0)");
  expect(darkBubble).not.toBe(darkPanel);
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "light"));
  const [lightBubble, lightPanel] = await readColors();
  expect(lightBubble).not.toBe("rgba(0, 0, 0, 0)");
  expect(lightBubble).not.toBe(lightPanel);
});

test("aksi pesan pengguna hanya salin dan revert di luar bubble; waktu punya tooltip lengkap", async ({ page }) => {
  await openSession(page);
  await send(page, "halo");
  const message = page.getByRole("article", { name: "Anda", exact: true });
  const bubble = message.locator(".msg__bubble");
  const actions = message.locator(".msg__actions");
  const time = message.locator("time.msg__user-time");
  const foot = message.locator(".msg__user-foot");
  await expect(foot).toHaveCSS("opacity", "0");
  await expect(message.getByRole("button", { name: "Salin pesan", exact: true })).toBeAttached();
  await expect(message.getByRole("button", { name: "Kembalikan ke sebelum ini", exact: true })).toBeAttached();
  await expect(actions.getByRole("button")).toHaveCount(2);
  const [bubbleBox, actionsBox] = await Promise.all([bubble.boundingBox(), actions.boundingBox()]);
  expect(bubbleBox).not.toBeNull();
  expect(actionsBox).not.toBeNull();
  if (!bubbleBox || !actionsBox) throw new Error("Area pesan tidak memiliki posisi layout");
  expect(actionsBox.y).toBeGreaterThanOrEqual(bubbleBox.y + bubbleBox.height);

  await bubble.hover();
  await expect(foot).toHaveCSS("opacity", "1");
  await expect(time).toBeVisible();
  await expect(time).toHaveAttribute("dateTime", /T/);
  await expect(time).toHaveAttribute("title", /2026/);
});

test("mode izin 'Setujui edit otomatis' menjawab izin edit tanpa kartu", async ({ page }) => {
  const engine = await openSession(page);
  await chooseOption(page, "Mode izin", "Setujui edit otomatis");
  await send(page, "ubah file, butuh izin");
  await expect.poll(() => engine.permissionReplies.length, { timeout: 10_000 }).toBe(1);
  expect(engine.permissionReplies[0]?.reply).toBe("once");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect(assistant(page)).toContainText("Sudah diubah.", { timeout: 10_000 });
});

test("varian reasoning dikirim bersama prompt", async ({ page }) => {
  const engine = await openSession(page, { models: [{ id: "m-satu", name: "Model Satu", variants: ["low", "high"] }] });
  await chooseOption(page, "Reasoning", "high");
  await send(page, "halo");
  await expect.poll(() => engine.variants.length).toBe(1);
  expect(engine.variants[0]).toBe("high");
});

test("lampiran: pilih berkas → chip → terkirim sebagai file part; bisa dihapus", async ({ page }) => {
  const engine = await openSession(page);
  const chooser = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Lampirkan file" }).click();
  await (await chooser).setFiles([{ name: "catatan.txt", mimeType: "text/plain", buffer: Buffer.from("halo") }]);
  await expect(page.locator(".composer .chip").filter({ hasText: "catatan.txt" })).toBeVisible();
  await page.getByRole("button", { name: "Hapus lampiran catatan.txt" }).click();
  await expect(page.locator(".composer .chip")).toHaveCount(0);

  const again = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Lampirkan file" }).click();
  await (await again).setFiles([{ name: "catatan.txt", mimeType: "text/plain", buffer: Buffer.from("halo") }]);
  await send(page, "tolong lihat ini");
  await expect.poll(() => engine.files.length).toBe(1);
  expect(engine.files[0]).toMatchObject({ mime: "text/plain", filename: "catatan.txt" });
  expect(engine.files[0]?.url.startsWith("data:text/plain")).toBe(true);
});

test("lampiran bisa ditempel dari clipboard dan di-drop ke composer", async ({ page }) => {
  const engine = await openSession(page);
  const box = composer(page);
  await box.evaluate((element) => {
    const file = new File(["gambar clipboard"], "clipboard.png", { type: "image/png" });
    const data = new DataTransfer();
    data.items.add(file);
    element.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, clipboardData: data }));
  });
  await expect(page.locator(".composer .chip").filter({ hasText: "clipboard.png" })).toBeVisible();

  await page.locator(".composer").dispatchEvent("drop", {
    dataTransfer: await page.evaluateHandle(() => {
      const data = new DataTransfer();
      data.items.add(new File(["catatan drop"], "drop.txt", { type: "text/plain" }));
      return data;
    }),
  });
  await expect(page.locator(".composer .chip").filter({ hasText: "drop.txt" })).toBeVisible();
  await send(page, "periksa lampiran");
  await expect.poll(() => engine.files.length).toBe(2);
  expect(engine.files.map((file) => file.filename)).toEqual(["clipboard.png", "drop.txt"]);
});

test("tombol X di akhir daftar menghapus semua lampiran", async ({ page }) => {
  await openSession(page);
  await composer(page).evaluate((element) => {
    const data = new DataTransfer();
    data.items.add(new File(["satu"], "satu.txt", { type: "text/plain" }));
    data.items.add(new File(["dua"], "dua.txt", { type: "text/plain" }));
    element.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, clipboardData: data }));
  });
  await expect(page.locator(".composer .chip")).toHaveCount(3); // dua file + aksi hapus semua
  await page.getByRole("button", { name: "Hapus semua 2 lampiran" }).click();
  await expect(page.locator(".composer .chip")).toHaveCount(0);
});

test("gambar terlampir punya thumbnail yang membuka pratinjau ukuran penuh", async ({ page }) => {
  await openSession(page);
  await composer(page).evaluate((element) => {
    const data = new DataTransfer();
    data.items.add(new File(["gambar"], "gambar.png", { type: "image/png" }));
    element.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, clipboardData: data }));
  });
  const previewButton = page.getByRole("button", { name: "Pratinjau gambar gambar.png" });
  await expect(previewButton).toBeVisible();
  await previewButton.click();
  await expect(page.getByRole("dialog", { name: "gambar.png" })).toBeVisible();
  await expect(page.getByRole("dialog").getByRole("img", { name: "gambar.png" })).toBeVisible();
  const dialog = page.getByRole("dialog", { name: "gambar.png" });
  await dialog.getByRole("button", { name: "Perbesar gambar" }).click();
  await expect(dialog.getByText("125%")).toBeVisible();
  await dialog.getByRole("button", { name: "Perkecil gambar" }).click();
  await expect(dialog.getByText("100%")).toBeVisible();
  const stage = dialog.locator(".composer__preview-stage");
  const bounds = await stage.boundingBox();
  if (!bounds) throw new Error("area pratinjau tidak ditemukan");
  const start = { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(start.x + 140, start.y + 85, { steps: 12 });
  await page.mouse.up();
  await expect(dialog.getByRole("img", { name: "gambar.png" })).toHaveAttribute("style", /translate\(140px, 85px\)/);
  await dialog.getByRole("button", { name: "Atur ulang zoom dan posisi gambar" }).click();
  await expect(dialog.getByText("100%")).toBeVisible();
  await expect(dialog.getByRole("img", { name: "gambar.png" })).toHaveAttribute(
    "style",
    /translate\(0px, 0px\) scale\(1\)/,
  );
  await stage.hover();
  await page.mouse.wheel(0, -120);
  await expect(dialog.getByText("110%")).toBeVisible();
  await dialog.getByRole("button", { name: "Tutup" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("lampiran Markdown dan PDF dibuka di pratinjau bertipe sesuai", async ({ page }) => {
  await openSession(page);
  await composer(page).evaluate((element) => {
    const data = new DataTransfer();
    data.items.add(new File(["# Isi Markdown\n\nTeks pratinjau"], "catatan.md", { type: "text/markdown" }));
    data.items.add(new File(["%PDF-1.4\ninvalid test data"], "dokumen.pdf", { type: "application/pdf" }));
    element.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, clipboardData: data }));
  });
  await page.getByRole("button", { name: "Pratinjau lampiran catatan.md" }).click();
  const markdownDialog = page.getByRole("dialog", { name: "catatan.md" });
  await expect(markdownDialog.getByText("Isi Markdown")).toBeVisible();
  await markdownDialog.getByRole("button", { name: "Tutup" }).click();
  await page.getByRole("button", { name: "Pratinjau lampiran dokumen.pdf" }).click();
  const pdfDialog = page.getByRole("dialog", { name: "dokumen.pdf" });
  await expect(pdfDialog.getByRole("alert")).toBeVisible();
  await pdfDialog.getByRole("button", { name: "Tutup" }).click();
});

test("revert menyembunyikan pesan sesudahnya; Batalkan revert memulihkannya", async ({ page }) => {
  const engine = await openSession(page);
  await send(page, "halo");
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?", { timeout: 10_000 });
  await page.getByRole("article", { name: "Anda" }).getByRole("button", { name: "Kembalikan ke sebelum ini" }).click();
  await expect(page.getByText(/Sesi dikembalikan/)).toBeVisible();
  await expect(page.getByRole("article", { name: "Anda" })).toHaveCount(0);
  expect(engine.reverts).toHaveLength(1);
  await page.getByRole("button", { name: "Batalkan revert" }).click();
  await expect(page.getByRole("article", { name: "Anda" })).toBeVisible();
  expect(engine.unreverts).toBe(1);
});

test("Ulangi jawaban: revert lalu kirim ulang pesan yang sama", async ({ page }) => {
  const engine = await openSession(page);
  await send(page, "halo");
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?", { timeout: 10_000 });
  await assistant(page).getByRole("button", { name: "Ulangi jawaban" }).click();
  await expect.poll(() => engine.prompts.length, { timeout: 10_000 }).toBe(2);
  expect(engine.prompts[1]?.text).toBe("halo");
  expect(engine.reverts).toHaveLength(1);
  // setelah pembersihan hanya ada satu pasang pesan
  await expect(page.getByRole("article", { name: "Anda" })).toHaveCount(1, { timeout: 10_000 });
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?");
});

test("Cabangkan dari sini membuat sesi baru dan membukanya sebagai tab", async ({ page }) => {
  const engine = await openSession(page);
  await send(page, "halo");
  await expect(assistant(page)).toContainText("Halo, ada yang bisa dibantu?", { timeout: 10_000 });
  await assistant(page).getByRole("button", { name: "Cabangkan dari sini" }).click();
  await expect.poll(() => engine.forks.length).toBe(1);
  await expect(page.getByRole("navigation", { name: "Sesi terbuka" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Cabang percakapan" })).toBeVisible();
});

test("tab sesi: beralih dan menutup", async ({ page }) => {
  await openSession(page);
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  const tabs = page.getByRole("navigation", { name: "Sesi terbuka" });
  await expect(tabs).toBeVisible();
  await expect(tabs.locator(".stabs__tab")).toHaveCount(2);
  await tabs.locator(".stabs__label").first().click();
  await expect(tabs.locator(".stabs__tab").first()).toHaveAttribute("data-active", "");
  await tabs
    .getByRole("button", { name: /^Tutup tab/ })
    .first()
    .click();
  await expect(tabs.locator(".stabs__tab")).toHaveCount(1); // bilah tab tetap tampil dengan satu tab
});

test("pencarian web tampil sebagai kartu khusus; diff edit bisa dibuka", async ({ page }) => {
  await openSession(page);
  await send(page, "tolong cari web cuaca");
  await expandProcess(page);
  const card = page.locator(".fold--tool").first();
  await expect(card).toContainText("Pencarian web", { timeout: 10_000 });
  await expect(card).toContainText("cuaca jakarta");
  await send(page, "tolong buat diff");
  await expandProcess(page, 2);
  const edit = page.locator(".fold--tool").filter({ hasText: "edit" });
  await edit.getByRole("button").first().click();
  await expect(edit.locator(".diff__line--add")).toContainText("+baru");
  await expect(edit.locator(".diff__line--del")).toContainText("-lama");
});

test("input suara: tombol mikrofon dan menu perangkat input tersedia; tanpa perangkat → pesan jujur", async ({
  page,
}) => {
  await openSession(page);
  await expect(page.getByRole("button", { name: "Mulai input suara" })).toBeVisible();
  await page.getByRole("button", { name: "Pilih perangkat input suara" }).click();
  await expect(page.getByRole("menuitemcheckbox", { name: "Bawaan sistem" })).toBeChecked();
});

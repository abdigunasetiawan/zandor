import { expect, type Page, test } from "@playwright/test";
import { type MockEngine, mockEngine } from "./mock-engine";

async function setup(page: Page, opts: Parameters<typeof mockEngine>[1] = {}): Promise<MockEngine> {
  const engine = await mockEngine(page, opts);
  await page.addInitScript(() => {
    (window as unknown as { __opened: string[] }).__opened = [];
    window.open = ((url?: string | URL) => {
      (window as unknown as { __opened: string[] }).__opened.push(String(url));
      return null;
    }) as typeof window.open;
  });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Ya, saya percaya folder ini" }).click();
  await expect(page.getByRole("button", { name: "alpha", exact: true })).toBeVisible();
  return engine;
}

const tools = (page: Page) => page.getByRole("region", { name: "Tools", exact: true });
const tab = (page: Page, name: string) =>
  page.getByRole("navigation", { name: "Rel alat" }).getByRole("button", { name, exact: true });
const screen = (page: Page) => tools(page).locator(".xterm-rows");

test("terminal: buat, ketik perintah, keluaran tampil, ukuran dikirim, nama tab sesuai shell", async ({ page }) => {
  const engine = await setup(page);
  await expect(tools(page).getByText(/Belum ada terminal/)).toBeVisible();
  await tools(page).getByRole("button", { name: "Terminal baru" }).first().click();
  await expect(screen(page)).toContainText("PS C:\\Work>", { timeout: 15_000 });
  await expect(tools(page).getByRole("button", { name: "powershell 1", exact: true })).toBeVisible();
  expect(engine.pty.list[0]?.cwd).toBe("C:\\Work\\alpha");
  await tools(page).getByTestId("terminal").click();
  await page.keyboard.type("ls");
  await page.keyboard.press("Enter");
  await expect(screen(page)).toContainText("hasil:ls", { timeout: 10_000 });
  expect(engine.pty.inputs.join("")).toContain("ls\r");
  await expect.poll(() => engine.pty.resizes.length).toBeGreaterThan(0);
  expect(engine.pty.resizes.at(-1)?.cols).toBeGreaterThan(10);
});

test("terminal: beberapa tab, pindah tab, tutup menghapus PTY di engine", async ({ page }) => {
  const engine = await setup(page);
  await tools(page).getByRole("button", { name: "Terminal baru" }).first().click();
  await expect(screen(page)).toContainText("PS C:\\Work>", { timeout: 15_000 });
  await tools(page).getByRole("button", { name: "Terminal baru" }).first().click();
  await expect(tools(page).getByRole("button", { name: "powershell 2", exact: true })).toBeVisible();
  await tools(page).getByRole("button", { name: "powershell 1", exact: true }).click();
  await expect(tools(page).locator(".term:not([hidden])")).toHaveCount(1);
  await tools(page).getByRole("button", { name: "Tutup terminal powershell 2" }).click();
  await expect.poll(() => engine.pty.deleted.length).toBe(1);
  await expect(tools(page).getByRole("button", { name: "powershell 2", exact: true })).toHaveCount(0);
});

test("terminal: setelah muat ulang, PTY yang masih hidup diadopsi dan riwayat diputar ulang", async ({ page }) => {
  await setup(page);
  await tools(page).getByRole("button", { name: "Terminal baru" }).first().click();
  await tools(page).getByTestId("terminal").click();
  await expect(screen(page)).toContainText("PS C:\\Work>", { timeout: 15_000 });
  await page.keyboard.type("pwd");
  await page.keyboard.press("Enter");
  await expect(screen(page)).toContainText("hasil:pwd", { timeout: 10_000 });
  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(tools(page).getByRole("button", { name: "powershell 1", exact: true })).toBeVisible({ timeout: 15_000 });
  await expect(screen(page)).toContainText("hasil:pwd", { timeout: 15_000 });
});

test("terminal: shell yang berakhir ditandai dan pesan selesai tampil", async ({ page }) => {
  const engine = await setup(page);
  await tools(page).getByRole("button", { name: "Terminal baru" }).first().click();
  await expect(screen(page)).toContainText("PS C:\\Work>", { timeout: 15_000 });
  engine.pty.exit(engine.pty.list[0]?.id ?? "");
  await expect(screen(page)).toContainText("[proses selesai]", { timeout: 15_000 });
  await expect(tools(page).getByRole("button", { name: /powershell 1 ✕/ })).toBeVisible();
});

test("terminal: koneksi putus tapi shell hidup → menyambung ulang tanpa kehilangan riwayat", async ({ page }) => {
  const engine = await setup(page);
  await tools(page).getByRole("button", { name: "Terminal baru" }).first().click();
  await tools(page).getByTestId("terminal").click();
  await expect(screen(page)).toContainText("PS C:\\Work>", { timeout: 15_000 });
  await page.keyboard.type("ab");
  await page.keyboard.press("Enter");
  await expect(screen(page)).toContainText("hasil:ab");
  const before = engine.pty.connects;
  // putus soket dari sisi engine tanpa mengakhiri PTY (status tetap running) → klien harus menyambung ulang
  const id = engine.pty.list[0]?.id ?? "";
  engine.pty.exit(id);
  const entry = engine.pty.list.find((x) => x.id === id);
  if (entry) entry.status = "running";
  await expect.poll(() => engine.pty.connects, { timeout: 15_000 }).toBeGreaterThan(before);
  await expect(screen(page)).toContainText("hasil:ab", { timeout: 15_000 });
});

test("terminal mengikuti project: terminal project lain tidak tampil", async ({ page }) => {
  await setup(page);
  await tools(page).getByRole("button", { name: "Terminal baru" }).first().click();
  await expect(tools(page).getByRole("button", { name: "powershell 1", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/beta");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Ya, saya percaya folder ini" }).click();
  await expect(tools(page).getByText(/Belum ada terminal/)).toBeVisible();
  await expect(tools(page).getByRole("button", { name: "powershell 1", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "alpha", exact: true }).first().click();
  await expect(tools(page).getByRole("button", { name: "powershell 1", exact: true })).toBeVisible();
});

test("git di luar aplikasi desktop: pesan jelas, tidak ada data palsu", async ({ page }) => {
  await setup(page);
  await tab(page, "Git").click();
  await expect(tools(page).getByText("Git hanya tersedia di aplikasi desktop.")).toBeVisible();
});

test("info sesi: model, token, konteks, biaya dari engine, dan jumlah tool call", async ({ page }) => {
  await setup(page);
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill("tolong baca catatan");
  await box.press("Enter");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Isinya halo dunia.", { timeout: 15_000 });
  await tab(page, "Info sesi").click();
  const info = tools(page);
  await expect(info.getByTestId("info-model")).toContainText("m-satu");
  await expect(info.getByTestId("info-tokens")).toContainText("1.200 masuk");
  await expect(info.getByTestId("info-cost")).toContainText("$0.0042");
  await expect(info.getByTestId("info-tools")).toContainText("1 (1 selesai, 0 gagal, 0 berjalan)");
  await expect(info.getByTestId("info-context")).toContainText("1.200");
});

test("info sesi tanpa sesi terpilih: petunjuk, bukan angka palsu", async ({ page }) => {
  await setup(page);
  await tab(page, "Info sesi").click();
  await expect(tools(page).getByText("Pilih sesi untuk melihat infonya.")).toBeVisible();
});

test("log aktivitas: tool call dan izin tercatat; filter sumber; bersihkan", async ({ page }) => {
  await setup(page);
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill("tolong baca catatan");
  await box.press("Enter");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Isinya halo dunia.", { timeout: 15_000 });
  await box.fill("ubah file, butuh izin");
  await box.press("Enter");
  await expect(page.getByRole("alertdialog", { name: /Izin diperlukan/ })).toBeVisible({ timeout: 15_000 });
  await tab(page, "Log aktivitas aplikasi").click();
  const log = tools(page).getByTestId("activity");
  await expect(log).toContainText("read selesai");
  await expect(log).toContainText("izin diminta: edit");
  await tools(page).getByRole("combobox", { name: "Saring sumber" }).click();
  await page.getByRole("option", { name: "Tool", exact: true }).click();
  await expect(log).toContainText("read selesai");
  await expect(log).not.toContainText("izin diminta");
  await tools(page).getByRole("button", { name: "Bersihkan" }).click();
  await expect(tools(page).getByText("Belum ada aktivitas.")).toBeVisible();
});

test("browser di luar aplikasi desktop: penjelasan jujur dan alamat dibuka di tab browser", async ({ page }) => {
  await setup(page);
  await tab(page, "Browser").click();
  await expect(tools(page).getByText(/Browser tertanam hanya tersedia di aplikasi desktop/)).toBeVisible();
  const addr = tools(page).getByRole("textbox", { name: "Alamat (URL)" });
  await addr.fill("example.com");
  await addr.press("Enter");
  const opened = await page.evaluate(() => (window as unknown as { __opened: string[] }).__opened);
  expect(opened).toEqual(["https://example.com/"]);
  await addr.fill("javascript:alert(1)");
  await addr.press("Enter");
  await expect(tools(page).getByRole("alert")).toContainText("http/https");
  expect(await page.evaluate(() => (window as unknown as { __opened: string[] }).__opened)).toHaveLength(1);
});

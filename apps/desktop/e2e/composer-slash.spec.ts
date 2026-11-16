import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });

async function startSession(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await composer(page).fill("halo");
  await composer(page).press("Enter");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?", {
    timeout: 15_000,
  });
}

test("ketik / menampilkan perintah bawaan + engine; Enter memilih, Enter lagi menjalankan perintah engine", async ({
  page,
}) => {
  const engine = await mockEngine(page);
  await startSession(page);
  const box = composer(page);
  await box.fill("/");
  const list = page.getByRole("listbox", { name: "Perintah" });
  await expect(list).toBeVisible();
  await expect(list.getByRole("option", { name: /\/btw/ })).toBeVisible();
  await expect(list.getByRole("option", { name: /\/init/ })).toBeVisible();
  await box.pressSequentially("ini");
  await expect(list.getByRole("option")).toHaveCount(1);
  await box.press("Enter");
  await expect(box).toHaveValue("/init ");
  await expect(list).toHaveCount(0);
  await box.press("Enter");
  await expect.poll(() => engine.commandRuns).toEqual([{ command: "init", arguments: "" }]);
  await expect(box).toHaveValue("");
});

test("Esc menutup popup; perintah tak dikenal tidak terkirim; // mengirim garis miring apa adanya", async ({
  page,
}) => {
  const engine = await mockEngine(page);
  await startSession(page);
  const box = composer(page);
  await box.fill("/co");
  await expect(page.getByRole("listbox", { name: "Perintah" })).toBeVisible();
  await box.press("Escape");
  await expect(page.getByRole("listbox", { name: "Perintah" })).toHaveCount(0);
  await box.fill("/tidakada");
  await box.press("Escape");
  await box.press("Enter");
  await expect(page.getByText(/Perintah \/tidakada tidak dikenal/)).toBeVisible();
  await expect(box).toHaveValue("/tidakada");
  await box.fill("//jalur/berkas");
  await box.press("Escape");
  await box.press("Enter");
  await expect.poll(() => engine.prompts.at(-1)?.text).toBe("/jalur/berkas");
});

test("/btw membuka cabang baru dan bertanya di sana; /compact meringkas; /mode mengganti mode izin", async ({
  page,
}) => {
  const engine = await mockEngine(page);
  await startSession(page);
  const box = composer(page);
  await box.fill("/btw apa itu X?");
  await box.press("Escape");
  await box.press("Enter");
  await expect.poll(() => engine.forks.length).toBe(1);
  await expect.poll(() => engine.prompts.at(-1)?.text).toBe("apa itu X?");
  await expect(page.getByRole("navigation", { name: "Sesi terbuka" }).locator(".stabs__tab")).toHaveCount(2);
  await expect(page.getByRole("heading", { name: "Cabang percakapan" })).toBeVisible();
  await expect(page.getByRole("article", { name: "Asisten" })).toBeVisible({ timeout: 15_000 });

  await box.fill("/compact");
  await box.press("Escape");
  await box.press("Enter");
  await expect.poll(() => engine.summaries.length).toBe(1);

  await box.fill("/mode auto-all");
  await box.press("Escape");
  await box.press("Enter");
  await expect(page.getByRole("combobox", { name: "Mode izin" })).toContainText(/Setujui semua/);
});

test("ketik @ mencari berkas project; memilih menyisipkan @jalur dan melampirkan berkas", async ({ page }) => {
  const engine = await mockEngine(page);
  engine.project.set("src/main.ts", { text: "export {};\n" });
  engine.project.set("README.md", { text: "# Halo" });
  await startSession(page);
  const box = composer(page);
  await box.fill("cek @mai");
  const list = page.getByRole("listbox", { name: "Berkas" });
  await expect(list.getByRole("option", { name: /main\.ts/ })).toBeVisible();
  await box.press("Tab");
  await expect(box).toHaveValue("cek @src/main.ts ");
  await expect(page.locator(".composer .chip", { hasText: "src/main.ts" })).toBeVisible();
  await box.press("Enter");
  await expect.poll(() => engine.files.at(-1)?.filename).toBe("src/main.ts");
  expect(engine.files.at(-1)?.url).toMatch(/^file:\/\/.*src\/main\.ts$/);
});

test("↑ di composer kosong memanggil ulang pesan terakhir", async ({ page }) => {
  await mockEngine(page);
  await startSession(page);
  const box = composer(page);
  await box.press("ArrowUp");
  await expect(box).toHaveValue("halo");
});

import { expect, type Page, test } from "@playwright/test";
import { type MockEngine, mockEngine } from "./mock-engine";

async function setup(page: Page): Promise<MockEngine> {
  const engine = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  return engine;
}

const rail = (page: Page) => page.getByRole("navigation", { name: "Rel alat" });
const tool = (page: Page, name: string) => rail(page).getByRole("button", { name, exact: true }).click();
const send = async (page: Page, text: string) => {
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill(text);
  await box.press("Enter");
};

test("rel alat: terminal, git, perubahan, berkas sesi, browser, info, aktivitas sesi, log aplikasi", async ({
  page,
}) => {
  await setup(page);
  for (const name of [
    "Terminal",
    "Git",
    "Perubahan sesi",
    "Berkas sesi",
    "Browser",
    "Info sesi",
    "Aktivitas sesi",
    "Log aktivitas aplikasi",
  ])
    await expect(rail(page).getByRole("button", { name, exact: true })).toBeVisible();
});

test("aktivitas sesi: garis waktu pesan, tool, dan jawaban; bisa disaring dan dibuka rinciannya", async ({ page }) => {
  await setup(page);
  await send(page, "buat kode");
  await expect(page.getByRole("article", { name: "Asisten" }).first()).toBeVisible({ timeout: 15_000 });
  await tool(page, "Aktivitas sesi");
  const list = page.getByTestId("session-activity");
  await expect(list.getByRole("listitem").first()).toContainText("buat kode");
  await expect(list.getByText("Menulis berkas")).toBeVisible();
  await expect(list.getByText("src/app.ts").first()).toBeVisible();

  await page.getByRole("combobox", { name: "Saring aktivitas sesi" }).click();
  await page.getByRole("option", { name: "Tool" }).click();
  await expect(list.getByRole("listitem")).toHaveCount(1);
  await list.getByRole("button", { name: "Rincian" }).click();
  await expect(list.locator(".sact__detail")).toBeVisible();
  await expect(list.getByRole("button", { name: "Ke pesan" })).toBeVisible();
});

test("berkas sesi: berkas yang disentuh agent, dengan operasinya", async ({ page }) => {
  await setup(page);
  await send(page, "buat kode");
  await expect(page.getByRole("article", { name: "Asisten" }).first()).toBeVisible({ timeout: 15_000 });
  await tool(page, "Berkas sesi");
  const files = page.getByTestId("session-files");
  await expect(files.getByRole("button", { name: /app\.ts/ })).toBeVisible();
  await expect(files.getByText("Ditulis")).toBeVisible();
});

test("perubahan sesi: daftar berkas dari snapshot engine dengan diff berwarna dan ringkasan +/−", async ({ page }) => {
  const engine = await setup(page);
  await expect.poll(() => engine.sessions.length).toBe(1);
  const sid = engine.sessions[0]?.id ?? "";
  engine.sessionDiffs.set(sid, [
    {
      file: "src/app.ts",
      patch: "--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1,2 +1,2 @@\n-const a = 1;\n+const a = 2;\n export {};",
      additions: 1,
      deletions: 1,
      status: "modified",
    },
    { file: "baru.md", patch: "", additions: 3, deletions: 0, status: "added" },
  ]);
  await tool(page, "Perubahan sesi");
  const panel = page.getByTestId("session-changes");
  await expect(page.getByText("2 berkas · +4 −1")).toBeVisible();
  await expect(panel.getByRole("button", { name: /app\.ts/ })).toBeVisible();
  await panel.getByRole("button", { name: /app\.ts/ }).click();
  await expect(page.getByTestId("session-diff").locator(".diff__line--add")).toHaveCount(1);
  await expect(page.getByTestId("session-diff").locator(".diff__line--del")).toHaveCount(1);
  await panel.getByRole("button", { name: /baru\.md/ }).click();
  await expect(page.getByText("Tidak ada patch teks untuk berkas ini")).toBeVisible();
});

test("perubahan sesi kosong dan info sesi: status, berkas disentuh, dan keadaan tanpa sesi", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await tool(page, "Info sesi");
  await expect(page.getByText("Pilih sesi untuk melihat infonya.")).toBeVisible(); // tanpa sesi terpilih
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(page.getByTestId("info-status")).toContainText("Siap");
  await expect(page.getByTestId("info-files")).toHaveText("0");
  await tool(page, "Perubahan sesi");
  await expect(page.getByText("Sesi ini belum mengubah berkas.")).toBeVisible();
});

test("sesi demo: info dan aktivitas memakai data demo berlabel", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Demo: Trajectory & Tools" }).click();
  await tool(page, "Aktivitas sesi");
  await expect(page.getByText("Ini sesi demo: berkas contoh tidak ada di disk.")).toBeVisible();
  await expect(page.getByTestId("session-activity").getByRole("listitem").first()).toBeVisible();
  await tool(page, "Info sesi");
  await expect(page.getByText("Sesi demo", { exact: true }).first()).toBeVisible();
});

import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

// PRD §13 butir 4: minimal 3 project dan 10 sesi tanpa crash.
test("3 project dan 12 sesi: buat, kirim pesan, berpindah-pindah, muat ulang — tanpa galat/crash", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
  page.on("console", (m) => m.type() === "error" && errors.push(`console: ${m.text()}`));
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");

  const names = ["alpha", "beta", "gamma"];
  for (const n of names) {
    await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
    await page.getByLabel("Path folder").fill(`C:/Work/${n}`);
    await page.getByRole("button", { name: "Tambah", exact: true }).click();
    await page.getByRole("button", { name: "Ya, saya percaya folder ini" }).click();
    await expect(page.getByRole("button", { name: n, exact: true })).toBeVisible();
  }

  // 12 sesi: 4 + 3 + 3 di project, 2 di Chat
  const plan: Array<[string, number]> = [
    ["alpha", 4],
    ["beta", 3],
    ["gamma", 3],
    ["Chat (tanpa project)", 2],
  ];
  for (const [group, count] of plan) {
    for (let i = 0; i < count; i++) {
      await page.getByRole("button", { name: `Sesi baru di ${group}` }).click();
      await expect(page.getByRole("textbox", { name: /^Tulis pesan/ })).toBeVisible();
    }
  }
  const rows = page.locator(".sidebar__section:not(.sidebar__section--demo) .tree__children .tree__session");
  await expect(rows).toHaveCount(12);

  // kirim pesan di tiga sesi berbeda
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  for (const i of [0, 5, 11]) {
    await rows.nth(i).click();
    await box.fill(`halo dari sesi ${i}`);
    await box.press("Enter");
    await expect(page.getByRole("article", { name: "Asisten" }).last()).toContainText("Halo, ada yang bisa dibantu?", {
      timeout: 15_000,
    });
  }

  // berpindah ke semua sesi dua putaran; UI harus tetap responsif dan menampilkan panel Chat
  for (let round = 0; round < 2; round++) {
    for (let i = 0; i < 12; i++) {
      await rows.nth(i).click();
      await expect(page.getByRole("textbox", { name: /^Tulis pesan/ })).toBeVisible();
    }
  }

  // sesi yang dikirimi pesan menyimpan riwayatnya
  await rows.nth(5).click();
  await expect(page.getByRole("article", { name: "Anda" })).toContainText("halo dari sesi 5");

  // muat ulang: project, sesi, dan pilihan bertahan
  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(
    page.locator(".sidebar__section:not(.sidebar__section--demo) .tree__children .tree__session"),
  ).toHaveCount(12);
  for (const n of names) await expect(page.getByRole("button", { name: n, exact: true })).toBeVisible();

  expect(errors).toEqual([]);
});

import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("dua sesi tanpa project: folder kerja terpisah; keduanya tetap terdaftar setelah muat ulang", async ({ page }) => {
  const { sessions } = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  const add = page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" });
  await add.click();
  await add.click();
  const rows = page.locator(".sidebar__section--chats .tree__session");
  await expect(rows).toHaveCount(2);
  expect(sessions).toHaveLength(2);
  expect(sessions[0]?.directory).not.toBe(sessions[1]?.directory);
  expect(sessions[0]?.directory).toMatch(/^C:\\data\\chats\\chat-/);
  // tooltip baris memperlihatkan folder sesi itu
  await expect(rows.first()).toHaveAttribute("title", /^Folder kerja sesi: C:\\data\\chats\\chat-/);

  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(rows).toHaveCount(2);
});

test("sesi dalam project memakai folder project (bukan folder Chat)", async ({ page }) => {
  const { sessions } = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await expect.poll(() => sessions.length).toBe(1);
  expect(sessions[0]?.directory).toBe("C:\\Work\\alpha");
});

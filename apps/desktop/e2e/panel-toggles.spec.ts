import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("lima tombol tampil/sembunyi panel: Chat dan Rel bisa disembunyikan, jarak antarpanel sama", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  const toggles = page.getByRole("group", { name: "Tampilkan/sembunyikan panel" }).getByRole("button");
  await expect(toggles).toHaveCount(5);

  const chat = page.getByRole("button", { name: "Chat", exact: true });
  await chat.click();
  await expect(chat).toHaveAttribute("aria-pressed", "false");
  await expect
    .poll(async () => (await page.locator('section[aria-label="Chat"]').boundingBox())?.width ?? 0)
    .toBeLessThanOrEqual(2);
  await chat.click();
  await expect(chat).toHaveAttribute("aria-pressed", "true");

  const rail = page.getByRole("button", { name: "Rel alat", exact: true });
  await rail.click();
  await expect(page.locator(".shell-rail")).toHaveCount(0);
  await rail.click();
  await expect(page.locator(".shell-rail")).toBeVisible();

  // jarak grup panel ke rail sama dengan lebar separator
  const gap = await page.evaluate(() => {
    const g = document.querySelector(".shell-group")?.getBoundingClientRect();
    const r = document.querySelector(".shell-rail")?.getBoundingClientRect();
    const s = document.querySelector(".shell-sep")?.getBoundingClientRect();
    return g && r && s ? { gap: Math.round(r.left - g.right), sep: Math.round(s.width) } : null;
  });
  expect(gap?.gap).toBe(gap?.sep);
});

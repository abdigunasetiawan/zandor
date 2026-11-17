// Kustomisasi Panel 2: popover di bilah tab chat menulis ke appearance.chat (token --z-chat-*), sinkron dengan halaman
// Pengaturan → Chat, bertahan setelah muat ulang, dan bisa dikembalikan ke bawaan.
import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const rootVar = (page: Page, name: string) =>
  page.evaluate((n) => document.documentElement.style.getPropertyValue(n), name);

async function openSession(page: Page) {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Ya, saya percaya folder ini" }).click();
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.getByRole("textbox", { name: /^Tulis pesan/ })).toBeVisible();
}

const openPopover = async (page: Page) => {
  await page.getByRole("button", { name: "Kustomisasi panel chat" }).click();
  const pop = page.getByRole("dialog");
  await expect(pop.getByRole("slider", { name: "Ukuran huruf" }).first()).toBeVisible();
  return pop;
};

test("kustomisasi Panel 2: kerapatan, ketebalan, slider, font → token CSS; bertahan; reset", async ({ page }) => {
  await openSession(page);
  let pop = await openPopover(page);

  // Kerapatan "Lega" mengisi jarak antarpesan + tinggi baris dan tombolnya aktif.
  await pop.getByRole("button", { name: "Lega" }).click();
  await expect.poll(() => rootVar(page, "--z-chat-gap")).toBe("1.5rem");
  expect(await rootVar(page, "--z-chat-line")).toBe("1.8");
  await expect(pop.getByRole("button", { name: "Lega" })).toHaveAttribute("aria-pressed", "true");

  // Ketebalan lewat Select.
  await pop.getByRole("combobox", { name: "Ketebalan huruf" }).click();
  await page.getByRole("option", { name: "Semi tebal (600)" }).click();
  await expect.poll(() => rootVar(page, "--z-chat-weight")).toBe("600");

  // Slider dengan keyboard: End = maksimum.
  const size = pop.getByRole("slider", { name: "Ukuran huruf" }).first();
  await size.focus();
  await size.press("End");
  await expect.poll(() => rootVar(page, "--z-chat-size")).toBe("1.6rem");

  // Font lewat FontPicker.
  await pop.getByRole("button", { name: "Jenis huruf" }).click();
  await page.getByRole("button", { name: "Serif", exact: true }).click();
  await expect.poll(() => rootVar(page, "--z-chat-font")).toBe("Georgia, 'Times New Roman', serif");

  // Token benar-benar dipakai pesan di Panel 2 (bukan hanya tersimpan).
  await page.keyboard.press("Escape");
  await page.getByRole("textbox", { name: /^Tulis pesan/ }).fill("halo");
  await page.getByRole("button", { name: "Kirim" }).click();
  const msg = page.getByRole("article", { name: "Anda" }).locator(".msg__text").first();
  await expect(msg).toHaveCSS("font-weight", "600");
  await expect(msg).toHaveCSS("font-family", /Georgia/);

  // Bertahan setelah muat ulang.
  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect.poll(() => rootVar(page, "--z-chat-size")).toBe("1.6rem");
  expect(await rootVar(page, "--z-chat-gap")).toBe("1.5rem");

  // Kembalikan bawaan menghapus semua token.
  pop = await openPopover(page);
  await pop.getByRole("button", { name: "Kembalikan bawaan" }).click();
  for (const v of ["--z-chat-gap", "--z-chat-line", "--z-chat-weight", "--z-chat-size", "--z-chat-font"]) {
    await expect.poll(() => rootVar(page, v)).toBe("");
  }
  await expect(pop.getByRole("button", { name: "Kembalikan bawaan" })).toBeDisabled();
});

test("kustomisasi Panel 2 sinkron dengan Pengaturan → Chat", async ({ page }) => {
  await openSession(page);
  const pop = await openPopover(page);
  await pop.getByRole("button", { name: "Rapat" }).click();
  await expect.poll(() => rootVar(page, "--z-chat-gap")).toBe("0.375rem");
  await page.keyboard.press("Escape");

  await page.goto("/#/settings/chat");
  await expect(page.getByRole("button", { name: "Rapat" })).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("slider", { name: "Jarak antarpesan" }).first()).toHaveValue("0.375");
});

import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

// Di build web (tanpa Tauri) host memori = pratinjau di memori proses: UI tetap diuji penuh, dengan label jujur.
async function setup(page: Page) {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await expect(page.getByRole("button", { name: "alpha", exact: true })).toBeVisible();
}

test("halaman Memori & Skill: dibuka dari sidebar, label pratinjau jujur, kembali ke chat", async ({ page }) => {
  await setup(page);
  await page.getByRole("button", { name: "Menu pengguna" }).click();
  await page.getByRole("menuitem", { name: "Memori & skill" }).click();
  await expect(page.getByRole("heading", { name: "Memori & Skill" })).toBeVisible();
  await expect(page.getByText(/TIDAK dibaca engine/)).toBeVisible();
  await expect(page.getByRole("switch", { name: "Aktifkan memori secara bawaan" })).toHaveAttribute(
    "aria-checked",
    "false",
  );
  await page.getByRole("button", { name: "Kembali ke chat" }).click();
  await expect(page.locator(".sidebar__status")).toBeVisible();
});

test("palet perintah membuka halaman Memori & Skill", async ({ page }) => {
  await setup(page);
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByRole("combobox", { name: "Command palette" });
  await input.fill("memori");
  await input.press("Enter");
  await expect(page.getByRole("heading", { name: "Memori & Skill" })).toBeVisible();
});

test("opt-in per project dan per sesi; memori global menolak secret", async ({ page }) => {
  await setup(page);
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.getByRole("textbox", { name: /^Tulis pesan/ })).toBeVisible();

  // Info sesi: bawaan mati → ikuti project/bawaan, status "nonaktif"
  await page.getByRole("navigation", { name: "Rel alat" }).getByRole("button", { name: "Info sesi" }).click();
  const info = page.getByTestId("info-memory");
  await expect(info).toContainText("Sekarang: nonaktif");

  // Aktifkan bawaan di halaman Memori → panel Info ikut berubah
  await page.getByRole("button", { name: "Menu pengguna" }).click();
  await page.getByRole("menuitem", { name: "Memori & skill" }).click();
  await page.getByRole("switch", { name: "Aktifkan memori secara bawaan" }).click();
  await expect(page.getByRole("switch", { name: "Aktifkan memori secara bawaan" })).toHaveAttribute(
    "aria-checked",
    "true",
  );
  await expect(page.getByRole("combobox", { name: "Project: alpha" })).toBeVisible();

  // Memori global: secret ditolak, teks biasa tersimpan
  await page.getByRole("tab", { name: "Memori", exact: true }).click();
  const box = page.getByRole("textbox", { name: "Memori global" });
  await box.fill("- kunci sk-CONTOHPALSU0123456789abcdefghij");
  await page.getByRole("button", { name: "Simpan", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("memuat secret");
  await box.fill("- Jawab dalam Bahasa Indonesia");
  await page.getByRole("button", { name: "Simpan", exact: true }).click();
  await expect(page.getByText("Tersimpan")).toBeVisible();

  await page.getByRole("tab", { name: /Antrean skill/ }).click();
  await expect(page.getByText("Belum ada usulan skill.")).toBeVisible();
  await page.getByRole("tab", { name: "Skill terpasang" }).click();
  await expect(page.getByText("Belum ada skill terpasang.")).toBeVisible();
});

test("toggle di composer: sesi project berbagi pengaturan project; sesi Chat diatur per sesi", async ({ page }) => {
  await setup(page);
  const mem = () => page.getByRole("button", { name: "Ingat memori" });
  const skill = () => page.getByRole("button", { name: "Skill creator" });
  const newIn = async (name: string) => {
    await page.getByRole("button", { name }).first().click();
    await expect(page.getByRole("textbox", { name: /^Tulis pesan/ })).toBeVisible();
  };

  await newIn("Sesi baru di alpha");
  await expect(mem()).toHaveAttribute("aria-pressed", "false");
  await mem().click();
  await skill().click();
  await expect(mem()).toHaveAttribute("aria-pressed", "true");
  await expect(skill()).toHaveAttribute("aria-pressed", "true");
  await expect(mem()).toHaveAttribute("title", /semua sesi di project ini/);

  // sesi lain di project yang sama mengikuti pengaturan project
  await newIn("Sesi baru di alpha");
  await expect(mem()).toHaveAttribute("aria-pressed", "true");
  await expect(skill()).toHaveAttribute("aria-pressed", "true");

  // sesi Chat (tanpa project): per sesi, tidak ikut pengaturan project
  await newIn("Sesi baru di Chat (tanpa project)");
  await expect(mem()).toHaveAttribute("aria-pressed", "false");
  await expect(mem()).toHaveAttribute("title", /hanya berlaku untuk sesi ini/i);
  await mem().click();
  await expect(mem()).toHaveAttribute("aria-pressed", "true");
  await newIn("Sesi baru di Chat (tanpa project)");
  await expect(mem()).toHaveAttribute("aria-pressed", "false");
});

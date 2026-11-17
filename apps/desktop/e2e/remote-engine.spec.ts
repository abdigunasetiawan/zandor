import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("engine remote: URL tidak aman ditolak, status jujur, project remote memakai engine remote", async ({ page }) => {
  const local = await mockEngine(page);
  const remote = await mockEngine(page, { host: "https://remote.test" });
  await page.route("https://mati.test/**", (r) => r.abort("connectionrefused"));
  await page.goto("/#/settings/engines");
  await expect(page.getByText("Belum ada engine remote")).toBeVisible();

  await page.getByRole("button", { name: "Tambah engine remote" }).click();
  const form = page.getByRole("dialog", { name: "Tambah engine remote" });
  await form.getByLabel("Nama", { exact: true }).fill("VPS");
  await form.getByLabel("URL engine").fill("http://10.0.0.2:4096");
  await expect(form.getByText(/harus HTTPS/)).toBeVisible();
  await expect(form.getByRole("button", { name: "Simpan" })).toBeDisabled();
  await form.getByLabel("URL engine").fill("https://mati.test");
  await form.getByLabel("Kata sandi").fill("rahasia");
  await form.getByRole("button", { name: "Simpan" }).click();
  const list = page.getByRole("list", { name: "Engine remote" });
  await expect(list.getByText("Gagal tersambung")).toBeVisible();

  // perbaiki URL → tersambung
  await list.getByRole("button", { name: "Ubah" }).click();
  const edit = page.getByRole("dialog", { name: "Ubah engine remote" });
  await edit.getByLabel("URL engine").fill("https://remote.test/");
  await edit.getByRole("button", { name: "Simpan" }).click();
  await expect(list.getByText("Tersambung · 1.19.5")).toBeVisible();

  await page.goto("/#/");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  const add = page.getByRole("dialog", { name: /Tambah project/ });
  await add.getByRole("combobox", { name: "Engine" }).click();
  await page.getByRole("option", { name: "VPS" }).click();
  await add.getByLabel("Path folder").fill("/srv/app");
  await add.getByRole("button", { name: "Tambah", exact: true }).click();
  const trust = page.getByRole("dialog", { name: "Percayai folder ini?" });
  await expect(trust.getByText("Remote: VPS")).toBeVisible();
  await expect(trust.getByRole("switch")).toHaveCount(0); // memori project hanya untuk engine lokal
  await trust.getByRole("button", { name: "Ya, saya percaya folder ini" }).click();

  await expect(page.getByRole("button", { name: "app Remote: VPS · Tersambung · 1.19.5" })).toBeVisible();
  await page.getByRole("button", { name: "Sesi baru di app" }).click();
  await expect.poll(() => remote.sessions.length).toBe(1);
  expect(remote.sessions[0]?.directory).toBe("/srv/app");
  expect(local.sessions.some((s) => s.directory === "/srv/app")).toBe(false);

  // engine yang masih dipakai project tidak bisa dihapus
  await page.goto("/#/settings/engines");
  await list.getByRole("button", { name: "Hapus" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Hapus" }).click();
  await expect(page.getByText(/masih dipakai project: app/)).toBeVisible();
});

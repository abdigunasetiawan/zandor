import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

async function setup(page: Page) {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await expect(page.getByRole("button", { name: "alpha", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Automasi", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Automasi Terjadwal" })).toBeVisible();
}

test("halaman Automasi: peringatan jujur (hanya saat terbuka, tidak menyetujui izin sendiri), kosong di awal", async ({
  page,
}) => {
  await setup(page);
  await expect(page.getByText(/hanya berjalan selama Zandor terbuka/)).toBeVisible();
  await expect(page.getByText(/tidak menyetujui izin sendiri/)).toBeVisible();
  await expect(page.getByText("Belum ada tugas terjadwal.")).toBeVisible();
});

test("buat tugas: validasi, tampil dengan jadwal dan jalan berikutnya, nonaktifkan, hapus; bertahan setelah muat ulang", async ({
  page,
}) => {
  await setup(page);
  await page.getByRole("button", { name: "Tugas baru" }).first().click();
  const form = page.getByRole("region", { name: "Formulir tugas" });
  await form.getByLabel("Nama tugas").fill("Ringkas perubahan");
  await form.getByLabel("Prompt", { exact: true }).fill("Ringkas perubahan hari ini");
  // validasi: interval terlalu kecil
  await form.getByRole("combobox", { name: "Jadwal" }).click();
  await page.getByRole("option", { name: "Setiap beberapa menit" }).click();
  await form.getByLabel("Setiap berapa menit (5–10080)").fill("1");
  await form.getByRole("button", { name: "Simpan" }).click();
  await expect(form.getByRole("alert")).toContainText("interval harus");
  await form.getByLabel("Setiap berapa menit (5–10080)").fill("30");
  await form.getByRole("button", { name: "Simpan" }).click();

  const card = page.getByRole("listitem", { name: "Ringkas perubahan" });
  await expect(card).toContainText("Setiap 30 menit");
  await expect(card).toContainText("Berikutnya:");
  await expect(card).toContainText("Ringkas perubahan hari ini");

  await page.reload(); // rute #/automation ikut bertahan
  await expect(page.getByRole("listitem", { name: "Ringkas perubahan" })).toBeVisible();

  const sw = page.getByRole("listitem", { name: "Ringkas perubahan" }).getByRole("switch", { name: "Aktif" });
  await sw.click();
  await expect(page.getByRole("listitem", { name: "Ringkas perubahan" })).toContainText("nonaktif");

  await page.getByRole("button", { name: "Hapus Ringkas perubahan" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Hapus", exact: true }).click();
  await expect(page.getByText("Belum ada tugas terjadwal.")).toBeVisible();
});

test("jadwal harian dengan hari dipilih ditampilkan; waktu sekali di masa lalu ditolak", async ({ page }) => {
  await setup(page);
  await page.getByRole("button", { name: "Tugas baru" }).first().click();
  const form = page.getByRole("region", { name: "Formulir tugas" });
  await form.getByLabel("Nama tugas").fill("Laporan pagi");
  await form.getByLabel("Prompt", { exact: true }).fill("Buat laporan");
  await form.getByLabel("Jam (HH:MM)").fill("07:30");
  await form.getByRole("button", { name: "Sen", exact: true }).click();
  await form.getByRole("button", { name: "Rab", exact: true }).click();
  await form.getByRole("button", { name: "Simpan" }).click();
  await expect(page.getByRole("listitem", { name: "Laporan pagi" })).toContainText("Harian 07:30 (Sen, Rab)");

  await page.getByRole("button", { name: "Tugas baru" }).first().click();
  const f2 = page.getByRole("region", { name: "Formulir tugas" });
  await f2.getByLabel("Nama tugas").fill("Kemarin");
  await f2.getByLabel("Prompt", { exact: true }).fill("x");
  await f2.getByRole("combobox", { name: "Jadwal" }).click();
  await page.getByRole("option", { name: "Sekali" }).click();
  await f2.getByLabel("Waktu").fill("2020-01-01T10:00");
  await f2.getByRole("button", { name: "Simpan" }).click();
  await expect(f2.getByRole("alert")).toContainText("sudah lewat");
});

test("Jalankan sekarang: membuat sesi di project, mengirim prompt, notifikasi in-app; sesi bisa dibuka", async ({
  page,
}) => {
  await setup(page);
  await page.getByRole("button", { name: "Tugas baru" }).first().click();
  const form = page.getByRole("region", { name: "Formulir tugas" });
  await form.getByLabel("Nama tugas").fill("Cek proyek");
  await form.getByLabel("Prompt", { exact: true }).fill("Periksa kondisi proyek");
  await form.getByRole("combobox", { name: "Project" }).click();
  await page.getByRole("option", { name: "alpha" }).click();
  await form.getByRole("button", { name: "Simpan" }).click();

  await page.getByRole("button", { name: "Jalankan sekarang Cek proyek" }).click();
  await expect(page.getByText(/Tugas Cek proyek dimulai/)).toBeVisible();
  await expect(page.getByText(/Tugas Cek proyek selesai/)).toBeVisible({ timeout: 20_000 });
  const card = page.getByRole("listitem", { name: "Cek proyek" });
  await expect(card).not.toContainText("Terakhir: —");

  await card.getByRole("button", { name: "Buka sesi terakhir Cek proyek" }).click();
  await expect(page.getByRole("article", { name: "Anda" })).toContainText("Periksa kondisi proyek");
  await expect(page.getByRole("article", { name: "Asisten" }).last()).toContainText("Halo");
});

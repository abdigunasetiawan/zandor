import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const next = (page: import("@playwright/test").Page) =>
  page.getByRole("main").getByRole("button", { name: "Berikutnya" }).click();

test("onboarding: sambutan, profil, provider, cek sistem, memori, impor dengan konflik, terima kasih", async ({
  page,
}) => {
  await mockEngine(page, { onboarding: true });
  await page.goto("/");
  const main = page.getByRole("main", { name: "Selamat datang di Zandor" });
  await expect(main).toBeVisible();
  await expect(page).toHaveURL(/#\/welcome$/);
  // tanpa header aplikasi
  await expect(page.getByRole("button", { name: "Pengaturan" })).toHaveCount(0);

  // 0. sambutan: bahasa dipilih di sini dan langsung berlaku
  const lang = main.getByRole("group", { name: "Bahasa" });
  await lang.getByText("English").click();
  await expect(page.getByRole("main", { name: "Welcome to Zandor" })).toBeVisible();
  await page.getByRole("group", { name: "Language" }).getByText("Indonesia").click();
  await expect(main).toBeVisible();
  // tema juga dipilih di sini
  const theme = main.getByRole("group", { name: "Tema" });
  await theme.getByText("Terang").click();
  await expect(theme.getByRole("radio", { name: "Terang" })).toBeChecked();
  await theme.getByText("Gelap").click();
  await expect(theme.getByRole("radio", { name: "Gelap" })).toBeChecked();
  await expect(main).toContainText("Sebelum mulai, kita siapkan beberapa hal: profil, provider dan model");
  await main.getByRole("button", { name: "Lanjut" }).click();

  // 1. profil: nama berspasi bisa diketik; placeholder bukan nama akun; tab ilustrasi aktif bawaan
  const name = main.getByLabel("Nama tampilan");
  await expect(name).toHaveAttribute("placeholder", "Nama Anda, mis. Rina");
  await name.pressSequentially("Rina Sari");
  await expect(name).toHaveValue("Rina Sari");
  await expect(main.getByRole("tab", { name: "Ilustrasi" })).toHaveAttribute("aria-selected", "true");
  await next(page);

  // 2. provider
  await expect(main.getByRole("heading", { name: "Provider dan model" })).toBeVisible();
  await expect(main.getByRole("status").first()).toContainText("1 model siap");
  await next(page);

  // 3. pemeriksaan sistem
  await expect(main.getByRole("list", { name: "Alat pengembangan" })).toContainText("Git");
  await expect(main.getByRole("list", { name: "Python" })).toContainText("pip");
  await next(page);

  // 4. memori & skill
  const mem = main.getByRole("group", { name: "Memori" });
  await expect(mem).toContainText("Tentukan sendiri");
  await mem.getByText("Otomatis").click();
  await expect(mem.getByRole("radio", { name: /Otomatis/ })).toBeChecked();
  await next(page);

  // 5. impor memori: dua AI berbeda pendapat soal nama → konflik wajib dipilih
  await expect(main.getByRole("button", { name: "Salin prompt" })).toBeVisible();
  const paste = main.getByLabel("Tempel balasan AI di sini…");
  await paste.fill("```\n- nama: Abdi\n- bahasa: Indonesia\n- proyek: Zandor\n```");
  await main.getByRole("button", { name: "Tambahkan" }).click();
  await main.getByRole("combobox", { name: "Balasan dari" }).click();
  await page.getByRole("option", { name: "Gemini" }).click();
  await paste.fill("- nama: Adi\n- bahasa: indonesia\n- proyek: Toko Online");
  await main.getByRole("button", { name: "Tambahkan" }).click();
  const conflict = main.getByRole("group", { name: "nama" });
  await expect(conflict).toContainText("dari ChatGPT");
  await expect(conflict).toContainText("dari Gemini");
  const save = main.getByRole("button", { name: "Simpan ke memori" });
  await expect(save).toBeDisabled();
  // tulis sendiri: kosong = belum dipilih; berisi = dipakai
  await conflict.getByText("Tulis sendiri").click();
  await expect(save).toBeDisabled();
  await conflict.getByLabel("Nilai untuk nama").fill("Abdi Guna");
  await expect(main.locator(".onb__prompt").last()).toHaveText(
    "- nama: Abdi Guna\n- bahasa: Indonesia\n- proyek: Zandor\n- proyek: Toko Online\n",
  );
  await conflict.getByText("Abdi", { exact: true }).click();
  // kunci bernilai banyak: bukan konflik, semua tercentang; buang satu, tambah satu
  const proyek = main.getByRole("group", { name: "proyek" });
  await expect(proyek.getByRole("checkbox")).toHaveCount(2);
  await proyek.getByText("Toko Online").click();
  await proyek.getByRole("button", { name: "Tambah nilai" }).click();
  await proyek.getByLabel("Nilai untuk proyek").fill("Blog pribadi");
  await expect(main.locator(".onb__prompt").last()).toHaveText(
    "- nama: Abdi\n- bahasa: Indonesia\n- proyek: Zandor\n- proyek: Blog pribadi\n",
  );
  await save.click();
  await expect(main.getByRole("status").filter({ hasText: "Memori tersimpan." })).toBeVisible();
  await next(page);

  // 6. terima kasih
  await expect(main).toContainText("Terima kasih sudah mencoba Zandor!");
  await main.getByRole("button", { name: "Mulai pakai Zandor" }).click();
  await expect(main).toBeHidden();

  await page.reload();
  await expect(page.getByRole("main", { name: "Selamat datang di Zandor" })).toBeHidden();
  await expect(page.getByText("Rina Sari").first()).toBeVisible();

  // bisa dibuka lagi dari palet
  await expect(async () => {
    await page.keyboard.press("Control+K");
    await expect(page.getByRole("dialog").getByRole("combobox")).toBeVisible({ timeout: 1000 });
  }).toPass();
  await page.getByRole("dialog").getByRole("combobox").fill("Tur pengenalan");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("main", { name: "Selamat datang di Zandor" })).toBeVisible();
});

test("impor memori tersimpan otomatis saat pindah langkah dan muncul di Pengaturan → Memori", async ({ page }) => {
  await mockEngine(page, { onboarding: true });
  await page.goto("/");
  const main = page.getByRole("main", { name: "Selamat datang di Zandor" });
  await main.getByRole("button", { name: "Lanjut" }).click();
  await main.getByRole("button", { name: "Impor memori" }).click();
  const paste = main.getByLabel("Tempel balasan AI di sini…");
  await paste.fill("- nama: Abdi\n- proyek: Zandor");
  await main.getByRole("button", { name: "Tambahkan" }).click();
  await main.getByRole("combobox", { name: "Balasan dari" }).click();
  await page.getByRole("option", { name: "Gemini" }).click();
  await paste.fill("- nama: Adi");
  await main.getByRole("button", { name: "Tambahkan" }).click();

  // konflik belum dipilih → tidak bisa pindah
  await next(page);
  await expect(main.getByRole("heading", { name: "Impor memori" })).toBeVisible();
  await expect(page.getByText("Masih ada 1 konflik yang belum dipilih.").first()).toBeVisible();

  // pilih, lalu Berikutnya tanpa menekan "Simpan ke memori" → tersimpan otomatis
  await main.getByRole("group", { name: "nama" }).getByText("Abdi", { exact: true }).click();
  await next(page);
  await expect(main.getByRole("heading", { name: "Terima kasih" })).toBeVisible();
  await main.getByRole("button", { name: "Mulai pakai Zandor" }).click();

  // navigasi di dalam aplikasi (memuat ulang menghapus memori pratinjau di browser)
  await page.evaluate(() => {
    window.location.hash = "#/memory";
  });
  await page.getByRole("tab", { name: "Memori", exact: true }).click();
  await expect(page.getByRole("tabpanel").getByRole("textbox").first()).toHaveValue(
    "- nama: Abdi\n- proyek: Zandor\n",
  );
});

import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });

async function openProjectSession(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: /^Sesi baru di .*alpha/ }).click();
  await expect(composer(page)).toBeVisible();
}

async function openChatSession(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(composer(page)).toBeVisible();
}

const send = async (page: Page, text: string) => {
  await composer(page).fill(text);
  await composer(page).press("Enter");
};

test("saat bekerja: hanya 'Bekerja selama…' + langkah terakhir (terlipat), logo model berkedip; berhenti saat selesai", async ({
  page,
}) => {
  await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tugas bekerja");
  await expect(page.getByText(/Bekerja selama \d+d/)).toBeVisible({ timeout: 10_000 });
  // langkah terakhir = Mencari di internet (+ kueri); proses tidak ditumpuk
  const step = page.locator(".work__step");
  await expect(step).toContainText("Mencari di internet");
  await expect(step).toContainText("cuaca jakarta");
  await expect(step).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator(".fold--tool")).toHaveCount(0);
  // logo model berkedip saat bekerja
  await expect(page.locator(".msg__meta[data-working] .z-model-logo--working")).toBeVisible();
  // bisa dibuka manual
  await step.click();
  await expect(step).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator(".fold--tool")).toHaveCount(1);
  await step.click();
  await expect(page.locator(".fold--tool")).toHaveCount(0);
  // berhenti → tidak berkedip lagi
  await page.getByRole("button", { name: "Hentikan" }).click();
  await expect(page.locator(".z-model-logo--working")).toHaveCount(0);
  await expect(page.locator(".msg__meta[data-working]")).toHaveCount(0);
});

test("setelah disajikan: ringkasan proses terlipat, jawaban, baris info (provider • model • durasi • waktu) dan aksi", async ({
  page,
}) => {
  await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tolong baca catatan");
  const msg = page.getByRole("article", { name: "Asisten" });
  await expect(msg).toContainText("Isinya halo dunia.", { timeout: 15_000 });
  const sum = msg.getByRole("button", { name: /^Proses · 1 langkah/ });
  await expect(sum).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator(".fold--tool")).toHaveCount(0);
  await sum.click();
  await expect(page.locator(".fold--tool")).toHaveCount(1);
  // baris info + aksi selalu terlihat
  const meta = msg.locator(".msg__meta");
  await expect(meta).toContainText("Mock");
  await expect(meta).toContainText("m-satu");
  await expect(meta).toContainText(/\d+d/);
  await expect(meta).not.toHaveAttribute("data-working", "");
  await expect(msg.getByRole("button", { name: "Salin pesan" })).toBeVisible();
  await expect(msg.getByRole("button", { name: /Cabangkan/ })).toBeVisible();
  await expect(msg.getByRole("button", { name: /Ulangi|Buat ulang/ })).toBeVisible();
});

test("kartu dokumen: muncul hanya di sesi TANPA project (Chat) saat agent membuat dokumen (Buka + Unduh); bukan untuk kode", async ({
  page,
}) => {
  await mockEngine(page);
  await openChatSession(page);
  await send(page, "tolong buat dokumen ringkasan");
  // engine memecah giliran jadi 2 pesan asisten (tool call lalu jawaban) → tetap SATU giliran: satu artikel, satu baris info
  await expect(page.getByRole("article", { name: "Asisten" })).toHaveCount(1, { timeout: 15_000 });
  await expect(page.locator(".msg__foot")).toHaveCount(1);
  await expect(page.getByRole("article", { name: "Asisten" }).getByRole("button", { name: "Salin pesan" })).toHaveCount(
    1,
  );
  const card = page.getByTestId("file-card");
  await expect(card).toHaveCount(1, { timeout: 15_000 });
  await expect(card).toContainText("ringkasan.md");
  await expect(card.getByRole("button", { name: /^Buka/ })).toBeVisible();
  await expect(card.getByRole("button", { name: "Unduh ringkasan.md" })).toBeVisible();
  await expect(card).not.toContainText("agen"); // tanpa teks tambahan seperti "Tidak ada teks untuk agen"

  // berkas kode bukan dokumen → tanpa kartu
  await send(page, "tolong buat kode");
  await expect(page.getByRole("article", { name: "Asisten" })).toHaveCount(2, { timeout: 15_000 });
  await expect(page.getByTestId("file-card")).toHaveCount(1);
});

test("kartu dokumen tidak tampil di sesi project (berkasnya sudah ada di pohon berkas)", async ({ page }) => {
  await mockEngine(page);
  await openProjectSession(page);
  await send(page, "tolong buat dokumen ringkasan");
  await expect(page.getByRole("article", { name: "Asisten" })).toBeVisible({ timeout: 15_000 });
  await expect(page.getByRole("button", { name: /^Proses · 1 langkah/ })).toBeVisible();
  await expect(page.getByTestId("file-card")).toHaveCount(0);
});

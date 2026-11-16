import { expect, type Page, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

const expandProcess = async (page: Page) => {
  const toggle = page.locator(".work__sum, .work__step").last();
  await expect(toggle).toBeVisible({ timeout: 10_000 });
  if ((await toggle.getAttribute("aria-expanded")) !== "true") await toggle.click();
};

async function openSession(page: Page) {
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:\\Work\\alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Ya, saya percaya folder ini" }).click();
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.getByRole("textbox", { name: /^Tulis pesan/ })).toBeVisible();
}

const composer = (page: Page) => page.getByRole("textbox", { name: /^Tulis pesan/ });

test("kirim pesan dan terima jawaban streaming ber-Markdown; pilihan model ikut terkirim", async ({ page }) => {
  const engine = await mockEngine(page, {
    models: [
      { id: "m-satu", name: "Model Satu" },
      { id: "m-dua", name: "Model Dua" },
    ],
  });
  await openSession(page);
  await expect(page.getByRole("button", { name: /^Model: / })).toBeVisible();

  await composer(page).fill("halo");
  await page.getByRole("button", { name: "Kirim" }).click();
  await expect(page.getByRole("article", { name: "Anda" })).toContainText("halo");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?", {
    timeout: 10_000,
  });
  expect(engine.prompts[0]).toMatchObject({ text: "halo", model: { providerID: "mock", modelID: "m-satu" } });
  await expect(composer(page)).toHaveValue("");
  // selesai → tombol kembali "Kirim", meta model tampil
  await expect(page.getByRole("button", { name: "Kirim" })).toBeVisible();
  await expect(page.locator(".msg__meta")).toContainText("m-satu");
});

test("composer siap ketik: sesi baru, Chat baru, dan setelah kirim lewat tombol", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  await expect(composer(page)).toBeFocused();
  await composer(page).fill("halo");
  await page.getByRole("button", { name: "Kirim" }).click();
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo", { timeout: 10_000 });
  await expect(composer(page)).toBeFocused();
  await page.getByRole("button", { name: /^Chat baru/ }).click();
  await expect(composer(page)).toBeFocused();
});

test("Enter mengirim, Shift+Enter baris baru", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  const box = composer(page);
  await box.fill("baris satu");
  await box.press("Shift+Enter");
  await box.type("baris dua");
  await expect(box).toHaveValue("baris satu\nbaris dua");
  expect(engine.prompts).toHaveLength(0);
  await box.press("Enter");
  await expect.poll(() => engine.prompts.length).toBe(1);
  expect(engine.prompts[0]?.text).toBe("baris satu\nbaris dua");
});

const pasteText = (page: Page, text: string) =>
  composer(page).evaluate((el, value) => {
    const data = new DataTransfer();
    data.setData("text/plain", value);
    el.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  }, text);

test("composer membesar mengikuti isi sampai batas lalu menggulir", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  const box = composer(page);
  const small = (await box.boundingBox())?.height ?? 0;
  await box.fill(Array(8).fill("baris").join("\n"));
  const grown = (await box.boundingBox())?.height ?? 0;
  expect(grown).toBeGreaterThan(small);
  await box.fill(Array(200).fill("baris").join("\n"));
  const capped = await box.evaluate((el) => ({ h: el.clientHeight, sh: el.scrollHeight, max: innerHeight * 0.4 }));
  expect(capped.h).toBeLessThanOrEqual(capped.max + 1);
  expect(capped.sh).toBeGreaterThan(capped.h);
});

test("teks panjang di composer: gulir naik-turun tidak menggeser composer", async ({ page }) => {
  // layar laptop pendek: di sini halaman sapaan dulu meluap dan ikut tergulir (guliran merambat dari textarea)
  await page.setViewportSize({ width: 1000, height: 560 });
  await mockEngine(page);
  await openSession(page);
  const box = composer(page);
  await box.fill(Array.from({ length: 300 }, (_, i) => `baris ${i + 1} dengan isi yang cukup panjang`).join("\n"));
  const card = page.locator(".composer__card").last();
  const before = await card.boundingBox();
  await box.hover();
  for (const dy of [-3000, 1500, -800, 3000, -3000]) {
    await page.mouse.wheel(0, dy);
    await page.waitForTimeout(60);
  }
  // kursor di akhir teks, lalu gulir ke atas: baris kursor keluar layar → kursor halus tak boleh tergambar di luar textarea
  await box.press("Control+End");
  await page.mouse.wheel(0, -100000);
  await page.waitForTimeout(150);
  const caretOut = await box.evaluate((el) => {
    const c = el.parentElement?.querySelector(".composer__caret") as HTMLElement;
    const r = c.getBoundingClientRect();
    const t = el.getBoundingClientRect();
    const shown = getComputedStyle(c).opacity !== "0" && c.dataset.visible !== undefined;
    return shown && (r.top < t.top - 1 || r.bottom > t.bottom + 1);
  });
  expect(caretOut).toBe(false);
  await box.press("Control+Home");
  await box.press("Control+End");
  await page.waitForTimeout(100);
  const after = await card.boundingBox();
  expect(Math.abs((after?.y ?? 0) - (before?.y ?? 0))).toBeLessThan(1);
  expect(Math.abs((after?.x ?? 0) - (before?.x ?? 0))).toBeLessThan(1);
  // tidak ada leluhur composer yang ikut tergulir
  const shifted = await box.evaluate((el) => {
    const out: string[] = [];
    for (let p = el.parentElement; p; p = p.parentElement)
      if (p.scrollTop !== 0 || p.scrollLeft !== 0) out.push(`${p.className}:${p.scrollTop},${p.scrollLeft}`);
    return out;
  });
  expect(shifted).toEqual([]);
});

test("tempel teks panjang menjadi lampiran teks, bisa dikembalikan, dan terkirim utuh", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  const long = Array.from({ length: 40 }, (_, i) => `baris ${i + 1}`).join("\n");
  await pasteText(page, long);
  await expect(composer(page)).toHaveValue("");
  await expect(page.getByText("Teks ditempel · 40 baris")).toBeVisible();
  // kembalikan jadi teks biasa
  await page.getByRole("button", { name: "Kembalikan jadi teks biasa di composer" }).click();
  await expect(composer(page)).toHaveValue(long);
  await expect(page.getByText("Teks ditempel · 40 baris")).toHaveCount(0);
  // tempel lagi lalu kirim: isi sampai ke engine sebagai berkas teks
  await composer(page).fill("");
  await pasteText(page, long);
  await composer(page).fill("ringkas ini");
  await composer(page).press("Enter");
  await expect.poll(() => engine.files.length).toBe(1);
  const sent = engine.files[0];
  expect(sent?.mime).toBe("text/plain");
  const b64 = sent?.url.slice(sent.url.indexOf(",") + 1) ?? "";
  expect(Buffer.from(b64, "base64").toString("utf8")).toBe(long);
});

test("tempel teks pendek tetap masuk ke composer", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  await pasteText(page, "pendek saja");
  await expect(page.getByText(/Teks ditempel/)).toHaveCount(0);
});

test("tool call tampil sebagai kartu yang bisa dibuka", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  await composer(page).fill("tolong baca catatan");
  await composer(page).press("Enter");
  await expandProcess(page);
  const card = page.locator(".fold--tool");
  await expect(card).toContainText("read", { timeout: 10_000 });
  await expect(card).toContainText("Selesai");
  await card.locator(".fold__head").click();
  await expect(card).toContainText("halo dunia");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Isinya halo dunia.");
});

test("kartu izin: Izinkan sekali dikirim ke engine lalu kartu hilang dan jawaban lanjut", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  await composer(page).fill("ubah file, butuh izin");
  await composer(page).press("Enter");
  const card = page.getByRole("alertdialog", { name: /Izin diperlukan: edit/ });
  await expect(card).toBeVisible({ timeout: 10_000 });
  await expect(card).toContainText(".env.local");
  await expect(card).toContainText("berkas rahasia"); // alasan dari mode bawaan "Otomatis, kecuali aksi penting"
  await card.getByRole("button", { name: "Izinkan sekali" }).click();
  await expect(card).toHaveCount(0);
  expect(engine.permissionReplies[0]?.reply).toBe("once");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Sudah diubah.", { timeout: 10_000 });
});

test("mode bawaan menyetujui edit berkas biasa tanpa kartu izin", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  await expect(page.getByRole("combobox", { name: "Mode izin" })).toHaveText(/Otomatis, kecuali aksi penting/);
  await composer(page).fill("ubah file aman, butuh izin");
  await composer(page).press("Enter");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Sudah diubah.", { timeout: 10_000 });
  expect(engine.permissionReplies[0]?.reply).toBe("once");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
});

test("kartu izin: Tolak", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  await composer(page).fill("hapus, butuh izin");
  await composer(page).press("Enter");
  const card = page.getByRole("alertdialog", { name: /Izin diperlukan/ });
  await card.getByRole("button", { name: "Tolak" }).click();
  await expect(card).toHaveCount(0);
  expect(engine.permissionReplies[0]?.reply).toBe("reject");
});

test("Hentikan membatalkan giliran yang sedang berjalan", async ({ page }) => {
  const engine = await mockEngine(page);
  await openSession(page);
  await composer(page).fill("tugas lambat");
  await composer(page).press("Enter");
  const stop = page.getByRole("button", { name: "Hentikan" });
  await expect(stop).toBeVisible({ timeout: 10_000 });
  await stop.click();
  await expect.poll(() => engine.aborted.length).toBe(1);
  await expect(page.getByRole("button", { name: "Kirim" })).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("Dihentikan")).toBeVisible();
});

test("tanpa provider: composer memberi tahu dan Kirim nonaktif", async ({ page }) => {
  await mockEngine(page, { models: [] });
  await openSession(page);
  await expect(page.getByText(/Belum ada model/)).toBeVisible();
  await composer(page).fill("halo");
  await expect(page.getByRole("button", { name: "Kirim" })).toBeDisabled();
});

test("riwayat dimuat ulang setelah muat ulang halaman", async ({ page }) => {
  await mockEngine(page);
  await openSession(page);
  await composer(page).fill("halo");
  await composer(page).press("Enter");
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?", {
    timeout: 10_000,
  });
  await page.reload();
  await expect(page.getByRole("article", { name: "Anda" })).toContainText("halo", { timeout: 10_000 });
  await expect(page.getByRole("article", { name: "Asisten" })).toContainText("Halo, ada yang bisa dibantu?");
});

test("pemilih model: popover (bukan modal), fokus ke pencarian, terakhir digunakan, grup provider bisa dilipat", async ({
  page,
}) => {
  await mockEngine(page, {
    models: [
      { id: "m-satu", name: "Model Satu" },
      { id: "m-dua", name: "Model Dua" },
      { id: "m-tiga", name: "Model Tiga" },
    ],
  });
  await openSession(page);
  await page.getByRole("button", { name: /^Model: / }).click();
  const pop = page.getByRole("dialog", { name: "Pilih model" });
  await expect(pop).toBeVisible();
  await expect(pop.getByRole("searchbox", { name: "Cari model atau provider" })).toBeFocused();
  // popover menempel pada pemicu (tepat di atasnya), bukan modal di tengah layar
  const box = await pop.boundingBox();
  const trig = await page.getByRole("button", { name: /^Model: / }).boundingBox();
  if (!box || !trig) throw new Error("ukuran tidak terbaca");
  expect(box.y + box.height).toBeLessThanOrEqual(trig.y + 1);
  expect(trig.y - (box.y + box.height)).toBeLessThan(40);

  // pilih → masuk "Terakhir digunakan"
  await pop
    .getByRole("button", { name: /Model Dua/ })
    .first()
    .click();
  await expect(pop).toHaveCount(0);
  await page.getByRole("button", { name: /^Model: Model Dua/ }).click();
  await expect(page.getByRole("region", { name: "Terakhir digunakan" })).toContainText("Model Dua");

  // grup provider bisa dilipat dan dibuka
  const group = page.getByRole("dialog", { name: "Pilih model" }).locator(".mgroup__toggle");
  await expect(group).toHaveAttribute("aria-expanded", "true");
  await group.click();
  await expect(group).toHaveAttribute("aria-expanded", "false");
  await group.click();
  await expect(group).toHaveAttribute("aria-expanded", "true");
});

test("pemilih model: tab Uji (ping) dan tab per provider", async ({ page }) => {
  await mockEngine(page, {
    models: [
      { id: "m-satu", name: "Model Satu" },
      { id: "m-dua", name: "Model Dua" },
    ],
  });
  await openSession(page);
  await page.getByRole("button", { name: /^Model: / }).click();
  const pop = page.getByRole("dialog", { name: "Pilih model" });
  await expect(pop.getByRole("tab", { name: "Model" })).toHaveAttribute("aria-selected", "true");
  await expect(pop.getByRole("tab", { name: "Uji" })).toBeVisible();

  // provider bawaan (bukan koneksi milik pengguna) tidak bisa diuji → penjelasan jujur
  await pop.getByRole("tab", { name: "Uji" }).click();
  await expect(pop).toContainText("hanya tersedia di aplikasi desktop");
  await expect(pop).toContainText("Tidak bisa diuji"); // model provider katalog tetap tampil, bukan disembunyikan

  // tab provider berisi model provider itu
  await pop.getByRole("tab", { name: /Mock/ }).click();
  await expect(pop.getByRole("button", { name: "Mock Model Dua" })).toBeVisible();
  await expect(pop).toContainText("2 model");
});

test("pemilih model: semua bagian di tab Model bisa dilipat", async ({ page }) => {
  await mockEngine(page, { models: [{ id: "m-satu", name: "Model Satu" }] });
  await openSession(page);
  await page.getByRole("button", { name: /^Model: / }).click();
  const pop = page.getByRole("dialog", { name: "Pilih model" });
  const all = pop.getByRole("button", { name: /^Semua provider/ });
  await expect(all).toHaveAttribute("aria-expanded", "true");
  await all.click();
  await expect(all).toHaveAttribute("aria-expanded", "false");
  await expect(pop.locator(".mgroup")).toHaveCount(0);
});

test("pemilih model: tinggi popover tetap saat berpindah tab", async ({ page }) => {
  await mockEngine(page, {
    models: [
      { id: "m-satu", name: "Model Satu" },
      { id: "m-dua", name: "Model Dua" },
    ],
  });
  await openSession(page);
  await page.getByRole("button", { name: /^Model: / }).click();
  const pop = page.getByRole("dialog", { name: "Pilih model" });
  await expect(pop).toBeVisible();
  const heights: number[] = [];
  for (const name of ["Model", "Uji", /Mock/]) {
    await pop.getByRole("tab", { name }).click();
    await page.waitForTimeout(200);
    heights.push((await pop.boundingBox())?.height ?? 0);
  }
  expect(heights[0]).toBeGreaterThan(100);
  expect(new Set(heights.map((h) => Math.round(h))).size).toBe(1);
});

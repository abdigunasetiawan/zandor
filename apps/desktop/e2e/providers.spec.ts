import { expect, type Page, test } from "@playwright/test";
import { type MockOptions, mockEngine } from "./mock-engine";

type Seen = { auth: string[]; pings: string[] };

/** Endpoint provider tiruan di http://provider.test/v1 (dicegat; tanpa jaringan). */
async function mockProvider(
  page: Page,
  opts: { models?: string[]; modelsStatus?: number; pingStatus?: Record<string, number> } = {},
): Promise<Seen> {
  const seen: Seen = { auth: [], pings: [] };
  const cors = {
    "access-control-allow-origin": "*",
    "access-control-allow-headers": "*",
    "access-control-allow-methods": "*",
  };
  await page.route("http://provider.test/**", async (route) => {
    const req = route.request();
    if (req.method() === "OPTIONS") return route.fulfill({ status: 204, headers: cors });
    const url = new URL(req.url());
    const h = { ...cors, "content-type": "application/json" };
    if (url.pathname === "/v1/models") {
      seen.auth.push(req.headers().authorization ?? "");
      const status = opts.modelsStatus ?? 200;
      if (status !== 200) return route.fulfill({ status, headers: h, body: JSON.stringify({ error: "kunci salah" }) });
      return route.fulfill({
        status: 200,
        headers: h,
        body: JSON.stringify({ data: (opts.models ?? ["m-a", "m-b"]).map((id) => ({ id })) }),
      });
    }
    if (url.pathname === "/v1/chat/completions") {
      const body = req.postDataJSON() as { model: string };
      seen.pings.push(body.model);
      const status = opts.pingStatus?.[body.model] ?? 200;
      return route.fulfill({
        status,
        headers: h,
        body: status === 200 ? "{}" : JSON.stringify({ error: "kuota habis" }),
      });
    }
    return route.fulfill({ status: 404, headers: h, body: "{}" });
  });
  return seen;
}

async function openProviders(page: Page, engine: MockOptions | null = {}) {
  if (engine) await mockEngine(page, engine);
  await page.addInitScript(() => {
    (window as unknown as { __opened: string[] }).__opened = [];
    window.open = ((url?: string | URL) => {
      (window as unknown as { __opened: string[] }).__opened.push(String(url));
      return null;
    }) as typeof window.open;
  });
  await page.goto("/#/providers");
  await expect(page.getByRole("heading", { name: "Provider & model" })).toBeVisible();
}

const addButton = (page: Page) => page.getByRole("button", { name: "Tambah koneksi" });
const dialog = (page: Page) =>
  page.getByRole("dialog").filter({ has: page.getByRole("heading", { name: /koneksi|Pilih model/ }) });

async function addCustom(page: Page) {
  await addButton(page).click();
  await dialog(page).getByLabel("Nama").fill("Provider Uji");
  await dialog(page).getByLabel("Base URL (kompatibel OpenAI)").fill("http://provider.test/v1");
  await dialog(page).getByLabel("Kunci API").fill("sk-uji-123");
}

test("tambah endpoint kustom: ambil model, pilih, tambah manual, simpan; kunci tidak tampil di halaman", async ({
  page,
}) => {
  const seen = await mockProvider(page);
  await openProviders(page);
  await expect(page.getByText(/Belum ada koneksi/)).toBeVisible();
  await addCustom(page);
  await dialog(page).getByRole("button", { name: "Ambil daftar model" }).click();
  await expect(dialog(page).getByText("2 model ditemukan")).toBeVisible();
  expect(seen.auth[0]).toBe("Bearer sk-uji-123");
  await dialog(page).getByRole("checkbox", { name: "m-a" }).check();
  await dialog(page).getByRole("textbox", { name: "ID model" }).fill("m-manual");
  await dialog(page).getByRole("button", { name: "Tambah", exact: true }).click();
  await expect(dialog(page).getByText("2 model dipilih")).toBeVisible();
  await dialog(page).getByRole("button", { name: "Simpan" }).click();

  const card = page.getByRole("listitem", { name: "Provider Uji" });
  await expect(card).toBeVisible();
  await expect(card).toContainText("http://provider.test/v1");
  await expect(card).toContainText("Kunci tersimpan");
  await expect(card).toContainText("m-a");
  await expect(card).toContainText("m-manual");
  await expect(page.locator("body")).not.toContainText("sk-uji-123");
});

test("gagal mengambil model ditampilkan jujur (kunci ditolak), tidak ada spinner abadi", async ({ page }) => {
  await mockProvider(page, { modelsStatus: 401 });
  await openProviders(page);
  await addCustom(page);
  await dialog(page).getByRole("button", { name: "Ambil daftar model" }).click();
  await expect(dialog(page).getByText(/Gagal mengambil model: .*401/)).toBeVisible();
  await expect(dialog(page).getByRole("button", { name: "Ambil daftar model" })).toBeEnabled();
});

test("validasi: simpan tanpa model ditolak dengan pesan", async ({ page }) => {
  await mockProvider(page);
  await openProviders(page);
  await addCustom(page);
  await dialog(page).getByRole("button", { name: "Simpan" }).click();
  await expect(page.getByText(/minimal satu model/)).toBeVisible();
  await expect(dialog(page)).toBeVisible();
});

test("uji model: OK dengan latensi, dan rate limit tampil apa adanya; hapus koneksi dengan konfirmasi", async ({
  page,
}) => {
  const seen = await mockProvider(page, { pingStatus: { "m-b": 429 } });
  await openProviders(page);
  await addCustom(page);
  await dialog(page).getByRole("button", { name: "Ambil daftar model" }).click();
  await dialog(page).getByRole("checkbox", { name: "m-a" }).check();
  await dialog(page).getByRole("checkbox", { name: "m-b" }).check();
  await dialog(page).getByRole("button", { name: "Simpan" }).click();
  const card = page.getByRole("listitem", { name: "Provider Uji" });

  await card.getByRole("button", { name: "Uji m-a" }).click();
  await expect(card).toContainText(/OK · \d+ ms/);
  await card.getByRole("button", { name: "Uji m-b" }).click();
  await expect(card).toContainText("Rate limit / kuota");
  await expect(card).toContainText("kuota habis");
  expect(seen.pings).toEqual(["m-a", "m-b"]);

  await card.getByRole("button", { name: "Hapus koneksi" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("Kunci di keychain ikut dihapus");
  await page.getByRole("alertdialog").getByRole("button", { name: "Hapus koneksi" }).click();
  await expect(card).toHaveCount(0);
});

test("tab Lokal: tidak ada server terdeteksi → penjelasan; preset mengisi base URL", async ({ page }) => {
  await openProviders(page);
  await addButton(page).click();
  await dialog(page).getByRole("tab", { name: "Lokal" }).click();
  await dialog(page).getByRole("button", { name: "Deteksi Ollama / LM Studio" }).click();
  await expect(dialog(page).getByText(/Tidak ada server lokal terdeteksi/)).toBeVisible();
  await dialog(page).getByRole("button", { name: "Ollama", exact: true }).click();
  await expect(dialog(page).getByLabel("Base URL (kompatibel OpenAI)")).toHaveValue("http://localhost:11434/v1");
});

test("tab Provider katalog: cari, hubungkan dengan kunci; provider yang butuh konfigurasi tambahan ditandai", async ({
  page,
}) => {
  await openProviders(page);
  await addButton(page).click();
  await dialog(page).getByRole("tab", { name: "Provider katalog" }).click();
  await expect(dialog(page).getByText("Cloudflare Workers AI")).toBeVisible();
  await expect(dialog(page).getByText(/Perlu konfigurasi tambahan/)).toBeVisible();
  await dialog(page).getByRole("searchbox", { name: "Cari provider katalog" }).fill("openr");
  await dialog(page)
    .getByRole("button", { name: /^Hubungkan OpenRouter/ })
    .click();
  await dialog(page).getByLabel("Kunci API OpenRouter").fill("sk-or-abc");
  await dialog(page).getByRole("button", { name: "Simpan" }).click();
  const card = page.getByRole("listitem", { name: "OpenRouter" });
  await expect(card).toContainText("Provider katalog");
  await expect(card).toContainText("Kunci tersimpan");
  await expect(page.locator("body")).not.toContainText("sk-or-abc");
});

test("sign-in OAuth otomatis: membuka browser sistem lalu selesai", async ({ page }) => {
  const engine = await mockEngine(page, { oauthMethod: "auto" });
  await openProviders(page, null);
  await addButton(page).click();
  await dialog(page).getByRole("tab", { name: "Sign-in" }).click();
  await dialog(page)
    .getByRole("button", { name: /Masuk · ChatGPT Pro\/Plus/ })
    .click();
  await expect(dialog(page).getByText(/Menunggu Anda menyelesaikan login/)).toBeVisible();
  const opened = await page.evaluate(() => (window as unknown as { __opened: string[] }).__opened);
  expect(opened[0]).toMatch(/^https:\/\/auth\.example\.test\/authorize/);
  await expect(page.getByText("Berhasil masuk ke OpenAI.")).toBeVisible({ timeout: 10_000 });
  expect(engine.oauth.authorizes[0]).toMatchObject({ id: "openai", method: 0 });
  expect(engine.oauth.callbacks).toHaveLength(1);
});

test("sign-in OAuth dengan kode: kode dikirim ke engine", async ({ page }) => {
  const engine = await mockEngine(page, { oauthMethod: "code" });
  await openProviders(page, null);
  await addButton(page).click();
  await dialog(page).getByRole("tab", { name: "Sign-in" }).click();
  await dialog(page)
    .getByRole("button", { name: /Masuk · ChatGPT Pro\/Plus/ })
    .click();
  await dialog(page).getByRole("textbox", { name: "Kode dari browser" }).fill("kode-xyz");
  await dialog(page).getByRole("button", { name: "Kirim kode" }).click();
  await expect(page.getByText("Berhasil masuk ke OpenAI.")).toBeVisible({ timeout: 10_000 });
  expect(engine.oauth.callbacks[0]).toMatchObject({ id: "openai", code: "kode-xyz" });
});

// ---------------- pemilih model ----------------

async function openChat(page: Page, models: MockOptions["models"], seedProvider = false) {
  await mockEngine(page, { models });
  if (seedProvider) {
    await page.addInitScript(() => {
      localStorage.setItem(
        "zandor.providers.dev",
        JSON.stringify([
          { id: "mock", name: "Mock", kind: "custom", baseUrl: "http://provider.test/v1", models: [{ id: "m-satu" }] },
        ]),
      );
    });
  }
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/alpha");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.getByRole("log")).toBeVisible();
}

test("pemilih model: pencarian langsung fokus, saring, sematkan, pilih; prompt memakai model terpilih", async ({
  page,
}) => {
  await openChat(page, [
    { id: "m-satu", name: "Model Satu" },
    { id: "m-dua", name: "Model Dua" },
    { id: "m-tiga", name: "Model Tiga" },
  ]);
  const engineCalls: string[] = [];
  page.on("request", (r) => {
    if (r.url().includes("prompt_async")) engineCalls.push(r.postData() ?? "");
  });
  await page.getByRole("button", { name: /^Model: Model Satu/ }).click();
  const search = dialog(page).getByRole("searchbox", { name: "Cari model atau provider" });
  await expect(search).toBeFocused();
  await search.fill("dua");
  await expect(dialog(page).getByRole("button", { name: /Model Tiga/ })).toHaveCount(0);
  await dialog(page).getByRole("button", { name: "Sematkan Model Dua" }).click();
  await expect(dialog(page).getByRole("region", { name: "Disematkan" })).toContainText("Model Dua");
  await dialog(page)
    .getByRole("region", { name: "Disematkan" })
    .getByRole("button", { name: /Model Dua/ })
    .first()
    .click();
  await expect(page.getByRole("button", { name: /^Model: Model Dua/ })).toBeVisible();

  await page.getByRole("textbox", { name: /^Tulis pesan/ }).fill("halo");
  await page.getByRole("textbox", { name: /^Tulis pesan/ }).press("Enter");
  await expect.poll(() => engineCalls.length).toBe(1);
  expect(engineCalls[0]).toContain('"modelID":"m-dua"');

  // sematan bertahan setelah muat ulang
  await page.reload();
  await expect(page.getByRole("log")).toBeVisible();
  await page.getByRole("button", { name: /^Model: Model Dua/ }).click();
  await expect(dialog(page).getByRole("region", { name: "Disematkan" })).toContainText("Model Dua");
});

test("pemilih model: ping model milik koneksi kustom; model provider lain tidak punya tombol uji", async ({ page }) => {
  await mockProvider(page, { pingStatus: { "m-satu": 429 } });
  await openChat(page, [{ id: "m-satu", name: "Model Satu" }], true);
  await page.getByRole("button", { name: /^Model: Model Satu/ }).click();
  await dialog(page).getByRole("button", { name: "Uji Model Satu" }).click();
  await expect(dialog(page)).toContainText("Rate limit / kuota");
});

test("pemilih model: 'Kelola provider…' membuka halaman provider", async ({ page }) => {
  await openChat(page, [{ id: "m-satu", name: "Model Satu" }]);
  await page.getByRole("button", { name: /^Model: Model Satu/ }).click();
  await dialog(page).getByRole("button", { name: "Kelola provider…" }).click();
  await expect(page.getByRole("heading", { name: "Provider & model" })).toBeVisible();
});

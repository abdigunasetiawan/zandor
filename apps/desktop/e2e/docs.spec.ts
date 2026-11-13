import { expect, type Locator, type Page, test } from "@playwright/test";
import { b64, csvRows, MATH_MERMAID_MD, makePdf, makePng, makeWav, SAMPLE_MD, WEBM_BASE64 } from "./fixtures";
import { type MockEngine, mockEngine } from "./mock-engine";

type Files = Record<string, string | Uint8Array>;

async function openFiles(page: Page, files: Files): Promise<MockEngine> {
  const engine = await mockEngine(page);
  for (const [path, content] of Object.entries(files)) {
    engine.project.set(path, typeof content === "string" ? { text: content } : { base64: b64(content) });
  }
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/docs");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("tab", { name: "Files" }).click();
  return engine;
}

/** Klik berkas di tab Files (berdasarkan path) → terbuka di Panel 3. */
async function openDoc(page: Page, path: string) {
  await page.locator(`.tree__file[title="${path}"]`).click();
}

const docs = (page: Page) => page.getByRole("region", { name: "Dokumen", exact: true });
const toolbar = (page: Page) => page.getByRole("toolbar", { name: "Alat dokumen" });
const nonBlankCanvas = (c: Locator) =>
  c.evaluate((el) => {
    const cv = el as HTMLCanvasElement;
    const d = cv.getContext("2d")?.getImageData(0, 0, cv.width, cv.height).data;
    if (!d) return false;
    let dark = 0;
    for (let i = 0; i < d.length; i += 4) if ((d[i] ?? 255) < 128 && (d[i + 3] ?? 0) > 0) dark++;
    return dark > 20;
  });

test("kode: Monaco menampilkan isi dengan pewarnaan; cari membuka widget cari; bungkus baris", async ({ page }) => {
  await openFiles(page, { "app.ts": "export const x: number = 42;\n// komentar panjang ".concat("kata ".repeat(60)) });
  await openDoc(page, "app.ts");
  await expect(docs(page).getByTestId("doc-path")).toHaveText("app.ts");
  const editor = docs(page).getByTestId("monaco");
  await expect(editor.locator(".view-lines")).toContainText("export const x", { timeout: 15_000 });
  await expect(editor.locator(".mtk1, .mtk6, .mtk4, .mtk8").first()).toBeVisible(); // token berwarna dari tokenizer
  await toolbar(page).getByRole("button", { name: "Cari di dokumen" }).click();
  await expect(editor.locator(".find-widget.visible")).toBeVisible();
  const wrap = toolbar(page).getByRole("button", { name: "Bungkus baris" });
  await wrap.click();
  await expect(wrap).toHaveAttribute("aria-pressed", "true");
});

test("Markdown: render (judul, tabel, daftar, tautan aman) dan tampilan mentah; snapshot ARIA", async ({ page }) => {
  await openFiles(page, { "README.md": SAMPLE_MD });
  await openDoc(page, "README.md");
  const md = docs(page).getByTestId("markdown");
  await expect(md.getByRole("heading", { name: "Judul Dokumen" })).toBeVisible({ timeout: 15_000 });
  await expect(md).toMatchAriaSnapshot({ name: "markdown-render.aria.yml" });
  await expect(md.getByRole("link", { name: "tautan" })).toHaveAttribute("rel", /noopener/);
  // mentah: Monaco menampilkan sumber
  await toolbar(page).getByRole("button", { name: "Tampilan mentah" }).click();
  await expect(docs(page).getByTestId("monaco").locator(".view-lines")).toContainText("# Judul Dokumen", {
    timeout: 15_000,
  });
  await toolbar(page).getByRole("button", { name: "Tampilan render" }).click();
  await expect(md).toBeVisible();
});

test("Markdown: KaTeX dirender, Mermaid valid menjadi SVG, Mermaid tidak valid menampilkan galat + sumber", async ({
  page,
}) => {
  await openFiles(page, { "math.md": MATH_MERMAID_MD });
  await openDoc(page, "math.md");
  const md = docs(page).getByTestId("markdown");
  await expect(md.locator(".katex").first()).toBeVisible({ timeout: 15_000 });
  expect(await md.locator(".katex").count()).toBeGreaterThanOrEqual(2); // inline + blok
  await expect(md.getByTestId("mermaid").locator("svg")).toBeVisible({ timeout: 30_000 });
  await expect(md.getByTestId("mermaid").locator("svg")).toContainText("Mulai");
  await expect(md.getByRole("alert")).toContainText("Diagram tidak valid");
  await expect(md.getByRole("alert")).toContainText("ini bukan diagram yang valid");
});

test("CSV: tabel tervirtualisasi (DOM kecil), header tetap, baris terakhir terjangkau; TSV; snapshot ARIA", async ({
  page,
}) => {
  await openFiles(page, { "data.csv": csvRows(2000), "t.tsv": "a\tb\n1\t2\n" });
  await openDoc(page, "data.csv");
  const table = docs(page).getByTestId("table");
  await expect(table).toContainText("Nama 1", { timeout: 15_000 });
  await expect(docs(page).getByText("2000 baris · 3 kolom")).toBeVisible();
  expect(await table.locator(".dtable__row").count()).toBeLessThan(80);
  await expect(table.locator(".dtable__cell--head")).toHaveText(["id", "nama", "nilai"]);
  await table.evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });
  await expect(table).toContainText("Nama 2000");
  await expect(table).not.toContainText("Nama 5 ");
  await openDoc(page, "t.tsv");
  await expect(docs(page).getByTestId("table")).toMatchAriaSnapshot({ name: "tsv-render.aria.yml" });
});

test("JSON: pohon dengan lipat/buka dan JSON tidak valid ditolak dengan pesan", async ({ page }) => {
  await openFiles(page, {
    "a.json": JSON.stringify({ nama: "zandor", angka: 3, daftar: [1, 2, 3], dalam: { x: null, y: true } }),
    "rusak.json": "{ tidak valid",
  });
  await openDoc(page, "a.json");
  const tree = docs(page).getByTestId("json");
  await expect(tree).toContainText('"zandor"', { timeout: 15_000 });
  await expect(tree).toContainText("[3]");
  await expect(tree).not.toContainText("0: 1"); // 'daftar' (kedalaman 1) awalnya terlipat
  await tree.getByRole("button", { name: /^daftar/ }).click();
  await expect(tree).toContainText("0: 1");
  await expect(tree).toContainText("2: 3");
  await tree.getByRole("button", { name: /^daftar/ }).click();
  await expect(tree).not.toContainText("0: 1");
  await openDoc(page, "rusak.json");
  await expect(docs(page).getByRole("alert")).toContainText("JSON tidak valid");
  await toolbar(page).getByRole("button", { name: "Tampilan mentah" }).click();
  await expect(docs(page).getByTestId("monaco").locator(".view-lines")).toContainText("tidak valid", {
    timeout: 15_000,
  });
});

test("HTML di sandbox penuh: konten tampil, skrip tidak berjalan, atribut sandbox kosong", async ({ page }) => {
  await openFiles(page, {
    "p.html": "<h1 id='h'>Halo Sandbox</h1><script>document.getElementById('h').textContent='DIRETAS'</script>",
  });
  await openDoc(page, "p.html");
  const frame = docs(page).getByTestId("html-frame");
  await expect(frame).toHaveAttribute("sandbox", "");
  const inner = page.frameLocator('[data-testid="html-frame"]');
  await expect(inner.locator("h1")).toHaveText("Halo Sandbox");
  await page.waitForTimeout(300);
  await expect(inner.locator("h1")).toHaveText("Halo Sandbox"); // skrip diblokir
});

test("SVG dan gambar: tampil dari blob; klik mengganti ukuran; SVG mentah di editor", async ({ page }) => {
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><rect width="40" height="30" fill="red"/></svg>';
  await openFiles(page, { "g.svg": svg, "k.png": makePng(8, 8) });
  await openDoc(page, "g.svg");
  const img = docs(page).getByTestId("image");
  await expect(img).toBeVisible({ timeout: 15_000 });
  expect(await img.evaluate((el) => (el as HTMLImageElement).naturalWidth)).toBe(40);
  await toolbar(page).getByRole("button", { name: "Tampilan mentah" }).click();
  await expect(docs(page).getByTestId("monaco").locator(".view-lines")).toContainText("<svg", { timeout: 15_000 });
  await openDoc(page, "k.png");
  const png = docs(page).getByTestId("image");
  await expect(png).toBeVisible();
  expect(await png.evaluate((el) => (el as HTMLImageElement).naturalWidth)).toBe(8);
  await png.click();
  await expect(docs(page).locator(".docview--image")).not.toHaveAttribute("data-fit", "");
});

test("PDF: halaman dirender ke canvas (tidak kosong), jumlah halaman, zoom", async ({ page }) => {
  await openFiles(page, { "d.pdf": makePdf(["HALAMAN SATU", "HALAMAN DUA", "HALAMAN TIGA"]) });
  await openDoc(page, "d.pdf");
  await expect(docs(page).getByTestId("pdf-pages")).toHaveText("Halaman 1 dari 3", { timeout: 30_000 });
  const first = docs(page).locator('.pdfpage[data-page="1"] canvas');
  await expect(first).toBeVisible();
  await expect.poll(() => nonBlankCanvas(first), { timeout: 15_000 }).toBe(true);
  const w1 = await first.evaluate((el) => (el as HTMLCanvasElement).clientWidth);
  await docs(page).getByRole("button", { name: "Perbesar" }).click();
  await expect.poll(() => first.evaluate((el) => (el as HTMLCanvasElement).clientWidth)).toBeGreaterThan(w1);
  // halaman jauh dirender malas saat digulir
  await docs(page)
    .getByTestId("pdf")
    .evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
  await expect(docs(page).locator('.pdfpage[data-page="3"] canvas')).toBeVisible();
  await expect(docs(page).getByTestId("pdf-pages")).toHaveText("Halaman 3 dari 3");
});

test("PDF rusak menampilkan galat, bukan layar kosong", async ({ page }) => {
  await openFiles(page, { "rusak.pdf": "bukan pdf sungguhan" });
  await openDoc(page, "rusak.pdf");
  await expect(docs(page).getByRole("alert")).toContainText("PDF tidak bisa dibuka", { timeout: 20_000 });
});

test("audio dan video: elemen media memuat metadata", async ({ page }) => {
  await openFiles(page, { "s.wav": makeWav(), "v.webm": new Uint8Array(Buffer.from(WEBM_BASE64, "base64")) });

  await openDoc(page, "s.wav");
  const audio = docs(page).getByTestId("audio");
  await expect(audio).toBeVisible({ timeout: 15_000 });
  await expect
    .poll(() => audio.evaluate((el) => (el as HTMLAudioElement).duration), { timeout: 15_000 })
    .toBeGreaterThan(0.2);

  await openDoc(page, "v.webm");
  const video = docs(page).getByTestId("video");
  await expect(video).toBeVisible();
  await expect.poll(() => video.evaluate((el) => (el as HTMLVideoElement).videoWidth), { timeout: 15_000 }).toBe(64);
});

test("format tanpa penampil (DOCX) dan biner: pesan jelas, bukan layar kosong", async ({ page }) => {
  await openFiles(page, { "laporan.docx": "PK\u0003\u0004isi", "x.dat": new Uint8Array([1, 0, 2, 0, 3]) });
  await openDoc(page, "laporan.docx");
  await expect(docs(page).getByText(/laporan\.docx tidak punya penampil/)).toBeVisible({ timeout: 15_000 });
  await openDoc(page, "x.dat");
  await expect(docs(page).getByText(/x\.dat tidak punya penampil/)).toBeVisible();
});

test("tab: beberapa berkas, aktifkan, tutup; tab dipulihkan setelah muat ulang", async ({ page }) => {
  await openFiles(page, { "a.ts": "const a = 1;", "b.md": "# B", "c.json": "[1]" });
  await openDoc(page, "a.ts");
  await openDoc(page, "b.md");
  await openDoc(page, "c.json");
  const tabs = page.getByRole("navigation", { name: "Dokumen terbuka" });
  await expect(tabs.locator(".stabs__tab")).toHaveCount(3);
  await tabs.getByRole("button", { name: "a.ts", exact: true }).click();
  await expect(docs(page).getByTestId("doc-path")).toHaveText("a.ts");
  await page.reload();
  await expect(page.getByRole("navigation", { name: "Dokumen terbuka" }).locator(".stabs__tab")).toHaveCount(3);
  await expect(docs(page).getByTestId("doc-path")).toHaveText("a.ts");
  await expect(docs(page).getByTestId("monaco").locator(".view-lines")).toContainText("const a", { timeout: 15_000 });
  await page.getByRole("button", { name: "Tutup tab a.ts" }).click();
  await expect(page.getByRole("navigation", { name: "Dokumen terbuka" }).locator(".stabs__tab")).toHaveCount(2);
});

test("toolbar: salin isi, salin path, unduh, muat ulang mengambil isi terbaru, tutup", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const engine = await openFiles(page, { "catatan.txt": "versi satu\n" });
  await openDoc(page, "catatan.txt");
  await expect(docs(page).getByTestId("monaco").locator(".view-lines")).toContainText("versi satu", {
    timeout: 15_000,
  });

  await toolbar(page).getByRole("button", { name: "Salin isi" }).click();
  // Windows menormalkan akhir baris clipboard menjadi CRLF; yang diuji: isinya persis, termasuk newline akhir
  const clip = (await page.evaluate(() => navigator.clipboard.readText())).replaceAll("\r\n", "\n");
  expect(clip).toBe("versi satu\n");
  await toolbar(page).getByRole("button", { name: "Salin path" }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toMatch(/docs.catatan\.txt$/);

  const download = page.waitForEvent("download");
  await toolbar(page).getByRole("button", { name: "Simpan salinan" }).click();
  expect((await download).suggestedFilename()).toBe("catatan.txt");

  // "buka di aplikasi lain" nonaktif di luar aplikasi desktop (tidak ada tombol palsu)
  await expect(toolbar(page).getByRole("button", { name: "Buka di aplikasi lain" })).toBeDisabled();

  engine.project.set("catatan.txt", { text: "versi dua\n" });
  await toolbar(page).getByRole("button", { name: "Muat ulang" }).click();
  await expect(docs(page).getByTestId("monaco").locator(".view-lines")).toContainText("versi dua", { timeout: 15_000 });

  await toolbar(page).getByRole("button", { name: "Tutup dokumen" }).click();
  await expect(page.getByText(/Klik file di tab Files/)).toBeVisible();
});

test("galat baca: pesan + tombol Muat ulang memulihkan", async ({ page }) => {
  const engine = await openFiles(page, { "ada.txt": "isi" });
  await openDoc(page, "ada.txt");
  await expect(docs(page).getByTestId("monaco")).toBeVisible({ timeout: 15_000 });
  engine.project.delete("ada.txt");
  await toolbar(page).getByRole("button", { name: "Muat ulang" }).click();
  await expect(docs(page).getByRole("alert")).toContainText("Gagal membuka dokumen");
  engine.project.set("ada.txt", { text: "pulih" });
  await docs(page).getByRole("alert").getByRole("button", { name: "Muat ulang" }).click();
  await expect(docs(page).getByTestId("monaco").locator(".view-lines")).toContainText("pulih", { timeout: 15_000 });
});

test("klik berkas dari kartu tool di chat membuka Panel 3", async ({ page }) => {
  await openFiles(page, { "catatan.txt": "halo dunia" });
  await page.getByRole("tab", { name: "Sesi", exact: true }).click();
  await page.getByRole("button", { name: "Sesi baru di docs" }).click();
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill("tolong baca catatan");
  await box.press("Enter");
  await page.getByRole("button", { name: "Buka catatan.txt di panel Dokumen" }).click({ timeout: 15_000 });
  await expect(docs(page).getByTestId("doc-path")).toHaveText("catatan.txt");
  await expect(docs(page).getByTestId("monaco").locator(".view-lines")).toContainText("halo dunia", {
    timeout: 15_000,
  });
});

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, type Page, test } from "@playwright/test";
import { b64 } from "./fixtures";
import { mockEngine } from "./mock-engine";

// Fixture ditulis pustaka sungguhan (scripts/make-office-fixtures.py), bukan XML rakitan tangan.
const fx = (name: string) =>
  readFileSync(
    join(
      fileURLToPath(new URL(".", import.meta.url)),
      "..",
      "..",
      "..",
      "packages",
      "core",
      "src",
      "docs",
      "fixtures",
      name,
    ),
  );

async function openFiles(page: Page, files: Record<string, Buffer | string>) {
  const engine = await mockEngine(page);
  for (const [path, content] of Object.entries(files))
    engine.project.set(path, typeof content === "string" ? { text: content } : { base64: b64(content) });
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill("C:/Work/docs");
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
  await page.getByRole("tab", { name: "Files" }).click();
}

const docs = (page: Page) => page.getByRole("region", { name: "Dokumen", exact: true });
const open = (page: Page, path: string) => page.locator(`.tree__file[title="${path}"]`).click();

test("DOCX: heading, format, daftar, tabel, gambar; teks mirip markup tidak jadi HTML", async ({ page }) => {
  await openFiles(page, { "laporan.docx": fx("contoh.docx") });
  await open(page, "laporan.docx");
  const view = docs(page).getByTestId("docx-view");
  await expect(view.getByRole("heading", { name: "Judul Laporan" })).toBeVisible();
  await expect(view.getByRole("heading", { name: "Sub judul" })).toBeVisible();
  await expect(view.locator("strong", { hasText: "tebal" })).toBeVisible();
  await expect(view.locator("em", { hasText: "miring" })).toBeVisible();
  await expect(view.locator("ul li")).toHaveText(["butir satu", "butir dua"]);
  await expect(view.locator("ol li")).toHaveText(["langkah satu", "langkah dua"]);
  await expect(view.locator("table tr").nth(1).locator("td")).toHaveText(["Budi", "30", "Jakarta"]);
  await expect(view.locator("img")).toHaveCount(1);
  expect(await view.locator("img").evaluate((el) => (el as HTMLImageElement).naturalWidth)).toBe(4);
  await expect(view.getByText('Teks <b>bukan</b> markup & "kutip" aman?')).toBeVisible();
  await expect(view.locator("b")).toHaveCount(0);
  await expect(docs(page).getByText(/best-effort/)).toBeVisible();
});

test("XLSX: sheet berganti, sel teks/angka/boolean/tanggal/rumus", async ({ page }) => {
  await openFiles(page, { "data.xlsx": fx("contoh.xlsx") });
  await open(page, "data.xlsx");
  const view = docs(page).getByTestId("xlsx-view").first();
  await expect(view.locator("tr").nth(1).locator("td")).toHaveText([
    "Apel",
    "3",
    "1500.5",
    "TRUE",
    "2026-11-14",
    "=B2*C2",
  ]);
  await expect(view.getByText("Jeruk <manis>")).toBeVisible();
  await docs(page).getByRole("tab", { name: "Ringkasan" }).click();
  await expect(docs(page).getByRole("cell", { name: "42" })).toBeVisible();
  await docs(page).getByRole("tab", { name: "Kosong" }).click();
  await expect(docs(page).getByText("Sheet kosong")).toBeVisible();
});

test("PPTX: slide berurutan dengan judul, teks, dan gambar", async ({ page }) => {
  await openFiles(page, { "deck.pptx": fx("contoh.pptx") });
  await open(page, "deck.pptx");
  const view = docs(page).getByTestId("pptx-view");
  await expect(view.getByRole("listitem")).toHaveCount(3);
  await expect(view.getByRole("heading", { name: "Presentasi Uji" })).toBeVisible();
  await expect(view.getByText("Poin dua")).toBeVisible();
  await expect(view.getByRole("listitem", { name: "Slide 3" }).locator("img")).toHaveCount(1);
});

test("Notebook: markdown dirender, kode + keluaran, gambar, galat; HTML/JS tidak dijalankan", async ({ page }) => {
  const dialogs: string[] = [];
  page.on("dialog", (d) => {
    dialogs.push(d.message());
    void d.dismiss();
  });
  await openFiles(page, { "analisis.ipynb": fx("contoh.ipynb") });
  await open(page, "analisis.ipynb");
  const view = docs(page).getByTestId("notebook-view");
  await expect(view.getByRole("heading", { name: "Judul Notebook" })).toBeVisible();
  await expect(view.locator("strong", { hasText: "tebal" })).toBeVisible();
  await expect(view.getByText("halo", { exact: false }).first()).toBeVisible();
  await expect(view.locator("img.nb__out")).toHaveCount(1);
  await expect(view.getByRole("alert")).toContainText("ZeroDivisionError: division by zero");
  await expect(view.getByText("catatan mentah")).toBeVisible();
  expect(dialogs).toEqual([]);
  await expect(page.locator("script", { hasText: "alert(1)" })).toHaveCount(0);
});

test("berkas Office rusak: pesan galat, bukan layar kosong; format lama tetap 'buka di aplikasi lain'", async ({
  page,
}) => {
  await openFiles(page, { "rusak.docx": "ini bukan zip", "lama.doc": "xxx" });
  await open(page, "rusak.docx");
  await expect(docs(page).getByRole("alert")).toContainText(/bukan paket Office/);
  await open(page, "lama.doc");
  await expect(docs(page).getByText(/lama.doc tidak punya penampil/)).toBeVisible();
});

test("edit di build web: tombol Edit dinonaktifkan dengan alasan jujur", async ({ page }) => {
  await openFiles(page, { "kode.ts": "export const a = 1;\n" });
  await open(page, "kode.ts");
  await expect(docs(page).getByTestId("monaco")).toBeVisible();
  const edit = docs(page).getByRole("button", { name: "Edit hanya tersedia di aplikasi desktop" });
  await expect(edit).toBeDisabled();
});

import { expect, type Page, test } from "@playwright/test";
import { type MockEngine, mockEngine } from "./mock-engine";

async function setup(page: Page): Promise<MockEngine> {
  const engine = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  return engine;
}

const ask = async (page: Page, text: string) => {
  const box = page.getByRole("textbox", { name: /^Tulis pesan/ });
  await box.fill(text);
  await box.press("Enter");
};

test("form pertanyaan: pilih, jawaban bebas, kirim → jawaban sampai ke engine dan kartu hilang", async ({ page }) => {
  const engine = await setup(page);
  await ask(page, "ajukan pertanyaan");
  const card = page.getByTestId("question-card");
  await expect(card).toBeVisible({ timeout: 15_000 });
  await expect(card.getByText("Framework mana yang dipakai?")).toBeVisible();
  await expect(card.getByText("Pilih satu.")).toBeVisible();

  const send = card.getByRole("button", { name: "Kirim jawaban" });
  await expect(card.getByText("Pertanyaan 1 dari 2")).toBeVisible();
  await expect(card.getByRole("checkbox")).toHaveCount(0); // satu per satu: pertanyaan 2 belum tampil
  await expect(send).toHaveCount(0);
  await expect(card.getByRole("button", { name: "Berikutnya" })).toBeDisabled(); // belum dijawab
  await card.getByRole("radio", { name: /React/ }).check();
  await card.getByRole("button", { name: "Berikutnya" }).click();
  await expect(card.getByText("Boleh pilih lebih dari satu.")).toBeVisible();
  await expect(card.getByText("Pertanyaan 2 dari 2")).toBeVisible();
  await expect(send).toBeDisabled();
  await card.getByRole("checkbox", { name: /Tes/ }).check();
  await card.getByLabel("Jawaban sendiri untuk pertanyaan 2").fill("lainnya");
  await expect(send).toBeEnabled();
  await send.click();

  await expect(card).toHaveCount(0);
  expect(engine.questionReplies.map((r) => r.answers)).toEqual([[["React"], ["Tes", "lainnya"]]]);
  await expect(page.getByRole("article", { name: "Asisten" }).last()).toContainText("Terima kasih");
});

test("pertanyaan tunggal: teks bebas menggantikan pilihan dan sebaliknya", async ({ page }) => {
  const engine = await setup(page);
  await ask(page, "ajukan pertanyaan");
  const card = page.getByTestId("question-card");
  await expect(card).toBeVisible({ timeout: 15_000 });
  await card.getByRole("radio", { name: /Svelte/ }).check();
  await card.getByLabel("Jawaban sendiri untuk pertanyaan 1").fill("Solid");
  await expect(card.getByRole("radio", { name: /Svelte/ })).not.toBeChecked();
  await card.getByRole("button", { name: "Berikutnya" }).click();
  await card.getByRole("checkbox", { name: /Lint/ }).check();
  await card.getByRole("button", { name: "Kirim jawaban" }).click();
  await expect(card).toHaveCount(0);
  expect(engine.questionReplies[0]?.answers).toEqual([["Solid"], ["Lint"]]);
});

test("Lewati menolak permintaan: engine menerima penolakan dan agent melanjutkan", async ({ page }) => {
  const engine = await setup(page);
  await ask(page, "ajukan pertanyaan");
  const card = page.getByTestId("question-card");
  await expect(card).toBeVisible({ timeout: 15_000 });
  await card.getByRole("button", { name: "Lewati" }).click();
  await expect(card).toHaveCount(0);
  expect(engine.questionReplies.map((r) => r.answers)).toEqual([null]);
  await expect(page.getByRole("button", { name: "Kirim", exact: true })).toBeVisible();
});

test("daftar rencana agent: status per item, ringkasan, dan pulih setelah muat ulang", async ({ page }) => {
  await setup(page);
  await ask(page, "buat rencana kerja");
  const todos = page.getByTestId("todo-list");
  await expect(todos).toBeVisible({ timeout: 15_000 });
  await expect(todos.getByText("Rencana agent (1/3 selesai)")).toBeVisible();
  await expect(todos.getByText("RENCANA KERJA")).toBeVisible();
  await expect(todos.getByText("Rincian")).toBeVisible();
  await expect(todos.getByRole("listitem")).toHaveCount(3);
  await expect(todos.getByRole("img", { name: "selesai" })).toHaveCount(1);
  await expect(todos.getByRole("img", { name: "sedang dikerjakan" })).toHaveCount(1);
  await expect(todos.getByRole("listitem").first()).toHaveAttribute("data-status", "completed");
  await expect(todos.getByRole("listitem").nth(1)).toHaveAttribute("data-status", "in_progress");

  await page.reload();
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.locator(".sidebar__section--chats .tree__session").first().click();
  await expect(page.getByTestId("todo-list").getByText("Rencana agent (1/3 selesai)")).toBeVisible();
});

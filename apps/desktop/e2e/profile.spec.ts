import { expect, test } from "@playwright/test";
import { mockEngine } from "./mock-engine";

test("profil: nama tampilan dan avatar ilustrasi dipakai di sidebar dan bertahan setelah muat ulang", async ({
  page,
}) => {
  await mockEngine(page);
  await page.goto("/#/settings/profile");
  const section = page.locator("#settings-profile");
  await expect(section.getByRole("heading", { name: "Profil" })).toBeVisible();

  await section.getByLabel("Nama tampilan").fill("Abdi Guna");
  await section.getByRole("tab", { name: "Ilustrasi" }).click();
  await section.getByRole("button", { name: "Pakai avatar 3" }).click();
  await expect(section.getByRole("button", { name: "Pakai avatar 3" })).toHaveAttribute("aria-pressed", "true");
  await expect(section.getByText(/Lorelei.*CC0/)).toBeVisible();
  if (process.env.ZSHOT) await page.locator(".settings").screenshot({ path: process.env.ZSHOT });

  await page.reload();
  await page.goto("/#/");
  const foot = page.locator(".sidebar__foot");
  await expect(foot.getByText("Abdi Guna")).toBeVisible();
  await expect(foot.getByRole("img", { name: "Abdi Guna" }).locator("img")).toHaveAttribute(
    "src",
    /^data:image\/svg\+xml/,
  );

  // kembali ke inisial dan nama akun
  await page.goto("/#/settings/profile");
  await section.getByLabel("Nama tampilan").fill("");
  await section.getByRole("tab", { name: "Inisial" }).click();
  await section.getByRole("button", { name: "Pakai inisial" }).click();
  await page.goto("/#/");
  await expect(foot.getByText("Abdi Guna")).toHaveCount(0);
});

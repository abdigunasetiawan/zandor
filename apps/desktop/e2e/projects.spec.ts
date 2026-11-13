import { expect, type Page, test } from "@playwright/test";

// Engine tiruan di memori, dicegat lewat page.route (tanpa jaringan). Kontrak terhadap engine asli diverifikasi
// di smoke native (e2e-native) dan tes integrasi supervisor.
type Sess = { id: string; title: string; directory: string; time: { created: number; updated: number } };

async function mockEngine(page: Page, opts: { fail?: boolean } = {}) {
  const sessions: Sess[] = [];
  let n = 0;
  await page.addInitScript(() => {
    if (!localStorage.getItem("zandor.engine.override")) {
      localStorage.setItem("zandor.engine.override", JSON.stringify({ url: "http://engine.test", password: "x" }));
      localStorage.setItem("zandor.chatsDir", "C:\\data\\chats");
    }
  });
  await page.route("http://engine.test/**", async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    const dir = url.searchParams.get("directory") ?? req.headers()["x-opencode-directory"] ?? "";
    const cors = {
      "access-control-allow-origin": "*",
      "access-control-allow-headers": "*",
      "access-control-allow-methods": "*",
    };
    const json = (body: unknown, status = 200) =>
      route.fulfill({ status, headers: { ...cors, "content-type": "application/json" }, body: JSON.stringify(body) });
    if (req.method() === "OPTIONS") return route.fulfill({ status: 204, headers: cors });
    if (opts.fail) return json({ message: "engine rusak" }, 500);
    const p = url.pathname;
    if (p === "/global/health") return json({ healthy: true, version: "1.19.5" });
    if (p === "/session" && req.method() === "GET") return json(sessions.filter((s) => s.directory === dir));
    if (p === "/session" && req.method() === "POST") {
      const body = req.postDataJSON() ?? {};
      const t = Date.now() + ++n;
      const s = {
        id: `ses_${n}`,
        title: body.title ?? `New session - ${new Date(t).toISOString()}`,
        directory: dir,
        time: { created: t, updated: t },
      };
      sessions.push(s);
      return json(s);
    }
    const m = p.match(/^\/session\/([^/]+)$/);
    if (m && req.method() === "PATCH") {
      const s = sessions.find((x) => x.id === m[1]);
      if (!s) return json({ message: "tidak ada" }, 404);
      Object.assign(s, req.postDataJSON() ?? {});
      return json(s);
    }
    if (m && req.method() === "DELETE") {
      const i = sessions.findIndex((x) => x.id === m[1]);
      if (i >= 0) sessions.splice(i, 1);
      return json(true);
    }
    if (p === "/file") {
      const path = url.searchParams.get("path") ?? ".";
      const node = (name: string, rel: string, type: string, ignored = false) => ({
        name,
        path: rel,
        absolute: `${dir}\\${rel}`,
        type,
        ignored,
      });
      const tree: Record<string, unknown[]> = {
        ".": [
          node("src", "src", "directory"),
          node("README.md", "README.md", "file"),
          node("node_modules", "node_modules", "directory", true),
        ],
        src: [node("main.ts", "src/main.ts", "file")],
      };
      return json(tree[path] ?? []);
    }
    return json({ message: `tidak ditangani: ${req.method()} ${p}` }, 404);
  });
  return sessions;
}

async function addProject(page: Page, path: string) {
  await page.getByRole("button", { name: "Tambah project (pilih folder)" }).click();
  await page.getByLabel("Path folder").fill(path);
  await page.getByRole("button", { name: "Tambah", exact: true }).click();
}

test("tambah project, buat sesi, rename, cari, hapus; data bertahan setelah muat ulang", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await expect(page.getByText("Belum ada project")).toBeVisible();

  await addProject(page, "C:\\Work\\alpha");
  await expect(page.getByRole("button", { name: "alpha", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Sesi baru di alpha" }).click();
  await expect(page.locator(".tree__label", { hasText: "Sesi tanpa judul" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Chat", exact: true })).toContainText("Project: alpha");

  // rename lewat menu ⋯ (tombol menu bernama sama dengan judul sesi; yang terakhir adalah tombol menu)
  await page.getByRole("button", { name: "Menu untuk Sesi tanpa judul" }).click({ force: true });
  await page.getByRole("menuitem", { name: "Ganti nama" }).click();
  await page.getByRole("textbox", { name: "Nama" }).fill("Perbaiki login");
  await page.getByRole("button", { name: "Simpan" }).click();
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" }).first()).toBeVisible();

  // pencarian
  const search = page.getByRole("searchbox", { name: "Cari sesi" });
  await search.fill("zzz");
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" })).toHaveCount(0);
  await search.fill("login");
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" }).first()).toBeVisible();
  await search.fill("");

  // muat ulang: registry (localStorage) + sesi (engine) tetap ada
  await page.reload();
  await expect(page.getByRole("button", { name: "alpha", exact: true })).toBeVisible();
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" }).first()).toBeVisible();

  // hapus sesi dengan konfirmasi
  await page.getByRole("button", { name: "Menu untuk Perbaiki login" }).click({ force: true });
  await page.getByRole("menuitem", { name: "Hapus sesi" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Hapus sesi" }).click();
  await expect(page.locator(".tree__label", { hasText: "Perbaiki login" })).toHaveCount(0);
});

test("lepas project dari daftar meminta konfirmasi dan tidak menghapus sesi", async ({ page }) => {
  const sessions = await mockEngine(page);
  await page.goto("/");
  await addProject(page, "C:\\Work\\beta");
  await page.getByRole("button", { name: "Sesi baru di beta" }).click();
  await expect(page.locator(".tree__label", { hasText: "Sesi tanpa judul" })).toBeVisible();
  await page.getByRole("button", { name: "beta", exact: true }).first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Lepas dari daftar" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("Folder dan sesinya di engine tidak dihapus");
  await page.getByRole("alertdialog").getByRole("button", { name: "Lepas dari daftar" }).click();
  await expect(page.getByRole("button", { name: "beta", exact: true })).toHaveCount(0);
  expect(sessions).toHaveLength(1);
});

test("sesi tanpa project dibuat di grup Chat", async ({ page }) => {
  const sessions = await mockEngine(page);
  await page.goto("/");
  await expect(page.locator(".sidebar__status")).toContainText("Engine siap");
  await page.getByRole("button", { name: "Sesi baru di Chat (tanpa project)" }).click();
  await expect(page.locator(".tree__label", { hasText: "Sesi tanpa judul" })).toBeVisible();
  expect(sessions[0]?.directory).toBe("C:\\data\\chats");
});

test("tab Files menampilkan pohon file project aktif, dimuat malas", async ({ page }) => {
  await mockEngine(page);
  await page.goto("/");
  await page.getByRole("tab", { name: "Files" }).click();
  await expect(page.getByText("Pilih project untuk melihat file.")).toBeVisible();
  await page.getByRole("tab", { name: "Sesi", exact: true }).click();
  await addProject(page, "C:\\Work\\gamma");
  await page.getByRole("tab", { name: "Files" }).click();
  await expect(page.getByText("README.md")).toBeVisible();
  await expect(page.getByText("main.ts")).toHaveCount(0);
  await page.getByRole("button", { name: "src", exact: true }).click();
  await expect(page.getByText("main.ts")).toBeVisible();
  // ikon Material dimuat (img), bukan ikon cadangan Lucide
  await expect(page.locator("img.file-icon").first()).toBeVisible();
  expect(
    await page
      .locator("img.file-icon")
      .evaluateAll((els) => els.every((e) => (e as HTMLImageElement).naturalWidth > 0)),
  ).toBe(true);
});

test("engine bermasalah: galat per-grup tampil, UI tidak runtuh", async ({ page }) => {
  await mockEngine(page, { fail: true });
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Tambah project (pilih folder)" })).toBeVisible();
  await expect(page.getByText(/Gagal memuat sesi/).first()).toBeVisible();
});

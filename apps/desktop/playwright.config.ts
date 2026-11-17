import { defineConfig } from "@playwright/test";

// e2e terhadap UI hasil build (vite preview), memakai Microsoft Edge yang sudah terpasang di Windows
// (mesin Chromium yang sama dengan WebView2) sehingga tidak perlu mengunduh browser.
export default defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  // Saat paralel (default ~setengah inti CPU) memuat aplikasi + daftar sesi bisa >5 dtk; 5 dtk bawaan memicu gagal palsu.
  expect: { timeout: 10_000 },
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: { baseURL: "http://127.0.0.1:4173", channel: "msedge", locale: "id-ID", trace: "retain-on-failure" },
  webServer: {
    command: "bun run build && bun run preview",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});

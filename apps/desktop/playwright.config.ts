import { defineConfig } from "@playwright/test";

// e2e terhadap UI hasil build (vite preview), memakai Microsoft Edge yang sudah terpasang di Windows
// (mesin Chromium yang sama dengan WebView2) sehingga tidak perlu mengunduh browser.
export default defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: { baseURL: "http://127.0.0.1:4173", channel: "msedge", trace: "retain-on-failure" },
  webServer: {
    command: "bun run build && bun run preview",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});

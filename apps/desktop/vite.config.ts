import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// ADR-016: build dev menampilkan 0.0.<jumlah commit>; rilis memakai semver package.json.
const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")) as { version: string };
let commitCount: string | null = null;
try {
  commitCount = execSync("git rev-list --count HEAD", { encoding: "utf8" }).trim();
} catch {
  commitCount = null;
}

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  define: {
    __APP_PACKAGE_VERSION__: JSON.stringify(pkg.version),
    __APP_COMMIT_COUNT__: JSON.stringify(commitCount),
    __APP_RELEASE__: JSON.stringify(process.env.ZANDOR_RELEASE === "1"),
  },
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});

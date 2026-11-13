import { defineConfig } from "vitest/config";

export default defineConfig({
  define: {
    __APP_PACKAGE_VERSION__: JSON.stringify("0.1.0"),
    __APP_COMMIT_COUNT__: JSON.stringify("7"),
    __APP_RELEASE__: JSON.stringify(false),
  },
  test: {
    include: ["packages/*/src/**/*.test.{ts,tsx}", "apps/*/src/**/*.test.{ts,tsx}", "scripts/**/*.test.ts"],
    exclude: ["**/node_modules/**", "spikes/**", "reference/**"],
    environment: "node",
    setupFiles: ["./vitest.setup.ts"],
  },
});

import { describe, expect, test } from "vitest";
import { isSensitiveEnv, scrubEnv } from "./env-guard";
import { ZandorPlugin } from "./index";

describe("env guard", () => {
  test("mengenali kunci sensitif", () => {
    for (const k of [
      "ZANDOR_SECRET_OPENAI",
      "ZANDOR_PROXY_TOKEN",
      "OPENCODE_SERVER_PASSWORD",
      "OPENCODE_CONFIG_CONTENT",
      "OPENCODE_AUTH_CONTENT",
    ])
      expect(isSensitiveEnv(k), k).toBe(true);
    for (const k of ["PATH", "HOME", "OPENCODE_DISABLE_AUTOUPDATE", "ZANDOR_MEMORY_DIR"])
      expect(isSensitiveEnv(k), k).toBe(false);
  });

  test("scrubEnv hanya mengosongkan kunci sensitif yang ada", () => {
    expect(scrubEnv({ PATH: "/bin", ZANDOR_SECRET_X: "sk-1", OPENCODE_SERVER_PASSWORD: "pw" })).toEqual({
      ZANDOR_SECRET_X: "",
      OPENCODE_SERVER_PASSWORD: "",
    });
  });

  test("hook shell.env menimpa nilai env proses", async () => {
    process.env.ZANDOR_SECRET_TEST = "sk-rahasia";
    try {
      const hooks = await ZandorPlugin({} as never);
      const output = { env: {} as Record<string, string> };
      await hooks["shell.env"]?.({ cwd: "." }, output);
      expect(output.env.ZANDOR_SECRET_TEST).toBe("");
      expect(output.env.PATH).toBeUndefined();
    } finally {
      delete process.env.ZANDOR_SECRET_TEST;
    }
  });
});

// Spike S4 — plugin opencode: jangan wariskan secret Zandor ke proses anak engine.
// Hook `shell.env` dipanggil opencode 1.19.5 sebelum menjalankan tool shell/bash dan saat membuat PTY;
// env akhir = { ...process.env, ...output.env }, jadi nilai di sini menimpa env proses engine.
const SENSITIVE = (k) =>
  k.startsWith("ZANDOR_SECRET_") ||
  k === "ZANDOR_PROXY_TOKEN" ||
  k === "OPENCODE_SERVER_PASSWORD" ||
  k === "OPENCODE_CONFIG_CONTENT" ||
  k === "OPENCODE_AUTH_CONTENT";

export const ZandorEnvGuard = async () => ({
  "shell.env": async (_input, output) => {
    for (const k of Object.keys(process.env)) if (SENSITIVE(k)) output.env[k] = "";
  },
});

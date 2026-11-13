// Hook `shell.env` (ADR-022): proses anak engine (tool bash, PTY) tidak boleh mewarisi secret Zandor
// atau token server engine. Env akhir opencode = { ...process.env, ...output.env }, jadi nilai kosong
// di sini menimpa nilai asli.

const EXACT = new Set([
  "ZANDOR_PROXY_TOKEN",
  "OPENCODE_SERVER_PASSWORD",
  "OPENCODE_CONFIG_CONTENT",
  "OPENCODE_AUTH_CONTENT",
]);

export function isSensitiveEnv(key: string): boolean {
  return key.startsWith("ZANDOR_SECRET_") || EXACT.has(key);
}

/** Kembalikan override env: setiap kunci sensitif yang ada di `env` diisi string kosong. */
export function scrubEnv(env: Record<string, string | undefined>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const k of Object.keys(env)) if (isSensitiveEnv(k)) out[k] = "";
  return out;
}

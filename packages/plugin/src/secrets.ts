// Pemindai secret untuk draf skill dan memori (PRD §4.7): tolak teks yang tampak memuat kunci/token.
const RULES: Array<[string, RegExp]> = [
  ["kunci API bergaya sk-", /\bsk-[A-Za-z0-9_-]{16,}/],
  ["token GitHub", /\bgh[pousr]_[A-Za-z0-9]{30,}/],
  ["token Slack", /\bxox[abprs]-[A-Za-z0-9-]{10,}/],
  ["kunci akses AWS", /\bAKIA[0-9A-Z]{16}\b/],
  ["kunci Google API", /\bAIza[0-9A-Za-z_-]{30,}/],
  ["token JWT", /\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/],
  ["kunci privat", /-----BEGIN [A-Z ]*PRIVATE KEY-----/],
  ["header Authorization", /\bauthorization\s*[:=]\s*(bearer|basic)\s+[A-Za-z0-9._~+/=-]{12,}/i],
  ["kredensial di URL", /\b[a-z][a-z0-9+.-]*:\/\/[^\s/:@]+:[^\s/@]{3,}@/i],
  ["variabel rahasia", /\b[A-Z0-9_]*(?:SECRET|TOKEN|PASSWORD|API_?KEY)[A-Z0-9_]*\s*[=:]\s*["']?[^\s"']{8,}/],
];

/** Daftar jenis secret yang terdeteksi (tanpa menyertakan nilainya). */
export function scanSecrets(text: string): string[] {
  return RULES.filter(([, re]) => re.test(text)).map(([name]) => name);
}

/** Ganti setiap kecocokan secret dengan `[disamarkan: <jenis>]` (mis. sebelum system prompt disimpan ke disk). */
export function redactSecrets(text: string): string {
  let out = text;
  for (const [name, re] of RULES) {
    const flags = re.flags.includes("g") ? re.flags : `${re.flags}g`;
    out = out.replace(new RegExp(re.source, flags), `[disamarkan: ${name}]`);
  }
  return out;
}

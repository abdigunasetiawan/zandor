// Normalisasi alamat browser (cermin `parse_url` di Rust): tanpa skema → https; hanya http/https; host wajib.
export function normalizeBrowserUrl(input: string): { ok: true; url: string } | { ok: false; error: string } {
  const t = input.trim();
  if (!t || t.length > 4096) return { ok: false, error: "URL kosong atau terlalu panjang" };
  const withScheme = t.includes("://") ? t : `https://${t}`;
  let u: URL;
  try {
    u = new URL(withScheme);
  } catch {
    return { ok: false, error: "URL tidak valid" };
  }
  if (u.protocol !== "http:" && u.protocol !== "https:")
    return { ok: false, error: "hanya URL http/https yang dibuka" };
  if (!u.hostname) return { ok: false, error: "URL tanpa host" };
  return { ok: true, url: u.toString() };
}

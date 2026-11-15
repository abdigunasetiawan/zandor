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

// ---------- tab browser (ADR-058): state murni; satu webview native per tab ----------

export type BrowserTab = { id: string; url: string };
export type BrowserTabs = { tabs: BrowserTab[]; active: string };

export const MAX_BROWSER_TABS = 8;

/** Mulai dengan satu tab kosong (id `b1`). */
export function initialBrowserTabs(): BrowserTabs {
  return { tabs: [{ id: "b1", url: "" }], active: "b1" };
}

/** Id baru yang tidak dipakai tab lain (huruf kecil+angka, cocok untuk label webview). */
export function nextBrowserTabId(state: BrowserTabs): string {
  const used = new Set(state.tabs.map((t) => t.id));
  for (let n = 1; ; n++) if (!used.has(`b${n}`)) return `b${n}`;
}

/** Tab baru langsung aktif. Mengembalikan state yang sama bila sudah mencapai batas. */
export function addBrowserTab(state: BrowserTabs, url = ""): BrowserTabs {
  if (state.tabs.length >= MAX_BROWSER_TABS) return state;
  const id = nextBrowserTabId(state);
  return { tabs: [...state.tabs, { id, url }], active: id };
}

export function selectBrowserTab(state: BrowserTabs, id: string): BrowserTabs {
  return state.tabs.some((t) => t.id === id) ? { ...state, active: id } : state;
}

/** Tutup tab. Yang aktif digantikan tetangganya; menutup tab terakhir membuat satu tab kosong baru. */
export function closeBrowserTab(state: BrowserTabs, id: string): BrowserTabs {
  const i = state.tabs.findIndex((t) => t.id === id);
  if (i < 0) return state;
  const tabs = state.tabs.filter((t) => t.id !== id);
  if (tabs.length === 0) {
    const fresh = initialBrowserTabs();
    return fresh.tabs[0]?.id === id ? { tabs: [{ id: "b2", url: "" }], active: "b2" } : fresh;
  }
  const active = state.active === id ? (tabs[Math.min(i, tabs.length - 1)]?.id ?? tabs[0]?.id ?? "") : state.active;
  return { tabs, active };
}

export function setBrowserTabUrl(state: BrowserTabs, id: string, url: string): BrowserTabs {
  return { ...state, tabs: state.tabs.map((t) => (t.id === id ? { ...t, url } : t)) };
}

/** Judul tab dari URL: host tanpa `www.`; kosong bila belum ada URL. */
export function browserTabLabel(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

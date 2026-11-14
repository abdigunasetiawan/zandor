// Adapter ProviderHost. Di Tauri: perintah Rust (kunci di keychain OS, probe/ping dari Rust agar kunci tidak menyentuh
// JS). Di luar Tauri (dev browser/e2e): fallback sementara — daftar di localStorage, kunci HANYA di memori, probe/ping
// lewat fetch. `secure=false` membuat UI memberi tahu bahwa kunci tidak disimpan permanen.
import type {
  LocalFound,
  OpenWorkDevice,
  OpenWorkPoll,
  ProbeResult,
  ProviderHost,
  ProviderSpec,
  ProviderView,
} from "@zandor/core";

const DEV_KEY = "zandor.providers.dev";

const invalid = (error: string): ProbeResult => ({
  ok: false,
  status: null,
  latencyMs: 0,
  models: [],
  kind: "invalid",
  error,
});

function createBrowserProviderHost(): ProviderHost {
  const keys = new Map<string, string>(); // memori saja: tidak pernah ditulis ke penyimpanan
  const read = (): ProviderSpec[] => {
    try {
      return JSON.parse(localStorage.getItem(DEV_KEY) ?? "[]") as ProviderSpec[];
    } catch {
      return [];
    }
  };
  const write = (v: ProviderSpec[]) => localStorage.setItem(DEV_KEY, JSON.stringify(v));

  const call = async (url: string, init: RequestInit, key?: string): Promise<ProbeResult> => {
    const started = performance.now();
    try {
      const ctl = new AbortController();
      const timer = setTimeout(() => ctl.abort(), 8000);
      const res = await fetch(url, {
        ...init,
        signal: ctl.signal,
        headers: { ...(init.headers ?? {}), ...(key ? { Authorization: `Bearer ${key}` } : {}) },
      });
      clearTimeout(timer);
      const text = await res.text();
      const kind =
        res.status < 300
          ? "ok"
          : res.status === 401 || res.status === 403
            ? "auth"
            : res.status === 404
              ? "notfound"
              : res.status === 429
                ? "ratelimit"
                : res.status >= 500
                  ? "server"
                  : "invalid";
      let models: string[] = [];
      try {
        const j = JSON.parse(text) as { data?: Array<{ id?: string }> };
        models = (j.data ?? []).map((m) => m.id ?? "").filter(Boolean);
      } catch {
        // bukan JSON model
      }
      return {
        ok: kind === "ok",
        status: res.status,
        latencyMs: Math.round(performance.now() - started),
        models,
        kind,
        error: kind === "ok" ? null : `HTTP ${res.status}: ${text.slice(0, 200)}`,
      };
    } catch (e) {
      return {
        ok: false,
        status: null,
        latencyMs: Math.round(performance.now() - started),
        models: [],
        kind: "network",
        error: `tidak bisa terhubung: ${e instanceof Error ? e.message : String(e)}`,
      };
    }
  };

  return {
    secure: false,
    async list(): Promise<ProviderView[]> {
      return read().map((p) => ({ ...p, hasKey: keys.has(p.id) }));
    },
    async save(p, apiKey) {
      const all = read().filter((x) => x.id !== p.id);
      write([...all, p]);
      if (apiKey) keys.set(p.id, apiKey);
    },
    async remove(id) {
      write(read().filter((p) => p.id !== id));
      keys.delete(id);
    },
    async probe(baseUrl, apiKey, providerId) {
      return call(
        `${baseUrl.replace(/\/+$/, "")}/models`,
        {},
        apiKey ?? (providerId ? keys.get(providerId) : undefined),
      );
    },
    async ping(providerId, model) {
      const p = read().find((x) => x.id === providerId);
      if (!p?.baseUrl) return invalid("provider tidak punya base URL");
      return call(
        `${p.baseUrl.replace(/\/+$/, "")}/chat/completions`,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ model, messages: [{ role: "user", content: "ping" }], max_tokens: 1, stream: false }),
        },
        keys.get(providerId),
      );
    },
    async detectLocal(): Promise<LocalFound[]> {
      return [];
    },
    async openExternal(url) {
      window.open(url, "_blank", "noopener,noreferrer");
    },
  };
}

export async function loadProviderHost(): Promise<ProviderHost> {
  const { isTauri, invoke } = await import("@tauri-apps/api/core");
  if (!isTauri()) return createBrowserProviderHost();
  return {
    secure: true,
    openwork: {
      start: () => invoke<OpenWorkDevice>("openwork_start"),
      poll: () => invoke<OpenWorkPoll>("openwork_poll"),
      async cancel() {
        await invoke("openwork_cancel");
      },
    },
    list: () => invoke<ProviderView[]>("providers_list"),
    async save(provider, apiKey) {
      await invoke("provider_save", { provider, apiKey: apiKey ?? null });
    },
    async remove(id) {
      await invoke("provider_delete", { id });
    },
    probe: (baseUrl, apiKey, providerId) =>
      invoke<ProbeResult>("provider_probe", { baseUrl, apiKey: apiKey ?? null, providerId: providerId ?? null }),
    ping: (providerId, model) => invoke<ProbeResult>("provider_ping", { providerId, model }),
    detectLocal: () => invoke<LocalFound[]>("providers_detect_local"),
    async openExternal(url) {
      await invoke("open_external", { url });
    },
  };
}

export { createBrowserProviderHost };

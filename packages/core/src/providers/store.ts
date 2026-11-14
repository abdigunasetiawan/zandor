// Provider & model (Fase 6): daftar koneksi yang dikelola Zandor (kunci di keychain via host), katalog provider engine,
// ping model, deteksi server lokal, dan alur sign-in OAuth lewat engine. Logika murni terhadap `ProviderHost` dan
// `EngineApi` agar bisa diuji dengan fake. Kunci API tidak pernah disimpan di state ini: hanya `hasKey`.
import type { AuthMethod, CatalogProvider, EngineApi, OAuthStart } from "../engine/api";

export type ProviderKind = "custom" | "local" | "builtin";
export type ProviderModelSpec = { id: string; name?: string };
export type ProviderSpec = {
  id: string;
  name: string;
  kind: ProviderKind;
  baseUrl?: string;
  models: ProviderModelSpec[];
};
export type ProviderView = ProviderSpec & { hasKey: boolean };

export type ProbeKind = "ok" | "auth" | "notfound" | "ratelimit" | "server" | "network" | "invalid";
export type ProbeResult = {
  ok: boolean;
  status: number | null;
  latencyMs: number;
  models: string[];
  kind: ProbeKind;
  error: string | null;
};
export type LocalFound = { id: string; name: string; baseUrl: string; models: string[] };

/** Akses ke sisi native (Tauri): keychain, berkas provider, dan probe jaringan. */
/** Masuk dengan OpenWork (OAuth device flow, ADR-040). Token/kunci tidak pernah sampai ke UI. */
export type OpenWorkDevice = { userCode: string; verificationUri: string; interval: number; expiresIn: number };
export type OpenWorkPoll =
  | { status: "pending" }
  | { status: "slowDown"; interval: number }
  | { status: "denied"; message: string }
  | { status: "expired" }
  | { status: "error"; message: string }
  | { status: "connected"; account: string; models: number };
export interface OpenWorkHost {
  start(): Promise<OpenWorkDevice>;
  poll(): Promise<OpenWorkPoll>;
  cancel(): Promise<void>;
}

export type OpenWorkState =
  | { status: "idle" }
  | { status: "starting" }
  | { status: "waiting"; userCode: string; verificationUri: string; expiresAt: number }
  | { status: "connected"; account: string; models: number }
  | { status: "error"; message: string };

export interface ProviderHost {
  /** Hanya aplikasi desktop: masuk dengan akun OpenWork. */
  openwork?: OpenWorkHost;
  /** true bila kunci disimpan di keychain OS. Di luar Tauri (dev/e2e) kunci hanya disimpan sementara. */
  secure: boolean;
  list(): Promise<ProviderView[]>;
  /** `apiKey` undefined = pertahankan kunci lama. Memulai ulang engine agar konfigurasi terbaca. */
  save(provider: ProviderSpec, apiKey?: string): Promise<void>;
  remove(id: string): Promise<void>;
  probe(baseUrl: string, apiKey?: string, providerId?: string): Promise<ProbeResult>;
  ping(providerId: string, model: string): Promise<ProbeResult>;
  detectLocal(): Promise<LocalFound[]>;
  openExternal(url: string): Promise<void>;
}

export type PingState = { status: "pending" } | { status: "done"; result: ProbeResult; at: number };

export type ProvidersState = {
  loaded: boolean;
  providers: ProviderView[];
  error: string | null;
  catalog: CatalogProvider[];
  catalogStatus: "idle" | "loading" | "ready" | "error";
  catalogError: string | null;
  authMethods: Record<string, AuthMethod[]>;
  /** Hasil ping terakhir per `providerId/modelId`. */
  pings: Record<string, PingState>;
  /** Id provider yang dianggap tersambung oleh engine (dari katalog). */
  connected: string[];
  openwork: OpenWorkState;
};

export type ProviderStoreOptions = {
  host: ProviderHost;
  /** Dipanggil setelah koneksi berubah supaya daftar model chat dimuat ulang. */
  onChanged?: () => void;
  now?: () => number;
  /** Batas tunggu ping di sisi UI (detik); host juga punya batas sendiri. */
  pingWatchdogMs?: number;
  /** Penjadwal untuk polling OpenWork (bisa diganti pada tes). */
  schedule?: (fn: () => void, ms: number) => () => void;
};

export const pingKey = (providerId: string, modelId: string) => `${providerId}/${modelId}`;

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** `Antigravity Tools` → `antigravity-tools`; unik terhadap `taken`; maksimal 40 karakter. */
export function slugifyProviderId(name: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  const base =
    name
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 36) || "provider";
  if (!used.has(base)) return base;
  for (let i = 2; i < 1000; i++) {
    const c = `${base}-${i}`;
    if (!used.has(c)) return c;
  }
  return `${base}-${Date.now() % 100000}`;
}

export function validateEndpoint(baseUrl: string): string | null {
  let u: URL;
  try {
    u = new URL(baseUrl.trim());
  } catch {
    return "URL tidak valid";
  }
  return u.protocol === "http:" || u.protocol === "https:" ? null : "URL harus diawali http:// atau https://";
}

export class ProviderStore {
  private snap: ProvidersState = {
    loaded: false,
    providers: [],
    error: null,
    catalog: [],
    catalogStatus: "idle",
    catalogError: null,
    authMethods: {},
    pings: {},
    connected: [],
    openwork: { status: "idle" },
  };
  private listeners = new Set<() => void>();
  private api: EngineApi | null = null;
  private readonly now: () => number;

  constructor(private readonly options: ProviderStoreOptions) {
    this.now = options.now ?? Date.now;
  }

  state(): ProvidersState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private update(patch: Partial<ProvidersState>) {
    this.snap = { ...this.snap, ...patch };
    for (const l of this.listeners) l();
  }

  get secure(): boolean {
    return this.options.host.secure;
  }

  async refresh(): Promise<void> {
    try {
      this.update({ providers: await this.options.host.list(), loaded: true, error: null });
    } catch (e) {
      this.update({ loaded: true, error: errMsg(e) });
    }
  }

  /** Engine siap/tidak siap: muat katalog saat siap. */
  setApi(api: EngineApi | null) {
    if (api === this.api) return;
    this.api = api;
    if (api) void this.refreshCatalog();
    else this.update({ catalogStatus: "idle", connected: [] });
  }

  // ---------- Masuk dengan OpenWork ----------

  private owCancel: (() => void) | null = null;
  private owRun = 0;
  private owInterval = 5000;

  /** Buka URL di browser sistem (dipakai halaman verifikasi OpenWork). */
  openExternal(url: string): Promise<void> {
    return this.options.host.openExternal(url);
  }

  get openworkAvailable(): boolean {
    return this.options.host.openwork !== undefined;
  }

  /** Mulai alur device-code: tampilkan kode, lalu tanya periodik sampai disetujui/ditolak/kedaluwarsa. */
  async startOpenWork(): Promise<void> {
    const ow = this.options.host.openwork;
    if (!ow) {
      this.update({
        openwork: { status: "error", message: "Masuk dengan OpenWork hanya tersedia di aplikasi desktop" },
      });
      return;
    }
    this.stopOpenWorkTimer();
    const run = ++this.owRun;
    this.update({ openwork: { status: "starting" } });
    let device: OpenWorkDevice;
    try {
      device = await ow.start();
    } catch (e) {
      if (run === this.owRun) this.update({ openwork: { status: "error", message: errMsg(e) } });
      return;
    }
    if (run !== this.owRun) return; // dibatalkan saat menunggu
    const expiresAt = this.now() + device.expiresIn * 1000;
    this.update({
      openwork: { status: "waiting", userCode: device.userCode, verificationUri: device.verificationUri, expiresAt },
    });
    this.owInterval = Math.max(3, device.interval) * 1000;
    this.scheduleOpenWorkPoll(ow, run, this.owInterval);
  }

  private scheduleOpenWorkPoll(ow: OpenWorkHost, run: number, ms: number) {
    const schedule =
      this.options.schedule ??
      ((fn, t) => {
        const id = setTimeout(fn, t);
        return () => clearTimeout(id);
      });
    this.owCancel = schedule(() => void this.pollOpenWork(ow, run), ms);
  }

  private async pollOpenWork(ow: OpenWorkHost, run: number) {
    if (run !== this.owRun) return;
    let r: OpenWorkPoll;
    try {
      r = await ow.poll();
    } catch (e) {
      if (run === this.owRun) this.update({ openwork: { status: "error", message: errMsg(e) } });
      return;
    }
    if (run !== this.owRun) return;
    switch (r.status) {
      case "pending":
        return this.scheduleOpenWorkPoll(ow, run, this.owInterval);
      case "slowDown":
        this.owInterval = Math.max(3, r.interval) * 1000;
        return this.scheduleOpenWorkPoll(ow, run, this.owInterval);
      case "connected":
        this.update({ openwork: { status: "connected", account: r.account, models: r.models } });
        await this.refresh().catch(() => undefined);
        this.options.onChanged?.();
        return;
      case "denied":
        return this.update({ openwork: { status: "error", message: r.message } });
      case "expired":
        return this.update({
          openwork: { status: "error", message: "Kode kedaluwarsa. Mulai lagi untuk mendapat kode baru." },
        });
      case "error":
        return this.update({ openwork: { status: "error", message: r.message } });
    }
  }

  private stopOpenWorkTimer() {
    this.owCancel?.();
    this.owCancel = null;
  }

  async cancelOpenWork(): Promise<void> {
    this.owRun++;
    this.stopOpenWorkTimer();
    await this.options.host.openwork?.cancel().catch(() => undefined);
    this.update({ openwork: { status: "idle" } });
  }

  async refreshCatalog(): Promise<void> {
    const api = this.api;
    if (!api) return;
    this.update({ catalogStatus: "loading", catalogError: null });
    try {
      const [catalog, authMethods] = await Promise.all([api.listCatalog(), api.listAuthMethods().catch(() => ({}))]);
      if (api !== this.api) return;
      this.update({
        catalog,
        authMethods,
        catalogStatus: "ready",
        connected: catalog.filter((p) => p.connected).map((p) => p.id),
      });
    } catch (e) {
      if (api !== this.api) return;
      this.update({ catalogStatus: "error", catalogError: errMsg(e) });
    }
  }

  // ---------- koneksi yang dikelola Zandor ----------

  private async commit(op: () => Promise<void>): Promise<void> {
    await op();
    await this.refresh();
    // Engine dimulai ulang oleh host; katalog dimuat ulang saat engine siap lagi (lewat setApi).
    this.options.onChanged?.();
  }

  /** Tambah/ubah endpoint kustom atau lokal. `id` kosong → dibuat dari nama. */
  async saveEndpoint(input: {
    id?: string;
    name: string;
    kind: "custom" | "local";
    baseUrl: string;
    apiKey?: string;
    models: ProviderModelSpec[];
  }): Promise<string> {
    const name = input.name.trim();
    if (!name) throw new Error("nama koneksi wajib diisi");
    const urlError = validateEndpoint(input.baseUrl);
    if (urlError) throw new Error(urlError);
    const models = dedupeModels(input.models);
    if (models.length === 0) throw new Error("pilih atau tambahkan minimal satu model");
    const id =
      input.id ??
      slugifyProviderId(
        name,
        this.snap.providers.map((p) => p.id),
      );
    const baseUrl = input.baseUrl.trim().replace(/\/+$/, "");
    await this.commit(() =>
      this.options.host.save({ id, name, kind: input.kind, baseUrl, models }, input.apiKey || undefined),
    );
    return id;
  }

  /** Simpan kunci API untuk provider katalog (builtin). Model datang dari katalog engine. */
  async saveBuiltin(input: { id: string; name: string; apiKey: string }): Promise<void> {
    if (!input.apiKey.trim()) throw new Error("kunci API wajib diisi");
    await this.commit(() =>
      this.options.host.save({ id: input.id, name: input.name, kind: "builtin", models: [] }, input.apiKey.trim()),
    );
  }

  async remove(id: string): Promise<void> {
    await this.commit(() => this.options.host.remove(id));
    this.update({
      pings: Object.fromEntries(Object.entries(this.snap.pings).filter(([k]) => !k.startsWith(`${id}/`))),
    });
  }

  probe(baseUrl: string, apiKey?: string, providerId?: string): Promise<ProbeResult> {
    const urlError = validateEndpoint(baseUrl);
    if (urlError) return Promise.resolve(invalid(urlError));
    return this.options.host.probe(baseUrl.trim(), apiKey || undefined, providerId).catch((e) => invalid(errMsg(e)));
  }

  detectLocal(): Promise<LocalFound[]> {
    return this.options.host.detectLocal().catch(() => []);
  }

  /** Ping satu model. Selalu berakhir: host punya batas waktu, dan ada penjaga di sisi UI. */
  async ping(providerId: string, modelId: string): Promise<ProbeResult> {
    const key = pingKey(providerId, modelId);
    this.update({ pings: { ...this.snap.pings, [key]: { status: "pending" } } });
    const watchdog = this.options.pingWatchdogMs ?? 100_000;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const result = await Promise.race([
      this.options.host.ping(providerId, modelId).catch((e) => invalid(errMsg(e))),
      new Promise<ProbeResult>((resolve) => {
        timer = setTimeout(
          () => resolve({ ...invalid("tidak ada respons dalam batas waktu"), kind: "network" }),
          watchdog,
        );
      }),
    ]);
    if (timer) clearTimeout(timer);
    this.update({ pings: { ...this.snap.pings, [key]: { status: "done", result, at: this.now() } } });
    return result;
  }

  // ---------- sign-in OAuth lewat engine ----------

  /** Mulai OAuth: buka URL di browser sistem. Kembalikan info untuk UI (metode auto/code). */
  async startOAuth(providerId: string, method: number, inputs?: Record<string, string>): Promise<OAuthStart> {
    if (!this.api) throw new Error("engine belum siap");
    const start = await this.api.oauthAuthorize(providerId, method, inputs);
    if (!start.url.startsWith("https://")) throw new Error("URL sign-in dari engine bukan https; dibatalkan");
    await this.options.host.openExternal(start.url);
    return start;
  }

  /** Selesaikan OAuth ("auto" menunggu browser; "code" memakai kode dari pengguna), lalu segarkan katalog dan model. */
  async finishOAuth(providerId: string, method: number, code?: string): Promise<void> {
    if (!this.api) throw new Error("engine belum siap");
    await this.api.oauthCallback(providerId, method, code);
    await this.refreshCatalog();
    this.options.onChanged?.();
  }
}

function invalid(error: string): ProbeResult {
  return { ok: false, status: null, latencyMs: 0, models: [], kind: "invalid", error };
}

function dedupeModels(models: ProviderModelSpec[]): ProviderModelSpec[] {
  const seen = new Set<string>();
  const out: ProviderModelSpec[] = [];
  for (const m of models) {
    const id = m.id.trim();
    if (!id || seen.has(id)) continue;
    seen.add(id);
    out.push(m.name?.trim() ? { id, name: m.name.trim() } : { id });
  }
  return out;
}

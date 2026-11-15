// Preferensi model: model bawaan (chat, gambar), model yang disembunyikan dari pemilih di composer, dan logo kustom
// provider/model. Hanya kenyamanan pengguna (bukan rahasia), disimpan lewat `ChatPrefs`-like key/value.
import type { ModelRef } from "../engine/api";

export type ModelPrefsStorage = { get(key: string): string | null; set(key: string, value: string): void };

export type ModelPrefsState = {
  /** Model bawaan untuk chat baru. */
  defaultChat: ModelRef | null;
  /** Model bawaan untuk generate gambar (disimpan; fitur gambar belum ada). */
  defaultImage: ModelRef | null;
  /** Kunci `providerID/modelID` yang disembunyikan dari pemilih model. */
  hidden: string[];
  /** Logo kustom: `providerID` → data URL gambar. */
  providerLogos: Record<string, string>;
  /** Logo kustom: `providerID/modelID` → data URL gambar. */
  modelLogos: Record<string, string>;
};

const KEY = "zandor.modelPrefs";
/** Batas ukuran logo (karakter data URL ≈ 1,33× byte); mencegah localStorage membengkak. */
export const MAX_LOGO_CHARS = 96_000;
const IMAGE_URL = /^data:image\/(png|jpeg|webp|gif|svg\+xml);base64,[A-Za-z0-9+/=]+$/;

export const modelRefKey = (r: ModelRef) => `${r.providerID}/${r.modelID}`;

/** true bila `url` aman dipakai sebagai logo: data URL gambar (tanpa jaringan) dan tidak terlalu besar. */
export function isValidLogo(url: string): boolean {
  return url.length <= MAX_LOGO_CHARS && IMAGE_URL.test(url);
}

function isRef(v: unknown): v is ModelRef {
  return (
    typeof v === "object" &&
    v !== null &&
    typeof (v as ModelRef).providerID === "string" &&
    typeof (v as ModelRef).modelID === "string"
  );
}

function logoMap(v: unknown): Record<string, string> {
  if (typeof v !== "object" || v === null) return {};
  const out: Record<string, string> = {};
  for (const [k, val] of Object.entries(v)) if (typeof val === "string" && isValidLogo(val)) out[k] = val;
  return out;
}

function parse(raw: string | null): ModelPrefsState {
  const empty: ModelPrefsState = {
    defaultChat: null,
    defaultImage: null,
    hidden: [],
    providerLogos: {},
    modelLogos: {},
  };
  if (!raw) return empty;
  try {
    const v = JSON.parse(raw) as Record<string, unknown>;
    return {
      defaultChat: isRef(v.defaultChat) ? v.defaultChat : null,
      defaultImage: isRef(v.defaultImage) ? v.defaultImage : null,
      hidden: Array.isArray(v.hidden) ? v.hidden.filter((k): k is string => typeof k === "string") : [],
      providerLogos: logoMap(v.providerLogos),
      modelLogos: logoMap(v.modelLogos),
    };
  } catch {
    return empty;
  }
}

export class ModelPrefs {
  private snap: ModelPrefsState;
  private listeners = new Set<() => void>();

  constructor(
    private readonly storage: ModelPrefsStorage,
    private readonly onWarning?: (m: string) => void,
  ) {
    this.snap = parse(storage.get(KEY));
  }

  state(): ModelPrefsState {
    return this.snap;
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }

  private set(patch: Partial<ModelPrefsState>) {
    this.snap = { ...this.snap, ...patch };
    try {
      this.storage.set(KEY, JSON.stringify(this.snap));
    } catch (e) {
      this.onWarning?.(`gagal menyimpan preferensi model: ${e instanceof Error ? e.message : String(e)}`);
    }
    for (const l of this.listeners) l();
  }

  setDefaultChat(ref: ModelRef | null) {
    this.set({ defaultChat: ref });
  }

  setDefaultImage(ref: ModelRef | null) {
    this.set({ defaultImage: ref });
  }

  isHidden(ref: ModelRef): boolean {
    return this.snap.hidden.includes(modelRefKey(ref));
  }

  /** Tampilkan/sembunyikan satu atau banyak model sekaligus ("pilih semua"/"batalkan semua"). */
  setVisible(refs: ModelRef[], visible: boolean) {
    const keys = new Set(refs.map(modelRefKey));
    const rest = this.snap.hidden.filter((k) => !keys.has(k));
    this.set({ hidden: visible ? rest : [...rest, ...keys] });
  }

  /** Logo kustom provider; `null` menghapus (kembali ke deteksi otomatis). Mengembalikan false bila tidak valid. */
  setProviderLogo(providerID: string, url: string | null): boolean {
    if (url === null) {
      const { [providerID]: _x, ...rest } = this.snap.providerLogos;
      this.set({ providerLogos: rest });
      return true;
    }
    if (!isValidLogo(url)) return false;
    this.set({ providerLogos: { ...this.snap.providerLogos, [providerID]: url } });
    return true;
  }

  setModelLogo(ref: ModelRef, url: string | null): boolean {
    const key = modelRefKey(ref);
    if (url === null) {
      const { [key]: _x, ...rest } = this.snap.modelLogos;
      this.set({ modelLogos: rest });
      return true;
    }
    if (!isValidLogo(url)) return false;
    this.set({ modelLogos: { ...this.snap.modelLogos, [key]: url } });
    return true;
  }
}

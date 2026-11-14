// Pengaturan aplikasi. SATU sumber kebenaran: SETTINGS_SCHEMA (JSON Schema 2020-12).
// - UI Pengaturan dirender dari skema ini.
// - docs/manual/settings.schema.json + settings.md dihasilkan dari skema (dijaga test agar tidak basi).
// - Agent bisa membaca skema untuk mengonfigurasi aplikasi (PRD §4.7).
// - Kata kunci `x-zandor` berisi kunci i18n dan pengelompokan untuk UI (diabaikan validator JSON Schema lain).

export const UI_SCALE_VALUES = [0.75, 0.875, 1, 1.125, 1.25, 1.375, 1.5] as const;

type Meta = {
  /** Kunci i18n judul di UI. */
  titleKey: string;
  /** Kunci i18n deskripsi (opsional). */
  descriptionKey?: string;
  group: "general" | "appearance" | "chat" | "notifications" | "tools" | "layout";
  /** Kunci i18n label tiap nilai enum (opsional; bila tidak ada, nilai ditampilkan apa adanya). */
  enumLabelKeys?: Record<string, string>;
};

export type StringSetting = {
  type: "string";
  title: string;
  description: string;
  enum: readonly string[];
  default: string;
  "x-zandor": Meta;
};
export type NumberSetting = {
  type: "number";
  title: string;
  description: string;
  enum?: readonly number[];
  minimum?: number;
  maximum?: number;
  default: number;
  "x-zandor": Meta;
};
export type BooleanSetting = {
  type: "boolean";
  title: string;
  description: string;
  default: boolean;
  "x-zandor": Meta;
};
export type SettingProperty = StringSetting | NumberSetting | BooleanSetting;

export const SETTINGS_SCHEMA = {
  $schema: "https://json-schema.org/draft/2020-12/schema",
  $id: "https://zandor.web.id/schemas/settings.schema.json",
  title: "Pengaturan Zandor",
  description: "Pengaturan aplikasi Zandor. Semua properti opsional; nilai yang tidak ada memakai default.",
  type: "object",
  additionalProperties: false,
  properties: {
    language: {
      type: "string",
      title: "Bahasa",
      description: "Bahasa antarmuka. Pada pemakaian pertama mengikuti bahasa sistem bila didukung.",
      enum: ["id", "en"],
      default: "id",
      "x-zandor": {
        titleKey: "settings.language",
        group: "general",
        enumLabelKeys: { id: "language.id", en: "language.en" },
      },
    },
    theme: {
      type: "string",
      title: "Tema",
      description: "Tema warna. `system` mengikuti preferensi sistem operasi.",
      enum: ["dark", "light", "system"],
      default: "system",
      "x-zandor": {
        titleKey: "theme.label",
        group: "appearance",
        enumLabelKeys: { dark: "theme.dark", light: "theme.light", system: "theme.system" },
      },
    },
    uiScale: {
      type: "number",
      title: "Skala UI",
      description: "Skala antarmuka (token --ui-scale). Tidak memengaruhi isi browser bawaan.",
      enum: UI_SCALE_VALUES,
      default: 1,
      "x-zandor": { titleKey: "scale.label", group: "appearance" },
    },
    rememberLayout: {
      type: "boolean",
      title: "Ingat tata letak panel",
      description: "Menyimpan ukuran dan keadaan lipat panel antar sesi aplikasi.",
      default: true,
      "x-zandor": {
        titleKey: "settings.rememberLayout",
        descriptionKey: "settings.rememberLayoutHelp",
        group: "layout",
      },
    },
    notifyToast: {
      type: "boolean",
      title: "Toast di dalam aplikasi",
      description: "Tampilkan toast saat sesi selesai, gagal, atau butuh aksi.",
      default: true,
      "x-zandor": { titleKey: "settings.notifyToast", group: "notifications" },
    },
    notifySystem: {
      type: "boolean",
      title: "Notifikasi sistem",
      description: "Kirim notifikasi sistem operasi (butuh izin dari Anda).",
      default: true,
      "x-zandor": { titleKey: "settings.notifySystem", group: "notifications" },
    },
    notifySound: {
      type: "boolean",
      title: "Suara",
      description: "Putar suara untuk peristiwa sesi.",
      default: true,
      "x-zandor": { titleKey: "settings.notifySound", group: "notifications" },
    },
    notifyOnlyAway: {
      type: "boolean",
      title: "Hanya saat tidak dilihat",
      description: "Lewati pemberitahuan untuk sesi yang sedang Anda lihat (jendela fokus).",
      default: true,
      "x-zandor": { titleKey: "settings.notifyOnlyAway", group: "notifications" },
    },
    notifyDone: {
      type: "boolean",
      title: "Sesi selesai",
      description: "Beritahu saat agent selesai bekerja.",
      default: true,
      "x-zandor": { titleKey: "settings.notifyDone", group: "notifications" },
    },
    notifyFailed: {
      type: "boolean",
      title: "Sesi gagal atau terkena batas",
      description: "Beritahu saat sesi gagal (termasuk rate limit/kuota).",
      default: true,
      "x-zandor": { titleKey: "settings.notifyFailed", group: "notifications" },
    },
    notifyAction: {
      type: "boolean",
      title: "Butuh tindakan Anda",
      description: "Beritahu saat agent meminta izin atau mengajukan pertanyaan.",
      default: true,
      "x-zandor": { titleKey: "settings.notifyAction", group: "notifications" },
    },
    notifyVolume: {
      type: "number",
      title: "Volume suara",
      description: "Volume suara pemberitahuan.",
      enum: [0.2, 0.4, 0.6, 0.8, 1],
      default: 0.6,
      "x-zandor": { titleKey: "settings.notifyVolume", group: "notifications" },
    },
    soundDone: {
      type: "string",
      title: "Suara: selesai",
      description: "Suara saat sesi selesai.",
      enum: ["chime", "pop", "bell", "soft", "alert", "low", "none", "custom"],
      default: "chime",
      "x-zandor": {
        titleKey: "settings.soundDone",
        group: "notifications",
        enumLabelKeys: {
          chime: "sound.chime",
          pop: "sound.pop",
          bell: "sound.bell",
          soft: "sound.soft",
          alert: "sound.alert",
          low: "sound.low",
          none: "sound.none",
          custom: "sound.custom",
        },
      },
    },
    soundFailed: {
      type: "string",
      title: "Suara: gagal",
      description: "Suara saat sesi gagal atau terkena batas.",
      enum: ["chime", "pop", "bell", "soft", "alert", "low", "none", "custom"],
      default: "low",
      "x-zandor": {
        titleKey: "settings.soundFailed",
        group: "notifications",
        enumLabelKeys: {
          chime: "sound.chime",
          pop: "sound.pop",
          bell: "sound.bell",
          soft: "sound.soft",
          alert: "sound.alert",
          low: "sound.low",
          none: "sound.none",
          custom: "sound.custom",
        },
      },
    },
    soundAction: {
      type: "string",
      title: "Suara: butuh aksi",
      description: "Suara saat izin atau pertanyaan menunggu.",
      enum: ["chime", "pop", "bell", "soft", "alert", "low", "none", "custom"],
      default: "bell",
      "x-zandor": {
        titleKey: "settings.soundAction",
        group: "notifications",
        enumLabelKeys: {
          chime: "sound.chime",
          pop: "sound.pop",
          bell: "sound.bell",
          soft: "sound.soft",
          alert: "sound.alert",
          low: "sound.low",
          none: "sound.none",
          custom: "sound.custom",
        },
      },
    },
    sendKey: {
      type: "string",
      title: "Tombol kirim pesan",
      description: "Enter mengirim (Shift+Enter baris baru), atau Ctrl+Enter mengirim (Enter baris baru).",
      enum: ["enter", "ctrl-enter"],
      default: "enter",
      "x-zandor": {
        titleKey: "settings.sendKey",
        group: "chat",
        enumLabelKeys: { enter: "settings.sendKey.enter", "ctrl-enter": "settings.sendKey.ctrl" },
      },
    },
    chatWidth: {
      type: "string",
      title: "Lebar isi chat",
      description: "Lebar kolom pesan di Panel Chat.",
      enum: ["narrow", "normal", "wide"],
      default: "normal",
      "x-zandor": {
        titleKey: "settings.chatWidth",
        group: "chat",
        enumLabelKeys: {
          narrow: "settings.chatWidth.narrow",
          normal: "settings.chatWidth.normal",
          wide: "settings.chatWidth.wide",
        },
      },
    },
    confirmDelete: {
      type: "boolean",
      title: "Konfirmasi sebelum menghapus sesi",
      description: "Tanyakan dulu sebelum menghapus sesi dari sidebar.",
      default: true,
      "x-zandor": { titleKey: "settings.confirmDelete", group: "chat" },
    },
    showDemos: {
      type: "boolean",
      title: "Tampilkan sesi demo",
      description: "Tampilkan tiga sesi demo bawaan di daftar Chat.",
      default: true,
      "x-zandor": { titleKey: "settings.showDemos", group: "chat" },
    },
    terminalFontSize: {
      type: "number",
      title: "Ukuran huruf terminal",
      description: "Ukuran huruf terminal (satuan titik).",
      enum: [11, 12, 13, 14, 16],
      default: 13,
      "x-zandor": { titleKey: "settings.terminalFontSize", group: "tools" },
    },
  },
} as const satisfies {
  properties: Record<string, SettingProperty>;
  [k: string]: unknown;
};

export type SettingKey = keyof typeof SETTINGS_SCHEMA.properties;
export type Settings = {
  language: "id" | "en";
  theme: "dark" | "light" | "system";
  uiScale: (typeof UI_SCALE_VALUES)[number];
  rememberLayout: boolean;
  notifyToast: boolean;
  notifySystem: boolean;
  notifySound: boolean;
  notifyOnlyAway: boolean;
  notifyDone: boolean;
  notifyFailed: boolean;
  notifyAction: boolean;
  notifyVolume: 0.2 | 0.4 | 0.6 | 0.8 | 1;
  soundDone: SoundSetting;
  soundFailed: SoundSetting;
  soundAction: SoundSetting;
  sendKey: "enter" | "ctrl-enter";
  chatWidth: "narrow" | "normal" | "wide";
  confirmDelete: boolean;
  showDemos: boolean;
  terminalFontSize: 11 | 12 | 13 | 14 | 16;
};

/** Pilihan suara pemberitahuan: nada bawaan (disintesis), tanpa suara, atau berkas milik pengguna. */
export type SoundSetting = "chime" | "pop" | "bell" | "soft" | "alert" | "low" | "none" | "custom";

export const SETTING_KEYS = Object.keys(SETTINGS_SCHEMA.properties) as SettingKey[];

export function defaultSettings(): Settings {
  const out: Record<string, unknown> = {};
  for (const k of SETTING_KEYS) out[k] = SETTINGS_SCHEMA.properties[k].default;
  return out as Settings;
}

/** Validasi satu nilai terhadap propertinya. null = valid, selain itu pesan galat. */
export function validateSetting(key: string, value: unknown): string | null {
  const prop = (SETTINGS_SCHEMA.properties as Record<string, SettingProperty>)[key];
  if (!prop) return `pengaturan tidak dikenal: ${key}`;
  if (prop.type === "boolean") return typeof value === "boolean" ? null : `${key}: harus boolean`;
  if (prop.type === "string") {
    if (typeof value !== "string") return `${key}: harus string`;
    return prop.enum.includes(value) ? null : `${key}: harus salah satu dari ${prop.enum.join(", ")}`;
  }
  if (typeof value !== "number" || !Number.isFinite(value)) return `${key}: harus angka`;
  if (prop.enum && !prop.enum.includes(value)) return `${key}: harus salah satu dari ${prop.enum.join(", ")}`;
  if (prop.minimum !== undefined && value < prop.minimum) return `${key}: minimum ${prop.minimum}`;
  if (prop.maximum !== undefined && value > prop.maximum) return `${key}: maksimum ${prop.maximum}`;
  return null;
}

export type ValidationResult = { valid: boolean; errors: string[]; value: Settings };

/** Validasi objek mentah. Properti salah/tak dikenal dicatat sebagai galat; `value` selalu utuh (default untuk yang salah). */
export function validateSettings(raw: unknown): ValidationResult {
  const value = defaultSettings();
  const errors: string[] = [];
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    return { valid: false, errors: ["pengaturan harus berupa objek"], value };
  }
  for (const [k, v] of Object.entries(raw)) {
    const err = validateSetting(k, v);
    if (err) errors.push(err);
    else (value as Record<string, unknown>)[k] = v;
  }
  return { valid: errors.length === 0, errors, value };
}

// ---------- Store ----------

export type SettingsStorage = { get(): string | null; set(value: string): void };

export type SettingsStoreOptions = {
  storage: SettingsStorage;
  /** Dipanggil untuk nilai tersimpan yang tidak valid (dipakai logger). */
  onInvalid?: (errors: string[]) => void;
};

export class SettingsStore {
  private values: Settings;
  private explicit = new Set<SettingKey>();
  private listeners = new Set<() => void>();
  private snapshot: Settings;

  constructor(private readonly options: SettingsStoreOptions) {
    this.values = defaultSettings();
    this.snapshot = { ...this.values };
    this.load();
  }

  private load() {
    let raw: unknown = null;
    try {
      const text = this.options.storage.get();
      raw = text ? JSON.parse(text) : null;
    } catch {
      this.options.onInvalid?.(["pengaturan tersimpan rusak (bukan JSON valid); memakai default"]);
      return;
    }
    if (raw === null) return;
    const result = validateSettings(raw);
    if (!result.valid) this.options.onInvalid?.(result.errors);
    for (const k of SETTING_KEYS) {
      if (Object.hasOwn(raw as object, k) && validateSetting(k, (raw as Record<string, unknown>)[k]) === null) {
        (this.values as Record<string, unknown>)[k] = (raw as Record<string, unknown>)[k];
        this.explicit.add(k);
      }
    }
    this.snapshot = { ...this.values };
  }

  private persist() {
    const out: Record<string, unknown> = {};
    for (const k of this.explicit) out[k] = this.values[k];
    try {
      this.options.storage.set(JSON.stringify(out));
    } catch {
      // penyimpanan penuh/tidak tersedia: nilai tetap berlaku untuk sesi ini
    }
  }

  private notify() {
    this.snapshot = { ...this.values };
    for (const l of this.listeners) l();
  }

  /** Salinan stabil: identitas hanya berubah bila ada nilai yang berubah. */
  all(): Settings {
    return this.snapshot;
  }

  get<K extends SettingKey>(key: K): Settings[K] {
    return this.values[key];
  }

  /** Apakah pengguna/aplikasi pernah menetapkan nilai ini secara eksplisit (bukan default). */
  isExplicit(key: SettingKey): boolean {
    return this.explicit.has(key);
  }

  /** Tetapkan nilai. Melempar bila tidak valid terhadap skema. */
  set<K extends SettingKey>(key: K, value: Settings[K]): void {
    const err = validateSetting(key, value);
    if (err) throw new Error(err);
    if (this.values[key] === value && this.explicit.has(key)) return;
    this.values[key] = value;
    this.explicit.add(key);
    this.persist();
    this.notify();
  }

  /** Kembalikan ke default (dan hapus dari penyimpanan). */
  reset(key: SettingKey): void {
    if (!this.explicit.has(key)) return;
    this.explicit.delete(key);
    (this.values as Record<string, unknown>)[key] = SETTINGS_SCHEMA.properties[key].default;
    this.persist();
    this.notify();
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
}

// ---------- Dokumentasi yang dihasilkan dari skema ----------

/** Skema sebagai JSON (apa adanya) untuk docs/manual/settings.schema.json. */
export function schemaJson(): string {
  return `${JSON.stringify(SETTINGS_SCHEMA, null, 2)}\n`;
}

/** Referensi Markdown pengaturan untuk docs/manual/settings.md. */
export function schemaMarkdown(): string {
  const rows = SETTING_KEYS.map((k) => {
    const p = SETTINGS_SCHEMA.properties[k] as SettingProperty;
    const allowed = "enum" in p && p.enum ? p.enum.map((v) => `\`${v}\``).join(", ") : p.type;
    return `| \`${k}\` | ${p.type} | ${allowed} | \`${JSON.stringify(p.default)}\` | ${p.description} |`;
  }).join("\n");
  return `# Referensi pengaturan

> Dihasilkan dari \`packages/core/src/settings.ts\` oleh \`bun scripts/gen-settings-docs.ts\`. Jangan diedit manual.
> Skema lengkap (JSON Schema 2020-12): [\`settings.schema.json\`](settings.schema.json).

Pengaturan disimpan sebagai satu objek JSON. Properti yang tidak ada memakai default; properti tak dikenal ditolak.

| Kunci | Tipe | Nilai yang diizinkan | Default | Keterangan |
| --- | --- | --- | --- | --- |
${rows}

## Contoh

\`\`\`json
{ "language": "en", "theme": "dark", "uiScale": 1.25, "rememberLayout": true }
\`\`\`
`;
}

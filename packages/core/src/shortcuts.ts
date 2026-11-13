// Sistem pintasan keyboard (PRD §4.5): global + scoped, tanpa ketergantungan DOM.
//
// - Combo ditulis `Mod+Shift+K`. `Mod` = Ctrl di Windows. TODO(port): Meta di macOS (ADR-027).
// - Scope ditentukan pemanggil (UI memakai atribut `data-shortcut-scope` pada leluhur elemen fokus).
//   Urutan resolusi: scope terdalam → terluar → "global".
// - Scope "modal" bersifat eksklusif: selama aktif, hanya binding scope "modal" dan binding global
//   bertanda `allowInModal` yang berlaku.
// - Saat mengetik di kolom input, binding hanya berlaku bila memakai Ctrl/Alt/Meta atau bertanda `allowInInput`.

export type Combo = { key: string; ctrl: boolean; alt: boolean; shift: boolean; meta: boolean };

export type KeyEventLike = {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
};

export const GLOBAL_SCOPE = "global";
export const MODAL_SCOPE = "modal";

const KEY_ALIASES: Record<string, string> = {
  space: " ",
  spacebar: " ",
  esc: "Escape",
  escape: "Escape",
  plus: "+",
  minus: "-",
  equal: "=",
  enter: "Enter",
  return: "Enter",
  tab: "Tab",
  backspace: "Backspace",
  delete: "Delete",
  del: "Delete",
  up: "ArrowUp",
  down: "ArrowDown",
  left: "ArrowLeft",
  right: "ArrowRight",
  home: "Home",
  end: "End",
  pageup: "PageUp",
  pagedown: "PageDown",
};

function normalizeKey(raw: string): string {
  const alias = KEY_ALIASES[raw.toLowerCase()];
  if (alias) return alias;
  if (raw.length === 1) return raw.toLowerCase();
  // F1..F24 dan nama tombol lain dipakai apa adanya (huruf pertama kapital).
  return raw.charAt(0).toUpperCase() + raw.slice(1);
}

/** Urai "Mod+Shift+K". Melempar bila format salah (tidak ada tombol utama atau modifier ganda). */
export function parseCombo(text: string): Combo {
  // "+" sebagai tombol utama ditulis "Plus" (atau "Ctrl++": karakter terakhir setelah pemisah).
  const parts = text.endsWith("++") ? [...text.slice(0, -2).split("+"), "Plus"] : text.split("+");
  const combo: Combo = { key: "", ctrl: false, alt: false, shift: false, meta: false };
  for (const rawPart of parts) {
    const part = rawPart.trim();
    if (!part) throw new Error(`combo tidak valid: "${text}"`);
    switch (part.toLowerCase()) {
      case "mod":
      case "ctrl":
      case "control":
        combo.ctrl = true;
        break;
      case "alt":
      case "option":
        combo.alt = true;
        break;
      case "shift":
        combo.shift = true;
        break;
      case "meta":
      case "cmd":
      case "win":
        combo.meta = true;
        break;
      default:
        if (combo.key) throw new Error(`combo punya lebih dari satu tombol utama: "${text}"`);
        combo.key = normalizeKey(part);
    }
  }
  if (!combo.key) throw new Error(`combo tanpa tombol utama: "${text}"`);
  return combo;
}

/** Bentuk kanonik untuk perbandingan, mis. "ctrl+shift+k". */
export function comboId(combo: Combo): string {
  return [
    combo.ctrl && "ctrl",
    combo.alt && "alt",
    combo.shift && "shift",
    combo.meta && "meta",
    combo.key.toLowerCase(),
  ]
    .filter(Boolean)
    .join("+");
}

const KEY_LABELS: Record<string, string> = { " ": "Space", "+": "Plus", "-": "-", "=": "=" };

/** Tampilan manusia: "Ctrl+Shift+K". */
export function formatCombo(input: string | Combo): string {
  const c = typeof input === "string" ? parseCombo(input) : input;
  const key = KEY_LABELS[c.key] ?? (c.key.length === 1 ? c.key.toUpperCase() : c.key);
  return [c.ctrl && "Ctrl", c.alt && "Alt", c.shift && "Shift", c.meta && "Meta", key].filter(Boolean).join("+");
}

export function comboFromEvent(e: KeyEventLike): Combo {
  return { key: normalizeKey(e.key), ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey };
}

export type ShortcutBinding = {
  /** Unik, mis. "view.toggleChat". */
  id: string;
  combo: string;
  scope: string;
  /** Kunci i18n deskripsi (untuk editor shortcut & manual). */
  description: string;
  allowInInput?: boolean;
  allowInModal?: boolean;
  run: (event: KeyEventLike) => void;
  /** Bila mengembalikan false, binding dilewati. */
  when?: () => boolean;
};

export type DispatchContext = { scopes: string[]; inInput: boolean };

export class ShortcutRegistry {
  private bindings = new Map<string, { binding: ShortcutBinding; combo: Combo }>();
  /** id binding → combo pilihan pengguna (menggantikan combo bawaan). */
  private overrides = new Map<string, string>();
  private listeners = new Set<() => void>();
  private snapshot: ShortcutBinding[] = [];

  private changed() {
    this.snapshot = [...this.bindings.values()].map((b) => b.binding);
    for (const l of this.listeners) l();
  }

  /** Berlangganan perubahan daftar binding (untuk useSyncExternalStore). Mengembalikan fungsi berhenti. */
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Daftarkan binding. Melempar bila id ganda atau combo yang sama sudah dipakai di scope yang sama. */
  register(binding: ShortcutBinding): () => void {
    if (this.bindings.has(binding.id)) throw new Error(`id shortcut ganda: ${binding.id}`);
    const own = this.overrides.get(binding.id);
    let combo = parseCombo(binding.combo);
    if (own) {
      try {
        const o = parseCombo(own);
        const taken = [...this.bindings.values()].some(
          (b) => b.binding.scope === binding.scope && comboId(b.combo) === comboId(o),
        );
        if (!taken) combo = o;
      } catch {
        // override rusak: pakai combo bawaan
      }
    }
    const clash = [...this.bindings.values()].find(
      (b) => b.binding.scope === binding.scope && comboId(b.combo) === comboId(combo),
    );
    if (clash) {
      throw new Error(`bentrok: ${formatCombo(combo)} di scope "${binding.scope}" sudah dipakai ${clash.binding.id}`);
    }
    this.bindings.set(binding.id, { binding, combo });
    this.changed();
    return () => {
      if (this.bindings.delete(binding.id)) this.changed();
    };
  }

  /** Daftar binding. Identitas array stabil selama daftar tidak berubah (aman untuk useSyncExternalStore). */
  list(): ShortcutBinding[] {
    return this.snapshot;
  }

  /** Binding yang berlaku untuk combo, atau null. Tidak menjalankan apa pun. */
  resolve(combo: Combo, ctx: DispatchContext): ShortcutBinding | null {
    const id = comboId(combo);
    const hasModifier = combo.ctrl || combo.alt || combo.meta;
    const modalOpen = ctx.scopes.includes(MODAL_SCOPE);
    const order = [...ctx.scopes.filter((s) => s !== GLOBAL_SCOPE), GLOBAL_SCOPE];
    for (const scope of order) {
      for (const { binding, combo: bc } of this.bindings.values()) {
        if (binding.scope !== scope || comboId(bc) !== id) continue;
        if (modalOpen && scope !== MODAL_SCOPE && !binding.allowInModal) continue;
        if (ctx.inInput && !hasModifier && !binding.allowInInput) continue;
        if (binding.when && !binding.when()) continue;
        return binding;
      }
    }
    return null;
  }

  /** Combo yang berlaku untuk id (override pengguna bila ada), atau null. */
  comboOf(id: string): string | null {
    const b = this.bindings.get(id);
    return b ? formatCombo(b.combo) : null;
  }

  /** Combo bawaan dari kode untuk id, atau null. */
  defaultComboOf(id: string): string | null {
    const b = this.bindings.get(id);
    return b ? formatCombo(b.binding.combo) : null;
  }

  /**
   * Ubah combo satu binding (`null` = kembali ke bawaan). Mengembalikan pesan galat (format salah / bentrok dengan
   * binding lain di scope yang sama) atau null bila berhasil.
   */
  setOverride(id: string, combo: string | null): string | null {
    const target = this.bindings.get(id);
    if (!target) return `shortcut tidak dikenal: ${id}`;
    let next: Combo;
    try {
      next = parseCombo(combo ?? target.binding.combo);
    } catch (e) {
      return e instanceof Error ? e.message : String(e);
    }
    const clash = [...this.bindings.entries()].find(
      ([otherId, o]) =>
        otherId !== id && o.binding.scope === target.binding.scope && comboId(o.combo) === comboId(next),
    );
    if (clash) return `bentrok: ${formatCombo(next)} sudah dipakai ${clash[0]} di scope "${target.binding.scope}"`;
    if (combo === null) this.overrides.delete(id);
    else this.overrides.set(id, combo);
    target.combo = next;
    this.changed();
    return null;
  }

  /** Terapkan seluruh override tersimpan (mengganti yang lama). Mengembalikan galat per id yang ditolak. */
  applyOverrides(map: Record<string, string>): Record<string, string> {
    const errors: Record<string, string> = {};
    for (const id of [...this.overrides.keys()]) {
      if (id in map) continue;
      if (this.bindings.has(id)) this.setOverride(id, null);
      else this.overrides.delete(id);
    }
    for (const [id, combo] of Object.entries(map)) {
      if (this.bindings.has(id)) {
        const err = this.setOverride(id, combo);
        if (err) errors[id] = err;
      } else this.overrides.set(id, combo); // dipasang saat binding terdaftar
    }
    return errors;
  }

  /** Jalankan binding yang cocok. true bila ada yang ditangani (pemanggil sebaiknya preventDefault). */
  dispatch(event: KeyEventLike, ctx: DispatchContext): boolean {
    const binding = this.resolve(comboFromEvent(event), ctx);
    if (!binding) return false;
    binding.run(event);
    return true;
  }
}

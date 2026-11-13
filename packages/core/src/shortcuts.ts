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

  /** Daftarkan binding. Melempar bila id ganda atau combo yang sama sudah dipakai di scope yang sama. */
  register(binding: ShortcutBinding): () => void {
    if (this.bindings.has(binding.id)) throw new Error(`id shortcut ganda: ${binding.id}`);
    const combo = parseCombo(binding.combo);
    const clash = [...this.bindings.values()].find(
      (b) => b.binding.scope === binding.scope && comboId(b.combo) === comboId(combo),
    );
    if (clash) {
      throw new Error(`bentrok: ${formatCombo(combo)} di scope "${binding.scope}" sudah dipakai ${clash.binding.id}`);
    }
    this.bindings.set(binding.id, { binding, combo });
    return () => {
      this.bindings.delete(binding.id);
    };
  }

  list(): ShortcutBinding[] {
    return [...this.bindings.values()].map((b) => b.binding);
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

  /** Jalankan binding yang cocok. true bila ada yang ditangani (pemanggil sebaiknya preventDefault). */
  dispatch(event: KeyEventLike, ctx: DispatchContext): boolean {
    const binding = this.resolve(comboFromEvent(event), ctx);
    if (!binding) return false;
    binding.run(event);
    return true;
  }
}

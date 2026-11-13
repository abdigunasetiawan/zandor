// Pengait React untuk ShortcutRegistry (logika murni di @zandor/core).
// Scope ditentukan dari leluhur elemen yang sedang fokus: atribut `data-shortcut-scope`.
import { GLOBAL_SCOPE, type ShortcutBinding, ShortcutRegistry } from "@zandor/core";
import { createContext, type ReactNode, useContext, useEffect, useMemo, useRef } from "react";

const RegistryContext = createContext<ShortcutRegistry | null>(null);

const INPUT_SELECTOR = "input, textarea, select, [contenteditable=''], [contenteditable='true']";

/** Scope dari elemen ke atas (terdalam dulu). "global" ditambahkan oleh registry. */
export function scopesFromElement(el: Element | null): string[] {
  const scopes: string[] = [];
  for (let node = el; node; node = node.parentElement) {
    const s = node.getAttribute("data-shortcut-scope");
    if (s && s !== GLOBAL_SCOPE) scopes.push(s);
  }
  return scopes;
}

export function ShortcutProvider({ registry, children }: { registry?: ShortcutRegistry; children: ReactNode }) {
  const reg = useMemo(() => registry ?? new ShortcutRegistry(), [registry]);
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing) return;
      const target = e.target instanceof Element ? e.target : document.activeElement;
      const handled = reg.dispatch(e, {
        scopes: scopesFromElement(target),
        inInput: Boolean(target?.closest(INPUT_SELECTOR)),
      });
      if (handled) {
        e.preventDefault();
        e.stopPropagation();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [reg]);
  return <RegistryContext.Provider value={reg}>{children}</RegistryContext.Provider>;
}

export function useShortcutRegistry(): ShortcutRegistry {
  const reg = useContext(RegistryContext);
  if (!reg) throw new Error("harus dipakai di dalam <ShortcutProvider>");
  return reg;
}

/** Daftarkan binding selama komponen terpasang. `run` selalu memakai versi terbaru (tanpa daftar ulang). */
export function useShortcut(binding: Omit<ShortcutBinding, "run"> & { run: ShortcutBinding["run"] }) {
  const reg = useShortcutRegistry();
  const run = useRef(binding.run);
  run.current = binding.run;
  const when = useRef(binding.when);
  when.current = binding.when;
  const { id, combo, scope, description, allowInInput, allowInModal } = binding;
  useEffect(
    () =>
      reg.register({
        id,
        combo,
        scope,
        description,
        allowInInput,
        allowInModal,
        run: (e) => run.current(e),
        when: () => when.current?.() ?? true,
      }),
    [reg, id, combo, scope, description, allowInInput, allowInModal],
  );
}

/** Menandai wilayah UI (panel/view) sebagai scope pintasan. Tidak memengaruhi tata letak. */
export function ShortcutScope({ name, children }: { name: string; children: ReactNode }) {
  return (
    <div data-shortcut-scope={name} className="z-scope">
      {children}
    </div>
  );
}

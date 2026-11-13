import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { useI18n } from "@zandor/i18n/react";
import { Fragment, type KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { Kbd } from "../atoms/Kbd";
import { cx } from "../cx";
import { SearchIcon } from "../icons";

export type PaletteItem = {
  id: string;
  label: string;
  /** Kelompok (ditampilkan redup di kanan). */
  group?: string;
  shortcut?: string;
  disabled?: boolean;
  /** Indeks karakter `label` yang cocok dengan query (untuk penyorotan). */
  matchIndices?: number[];
};

export type PaletteProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  query: string;
  onQueryChange: (query: string) => void;
  items: PaletteItem[];
  /** Dipanggil saat item dipilih (Enter/klik). Palette menutup dirinya setelahnya. */
  onSelect: (id: string) => void;
  placeholder?: string;
  /** Wajib: nama palette untuk pembaca layar. */
  "aria-label": string;
  emptyText?: string;
};

function Label({ text, indices }: { text: string; indices?: number[] }) {
  if (!indices || indices.length === 0) return <>{text}</>;
  const set = new Set(indices);
  return (
    <>
      {Array.from(text).map((ch, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: posisi karakter adalah identitasnya
        <Fragment key={i}>{set.has(i) ? <mark className="z-palette__match">{ch}</mark> : ch}</Fragment>
      ))}
    </>
  );
}

const nextEnabled = (items: PaletteItem[], from: number, dir: 1 | -1): number => {
  for (let step = 1; step <= items.length; step++) {
    const i = (from + dir * step + items.length * 2) % items.length;
    if (!items[i]?.disabled) return i;
  }
  return -1;
};

/** Palette perintah: kolom cari + daftar hasil, navigasi keyboard penuh (combobox/listbox WAI-ARIA). */
export function Palette({
  open,
  onOpenChange,
  query,
  onQueryChange,
  items,
  onSelect,
  placeholder,
  emptyText,
  ...aria
}: PaletteProps) {
  const { t } = useI18n();
  const listId = useId();
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);

  // Setiap hasil berubah, sorot item aktif pertama yang tidak dinonaktifkan.
  const firstEnabled = items.findIndex((i) => !i.disabled);
  // biome-ignore lint/correctness/useExhaustiveDependencies: dijalankan saat pencarian/hasil/pembukaan berubah
  useEffect(() => {
    setActive(firstEnabled === -1 ? 0 : firstEnabled);
  }, [query, items.length, open, firstEnabled]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: dijalankan saat item aktif berubah
  useEffect(() => {
    listRef.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  const choose = (item: PaletteItem | undefined) => {
    if (!item || item.disabled) return;
    onSelect(item.id);
    onOpenChange(false);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      const n = nextEnabled(items, active, 1);
      if (n !== -1) setActive(n);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const n = nextEnabled(items, active, -1);
      if (n !== -1) setActive(n);
    } else if (e.key === "Home") {
      e.preventDefault();
      const first = items.findIndex((i) => !i.disabled);
      if (first !== -1) setActive(first);
    } else if (e.key === "End") {
      e.preventDefault();
      const last = items.findLastIndex((i) => !i.disabled);
      if (last !== -1) setActive(last);
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(items[active]);
    }
  };

  const optionId = (i: number) => `${listId}-opt-${i}`;

  return (
    <BaseDialog.Root open={open} onOpenChange={onOpenChange}>
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className="z-backdrop" />
        <BaseDialog.Popup className="z-palette" data-shortcut-scope="modal" aria-label={aria["aria-label"]}>
          <BaseDialog.Title className="z-visually-hidden">{aria["aria-label"]}</BaseDialog.Title>
          <div className="z-palette__bar">
            <SearchIcon size="sm" className="z-palette__icon" />
            <input
              className="z-palette__input"
              role="combobox"
              aria-expanded
              aria-controls={listId}
              aria-activedescendant={items.length > 0 ? optionId(active) : undefined}
              aria-autocomplete="list"
              aria-label={aria["aria-label"]}
              placeholder={placeholder}
              value={query}
              onChange={(e) => onQueryChange(e.target.value)}
              onKeyDown={onKeyDown}
              spellCheck={false}
              autoComplete="off"
            />
          </div>
          {items.length === 0 ? (
            <p className="z-palette__empty" role="status">
              {emptyText ?? t("ui.noResults")}
            </p>
          ) : (
            // biome-ignore lint/a11y/noNoninteractiveElementToInteractiveRole: pola listbox WAI-ARIA; fokus tetap di input (aria-activedescendant)
            <ul className="z-palette__list" id={listId} role="listbox" ref={listRef} aria-label={aria["aria-label"]}>
              {items.map((item, i) => (
                // Fokus tetap di input (pola aria-activedescendant); opsi dipilih dengan mouse atau Enter.
                // biome-ignore lint/a11y/useFocusableInteractive: fokus tetap di input combobox (aria-activedescendant)
                // biome-ignore lint/a11y/useKeyWithClickEvents: keyboard (panah/Enter) ditangani di input combobox
                <li
                  key={item.id}
                  id={optionId(i)}
                  // biome-ignore lint/a11y/noNoninteractiveElementToInteractiveRole: pola listbox/option WAI-ARIA
                  role="option"
                  aria-selected={i === active}
                  aria-disabled={item.disabled || undefined}
                  className={cx("z-palette__item", i === active && "z-palette__item--active")}
                  onMouseMove={() => !item.disabled && setActive(i)}
                  onClick={() => choose(item)}
                >
                  <span className="z-palette__label">
                    <Label text={item.label} indices={item.matchIndices} />
                  </span>
                  {item.group && <span className="z-palette__group">{item.group}</span>}
                  {item.shortcut && <Kbd keys={item.shortcut} />}
                </li>
              ))}
            </ul>
          )}
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}

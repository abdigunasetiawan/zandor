// Varian `Select` untuk daftar panjang: kolom cari di atas daftar + pengelompokan otomatis (`SelectItem.group`) dengan
// grup yang bisa dibuka/ditutup. Dipakai otomatis oleh `Select` bila item banyak (lihat SEARCH_THRESHOLD) atau
// `searchable`. Grup yang berisi pilihan aktif terbuka; saat mencari semua grup yang cocok terbuka.
import { Popover as BasePopover } from "@base-ui/react/popover";
import { useI18n } from "@zandor/i18n/react";
import { type KeyboardEvent, useId, useMemo, useRef, useState } from "react";
import { cx } from "../cx";
import { CheckIcon, ChevronDownIcon, ChevronRightIcon, CloseIcon, SearchIcon } from "../icons";
import { OFFSET_TIGHT } from "../layout";
import type { SelectProps } from "./Fields";

/** Daftar dengan lebih dari ini item otomatis mendapat pencarian dan grup. */
export const SEARCH_THRESHOLD = 12;

type Section = { group: string | null; items: SelectProps["items"] };

/** Kelompokkan item menurut `group` (urutan kemunculan pertama); item tanpa grup di bagian atas. */
export function groupItems(items: SelectProps["items"]): Section[] {
  const out: Section[] = [];
  const at = new Map<string | null, Section>();
  for (const it of items) {
    const g = it.group ?? null;
    let s = at.get(g);
    if (!s) {
      s = { group: g, items: [] };
      at.set(g, s);
      if (g === null) out.unshift(s);
      else out.push(s);
    }
    s.items.push(it);
  }
  return out;
}

const matches = (q: string, it: SelectProps["items"][number]) =>
  !q || `${it.label} ${it.description ?? ""} ${it.group ?? ""} ${it.value}`.toLowerCase().includes(q);

export function SearchSelect({
  items,
  value,
  onValueChange,
  placeholder,
  disabled,
  invalid,
  size = "md",
  className,
  icon,
  "aria-label": ariaLabel,
}: SelectProps) {
  const { t } = useI18n();
  const id = useId();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  /** Grup yang dibuka/ditutup pengguna (menimpa bawaan). */
  const [toggled, setToggled] = useState<Record<string, boolean>>({});
  const [active, setActive] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);

  const selected = items.find((i) => i.value === value) ?? null;
  const q = query.trim().toLowerCase();
  const sections = useMemo(() => groupItems(items), [items]);
  const grouped = sections.some((s) => s.group !== null);
  const selectedGroup = selected?.group ?? null;

  const view = sections
    .map((s) => {
      const hits = s.items.filter((it) => matches(q, it));
      const expanded =
        s.group === null ||
        !!q ||
        (toggled[s.group] ?? (s.group === selectedGroup || sections.filter((x) => x.group).length === 1));
      return { ...s, hits, expanded };
    })
    .filter((s) => s.hits.length > 0);
  const visible = view.flatMap((s) => (s.expanded ? s.hits.filter((i) => !i.disabled) : []));
  const optId = (v: string) => `${id}-o-${encodeURIComponent(v)}`;

  const choose = (v: string) => {
    onValueChange(v);
    setOpen(false);
  };
  const move = (delta: number) => {
    if (!visible.length) return;
    const i = visible.findIndex((x) => x.value === active);
    const next = visible[(i + delta + visible.length) % visible.length] ?? visible[0];
    if (!next) return;
    setActive(next.value);
    list.current?.querySelector(`[id="${CSS.escape(optId(next.value))}"]`)?.scrollIntoView({ block: "nearest" });
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      move(e.key === "ArrowDown" ? 1 : -1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const target = visible.find((x) => x.value === active) ?? (q ? visible[0] : undefined);
      if (target) choose(target.value);
    } else if (e.key === "Escape" && query) {
      e.preventDefault();
      e.stopPropagation();
      setQuery("");
    }
  };

  return (
    <BasePopover.Root
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (o) {
          setQuery("");
          setActive(value);
        }
      }}
    >
      <BasePopover.Trigger
        disabled={disabled}
        className={cx("z-select", `z-select--${size}`, className)}
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel}
        aria-invalid={invalid || undefined}
      >
        {icon}
        <span className="z-select__value" data-placeholder={selected ? undefined : ""}>
          {selected ? (selected.group ? `${selected.group} · ${selected.label}` : selected.label) : (placeholder ?? "")}
        </span>
        <span className="z-select__icon">
          <ChevronDownIcon size="sm" />
        </span>
      </BasePopover.Trigger>
      <BasePopover.Portal>
        <BasePopover.Positioner sideOffset={OFFSET_TIGHT} align="start" className="z-popup-positioner">
          <BasePopover.Popup className="z-popup z-select__popup z-sselect" initialFocus={input}>
            <search className="z-search z-sselect__search">
              <SearchIcon size="sm" className="z-search__icon" />
              <input
                ref={input}
                type="search"
                className="z-input z-input--sm z-search__input"
                value={query}
                placeholder={t("ui.search")}
                aria-label={t("ui.search")}
                role="combobox"
                aria-expanded="true"
                aria-controls={`${id}-list`}
                aria-activedescendant={active && visible.some((x) => x.value === active) ? optId(active) : undefined}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setActive(null);
                }}
                onKeyDown={onKey}
              />
              {query && (
                <button
                  type="button"
                  className="z-search__clear"
                  aria-label={t("ui.clear")}
                  onClick={() => {
                    setQuery("");
                    input.current?.focus();
                  }}
                >
                  <CloseIcon size="sm" />
                </button>
              )}
            </search>
            <div ref={list} id={`${id}-list`} role="listbox" aria-label={ariaLabel} className="z-sselect__list">
              {view.length === 0 && <p className="z-sselect__empty">{t("ui.noResults")}</p>}
              {view.map((s) => (
                <div key={s.group ?? "\u0000"} role="presentation" className="z-sselect__section">
                  {grouped && s.group !== null && (
                    <button
                      type="button"
                      tabIndex={-1}
                      className="z-sselect__group"
                      aria-expanded={s.expanded}
                      disabled={!!q}
                      onClick={() => setToggled((m) => ({ ...m, [s.group as string]: !s.expanded }))}
                    >
                      {s.expanded ? <ChevronDownIcon size="sm" /> : <ChevronRightIcon size="sm" />}
                      <span className="z-sselect__groupname">{s.group}</span>
                      <span className="z-sselect__count">{s.hits.length}</span>
                    </button>
                  )}
                  {s.expanded &&
                    s.hits.map((it) => (
                      // biome-ignore lint/a11y/useFocusableInteractive: fokus tetap di kolom cari (aria-activedescendant)
                      <div
                        key={it.value}
                        id={optId(it.value)}
                        role="option"
                        aria-selected={it.value === value}
                        aria-disabled={it.disabled || undefined}
                        data-highlighted={it.value === active ? "" : undefined}
                        data-disabled={it.disabled ? "" : undefined}
                        className={cx("z-item", it.description && "z-item--described", grouped && "z-sselect__opt")}
                        onMouseMove={() => !it.disabled && setActive(it.value)}
                        onClick={() => !it.disabled && choose(it.value)}
                        onKeyDown={undefined}
                      >
                        {it.icon && (
                          <span className="z-item__icon" aria-hidden="true">
                            {it.icon}
                          </span>
                        )}
                        {it.description ? (
                          <span className="z-item__text">
                            <span className="z-item__label">{it.label}</span>
                            <span className="z-item__desc">{it.description}</span>
                          </span>
                        ) : (
                          <span className="z-sselect__label">{it.label}</span>
                        )}
                        {it.value === value && (
                          <span className="z-item__indicator">
                            <CheckIcon size="sm" />
                          </span>
                        )}
                      </div>
                    ))}
                </div>
              ))}
            </div>
          </BasePopover.Popup>
        </BasePopover.Positioner>
      </BasePopover.Portal>
    </BasePopover.Root>
  );
}

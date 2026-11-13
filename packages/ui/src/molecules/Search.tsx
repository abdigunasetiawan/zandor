import { Combobox as BaseCombobox } from "@base-ui/react/combobox";
import { useI18n } from "@zandor/i18n/react";
import { Input } from "../atoms/Fields";
import { cx } from "../cx";
import { ChevronDownIcon, CloseIcon, SearchIcon } from "../icons";
import { OFFSET_TIGHT } from "../layout";

export type ComboboxItem = { value: string; label: string };

export type ComboboxProps = {
  items: ComboboxItem[];
  value: string | null;
  onValueChange: (value: string | null) => void;
  placeholder?: string;
  /** Wajib: nama kontrol untuk pembaca layar. */
  "aria-label": string;
  className?: string;
};

/** Pilih satu item dengan pencarian ketik-untuk-menyaring. */
export function Combobox({ items, value, onValueChange, placeholder, className, ...aria }: ComboboxProps) {
  const { t } = useI18n();
  const selected = items.find((i) => i.value === value) ?? null;
  return (
    <BaseCombobox.Root
      items={items}
      value={selected}
      onValueChange={(item) => onValueChange(item ? item.value : null)}
      itemToStringLabel={(item) => item.label}
    >
      <BaseCombobox.InputGroup className={cx("z-combobox", className)}>
        <BaseCombobox.Input className="z-combobox__input" placeholder={placeholder} aria-label={aria["aria-label"]} />
        <BaseCombobox.Trigger className="z-combobox__trigger" aria-label={aria["aria-label"]}>
          <ChevronDownIcon size="sm" />
        </BaseCombobox.Trigger>
      </BaseCombobox.InputGroup>
      <BaseCombobox.Portal>
        <BaseCombobox.Positioner sideOffset={OFFSET_TIGHT} className="z-popup-positioner">
          <BaseCombobox.Popup className="z-popup z-combobox__popup">
            <BaseCombobox.Empty className="z-combobox__empty">{t("ui.noResults")}</BaseCombobox.Empty>
            <BaseCombobox.List>
              {(item: ComboboxItem) => (
                <BaseCombobox.Item key={item.value} value={item} className="z-item">
                  {item.label}
                </BaseCombobox.Item>
              )}
            </BaseCombobox.List>
          </BaseCombobox.Popup>
        </BaseCombobox.Positioner>
      </BaseCombobox.Portal>
    </BaseCombobox.Root>
  );
}

export type SearchFieldProps = {
  value: string;
  onValueChange: (value: string) => void;
  placeholder?: string;
  /** Wajib: nama kontrol untuk pembaca layar. */
  "aria-label": string;
  className?: string;
};

/** Kolom pencarian dengan ikon, tombol bersihkan, dan Esc untuk mengosongkan. */
export function SearchField({ value, onValueChange, placeholder, className, ...aria }: SearchFieldProps) {
  const { t } = useI18n();
  return (
    <search className={cx("z-search", className)}>
      <SearchIcon size="sm" className="z-search__icon" />
      <Input
        type="search"
        className="z-search__input"
        value={value}
        placeholder={placeholder ?? t("ui.search")}
        aria-label={aria["aria-label"]}
        onChange={(e) => onValueChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && value) {
            e.preventDefault();
            e.stopPropagation();
            onValueChange("");
          }
        }}
      />
      {value && (
        <button type="button" className="z-search__clear" aria-label={t("ui.clear")} onClick={() => onValueChange("")}>
          <CloseIcon size="sm" />
        </button>
      )}
    </search>
  );
}

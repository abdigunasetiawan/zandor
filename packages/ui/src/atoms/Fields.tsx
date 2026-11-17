import { Checkbox as BaseCheckbox } from "@base-ui/react/checkbox";
import { Input as BaseInput } from "@base-ui/react/input";
import { Select as BaseSelect } from "@base-ui/react/select";
import { Slider as BaseSlider } from "@base-ui/react/slider";
import { Switch as BaseSwitch } from "@base-ui/react/switch";
import { type ComponentPropsWithRef, type ReactNode, useId } from "react";
import { cx } from "../cx";
import { CheckIcon, ChevronDownIcon } from "../icons";
import { OFFSET_TIGHT } from "../layout";
import type { ControlSize } from "./Button";
import { SEARCH_THRESHOLD, SearchSelect } from "./SearchSelect";

export type InputProps = Omit<ComponentPropsWithRef<"input">, "size"> & { invalid?: boolean; size?: ControlSize };

export function Input({ invalid, size = "md", className, ...rest }: InputProps) {
  return (
    <BaseInput className={cx("z-input", `z-input--${size}`, className)} aria-invalid={invalid || undefined} {...rest} />
  );
}

export type TextareaProps = ComponentPropsWithRef<"textarea"> & { invalid?: boolean };

export function Textarea({ invalid, className, rows = 3, ...rest }: TextareaProps) {
  return (
    <textarea
      className={cx("z-input", "z-textarea", className)}
      rows={rows}
      aria-invalid={invalid || undefined}
      {...rest}
    />
  );
}

/**
 * `description` (opsional) tampil di bawah label hanya di daftar pilihan, tidak di pemicu. `icon` (opsional, dekoratif)
 * tampil di depan label di daftar; untuk menampilkannya di pemicu, teruskan ikon item terpilih lewat prop `icon`.
 */
export type SelectItem = {
  value: string;
  label: string;
  description?: string;
  disabled?: boolean;
  icon?: ReactNode;
  /** Nama grup (mis. provider). Item bergrup dikelompokkan otomatis dalam grup yang bisa dibuka/ditutup. */
  group?: string;
};

export type SelectProps = {
  items: SelectItem[];
  value: string | null;
  onValueChange: (value: string) => void;
  placeholder?: string;
  "aria-label"?: string;
  disabled?: boolean;
  invalid?: boolean;
  size?: ControlSize;
  className?: string;
  /** Ikon di awal pemicu (dekoratif). */
  icon?: ReactNode;
  /**
   * Kolom cari + grup yang bisa dilipat. Bawaan: otomatis bila item lebih dari `SEARCH_THRESHOLD` atau ada item
   * bergrup. `false` memaksa daftar biasa.
   */
  searchable?: boolean;
};

export function Select(props: SelectProps) {
  const auto = props.items.length > SEARCH_THRESHOLD || props.items.some((i) => i.group);
  return (props.searchable ?? auto) ? <SearchSelect {...props} /> : <PlainSelect {...props} />;
}

function PlainSelect({
  items,
  value,
  onValueChange,
  placeholder,
  disabled,
  invalid,
  size = "md",
  className,
  icon,
  searchable: _searchable,
  ...aria
}: SelectProps) {
  return (
    <BaseSelect.Root
      items={items}
      value={value}
      disabled={disabled}
      onValueChange={(v) => {
        if (typeof v === "string") onValueChange(v);
      }}
    >
      <BaseSelect.Trigger
        className={cx("z-select", `z-select--${size}`, className)}
        aria-label={aria["aria-label"]}
        aria-invalid={invalid || undefined}
      >
        {icon}
        <BaseSelect.Value placeholder={placeholder} className="z-select__value" />
        <BaseSelect.Icon className="z-select__icon">
          <ChevronDownIcon size="sm" />
        </BaseSelect.Icon>
      </BaseSelect.Trigger>
      <BaseSelect.Portal>
        <BaseSelect.Positioner sideOffset={OFFSET_TIGHT} className="z-popup-positioner">
          <BaseSelect.Popup className="z-popup z-select__popup">
            <BaseSelect.List className="z-select__list">
              {items.map((item) => (
                <BaseSelect.Item
                  key={item.value}
                  value={item.value}
                  disabled={item.disabled}
                  className={cx("z-item", item.description && "z-item--described")}
                >
                  {item.icon && (
                    <span className="z-item__icon" aria-hidden="true">
                      {item.icon}
                    </span>
                  )}
                  {item.description ? (
                    <span className="z-item__text">
                      <BaseSelect.ItemText className="z-item__label">{item.label}</BaseSelect.ItemText>
                      <span className="z-item__desc">{item.description}</span>
                    </span>
                  ) : (
                    <BaseSelect.ItemText>{item.label}</BaseSelect.ItemText>
                  )}
                  <BaseSelect.ItemIndicator className="z-item__indicator">
                    <CheckIcon size="sm" />
                  </BaseSelect.ItemIndicator>
                </BaseSelect.Item>
              ))}
            </BaseSelect.List>
          </BaseSelect.Popup>
        </BaseSelect.Positioner>
      </BaseSelect.Portal>
    </BaseSelect.Root>
  );
}

type ChoiceBase = { label: ReactNode; disabled?: boolean; className?: string; "aria-label"?: string };

export type SwitchProps = ChoiceBase & { checked: boolean; onCheckedChange: (checked: boolean) => void };

export function Switch({ label, checked, onCheckedChange, disabled, className, ...aria }: SwitchProps) {
  const labelId = useId();
  return (
    <span className={cx("z-choice", className)}>
      <BaseSwitch.Root
        checked={checked}
        onCheckedChange={(c) => onCheckedChange(c)}
        disabled={disabled}
        className="z-switch"
        aria-labelledby={aria["aria-label"] ? undefined : labelId}
        aria-label={aria["aria-label"]}
      >
        <BaseSwitch.Thumb className="z-switch__thumb" />
      </BaseSwitch.Root>
      <span id={labelId} className="z-label z-choice__label">
        {label}
      </span>
    </span>
  );
}

export type CheckboxProps = ChoiceBase & {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  indeterminate?: boolean;
};

export function Checkbox({
  label,
  checked,
  onCheckedChange,
  indeterminate,
  disabled,
  className,
  ...aria
}: CheckboxProps) {
  const labelId = useId();
  return (
    <span className={cx("z-choice", className)}>
      <BaseCheckbox.Root
        checked={checked}
        indeterminate={indeterminate}
        onCheckedChange={(c) => onCheckedChange(c)}
        disabled={disabled}
        className="z-checkbox"
        aria-labelledby={aria["aria-label"] ? undefined : labelId}
        aria-label={aria["aria-label"]}
      >
        <BaseCheckbox.Indicator className="z-checkbox__indicator">
          {indeterminate ? <span className="z-checkbox__dash" /> : <CheckIcon size="sm" />}
        </BaseCheckbox.Indicator>
      </BaseCheckbox.Root>
      <span id={labelId} className="z-label z-choice__label">
        {label}
      </span>
    </span>
  );
}

export type SliderProps = {
  value: number;
  onValueChange: (value: number) => void;
  min: number;
  max: number;
  step?: number;
  /** Wajib: nama kontrol untuk pembaca layar. */
  label: string;
  disabled?: boolean;
  className?: string;
};

export function Slider({ value, onValueChange, min, max, step = 1, label, disabled, className }: SliderProps) {
  return (
    <BaseSlider.Root
      value={value}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      onValueChange={(v) => onValueChange(Array.isArray(v) ? (v[0] ?? min) : v)}
      className={cx("z-slider", className)}
    >
      <BaseSlider.Control className="z-slider__control">
        <BaseSlider.Track className="z-slider__track">
          <BaseSlider.Indicator className="z-slider__indicator" />
          <BaseSlider.Thumb className="z-slider__thumb" aria-label={label} />
        </BaseSlider.Track>
      </BaseSlider.Control>
    </BaseSlider.Root>
  );
}

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

export type SelectItem = { value: string; label: string; disabled?: boolean };

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
};

export function Select({
  items,
  value,
  onValueChange,
  placeholder,
  disabled,
  invalid,
  size = "md",
  className,
  icon,
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
                <BaseSelect.Item key={item.value} value={item.value} disabled={item.disabled} className="z-item">
                  <BaseSelect.ItemText>{item.label}</BaseSelect.ItemText>
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

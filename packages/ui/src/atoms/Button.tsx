import type { ComponentPropsWithRef, ReactNode } from "react";
import { cx } from "../cx";
import { Spinner } from "./Feedback";
import { Tooltip } from "./Tooltip";

export type ButtonVariant = "solid" | "outline" | "ghost" | "danger";
export type ControlSize = "sm" | "md" | "lg";

export type ButtonProps = ComponentPropsWithRef<"button"> & {
  variant?: ButtonVariant;
  size?: ControlSize;
  /** Menampilkan spinner dan menonaktifkan klik; tombol tetap terbaca sebagai sibuk. */
  loading?: boolean;
};

export function Button({
  variant = "outline",
  size = "md",
  loading = false,
  className,
  children,
  disabled,
  type = "button",
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cx("z-button", `z-button--${variant}`, `z-button--${size}`, className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading && <Spinner size="sm" />}
      {children}
    </button>
  );
}

export type IconButtonProps = Omit<ButtonProps, "children" | "aria-label"> & {
  /** Wajib: nama aksi. Dipakai sebagai aria-label dan tooltip. */
  label: string;
  icon: ReactNode;
};

export function IconButton({ label, icon, variant = "ghost", size = "md", className, ...rest }: IconButtonProps) {
  return (
    <Tooltip content={label}>
      <Button variant={variant} size={size} aria-label={label} className={cx("z-icon-button", className)} {...rest}>
        {icon}
      </Button>
    </Tooltip>
  );
}

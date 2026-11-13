import { useI18n } from "@zandor/i18n/react";
import type { HTMLAttributes } from "react";
import { cx } from "../cx";

export function Spinner({
  size = "md",
  className,
  ...rest
}: HTMLAttributes<HTMLSpanElement> & { size?: "sm" | "md" | "lg" }) {
  const { t } = useI18n();
  return (
    <span
      role="status"
      aria-label={t("ui.loading")}
      className={cx("z-spinner", `z-spinner--${size}`, className)}
      {...rest}
    />
  );
}

/** Placeholder bentuk konten saat memuat. Dekoratif: tidak dibaca pembaca layar. */
export function Skeleton({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div aria-hidden="true" className={cx("z-skeleton", className)} {...rest} />;
}

export type BadgeTone = "neutral" | "outline" | "danger" | "success" | "warning";

export function Badge({
  tone = "neutral",
  className,
  ...rest
}: HTMLAttributes<HTMLSpanElement> & { tone?: BadgeTone }) {
  return <span className={cx("z-badge", `z-badge--${tone}`, className)} {...rest} />;
}

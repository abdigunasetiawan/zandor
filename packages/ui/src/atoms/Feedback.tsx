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

export type WorkingVariant = "wave" | "ripple" | "orbit" | "grid";
const WORKING_PARTS: Record<WorkingVariant, number> = { wave: 5, ripple: 3, orbit: 3, grid: 9 };

/**
 * Indikator "sedang bekerja" (pengganti spinner biasa): `wave` = gelombang batang, `ripple` = titik berdenyut dengan riak,
 * `orbit` = tiga titik mengorbit, `grid` = sembilan kotak berkilau diagonal (bawaan indikator kerja agent). Murni CSS, mengikuti warna teks (`currentColor`), dan diam saat gerak dikurangi.
 */
export function Working({
  variant = "wave",
  size = "md",
  label,
  className,
  ...rest
}: Omit<HTMLAttributes<HTMLSpanElement>, "children"> & {
  variant?: WorkingVariant;
  size?: "dot" | "sm" | "md";
  label?: string;
}) {
  const { t } = useI18n();
  return (
    <span
      role="status"
      aria-label={label ?? t("ui.working")}
      className={cx("z-working", `z-working--${variant}`, `z-working--${size}`, className)}
      {...rest}
    >
      {Array.from({ length: WORKING_PARTS[variant] }, (_, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: bagian dekoratif statis
        <i key={i} style={{ "--i": i } as React.CSSProperties} />
      ))}
    </span>
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

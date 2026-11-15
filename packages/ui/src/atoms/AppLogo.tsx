import { cx } from "../cx";

export type AppLogoSize = "sm" | "md" | "lg";

export interface AppLogoProps {
  /** Ukuran lewat token `--z-icon-*`. Default `md`. */
  size?: AppLogoSize;
  /** Nama yang dibacakan pembaca layar. Tanpa label, logo dianggap dekoratif (`aria-hidden`). */
  label?: string;
  className?: string;
}

/**
 * Logo Zandor (varian "Tajam"): huruf Z dari tiga balok terpisah, merujuk panel Chat, Dokumen, dan Tools.
 * Monokrom lewat `currentColor`, jadi mengikuti warna teks wadahnya. Sumber yang sama dipakai untuk
 * favicon (`apps/desktop/public/favicon.svg`) dan ikon aplikasi (`src-tauri/icons/app-icon.svg`).
 */
export function AppLogo({ size = "md", label, className }: AppLogoProps) {
  return (
    <svg
      className={cx("z-app-logo", `z-app-logo--${size}`, className)}
      viewBox="0 0 64 64"
      fill="currentColor"
      xmlns="http://www.w3.org/2000/svg"
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      {label && <title>{label}</title>}
      <rect x="10" y="10" width="44" height="10" />
      <polygon points="40,23 54,23 24,41 10,41" />
      <rect x="10" y="44" width="44" height="10" />
    </svg>
  );
}

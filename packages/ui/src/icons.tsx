// Ikon UI = Lucide (ISC, ADR-031). Nama ekspor tetap sama dengan versi gambar-sendiri sebelumnya, jadi
// komponen pemakai tidak berubah. Ukuran mengikuti token (--z-icon-*); dekoratif → aria-hidden.
// Ikon TIPE FILE (.ts, .md, folder, ...) bukan dari sini: memakai Material Icon Theme (lihat file-icons).
import {
  ArrowLeft,
  ArrowRight,
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  FileText,
  GitBranch,
  Globe,
  Info,
  List,
  type LucideIcon,
  Minus,
  PanelLeft,
  PanelRight,
  Plus,
  Search,
  Settings,
  Square,
  Terminal,
  X,
} from "lucide-react";
import type { SVGProps } from "react";
import { cx } from "./cx";

export type IconProps = Omit<SVGProps<SVGSVGElement>, "children" | "ref"> & { size?: "sm" | "md" | "lg" };

/** Ketebalan garis seragam untuk seluruh UI. */
const STROKE = 1.5;

function icon(Glyph: LucideIcon, name: string) {
  const Component = ({ size = "md", className, ...rest }: IconProps) => (
    <Glyph
      strokeWidth={STROKE}
      aria-hidden="true"
      focusable="false"
      className={cx("z-icon", `z-icon--${size}`, className)}
      {...(rest as Record<string, unknown>)}
    />
  );
  Component.displayName = name;
  return Component;
}

export const CheckIcon = icon(Check, "CheckIcon");
export const ChevronDownIcon = icon(ChevronDown, "ChevronDownIcon");
export const ChevronRightIcon = icon(ChevronRight, "ChevronRightIcon");
export const CloseIcon = icon(X, "CloseIcon");
export const SearchIcon = icon(Search, "SearchIcon");
export const PlusIcon = icon(Plus, "PlusIcon");
export const ArrowLeftIcon = icon(ArrowLeft, "ArrowLeftIcon");
export const ArrowRightIcon = icon(ArrowRight, "ArrowRightIcon");
export const MinimizeIcon = icon(Minus, "MinimizeIcon");
export const MaximizeIcon = icon(Square, "MaximizeIcon");
export const RestoreIcon = icon(Copy, "RestoreIcon");
export const PanelLeftIcon = icon(PanelLeft, "PanelLeftIcon");
export const PanelRightIcon = icon(PanelRight, "PanelRightIcon");
export const PanelDocIcon = icon(FileText, "PanelDocIcon");
export const TerminalIcon = icon(Terminal, "TerminalIcon");
export const GitIcon = icon(GitBranch, "GitIcon");
export const GlobeIcon = icon(Globe, "GlobeIcon");
export const InfoIcon = icon(Info, "InfoIcon");
export const ListIcon = icon(List, "ListIcon");
export const SettingsIcon = icon(Settings, "SettingsIcon");

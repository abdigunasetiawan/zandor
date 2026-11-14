// Ikon UI = Lucide (ISC, ADR-031). Nama ekspor tetap sama dengan versi gambar-sendiri sebelumnya, jadi
// komponen pemakai tidak berubah. Ukuran mengikuti token (--z-icon-*); dekoratif → aria-hidden.
// Ikon TIPE FILE (.ts, .md, folder, ...) bukan dari sini: memakai Material Icon Theme (lihat file-icons).
import {
  Activity,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Bookmark,
  Brain,
  Check,
  ChevronDown,
  ChevronRight,
  ChevronsDownUp,
  ChevronsUpDown,
  Clock,
  Code,
  Copy,
  Download,
  Ellipsis,
  ExternalLink,
  Eye,
  File,
  FileDiff,
  FilePlus,
  Files,
  FileText,
  Folder,
  FolderOpen,
  FolderPlus,
  GitBranch,
  Globe,
  GripVertical,
  Image,
  ImageDown,
  Info,
  Link2,
  List,
  type LucideIcon,
  MessageSquare,
  Mic,
  Minus,
  PanelLeft,
  PanelRight,
  Paperclip,
  Pencil,
  Pin,
  Plus,
  RotateCw,
  Save,
  Search,
  Settings,
  Shield,
  Square,
  Terminal,
  Trash2,
  Undo2,
  WrapText,
  X,
  Zap,
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
export const BrainIcon = icon(Brain, "BrainIcon");
export const ClockIcon = icon(Clock, "ClockIcon");
export const EditIcon = icon(Pencil, "EditIcon");
export const SaveIcon = icon(Save, "SaveIcon");
export const FolderIcon = icon(Folder, "FolderIcon");
export const FolderOpenIcon = icon(FolderOpen, "FolderOpenIcon");
export const FileIcon = icon(File, "FileIcon");
export const DiffIcon = icon(FileDiff, "DiffIcon");
export const FilesIcon = icon(Files, "FilesIcon");
export const ActivityIcon = icon(Activity, "ActivityIcon");
export const GripIcon = icon(GripVertical, "GripIcon");
export const FilePlusIcon = icon(FilePlus, "FilePlusIcon");
export const FolderPlusIcon = icon(FolderPlus, "FolderPlusIcon");
export const CollapseAllIcon = icon(ChevronsDownUp, "CollapseAllIcon");
export const ExpandAllIcon = icon(ChevronsUpDown, "ExpandAllIcon");
export const MoreIcon = icon(Ellipsis, "MoreIcon");
export const RefreshIcon = icon(RotateCw, "RefreshIcon");
export const ChatIcon = icon(MessageSquare, "ChatIcon");
export const CopyIcon = icon(Copy, "CopyIcon");
export const UndoIcon = icon(Undo2, "UndoIcon");
export const PaperclipIcon = icon(Paperclip, "PaperclipIcon");
export const PinIcon = icon(Pin, "PinIcon");
export const TrashIcon = icon(Trash2, "TrashIcon");
export const PingIcon = icon(Zap, "PingIcon");
export const CodeIcon = icon(Code, "CodeIcon");
export const DownloadIcon = icon(Download, "DownloadIcon");
export const ExternalLinkIcon = icon(ExternalLink, "ExternalLinkIcon");
export const EyeIcon = icon(Eye, "EyeIcon");
export const WrapIcon = icon(WrapText, "WrapIcon");
export const ArrowUpIcon = icon(ArrowUp, "ArrowUpIcon");
export const ShieldIcon = icon(Shield, "ShieldIcon");
export const StopIcon = icon(Square, "StopIcon");
export const MicIcon = icon(Mic, "MicIcon");
export const LinkIcon = icon(Link2, "LinkIcon");
export const BookmarkIcon = icon(Bookmark, "BookmarkIcon");
export const ImageDownIcon = icon(ImageDown, "ImageDownIcon");
export const FileTextIcon = icon(FileText, "FileTextIcon");
export const ImageIcon = icon(Image, "ImageIcon");

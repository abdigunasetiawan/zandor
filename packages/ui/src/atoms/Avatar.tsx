import { Avatar as BaseAvatar } from "@base-ui/react/avatar";
import { cx } from "../cx";
import type { ControlSize } from "./Button";

export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  const first = words[0] ?? "";
  const last = words.length > 1 ? (words[words.length - 1] ?? "") : "";
  return (Array.from(first)[0] ?? "") + (Array.from(last)[0] ?? "");
}

export type AvatarProps = { name: string; src?: string; size?: ControlSize; className?: string };

export function Avatar({ name, src, size = "md", className }: AvatarProps) {
  return (
    <BaseAvatar.Root className={cx("z-avatar", `z-avatar--${size}`, className)} role="img" aria-label={name}>
      {src && <BaseAvatar.Image src={src} alt="" className="z-avatar__image" />}
      <BaseAvatar.Fallback className="z-avatar__fallback">{initials(name).toUpperCase()}</BaseAvatar.Fallback>
    </BaseAvatar.Root>
  );
}

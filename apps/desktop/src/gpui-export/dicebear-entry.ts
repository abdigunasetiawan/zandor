// Entry bundel DiceBear untuk apps/desktop-gpui (dijalankan lokal di QuickJS). Sama dengan profile.ts: seed → SVG.
import * as adventurer from "@dicebear/adventurer";
import * as bigSmile from "@dicebear/big-smile";
import { createAvatar } from "@dicebear/core";
import * as funEmoji from "@dicebear/fun-emoji";
import * as lorelei from "@dicebear/lorelei";
import * as notionists from "@dicebear/notionists";
import * as openPeeps from "@dicebear/open-peeps";
import * as pixelArt from "@dicebear/pixel-art";
import * as thumbs from "@dicebear/thumbs";

// biome-ignore lint/suspicious/noExplicitAny: tiap gaya punya tipe opsi sendiri
const STYLES: Record<string, any> = { lorelei, notionists, openPeeps, adventurer, bigSmile, funEmoji, pixelArt, thumbs };

(globalThis as unknown as Record<string, unknown>).zandorAvatar = (style: string, seed: string): string => {
  const s = STYLES[style];
  return s ? createAvatar(s, { seed, size: 128 }).toString() : "";
};
(globalThis as unknown as Record<string, unknown>).zandorAvatarCredit = (style: string): string => {
  const m = STYLES[style]?.meta ?? {};
  return JSON.stringify({ title: m.title ?? style, creator: m.creator ?? "", license: m.license?.name ?? "", source: m.source ?? m.homepage ?? "" });
};

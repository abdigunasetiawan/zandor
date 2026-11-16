// Profil pengguna: nama tampilan + avatar (gambar unggahan atau ilustrasi DiceBear yang dirender LOKAL — tanpa permintaan
// jaringan, jadi nama/seed tidak pernah keluar dari perangkat). Hasil ilustrasi berupa SVG data URI yang ditampilkan lewat
// <img>, sehingga skrip di dalam SVG tidak pernah dijalankan.
import * as adventurer from "@dicebear/adventurer";
import * as bigSmile from "@dicebear/big-smile";
import { createAvatar, type Style } from "@dicebear/core";
import * as funEmoji from "@dicebear/fun-emoji";
import * as lorelei from "@dicebear/lorelei";
import * as notionists from "@dicebear/notionists";
import * as openPeeps from "@dicebear/open-peeps";
import * as pixelArt from "@dicebear/pixel-art";
import * as thumbs from "@dicebear/thumbs";
import type { AvatarStyle, ProfileAvatar } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { useMemo } from "react";
import { useAppearance } from "./services";
import { useOsUser } from "./use-os-user";

// biome-ignore lint/suspicious/noExplicitAny: tiap gaya DiceBear punya tipe opsi sendiri; di sini hanya seed yang dipakai.
const STYLES: Record<AvatarStyle, Style<any>> = {
  lorelei,
  notionists,
  openPeeps,
  adventurer,
  bigSmile,
  funEmoji,
  pixelArt,
  thumbs,
};

const cache = new Map<string, string>();

/** SVG data URI untuk ilustrasi (di-cache per gaya+seed). */
export function illustrationUrl(style: AvatarStyle, seed: string): string {
  const key = `${style}:${seed}`;
  let url = cache.get(key);
  if (!url) {
    url = createAvatar(STYLES[style], { seed, size: 128 }).toDataUri();
    cache.set(key, url);
  }
  return url;
}

/** Atribusi gaya dari metadata paket (judul, pembuat, lisensi, tautan sumber). */
export function styleCredit(style: AvatarStyle): { title: string; creator: string; license: string; source?: string } {
  const m = STYLES[style].meta ?? {};
  return {
    title: m.title ?? style,
    creator: m.creator ?? "",
    license: m.license?.name ?? "",
    source: m.source ?? m.homepage,
  };
}

export function avatarSrc(a: ProfileAvatar | undefined): string | undefined {
  if (!a) return undefined;
  return a.kind === "image" ? a.src : illustrationUrl(a.style, a.seed);
}

export function randomSeed(): string {
  return crypto.randomUUID().replace(/-/g, "").slice(0, 10);
}

/** Nama dan avatar efektif: nama kustom → nama akun OS → teks bawaan; avatar kosong → inisial. */
export function useProfile(): { name: string; src: string | undefined; osName: string | null } {
  const { t } = useI18n();
  const osName = useOsUser();
  const { profile } = useAppearance();
  const src = useMemo(() => avatarSrc(profile.avatar), [profile.avatar]);
  return { name: profile.name ?? osName ?? t("sb.user.fallback"), src, osName };
}

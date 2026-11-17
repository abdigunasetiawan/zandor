// Titik status sesi (menggantikan bullet di sidebar dan dipakai di tab): idle = titik redup, bekerja = indikator grid berkilau,
// butuh aksi = cincin berdenyut, selesai (belum dilihat) = titik terisi, gagal/batas = titik terisi berwarna bahaya.
import type { SessionIndicator } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Working } from "@zandor/ui";
import { useSessionIndicator } from "./services";

export function SessionDot({ state }: { state: SessionIndicator }) {
  const { t } = useI18n();
  const label = t(`sdot.${state}` as never);
  if (state === "busy") return <Working variant="grid" size="dot" label={label} className="sdot sdot--busy" />;
  return <span className="sdot" data-state={state} role="img" aria-label={label} title={label} />;
}

/** Titik untuk satu sesi (membaca state chat + penanda perhatian). */
export function SessionDotFor({ sessionId }: { sessionId: string }) {
  return <SessionDot state={useSessionIndicator(sessionId)} />;
}

/** Titik di tab sesi: hanya tampil bila ada sesuatu yang perlu diperhatikan (bukan saat idle). */
export function TabStatus({ sessionId }: { sessionId: string }) {
  const state = useSessionIndicator(sessionId);
  return state === "idle" ? null : <SessionDot state={state} />;
}

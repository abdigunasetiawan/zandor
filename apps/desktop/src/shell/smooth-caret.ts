import { type RefObject, useCallback, useEffect, useRef, useState } from "react";

/** Gaya yang harus disalin ke div cermin agar tata letak teksnya identik dengan textarea. */
const MIRROR_PROPS = [
  "boxSizing",
  "width",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "borderTopWidth",
  "borderRightWidth",
  "borderBottomWidth",
  "borderLeftWidth",
  "fontFamily",
  "fontSize",
  "fontWeight",
  "fontStyle",
  "letterSpacing",
  "lineHeight",
  "textTransform",
  "wordSpacing",
  "tabSize",
] as const;

export type CaretPos = { x: number; y: number; h: number; visible: boolean; typing: boolean };

/** Koordinat kursor (relatif ke textarea) lewat div cermin tersembunyi. */
export function caretCoords(el: HTMLTextAreaElement, index: number) {
  const mirror = document.createElement("div");
  const cs = getComputedStyle(el);
  for (const p of MIRROR_PROPS) mirror.style[p] = cs[p];
  mirror.style.position = "absolute";
  mirror.style.whiteSpace = "pre-wrap";
  mirror.style.overflowWrap = "break-word";
  mirror.style.top = "0";
  mirror.style.left = "0";
  mirror.textContent = el.value.slice(0, index);
  const marker = document.createElement("span");
  marker.textContent = "​"; // cukup satu karakter tak terlihat: cermin tidak setinggi seluruh teks
  mirror.appendChild(marker);
  // Wadah berukuran 0 + overflow hidden: cermin (bisa sangat tinggi untuk teks panjang) tidak pernah
  // memperluas area gulir leluhur — jika tidak, panel chat ikut tergulir dan composer "mengambang".
  // Diletakkan di samping textarea, bukan di <body>: penjaga overlay (ADR-025) mengamati anak <body>.
  const clip = document.createElement("div");
  clip.setAttribute("aria-hidden", "true");
  Object.assign(clip.style, {
    position: "absolute",
    top: "0",
    left: "0",
    width: "0",
    height: "0",
    overflow: "hidden",
    visibility: "hidden",
    pointerEvents: "none",
  });
  clip.appendChild(mirror);
  (el.parentElement ?? document.body).appendChild(clip);
  const lh = Number.parseFloat(cs.lineHeight) || Number.parseFloat(cs.fontSize) * 1.5;
  const pos = { x: marker.offsetLeft, y: marker.offsetTop - el.scrollTop, h: lh };
  clip.remove();
  return pos;
}

/**
 * Kursor halus ala Monkeytype: kursor asli disembunyikan, gantinya batang yang meluncur
 * ke posisi baru. Saat diam berkedip; saat mengetik tetap menyala.
 */
export function useSmoothCaret(ref: RefObject<HTMLTextAreaElement | null>) {
  const [pos, setPos] = useState<CaretPos>({ x: 0, y: 0, h: 0, visible: false, typing: false });
  const idle = useRef<ReturnType<typeof setTimeout>>(undefined);

  const update = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    const focused = document.activeElement === el;
    const collapsed = el.selectionStart === el.selectionEnd;
    if (!focused || !collapsed) {
      setPos((p) => ({ ...p, visible: false }));
      return;
    }
    const c = caretCoords(el, el.selectionStart);
    // baris kursor tergulir keluar dari area textarea yang terlihat → sembunyikan (jangan digambar di luar kartu)
    if (el.clientHeight > 0 && (c.y < 0 || c.y + c.h > el.clientHeight + 1)) {
      setPos((p) => ({ ...p, ...c, visible: false }));
      return;
    }
    setPos({ ...c, visible: true, typing: true });
    clearTimeout(idle.current);
    idle.current = setTimeout(() => setPos((p) => ({ ...p, typing: false })), 500);
  }, [ref]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    // Ditunda ke frame berikutnya: setState sinkron di listener native `input` membuat React
    // merender ulang SEBELUM onChange menyimpan teks, sehingga textarea terkontrol kembali kosong.
    let frame = 0;
    const schedule = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    };
    const evs = ["input", "focus", "blur", "keyup", "mouseup", "scroll"] as const;
    for (const e of evs) el.addEventListener(e, schedule);
    const onSelDeferred = () => {
      if (document.activeElement === el) schedule();
    };
    document.addEventListener("selectionchange", onSelDeferred);
    const ro = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(schedule);
    ro?.observe(el);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("selectionchange", onSelDeferred);
      for (const e of evs) el.removeEventListener(e, schedule);
      ro?.disconnect();
      clearTimeout(idle.current);
    };
  }, [ref, update]);

  return pos;
}

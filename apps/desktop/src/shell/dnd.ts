// Seret-lepas (HTML5) untuk mengurutkan daftar: tahan lalu seret sebuah item ke atas/bawah item lain. Mengembalikan props yang
// dipasang di tiap item; penanda tujuan jatuh diberikan lewat atribut `data-drop` ("before"/"after") dan `data-dragging`.
import { type DragEvent, useState } from "react";

export type DragReorder = {
  dragId: string | null;
  itemProps: (id: string) => {
    draggable: true;
    "data-dragging"?: "";
    "data-drop"?: "before" | "after";
    onDragStart: (e: DragEvent<HTMLElement>) => void;
    onDragOver: (e: DragEvent<HTMLElement>) => void;
    onDrop: (e: DragEvent<HTMLElement>) => void;
    onDragEnd: () => void;
  };
};

/** `onMove(dragId, targetId, before)` dipanggil saat item dilepas di atas/bawah item target. */
export function useDragReorder(onMove: (dragId: string, targetId: string, before: boolean) => void): DragReorder {
  const [dragId, setDragId] = useState<string | null>(null);
  const [over, setOver] = useState<{ id: string; before: boolean } | null>(null);
  const reset = () => {
    setDragId(null);
    setOver(null);
  };
  return {
    dragId,
    itemProps: (id) => ({
      draggable: true,
      "data-dragging": dragId === id ? "" : undefined,
      "data-drop": over?.id === id && dragId !== id ? (over.before ? "before" : "after") : undefined,
      onDragStart: (e) => {
        e.stopPropagation();
        setDragId(id);
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", id);
      },
      onDragOver: (e) => {
        if (!dragId || dragId === id) return;
        e.preventDefault();
        e.stopPropagation();
        const r = e.currentTarget.getBoundingClientRect();
        const before = e.clientY < r.top + r.height / 2;
        if (over?.id !== id || over.before !== before) setOver({ id, before });
      },
      onDrop: (e) => {
        e.preventDefault();
        e.stopPropagation();
        if (dragId && dragId !== id && over) onMove(dragId, id, over.before);
        reset();
      },
      onDragEnd: reset,
    }),
  };
}

/** Indeks tujuan akhir (setelah item dicabut) untuk memindahkan `dragId` ke atas/bawah `targetId` dalam `ids`. */
export function targetIndex(ids: string[], dragId: string, targetId: string, before: boolean): number {
  const from = ids.indexOf(dragId);
  let to = ids.indexOf(targetId);
  if (from < 0 || to < 0) return from;
  if (!before) to += 1;
  if (from < to) to -= 1;
  return to;
}

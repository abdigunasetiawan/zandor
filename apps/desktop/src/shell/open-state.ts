// State buka/tutup lipatan proses (giliran, tool, berpikir) yang TAHAN remount. Daftar pesan divirtualisasi dan giliran
// asisten bisa dipasang ulang saat pesan engine baru bergabung; state lokal `useState` ikut hilang sehingga lipatan yang
// sudah dibuka pengguna menutup sendiri. Di sini state disimpan per kunci stabil (id giliran/part) di luar komponen.
// Hanya di memori (tidak persisten antar-sesi aplikasi): bawaan tetap tertutup.
import { useCallback, useState } from "react";

const store = new Map<string, boolean>();

export const isOpen = (key: string): boolean => store.get(key) ?? false;
export const setOpenState = (key: string, open: boolean): void => {
  if (open) store.set(key, true);
  else store.delete(key);
};
/** Hanya untuk tes. */
export const resetOpenState = (): void => store.clear();

/** `[terbuka, ubah]` untuk kunci tertentu; pembacaan awal dari penyimpanan sehingga pemasangan ulang mempertahankan nilai. */
export function useOpenState(key: string): [boolean, (next: boolean | ((prev: boolean) => boolean)) => void] {
  const [open, setOpen] = useState(() => isOpen(key));
  // kunci bisa berganti pada komponen yang sama (mis. daftar yang di-reuse): sinkronkan dari penyimpanan
  const [seen, setSeen] = useState(key);
  if (seen !== key) {
    setSeen(key);
    setOpen(isOpen(key));
  }
  const set = useCallback(
    (next: boolean | ((prev: boolean) => boolean)) => {
      setOpen((prev) => {
        const value = typeof next === "function" ? next(prev) : next;
        setOpenState(key, value);
        return value;
      });
    },
    [key],
  );
  return [open, set];
}

// Baris paling atas panel 2/3/4: wadah bersama untuk tab (atau judul alat) dengan tombol tutup panel di ujung kanan.
// Tinggi, latar, dan pemisah seragam di semua panel; tombol tutup berada di luar area tab yang bisa digulir sehingga
// selalu menempel di ujung. Tab/judul diberikan sebagai children (mis. <nav className="stabs"> atau judul alat).
import { CloseIcon, IconButton } from "@zandor/ui";
import type { ReactNode } from "react";

export function PanelTabBar({
  children,
  closeLabel,
  onClose,
}: {
  children?: ReactNode;
  closeLabel: string;
  /** Tanpa ini, tombol tutup tidak ditampilkan (mis. panel sedang terlipat). */
  onClose?: () => void;
}) {
  return (
    <div className="ptb">
      <div className="ptb__main">{children}</div>
      {onClose && (
        <IconButton label={closeLabel} size="sm" className="ptb__close" icon={<CloseIcon />} onClick={onClose} />
      )}
    </div>
  );
}

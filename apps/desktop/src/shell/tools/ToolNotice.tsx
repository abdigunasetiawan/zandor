// Pesan keadaan (kosong, tidak tersedia, belum ada sesi) yang seragam di semua panel alat: ikon + teks di tengah panel, atau
// satu baris ringkas (`inline`) di atas daftar. Menggantikan paragraf polos agar tampilannya konsisten.
import type { ReactNode } from "react";

export function ToolNotice({ icon, inline, children }: { icon?: ReactNode; inline?: boolean; children: ReactNode }) {
  if (inline) {
    return (
      <p className="tnote tnote--inline z-small" role="status">
        {children}
      </p>
    );
  }
  return (
    <div className="tnote" role="status">
      {icon && (
        <span className="tnote__icon" aria-hidden="true">
          {icon}
        </span>
      )}
      <p className="z-small tnote__body">{children}</p>
    </div>
  );
}

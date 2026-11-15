// Bingkai halaman bersama untuk Memori & skill, Tampilan lanjutan, dan Automasi. Berdiri sendiri (rute lama) ia berupa
// <main> dengan tombol kembali; tertanam di Pengaturan ia berupa bagian kategori dengan judul h2 (ADR-059).
import { useI18n } from "@zandor/i18n/react";
import { ArrowLeftIcon, IconButton } from "@zandor/ui";
import type { ReactNode } from "react";
import { useServices } from "./services";

export function PageFrame({
  title,
  embedded = false,
  actions,
  children,
}: {
  title: string;
  embedded?: boolean;
  /** Tombol aksi di kanan judul (mis. Reset, Tugas baru). */
  actions?: ReactNode;
  children: ReactNode;
}) {
  const { t } = useI18n();
  const { nav } = useServices();
  if (embedded)
    return (
      <section className="mempage mempage--embedded" aria-label={title}>
        <header className="mempage__head">
          <h2 className="z-h4 mempage__title">{title}</h2>
          {actions}
        </header>
        {children}
      </section>
    );
  return (
    <main className="mempage">
      <header className="mempage__head">
        <IconButton label={t("mem.back")} icon={<ArrowLeftIcon />} onClick={() => nav.push("/")} />
        <h1 className="z-h3 mempage__title">{title}</h1>
        {actions}
      </header>
      {children}
    </main>
  );
}

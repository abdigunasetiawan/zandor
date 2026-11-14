// Kartu berkas di percakapan: [ikon jenis berkas] nama … [Buka di Panel Dokumen] [Unduh]. Dipakai untuk dokumen yang
// dibuat agent (tool `write`) dan part berkas pada sesi di dalam project; gambar data: URI tampil sebagai pratinjau.
// Gambar hanya dimuat dari data: URI (bukan dari jaringan) agar isi dari model tidak bisa mengeksfiltrasi data lewat URL.
import { useI18n } from "@zandor/i18n/react";
import { Button, DownloadIcon, ExternalLinkIcon, IconButton, useToast } from "@zandor/ui";
import { useDemo } from "./demo-context";
import { FileTypeIcon } from "./FileTypeIcon";
import { SAFE_IMAGE_URI } from "./image-uri";
import { useDocActions } from "./services";

/** Path lokal dari URL file:// (Windows: file:///C:/x → C:/x); null bila bukan file://. */
export function fileUrlToPath(url: string): string | null {
  if (!url.startsWith("file://")) return null;
  try {
    const p = decodeURIComponent(new URL(url).pathname);
    return /^\/[A-Za-z]:/.test(p) ? p.slice(1) : p;
  } catch {
    return null;
  }
}

export function FileCard({
  filename,
  mime,
  url,
  path,
}: {
  filename?: string;
  mime?: string;
  url?: string;
  /** Path dokumen (relatif terhadap project atau absolut); dipakai bila bukan part berkas. */
  path?: string;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const demo = useDemo();
  const actions = useDocActions();
  const target = path ?? (url ? fileUrlToPath(url) : null);
  const name = filename ?? (target ?? url ?? "").split(/[\\/]/).pop() ?? t("file.unnamed");

  if (mime?.startsWith("image/") && url && SAFE_IMAGE_URI.test(url)) {
    return (
      <figure className="fcard fcard--image" data-testid="file-card">
        <img className="fcard__img" src={url} alt={name} draggable={false} />
        <figcaption className="fcard__caption z-small">
          {name} · {mime}
        </figcaption>
      </figure>
    );
  }

  const notReal = () => toast.show({ title: t("file.demoNote"), tone: "warning" });
  const open = () => {
    if (demo || !target) return notReal();
    if (!actions.open(target)) toast.show({ title: t("doc.outside"), tone: "warning" });
  };
  const download = async () => {
    if (demo || !target) return notReal();
    try {
      const r = await actions.download(target);
      if (r === "saved") toast.show({ title: t("doc.saved"), tone: "success", timeoutMs: 2000 });
      if (r === "outside") toast.show({ title: t("doc.outside"), tone: "warning" });
    } catch (e) {
      toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
    }
  };

  return (
    <section className="fcard fcard--doc" aria-label={name} data-testid="file-card" data-kind={mime ?? "doc"}>
      <FileTypeIcon name={name} type="file" />
      <span className="fcard__name">{name}</span>
      <Button size="sm" onClick={open} aria-label={t("doc.open", { name })}>
        <ExternalLinkIcon size="sm" /> {t("doc.openShort")}
      </Button>
      <IconButton
        label={t("file.download", { name })}
        size="sm"
        icon={<DownloadIcon />}
        onClick={() => void download()}
      />
    </section>
  );
}

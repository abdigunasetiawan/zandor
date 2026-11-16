// Aksi bersama untuk blok render kaya (kode, tabel, rumus, diagram, gambar): salin teks, salin gambar (PNG ke clipboard),
// simpan berkas lewat DocHost (dialog Simpan sebagai di desktop, unduhan di web). Bilah aksi `RichBar` mengambang di pojok
// kanan atas blok dan muncul saat hover/fokus; elemennya `data-no-export` agar tidak ikut tergambar saat ekspor.
import { useI18n } from "@zandor/i18n/react";
import { IconButton, Menu, type MenuEntry, MoreIcon, useToast } from "@zandor/ui";
import type { ReactNode } from "react";
import { renderNode, textBytes } from "../message-export";
import { useOptionalServices } from "../services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function download(name: string, bytes: Uint8Array) {
  const url = URL.createObjectURL(new Blob([bytes as BlobPart]));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export function useRichActions() {
  const { t } = useI18n();
  const toast = useToast();
  const services = useOptionalServices();
  const fail = (e: unknown) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
  const ok = (title: string) => toast.show({ title, tone: "success", timeoutMs: 1500 });

  const copyText = async (text: string, done = t("chat.copied")) => {
    try {
      await navigator.clipboard.writeText(text);
      ok(done);
    } catch (e) {
      fail(e);
    }
  };
  /** Salin HTML berformat + teks polos sekaligus (tempel ke Word/Docs tetap berupa tabel). */
  const copyRich = async (html: string, plain: string) => {
    try {
      if (typeof ClipboardItem === "undefined" || !navigator.clipboard.write) {
        await navigator.clipboard.writeText(plain);
      } else {
        await navigator.clipboard.write([
          new ClipboardItem({
            "text/html": new Blob([html], { type: "text/html" }),
            "text/plain": new Blob([plain], { type: "text/plain" }),
          }),
        ]);
      }
      ok(t("chat.copied"));
    } catch (e) {
      fail(e);
    }
  };
  const copyImage = async (node: HTMLElement | null) => {
    try {
      if (!node) throw new Error(t("msg.noContent"));
      if (typeof ClipboardItem === "undefined" || !navigator.clipboard.write)
        throw new Error(t("rich.noImageClipboard"));
      const bytes = await renderNode(node, "png");
      await navigator.clipboard.write([
        new ClipboardItem({ "image/png": new Blob([bytes as BlobPart], { type: "image/png" }) }),
      ]);
      ok(t("rich.imageCopied"));
    } catch (e) {
      fail(e);
    }
  };
  const saveBytes = async (name: string, bytes: Uint8Array) => {
    try {
      const saved = services ? await services.docHost.saveBytes(name, bytes) : (download(name, bytes), true);
      if (saved) ok(t("msg.saved"));
    } catch (e) {
      fail(e);
    }
  };
  const saveText = (name: string, text: string) => saveBytes(name, textBytes(text));
  const saveNode = async (node: HTMLElement | null, kind: "png" | "svg", name: string) => {
    try {
      if (!node) throw new Error(t("msg.noContent"));
      await saveBytes(name, await renderNode(node, kind));
    } catch (e) {
      fail(e);
    }
  };
  return { copyText, copyRich, copyImage, saveBytes, saveText, saveNode };
}

export type RichAction = { id: string; label: string; icon: ReactNode; onSelect: () => void; pressed?: boolean };

/** Bilah aksi blok: aksi utama sebagai ikon, sisanya di menu ⋯ (berikon). */
export function RichBar({
  primary,
  more = [],
  className,
}: {
  primary: RichAction[];
  more?: RichAction[];
  className?: string;
}) {
  const { t } = useI18n();
  const entries: MenuEntry[] = more.map((a) => ({
    type: "item",
    id: a.id,
    label: a.label,
    icon: a.icon,
    onSelect: a.onSelect,
  }));
  return (
    <div className={className ?? "rbar"} data-no-export="">
      {primary.map((a) => (
        <IconButton key={a.id} label={a.label} size="sm" icon={a.icon} aria-pressed={a.pressed} onClick={a.onSelect} />
      ))}
      {entries.length > 0 && (
        <Menu trigger={<IconButton label={t("rich.more")} size="sm" icon={<MoreIcon />} />} entries={entries} />
      )}
    </div>
  );
}

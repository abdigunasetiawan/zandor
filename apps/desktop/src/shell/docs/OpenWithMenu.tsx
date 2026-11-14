// "Buka di aplikasi lain": popover berisi aplikasi yang terdaftar untuk ekstensi berkas (dengan ikon aslinya), ditambah aplikasi
// bawaan sistem dan "Tampilkan di File Explorer". Daftar dibaca dari sistem (Rust) saat popover dibuka; hanya aplikasi desktop.
import { useI18n } from "@zandor/i18n/react";
import { Badge, ExternalLinkIcon, FolderIcon, IconButton, Popover, Spinner, useToast } from "@zandor/ui";
import { useState } from "react";
import type { DocHost, OpenApp } from "../doc-host";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

type Load = { status: "idle" | "loading" | "ready" | "error"; apps: OpenApp[]; error: string | null };

export function OpenWithMenu({
  host,
  root,
  path,
  textLike,
}: {
  host: DocHost;
  root: string;
  path: string;
  /** Berkas teks/kode: sertakan editor teks (VS Code, Notepad) bila terpasang. */
  textLike: boolean;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const [load, setLoad] = useState<Load>({ status: "idle", apps: [], error: null });
  const available = host.native && !!host.openWith && !!host.openWithList;

  const fetchApps = (refresh: boolean) => {
    if (!host.openWithList) return;
    setLoad((l) => ({ ...l, status: "loading", error: null }));
    host.openWithList(root, path, textLike, refresh).then(
      (apps) => setLoad({ status: "ready", apps, error: null }),
      (e) => setLoad({ status: "error", apps: [], error: errText(e) }),
    );
  };

  const run = async (fn: () => Promise<void>) => {
    setOpen(false);
    try {
      await fn();
    } catch (e) {
      toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
    }
  };

  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (o) fetchApps(false);
      }}
      title={t("doc.openWith.title")}
      side="bottom"
      align="end"
      className="owmenu"
      trigger={
        <IconButton
          label={available ? t("doc.openExternal") : t("doc.openExternalUnavailable")}
          size="sm"
          icon={<ExternalLinkIcon />}
          disabled={!available}
        />
      }
    >
      <ul className="owmenu__list" aria-label={t("doc.openWith.title")}>
        <li>
          <button type="button" className="owmenu__item" onClick={() => void run(() => host.openExternal(root, path))}>
            <span className="owmenu__icon owmenu__icon--generic" aria-hidden="true">
              <ExternalLinkIcon size="sm" />
            </span>
            <span className="owmenu__name">{t("doc.openWith.system")}</span>
          </button>
        </li>
        {load.status === "loading" && (
          <li className="owmenu__note z-small" role="status">
            <Spinner size="sm" /> {t("doc.openWith.loading")}
          </li>
        )}
        {load.status === "error" && (
          <li className="owmenu__note owmenu__note--error z-small" role="alert">
            {t("doc.openWith.error", { error: load.error ?? "" })}{" "}
            <button type="button" className="owmenu__retry" onClick={() => fetchApps(true)}>
              {t("doc.reload")}
            </button>
          </li>
        )}
        {load.status === "ready" && load.apps.length === 0 && (
          <li className="owmenu__note z-small">{t("doc.openWith.none")}</li>
        )}
        {load.apps.map((a) => (
          <li key={a.path}>
            <button
              type="button"
              className="owmenu__item"
              title={a.path}
              onClick={() => void run(() => host.openWith?.(root, path, a.path, textLike) ?? Promise.resolve())}
            >
              {a.icon ? (
                <img className="owmenu__icon" alt="" src={`data:image/png;base64,${a.icon}`} />
              ) : (
                <span className="owmenu__icon owmenu__icon--generic" aria-hidden="true">
                  <ExternalLinkIcon size="sm" />
                </span>
              )}
              <span className="owmenu__name">{a.name}</span>
              {a.isDefault && <Badge>{t("doc.openWith.default")}</Badge>}
            </button>
          </li>
        ))}
        {host.reveal && (
          <li className="owmenu__sep">
            <button
              type="button"
              className="owmenu__item"
              onClick={() => void run(() => host.reveal?.(root, path) ?? Promise.resolve())}
            >
              <span className="owmenu__icon owmenu__icon--generic" aria-hidden="true">
                <FolderIcon size="sm" />
              </span>
              <span className="owmenu__name">{t("doc.openWith.reveal")}</span>
            </button>
          </li>
        )}
      </ul>
    </Popover>
  );
}

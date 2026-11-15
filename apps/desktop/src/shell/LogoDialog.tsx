// Editor logo provider/model: pratinjau deteksi otomatis, unggah gambar sendiri, atau kembali ke otomatis.
import { useI18n } from "@zandor/i18n/react";
import { Button, Dialog, ModelLogo, TrashIcon, UploadIcon, useToast } from "@zandor/ui";
import { useRef, useState } from "react";
import { fileToLogo, LOGO_ACCEPT } from "./logo-file";

export function LogoDialog({
  title,
  providerID,
  modelID,
  name,
  custom,
  onSave,
  onClose,
}: {
  title: string;
  providerID: string;
  modelID?: string;
  name: string;
  /** Logo kustom saat ini (data URL) atau null bila memakai deteksi otomatis. */
  custom: string | null;
  /** `null` = hapus logo kustom. Mengembalikan false bila ditolak. */
  onSave: (url: string | null) => boolean;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const fileRef = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useState<string | null>(custom);
  const [error, setError] = useState<string | null>(null);

  const pick = async (file: File | undefined) => {
    if (!file) return;
    setError(null);
    try {
      setDraft(await fileToLogo(file));
    } catch (e) {
      const kind = e instanceof Error ? e.message : "decode";
      setError(t(`logo.err.${kind === "type" || kind === "size" ? kind : "decode"}` as never));
    }
  };
  const save = () => {
    if (!onSave(draft)) {
      setError(t("logo.err.decode"));
      return;
    }
    toast.show({ title: t("logo.saved"), tone: "success", timeoutMs: 2000 });
    onClose();
  };

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={title}
      footer={
        <>
          <Button onClick={onClose}>{t("ui.cancel")}</Button>
          <Button variant="solid" onClick={save} disabled={draft === custom}>
            {t("logo.apply")}
          </Button>
        </>
      }
    >
      <div className="logoedit">
        <div className="logoedit__preview">
          <figure className="logoedit__fig">
            <span className="logoedit__tile">
              <ModelLogo providerID={providerID} modelID={modelID} name={name} auto />
            </span>
            <figcaption className="z-small">{t("logo.auto")}</figcaption>
          </figure>
          <figure className="logoedit__fig">
            <span className="logoedit__tile" data-empty={draft ? undefined : ""}>
              {draft ? <img className="z-model-logo" src={draft} alt="" draggable={false} /> : null}
            </span>
            <figcaption className="z-small">{draft ? t("logo.custom") : t("logo.noCustom")}</figcaption>
          </figure>
        </div>
        <p className="z-small">{t("logo.help")}</p>
        <input
          ref={fileRef}
          type="file"
          hidden
          accept={LOGO_ACCEPT}
          aria-label={t("logo.upload")}
          onChange={(e) => {
            void pick(e.target.files?.[0]);
            e.target.value = "";
          }}
        />
        <div className="logoedit__actions">
          <Button onClick={() => fileRef.current?.click()}>
            <UploadIcon size="sm" /> {t("logo.upload")}
          </Button>
          <Button variant="ghost" disabled={!draft} onClick={() => setDraft(null)}>
            <TrashIcon size="sm" /> {t("logo.useAuto")}
          </Button>
        </div>
        {error && (
          <p className="z-small logoedit__error" role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

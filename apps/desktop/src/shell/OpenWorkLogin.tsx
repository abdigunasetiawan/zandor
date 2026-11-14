// Dialog "Masuk dengan OpenWork" (OAuth device flow). UI hanya menampilkan kode dan status; token dan kunci gateway tidak
// pernah sampai ke sini (ditukar dan disimpan di keychain oleh Rust).
import { useI18n } from "@zandor/i18n/react";
import { Button, Dialog, Spinner, useToast } from "@zandor/ui";
import { useEffect, useState } from "react";
import { useProviders, useServices } from "./services";

export function OpenWorkLogin({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const { openwork } = useProviders();
  const toast = useToast();
  const [now, setNow] = useState(() => Date.now());

  // mulai otomatis saat dialog dibuka; batalkan saat ditutup
  useEffect(() => {
    void store.startOpenWork();
    return () => {
      void store.cancelOpenWork();
    };
  }, [store]);

  useEffect(() => {
    if (openwork.status !== "waiting") return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [openwork.status]);

  const openPage = async (url: string) => {
    try {
      await store.openExternal(url);
    } catch (e) {
      toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
    }
  };

  const left = openwork.status === "waiting" ? Math.max(0, Math.round((openwork.expiresAt - now) / 1000)) : 0;

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("ow.title")}
      footer={
        <Button onClick={onClose} variant={openwork.status === "connected" ? "solid" : "outline"}>
          {openwork.status === "connected" ? t("ow.done") : t("ow.cancel")}
        </Button>
      }
    >
      <div className="owlogin" data-testid="openwork-login">
        <p className="z-small mempage__muted">{t("ow.intro")}</p>
        {openwork.status === "starting" && (
          <p role="status">
            <Spinner size="sm" /> {t("ow.starting")}
          </p>
        )}
        {openwork.status === "waiting" && (
          <>
            <p className="z-small">{t("ow.step1")}</p>
            <Button variant="solid" onClick={() => void openPage(openwork.verificationUri)}>
              {t("ow.openPage")}
            </Button>
            <p className="z-small">{t("ow.step2")}</p>
            <p className="owlogin__code z-h2" data-testid="openwork-code">
              {openwork.userCode}
            </p>
            <p role="status" className="z-small mempage__muted">
              <Spinner size="sm" /> {t("ow.waiting", { s: left })}
            </p>
          </>
        )}
        {openwork.status === "connected" && (
          <p role="status" className="owlogin__ok">
            {t("ow.connected", { account: openwork.account, n: openwork.models })}
          </p>
        )}
        {openwork.status === "error" && (
          <>
            <p className="chat__note chat__note--error z-small" role="alert">
              {openwork.message}
            </p>
            <Button onClick={() => void store.startOpenWork()}>{t("ow.retry")}</Button>
          </>
        )}
        <p className="z-small mempage__muted">{t("ow.privacy")}</p>
      </div>
    </Dialog>
  );
}

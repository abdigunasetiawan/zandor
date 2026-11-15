// Masuk dengan OpenWork (OAuth device flow) sebagai kartu inline di tab Sign-in (bukan popup, ADR-056). UI hanya
// menampilkan kode dan status; token dan kunci gateway tidak pernah sampai ke sini (ditukar dan disimpan di keychain
// oleh Rust). Tiap login menambah satu akun pada provider "openwork".
import { useI18n } from "@zandor/i18n/react";
import { Button, ModelLogo, Spinner, useToast } from "@zandor/ui";
import { useEffect, useState } from "react";
import { AccountList } from "./AccountList";
import { useProviders, useServices } from "./services";

export function OpenWorkSignIn() {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const state = useProviders();
  const { openwork } = state;
  const toast = useToast();
  const [now, setNow] = useState(() => Date.now());
  const saved = state.providers.find((p) => p.id === "openwork");
  const running = openwork.status === "starting" || openwork.status === "waiting";

  // jangan biarkan polling berjalan setelah panel ditutup
  useEffect(
    () => () => {
      void store.cancelOpenWork();
    },
    [store],
  );
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
  const hasAccounts = (saved?.accounts?.length ?? 0) > 0;

  return (
    <li className="signin__prov" aria-label="OpenWork">
      <div className="signin__head">
        <ModelLogo providerID="openwork" name="OpenWork" className="ms__logo" />
        <span className="z-label signin__name">OpenWork Cloud</span>
      </div>
      <p className="z-small mempage__muted">{t("ow.intro")}</p>
      {!store.openworkAvailable ? (
        <p className="z-small" role="status">
          {t("ow.desktopOnly")}
        </p>
      ) : (
        <div className="owlogin" data-testid="openwork-login">
          {openwork.status === "starting" && (
            <p role="status" className="z-small">
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
                <span className="visually-hidden">{t("ow.codeLabel")}: </span>
                {openwork.userCode}
              </p>
              <p role="status" className="z-small mempage__muted">
                <Spinner size="sm" /> {t("ow.waiting", { s: left })}
              </p>
            </>
          )}
          {openwork.status === "connected" && (
            <p role="status" className="owlogin__ok z-small">
              {t("ow.connected", { account: openwork.account, n: openwork.models })}
            </p>
          )}
          {openwork.status === "error" && (
            <p className="chat__note chat__note--error z-small" role="alert">
              {openwork.message}
            </p>
          )}
          <div className="owlogin__actions">
            {running ? (
              <Button onClick={() => void store.cancelOpenWork()}>{t("ow.cancel")}</Button>
            ) : (
              <Button variant="solid" onClick={() => void store.startOpenWork()}>
                {openwork.status === "error" ? t("ow.retry") : hasAccounts ? t("acc.add") : t("ow.signIn")}
                <span className="visually-hidden"> OpenWork</span>
              </Button>
            )}
          </div>
        </div>
      )}
      {saved && hasAccounts && <AccountList provider={saved} withAdd={false} />}
      <p className="z-small mempage__muted">{t("ow.privacy")}</p>
    </li>
  );
}

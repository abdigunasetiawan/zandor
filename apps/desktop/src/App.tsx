// Kerangka aplikasi. Belum ada fitur produk; layar ini jujur menyatakannya (prinsip "tidak ada UI palsu").
import { buildInfo, type Logger } from "@zandor/core";
import { LOCALES, type Locale } from "@zandor/i18n";
import { I18nProvider, useI18n } from "@zandor/i18n/react";
import { ConfirmProvider, ErrorBoundary, Select, ShortcutProvider, ToastProvider } from "@zandor/ui";
import { useEffect, useState } from "react";
import { DesignSystemPage } from "./design-system/DesignSystemPage";

export const build = buildInfo({
  packageVersion: __APP_PACKAGE_VERSION__,
  commitCount: __APP_COMMIT_COUNT__,
  isRelease: __APP_RELEASE__,
});

export type Route = { page: "shell" } | { page: "design-system"; section: string | null };

/** `#/design-system[/<bagian>]` → halaman design system; selainnya → kerangka aplikasi. */
export function parseRoute(hash: string): Route {
  const m = hash.match(/^#\/design-system(?:\/([a-z-]+))?\/?$/);
  return m ? { page: "design-system", section: m[1] ?? null } : { page: "shell" };
}

function useRoute(): Route {
  const [hash, setHash] = useState(window.location.hash);
  useEffect(() => {
    const onChange = () => setHash(window.location.hash);
    window.addEventListener("hashchange", onChange);
    return () => window.removeEventListener("hashchange", onChange);
  }, []);
  return parseRoute(hash);
}

function Shell({ locale, onLocale }: { locale: Locale; onLocale: (l: Locale) => void }) {
  const { t } = useI18n();
  return (
    <main className="shell">
      <header className="shell__header">
        <h1 className="z-h4">{t("app.name")}</h1>
        <div className="shell__lang">
          <span className="z-label">{t("settings.language")}</span>
          <Select
            aria-label={t("settings.language")}
            value={locale}
            onValueChange={(v) => onLocale(v as Locale)}
            items={LOCALES.map((l) => ({ value: l, label: t(`language.${l}`) }))}
          />
        </div>
      </header>
      <section className="shell__empty" aria-labelledby="empty-title">
        <h2 id="empty-title" className="z-h3">
          {t("shell.emptyTitle")}
        </h2>
        <p className="z-p shell__muted">{t("shell.emptyBody")}</p>
      </section>
      <footer className="shell__footer">
        <span data-testid="version" className="z-small">
          {t("app.version", { version: build.version })}
        </span>
        <a className="z-link" href="#/design-system">
          {t("shell.designSystem")}
        </a>
      </footer>
    </main>
  );
}

function Routed({ locale, onLocale }: { locale: Locale; onLocale: (l: Locale) => void }) {
  const route = useRoute();
  useEffect(() => {
    if (route.page === "design-system" && route.section) {
      document.getElementById(route.section)?.scrollIntoView?.({ block: "start" });
    }
  }, [route]);
  return route.page === "design-system" ? <DesignSystemPage /> : <Shell locale={locale} onLocale={onLocale} />;
}

export function App({ initialLocale, logger }: { initialLocale: Locale; logger: Logger }) {
  const [locale, setLocale] = useState<Locale>(initialLocale);
  const onLocale = (l: Locale) => {
    setLocale(l);
    document.documentElement.lang = l;
    try {
      localStorage.setItem("zandor.locale", l);
    } catch {
      // penyimpanan tidak tersedia: bahasa tetap berlaku untuk sesi ini
    }
  };
  return (
    <I18nProvider locale={locale}>
      <ErrorBoundary name="app" logger={logger}>
        <ShortcutProvider>
          <ToastProvider>
            <ConfirmProvider>
              <Routed locale={locale} onLocale={onLocale} />
            </ConfirmProvider>
          </ToastProvider>
        </ShortcutProvider>
      </ErrorBoundary>
    </I18nProvider>
  );
}

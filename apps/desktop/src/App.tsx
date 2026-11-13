// Kerangka aplikasi Fase 1. Belum ada fitur; layar ini jujur menyatakannya (prinsip "tidak ada UI palsu").
import { buildInfo, type Logger } from "@zandor/core";
import { LOCALES, type Locale } from "@zandor/i18n";
import { I18nProvider, useI18n } from "@zandor/i18n/react";
import { ErrorBoundary } from "@zandor/ui";
import { useState } from "react";

export const build = buildInfo({
  packageVersion: __APP_PACKAGE_VERSION__,
  commitCount: __APP_COMMIT_COUNT__,
  isRelease: __APP_RELEASE__,
});

function Shell({ locale, onLocale }: { locale: Locale; onLocale: (l: Locale) => void }) {
  const { t } = useI18n();
  return (
    <main className="shell">
      <header className="shell__header">
        <h1>{t("app.name")}</h1>
        <label className="shell__lang">
          <span>{t("settings.language")}</span>
          <select
            value={locale}
            onChange={(e) => onLocale(e.target.value as Locale)}
            aria-label={t("settings.language")}
          >
            {LOCALES.map((l) => (
              <option key={l} value={l}>
                {t(`language.${l}`)}
              </option>
            ))}
          </select>
        </label>
      </header>
      <section className="shell__empty" aria-labelledby="empty-title">
        <h2 id="empty-title">{t("shell.emptyTitle")}</h2>
        <p>{t("shell.emptyBody")}</p>
      </section>
      <footer className="shell__footer" data-testid="version">
        {t("app.version", { version: build.version })}
      </footer>
    </main>
  );
}

export function App({ initialLocale, logger }: { initialLocale: Locale; logger: Logger }) {
  const [locale, setLocale] = useState<Locale>(initialLocale);
  return (
    <I18nProvider locale={locale}>
      <ErrorBoundary name="shell" logger={logger}>
        <Shell
          locale={locale}
          onLocale={(l) => {
            setLocale(l);
            document.documentElement.lang = l;
            try {
              localStorage.setItem("zandor.locale", l);
            } catch {
              // penyimpanan tidak tersedia: bahasa tetap berlaku untuk sesi ini
            }
          }}
        />
      </ErrorBoundary>
    </I18nProvider>
  );
}

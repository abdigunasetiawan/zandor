import { createContext, type ReactNode, useContext, useMemo } from "react";
import { createTranslator, DEFAULT_LOCALE, type Locale, type Translate } from "./index";

type I18nValue = { locale: Locale; t: Translate };

const I18nContext = createContext<I18nValue>({ locale: DEFAULT_LOCALE, t: createTranslator(DEFAULT_LOCALE) });

export function I18nProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  const value = useMemo(() => ({ locale, t: createTranslator(locale) }), [locale]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
  return useContext(I18nContext);
}

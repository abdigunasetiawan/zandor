import { render } from "@testing-library/react";
import type { Locale } from "@zandor/i18n";
import { I18nProvider } from "@zandor/i18n/react";
import type { ReactElement } from "react";

/** Render dengan provider i18n (default: Indonesia). */
export function renderUi(ui: ReactElement, locale: Locale = "id") {
  return render(<I18nProvider locale={locale}>{ui}</I18nProvider>);
}

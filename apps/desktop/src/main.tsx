import { consoleSink, createLogger } from "@zandor/core";
import { detectLocale } from "@zandor/i18n";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

const logger = createLogger("app", { level: import.meta.env.DEV ? "debug" : "info", sinks: [consoleSink] });

function storedLocale(): string | null {
  try {
    return localStorage.getItem("zandor.locale");
  } catch {
    return null;
  }
}

const locale = detectLocale(storedLocale() ?? navigator.language);
document.documentElement.lang = locale;

const root = document.getElementById("root");
if (!root) throw new Error("elemen #root tidak ada");
createRoot(root).render(
  <StrictMode>
    <App initialLocale={locale} logger={logger} />
  </StrictMode>,
);

import { consoleSink, createLogger } from "@zandor/core";
import { detectLocale } from "@zandor/i18n";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App, hashToPath } from "./App";
import { loadEngineHost } from "./shell/engine-host";
import { createServices } from "./shell/services";
import "@zandor/ui";
import "./styles.css";

const SETTINGS_KEY = "zandor.settings";

const logger = createLogger("app", { level: import.meta.env.DEV ? "debug" : "info", sinks: [consoleSink] });

const host = await loadEngineHost();
const services = createServices({
  logger,
  host,
  initialPath: hashToPath(window.location.hash),
  storage: {
    get: () => {
      try {
        return localStorage.getItem(SETTINGS_KEY);
      } catch {
        return null;
      }
    },
    set: (v) => localStorage.setItem(SETTINGS_KEY, v),
  },
});

// Pemakaian pertama: ikuti bahasa sistem bila didukung, lalu simpan sebagai pilihan eksplisit.
if (!services.settings.isExplicit("language")) {
  services.settings.set("language", detectLocale(navigator.language));
}

const root = document.getElementById("root");
if (!root) throw new Error("elemen #root tidak ada");
createRoot(root).render(
  <StrictMode>
    <App services={services} />
  </StrictMode>,
);

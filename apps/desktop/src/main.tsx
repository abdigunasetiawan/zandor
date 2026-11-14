import { ActivityLog, consoleSink, createLogger } from "@zandor/core";
import { detectLocale } from "@zandor/i18n";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App, hashToPath } from "./App";
import { loadDocHost } from "./shell/doc-host";
import { loadEngineHost } from "./shell/engine-host";
import { loadGitHost } from "./shell/git-host";
import { loadMemoryHost } from "./shell/memory-host";
import { loadNotifyHost } from "./shell/notify-host";
import { loadOpenVsxHost } from "./shell/openvsx-host";
import { loadProviderHost } from "./shell/provider-host";
import { createServices } from "./shell/services";
import "@zandor/ui";
import "./styles.css";

const SETTINGS_KEY = "zandor.settings";
const APPEARANCE_KEY = "zandor.appearance";
const AUTOMATION_KEY = "zandor.automation";

const activity = new ActivityLog();
const logger = createLogger("app", {
  level: import.meta.env.DEV ? "debug" : "info",
  sinks: [consoleSink, activity.sink],
});

const host = await loadEngineHost();
const providerHost = await loadProviderHost();
let servicesRef: ReturnType<typeof createServices> | null = null;
const docHost = await loadDocHost(() => servicesRef?.projects.engineApi() ?? null);
const services = createServices({
  logger,
  host,
  providerHost,
  docHost,
  gitHost: await loadGitHost(),
  memoryHost: await loadMemoryHost(),
  openVsxHost: await loadOpenVsxHost(),
  notifyHost: await loadNotifyHost(),
  automationStorage: {
    get: () => {
      try {
        return localStorage.getItem(AUTOMATION_KEY);
      } catch {
        return null;
      }
    },
    set: (v) => localStorage.setItem(AUTOMATION_KEY, v),
  },
  appearanceStorage: {
    get: () => {
      try {
        return localStorage.getItem(APPEARANCE_KEY);
      } catch {
        return null;
      }
    },
    set: (v) => localStorage.setItem(APPEARANCE_KEY, v),
  },
  activity,
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

servicesRef = services;

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

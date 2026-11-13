// Akar aplikasi: layanan, penyedia, top bar, dan halaman (beranda = lima panel, Pengaturan, design system).
import { computeVars, createEngineApi, fromSnapshot, type RawEngineSnapshot } from "@zandor/core";
import { I18nProvider } from "@zandor/i18n/react";
import {
  applyScale,
  applyTheme,
  applyVars,
  ConfirmProvider,
  ErrorBoundary,
  ShortcutProvider,
  ToastProvider,
} from "@zandor/ui";
import { useEffect } from "react";
import { DesignSystemPage } from "./design-system/DesignSystemPage";
import { AppearancePage } from "./shell/AppearancePage";
import { CommandCenter } from "./shell/CommandCenter";
import { MemoryPage } from "./shell/MemoryPage";
import { ProvidersPage } from "./shell/ProvidersPage";
import { SettingsPage } from "./shell/SettingsPage";
import { ShellLayout } from "./shell/ShellLayout";
import {
  type Services,
  ServicesProvider,
  useAppearance,
  useNavState,
  useServices,
  useSettings,
} from "./shell/services";
import { TitleBar } from "./shell/TitleBar";

export type Route =
  | { page: "home" }
  | { page: "settings" }
  | { page: "providers" }
  | { page: "memory" }
  | { page: "appearance" }
  | { page: "design-system"; section: string | null };

/** "/", "/settings", "/design-system[/bagian]" → rute. Selain itu = beranda. */
export function parseRoute(path: string): Route {
  if (/^\/settings\/?$/.test(path)) return { page: "settings" };
  if (/^\/providers\/?$/.test(path)) return { page: "providers" };
  if (/^\/memory\/?$/.test(path)) return { page: "memory" };
  if (/^\/appearance\/?$/.test(path)) return { page: "appearance" };
  const m = path.match(/^\/design-system(?:\/([a-z-]+))?\/?$/);
  return m ? { page: "design-system", section: m[1] ?? null } : { page: "home" };
}

/** "#/settings" → "/settings"; kosong/tak dikenal → "/". */
export function hashToPath(hash: string): string {
  return hash.startsWith("#/") ? hash.slice(1) : "/";
}

/** Terapkan pengaturan ke DOM (tema, skala, bahasa) dan sinkronkan riwayat navigasi dengan hash URL. */
function Effects() {
  const { nav, host, connection, projects, chat, providers, docs, terminals, activity, logger } = useServices();
  const settings = useSettings();
  const { current } = useNavState();

  useEffect(() => applyTheme(settings.theme), [settings.theme]);
  useEffect(() => applyScale(settings.uiScale), [settings.uiScale]);
  // Tema kustom/tipografi/panel chat (Fase 10): dihitung untuk mode efektif; mode "system" mengikuti OS.
  const appearance = useAppearance();
  useEffect(() => {
    const mq = window.matchMedia?.("(prefers-color-scheme: light)");
    let applied: string[] = [];
    const run = () => {
      const mode = settings.theme === "system" ? (mq?.matches ? "light" : "dark") : settings.theme;
      applied = applyVars(computeVars(appearance, mode), applied);
    };
    run();
    mq?.addEventListener("change", run);
    return () => {
      mq?.removeEventListener("change", run);
      applyVars({}, applied);
    };
  }, [appearance, settings.theme]);
  useEffect(() => {
    document.documentElement.lang = settings.language;
  }, [settings.language]);

  useEffect(() => {
    const target = current === "/" ? "" : `#${current}`;
    if (window.location.hash !== target) history.replaceState(null, "", target || window.location.pathname);
  }, [current]);

  // Engine: muat registry project, ikuti status supervisor, dan beri ProjectStore klien saat engine siap.
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | null = null;
    const apply = (raw: RawEngineSnapshot) => {
      if (!alive) return;
      const state = fromSnapshot(raw);
      if (connection.state().status !== state.status) {
        activity.add(
          "engine",
          state.status === "error" ? "error" : "info",
          `engine ${state.status}${state.version ? ` ${state.version}` : ""}${state.error ? `: ${state.error}` : ""}`,
        );
      }
      connection.set(state);
      const api = state.info ? createEngineApi(state.info) : null;
      projects.setApi(api);
      chat.setApi(api);
      providers.setApi(api);
      terminals.setClient(api ? api.pty : null);
      // tab dokumen dipulihkan setelah engine siap; di Tauri membaca lewat Rust sehingga tidak butuh engine
      if (api || host.native) docs.restore();
    };
    void (async () => {
      try {
        const dir = await host.chatsDir();
        projects.setChatDir(dir);
        await projects.init();
        void providers.refresh();
        apply(await host.snapshot());
        const off = await host.onChange(apply);
        if (alive) stop = off;
        else off();
      } catch (e) {
        logger.error("gagal menyambung ke engine", { error: String(e) });
      }
    })();
    return () => {
      alive = false;
      stop?.();
    };
  }, [host, connection, projects, chat, providers, docs, terminals, activity, logger]);

  useEffect(() => {
    const onHash = () => nav.push(hashToPath(window.location.hash));
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, [nav]);

  return null;
}

function Page() {
  const { current } = useNavState();
  const route = parseRoute(current);
  const page = route.page;
  const section = route.page === "design-system" ? route.section : null;
  useEffect(() => {
    if (page === "design-system" && section) document.getElementById(section)?.scrollIntoView?.({ block: "start" });
  }, [page, section]);
  if (page === "settings") return <SettingsPage />;
  if (page === "providers") return <ProvidersPage />;
  if (page === "memory") return <MemoryPage />;
  if (page === "appearance") return <AppearancePage />;
  if (page === "design-system") return <DesignSystemPage />;
  return <ShellLayout />;
}

export function App({ services }: { services: Services }) {
  return (
    <ServicesProvider services={services}>
      <Root />
    </ServicesProvider>
  );
}

function Root() {
  const { logger } = useServices();
  const settings = useSettings();
  return (
    <I18nProvider locale={settings.language}>
      <ErrorBoundary name="app" logger={logger}>
        <ShortcutProvider>
          <ToastProvider>
            <ConfirmProvider>
              <CommandCenter>
                <Effects />
                <div className="app">
                  <TitleBar />
                  <div className="app__content">
                    <Page />
                  </div>
                </div>
              </CommandCenter>
            </ConfirmProvider>
          </ToastProvider>
        </ShortcutProvider>
      </ErrorBoundary>
    </I18nProvider>
  );
}

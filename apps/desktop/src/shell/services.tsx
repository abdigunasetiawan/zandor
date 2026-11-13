// Layanan bersama aplikasi (satu instance per aplikasi, dibuat di main.tsx). Komponen mengaksesnya lewat hook.
import {
  type CommandDef,
  CommandRegistry,
  type Logger,
  NavigationHistory,
  type SettingKey,
  type Settings,
  SettingsStore,
} from "@zandor/core";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useState,
  useSyncExternalStore,
} from "react";
import { loadWindowApi, type WindowApi } from "./window-api";

export const TOOL_IDS = ["terminal", "git", "browser", "info", "log"] as const;
export type ToolId = (typeof TOOL_IDS)[number];
export const PANEL_IDS = ["sidebar", "chat", "docs", "tools"] as const;
export type PanelId = (typeof PANEL_IDS)[number];

export type PanelState = {
  collapsed: Record<PanelId, boolean>;
  activeTool: ToolId /** true bila tata letak panel sedang terpasang */;
  ready: boolean;
};
export type PanelActions = {
  toggle: (id: PanelId) => void;
  openTool: (tool: ToolId) => void;
  reset: () => void;
};

/** Status panel yang bisa diamati; aksinya dipasang ShellLayout selama terpasang. */
export class PanelBridge {
  private snap: PanelState = {
    collapsed: { sidebar: false, chat: false, docs: false, tools: false },
    activeTool: "terminal",
    ready: false,
  };
  private listeners = new Set<() => void>();
  actions: PanelActions | null = null;

  /** Dipasang ShellLayout saat terpasang (ready=true) dan dilepas saat dilepas (ready=false). */
  setActions(actions: PanelActions | null) {
    this.actions = actions;
    this.update({ ready: actions !== null });
  }

  state(): PanelState {
    return this.snap;
  }

  update(next: Partial<PanelState>) {
    const merged = { ...this.snap, ...next };
    const same =
      merged.activeTool === this.snap.activeTool &&
      merged.ready === this.snap.ready &&
      PANEL_IDS.every((id) => merged.collapsed[id] === this.snap.collapsed[id]);
    if (same) return;
    this.snap = merged;
    for (const l of this.listeners) l();
  }

  subscribe(l: () => void): () => void {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  }
}

export type Services = {
  settings: SettingsStore;
  commands: CommandRegistry;
  nav: NavigationHistory<string>;
  panels: PanelBridge;
  logger: Logger;
};

export function createServices(options: {
  storage: { get(): string | null; set(v: string): void };
  initialPath: string;
  logger: Logger;
}): Services {
  return {
    settings: new SettingsStore({
      storage: options.storage,
      onInvalid: (errors) => options.logger.warn("pengaturan tersimpan tidak valid", { errors }),
    }),
    commands: new CommandRegistry(),
    nav: new NavigationHistory(options.initialPath),
    panels: new PanelBridge(),
    logger: options.logger,
  };
}

const ServicesContext = createContext<Services | null>(null);

export function ServicesProvider({ services, children }: { services: Services; children: ReactNode }) {
  return <ServicesContext.Provider value={services}>{children}</ServicesContext.Provider>;
}

export function useServices(): Services {
  const s = useContext(ServicesContext);
  if (!s) throw new Error("harus dipakai di dalam <ServicesProvider>");
  return s;
}

export function useSettings(): Settings {
  const { settings } = useServices();
  return useSyncExternalStore(
    (cb) => settings.subscribe(cb),
    () => settings.all(),
    () => settings.all(),
  );
}

export function useSetting<K extends SettingKey>(key: K): [Settings[K], (value: Settings[K]) => void] {
  const { settings } = useServices();
  const all = useSettings();
  const set = useCallback((v: Settings[K]) => settings.set(key, v), [settings, key]);
  return [all[key], set];
}

export function useNavState() {
  const { nav } = useServices();
  return useSyncExternalStore(
    (cb) => nav.subscribe(cb),
    () => nav.state(),
    () => nav.state(),
  );
}

export function usePanelState(): PanelState {
  const { panels } = useServices();
  return useSyncExternalStore(
    (cb) => panels.subscribe(cb),
    () => panels.state(),
    () => panels.state(),
  );
}

export function useCommandList(): CommandDef[] {
  const { commands } = useServices();
  return useSyncExternalStore(
    (cb) => commands.subscribe(cb),
    () => commands.list(),
    () => commands.list(),
  );
}

/** Adapter jendela Tauri; null di luar Tauri (mis. browser/e2e) sehingga kontrol jendela tidak tampil. */
export function useWindowApi(): WindowApi | null {
  const [api, setApi] = useState<WindowApi | null>(null);
  useEffect(() => {
    let alive = true;
    void loadWindowApi().then((a) => alive && setApi(a));
    return () => {
      alive = false;
    };
  }, []);
  return api;
}

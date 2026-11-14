// Layanan bersama aplikasi (satu instance per aplikasi, dibuat di main.tsx). Komponen mengaksesnya lewat hook.

import type { GitHost, ProviderHost } from "@zandor/core";
import {
  type ActivityEntry,
  type ActivityLog,
  type Appearance,
  type AppearanceStorage,
  AppearanceStore,
  AttentionTracker,
  type AutomationState,
  type AutomationStorage,
  AutomationStore,
  ChatStore,
  type ChatStoreState,
  type CommandDef,
  CommandRegistry,
  ConnectionStore,
  DocStore,
  type DocsState,
  type EngineState,
  type GitState,
  GitStore,
  isAbsolutePath,
  type Logger,
  locateInRoots,
  type MemoryHost,
  type MemoryState,
  MemoryStore,
  NavigationHistory,
  type OpenVsxHost,
  ProjectStore,
  type ProjectsState,
  ProviderStore,
  type ProvidersState,
  type SessionIndicator,
  type SettingKey,
  type Settings,
  SettingsStore,
  SidebarPrefs,
  type SidebarPrefsState,
  type TerminalState,
  TerminalStore,
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
import type { DocHost } from "./doc-host";
import type { EngineHost } from "./engine-host";
import type { NotifyHost } from "./notify-host";
import { loadWindowApi, type WindowApi } from "./window-api";

export const TOOL_IDS = ["terminal", "git", "changes", "files", "browser", "info", "session", "log"] as const;
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
  /** Buka panel bila terlipat. */
  reveal: (id: PanelId) => void;
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
  host: EngineHost;
  connection: ConnectionStore;
  projects: ProjectStore;
  chat: ChatStore;
  providers: ProviderStore;
  memory: MemoryStore;
  appearance: AppearanceStore;
  automation: AutomationStore;
  notify: NotifyHost;
  openVsx: OpenVsxHost;
  docs: DocStore;
  docHost: DocHost;
  terminals: TerminalStore;
  git: GitStore;
  activity: ActivityLog;
  /** Indikator sesi + peristiwa yang layak diberitahukan (selesai/gagal/butuh aksi). */
  attention: AttentionTracker;
  /** Sematan dan urutan buatan pengguna untuk sesi/project di sidebar. */
  sidebarPrefs: SidebarPrefs;
};

export function createServices(options: {
  storage: { get(): string | null; set(v: string): void };
  initialPath: string;
  logger: Logger;
  host: EngineHost;
  providerHost: ProviderHost;
  docHost: DocHost;
  gitHost: GitHost;
  memoryHost: MemoryHost;
  appearanceStorage: AppearanceStorage;
  automationStorage: AutomationStorage;
  notifyHost: NotifyHost;
  openVsxHost: OpenVsxHost;
  /** Log aktivitas bersama; dibuat di main.tsx supaya sink logger sudah terpasang sejak awal. */
  activity: ActivityLog;
}): Services {
  const chat = new ChatStore({
    prefs: {
      get: (k) => {
        try {
          return localStorage.getItem(k);
        } catch {
          return null;
        }
      },
      set: (k, v) => {
        try {
          localStorage.setItem(k, v);
        } catch {
          // sengaja diabaikan: pilihan model hanya kenyamanan
        }
      },
    },
    onWarning: (m) => options.logger.warn(m),
    onEvent: (ev) => options.activity.fromEngineEvent(ev),
  });
  return {
    settings: new SettingsStore({
      storage: options.storage,
      onInvalid: (errors) => options.logger.warn("pengaturan tersimpan tidak valid", { errors }),
    }),
    commands: new CommandRegistry(),
    nav: new NavigationHistory(options.initialPath),
    panels: new PanelBridge(),
    logger: options.logger,
    host: options.host,
    connection: new ConnectionStore(),
    projects: new ProjectStore({
      storage: { load: () => options.host.registryLoad(), save: (t) => options.host.registrySave(t) },
      chatFolders: {
        create: (name) => options.host.chatFolderNew(name),
        list: () => options.host.chatFolders(),
        removeIfEmpty: (path) => options.host.chatFolderRemoveIfEmpty(path),
      },
      onWarning: (m) => options.logger.warn(m),
    }),
    chat,
    docHost: options.docHost,
    terminals: new TerminalStore(),
    git: new GitStore(options.gitHost),
    memory: new MemoryStore(options.memoryHost),
    appearance: new AppearanceStore(options.appearanceStorage),
    automation: new AutomationStore(options.automationStorage),
    notify: options.notifyHost,
    openVsx: options.openVsxHost,
    activity: options.activity,
    attention: new AttentionTracker(),
    sidebarPrefs: new SidebarPrefs({
      get: (k) => {
        try {
          return localStorage.getItem(k);
        } catch {
          return null;
        }
      },
      set: (k, v) => {
        try {
          localStorage.setItem(k, v);
        } catch {
          // sengaja diabaikan
        }
      },
    }),
    docs: new DocStore({
      source: options.docHost.source,
      prefs: {
        get: (k) => {
          try {
            return localStorage.getItem(k);
          } catch {
            return null;
          }
        },
        set: (k, v) => {
          try {
            localStorage.setItem(k, v);
          } catch {
            // sengaja diabaikan
          }
        },
      },
    }),
    providers: new ProviderStore({
      host: options.providerHost,
      onChanged: () => void chat.refreshModels(),
    }),
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

export function useEngineState(): EngineState {
  const { connection } = useServices();
  return useSyncExternalStore(
    (cb) => connection.subscribe(cb),
    () => connection.state(),
    () => connection.state(),
  );
}

export function useProjects(): ProjectsState {
  const { projects } = useServices();
  return useSyncExternalStore(
    (cb) => projects.subscribe(cb),
    () => projects.state(),
    () => projects.state(),
  );
}

export function useChat(): ChatStoreState {
  const { chat } = useServices();
  return useSyncExternalStore(
    (cb) => chat.subscribe(cb),
    () => chat.state(),
    () => chat.state(),
  );
}

export function useProviders(): ProvidersState {
  const { providers } = useServices();
  return useSyncExternalStore(
    (cb) => providers.subscribe(cb),
    () => providers.state(),
    () => providers.state(),
  );
}

export function useDocs(): DocsState {
  const { docs } = useServices();
  return useSyncExternalStore(
    (cb) => docs.subscribe(cb),
    () => docs.state(),
    () => docs.state(),
  );
}

/**
 * Buka berkas di Panel 3. `path` boleh relatif (terhadap direktori sesi terpilih) atau absolut (harus berada di salah
 * satu project/folder Chat). Mengembalikan false bila berada di luar project (tidak dibuka).
 */
export type DocTarget = { root: string; path: string };

/** Aksi berkas dokumen dari percakapan: temukan di akar yang terdaftar, buka di Panel Dokumen, atau simpan salinan (unduh). */
export function useDocActions() {
  const { docs, docHost, panels, projects } = useServices();
  const resolve = useCallback(
    (path: string, root?: string): DocTarget | null => {
      const state = projects.state();
      const roots = [...state.projects.map((p) => p.path), ...(state.chatDir ? [state.chatDir] : [])];
      if (isAbsolutePath(path)) return locateInRoots(roots, path);
      const base = root ?? projects.selected()?.session.directory ?? null;
      if (base && roots.some((r) => r.toLowerCase() === base.toLowerCase())) return { root: base, path };
      if (base) return locateInRoots(roots, `${base.replace(/[\\/]+$/, "")}/${path}`);
      return null;
    },
    [projects],
  );
  const open = useCallback(
    (path: string, root?: string): boolean => {
      const target = resolve(path, root);
      if (!target) return false;
      docs.open(target.root, target.path);
      panels.actions?.reveal("docs");
      return true;
    },
    [docs, panels, resolve],
  );
  /** "saved" | "cancelled" | "outside" (berkas di luar project/Chat). Galat nyata dilempar. */
  const download = useCallback(
    async (path: string, root?: string): Promise<"saved" | "cancelled" | "outside"> => {
      const target = resolve(path, root);
      if (!target) return "outside";
      const name = target.path.split(/[\\/]/).pop() ?? "berkas";
      const bytes = docHost.native ? null : await docHost.source.read(target.root, target.path);
      return (await docHost.saveCopy(target.root, target.path, name, bytes)) ? "saved" : "cancelled";
    },
    [docHost, resolve],
  );
  return { resolve, open, download };
}

export function useOpenDoc(): (path: string, root?: string) => boolean {
  return useDocActions().open;
}

export function useTerminals(): TerminalState {
  const { terminals } = useServices();
  return useSyncExternalStore(
    (cb) => terminals.subscribe(cb),
    () => terminals.state(),
    () => terminals.state(),
  );
}

export function useSidebarPrefs(): SidebarPrefsState {
  const { sidebarPrefs } = useServices();
  return useSyncExternalStore(
    (cb) => sidebarPrefs.subscribe(cb),
    () => sidebarPrefs.state(),
    () => sidebarPrefs.state(),
  );
}

/** Status tampilan satu sesi (idle/bekerja/butuh aksi/selesai/gagal/batas). */
export function useSessionIndicator(sessionId: string): SessionIndicator {
  const { attention } = useServices();
  const chat = useChat();
  useSyncExternalStore(
    (cb) => attention.subscribe(cb),
    () => attention.state(),
    () => attention.state(),
  );
  return attention.indicator(sessionId, chat.sessions[sessionId]);
}

export function useGit(): GitState {
  const { git } = useServices();
  return useSyncExternalStore(
    (cb) => git.subscribe(cb),
    () => git.state(),
    () => git.state(),
  );
}

export function useActivity(): readonly ActivityEntry[] {
  const { activity } = useServices();
  return useSyncExternalStore(
    (cb) => activity.subscribe(cb),
    () => activity.list(),
    () => activity.list(),
  );
}

export function useMemory(): MemoryState {
  const { memory } = useServices();
  return useSyncExternalStore(
    (cb) => memory.subscribe(cb),
    () => memory.state(),
    () => memory.state(),
  );
}

export function useAppearance(): Appearance {
  const { appearance } = useServices();
  return useSyncExternalStore(
    (cb) => appearance.subscribe(cb),
    () => appearance.state(),
    () => appearance.state(),
  );
}

export function useAutomation(): AutomationState {
  const { automation } = useServices();
  return useSyncExternalStore(
    (cb) => automation.subscribe(cb),
    () => automation.state(),
    () => automation.state(),
  );
}

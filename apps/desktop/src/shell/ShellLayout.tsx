// Lima panel (ADR-015/025): P1 Sidebar | P2 Chat | P3 Dokumen | P4 Tools | P5 Rail.
// Auto-collapse ditentukan perencana murni (@zandor/core `desiredCollapsed`), bukan reaksi pustaka UI.
import { desiredCollapsed, formatCombo, type PanelSpec, remToPx } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Badge,
  ErrorBoundary,
  GitIcon,
  GlobeIcon,
  IconButton,
  InfoIcon,
  ListIcon,
  ShortcutScope,
  Tabs,
  TerminalIcon,
} from "@zandor/ui";
import { type ReactNode, useCallback, useEffect, useMemo, useRef } from "react";
import { Group, Panel, Separator, useDefaultLayout, useGroupRef, usePanelRef } from "react-resizable-panels";
import { ChatPanel } from "./ChatPanel";
import { DocPanel } from "./docs/DocPanel";
import { Sidebar } from "./Sidebar";
import { PANEL_IDS, type PanelId, TOOL_IDS, type ToolId, usePanelState, useServices, useSetting } from "./services";
import { ActivityPanel } from "./tools/ActivityPanel";
import { BrowserPanel } from "./tools/BrowserPanel";
import { GitPanel } from "./tools/GitPanel";
import { InfoPanel } from "./tools/InfoPanel";
import { TerminalPanel } from "./tools/TerminalPanel";

type PanelDef = { id: PanelId; minRem: number; collapsible: boolean; priority: number; defaultSize: string };

/** Lebar minimum dalam rem (ikut --ui-scale). Chat tidak bisa dilipat (ADR-015). */
export const PANEL_DEFS: PanelDef[] = [
  { id: "sidebar", minRem: 12, collapsible: true, priority: 2, defaultSize: "18%" },
  { id: "chat", minRem: 22, collapsible: false, priority: 99, defaultSize: "34%" },
  { id: "docs", minRem: 16, collapsible: true, priority: 1, defaultSize: "22%" },
  { id: "tools", minRem: 18, collapsible: true, priority: 3, defaultSize: "26%" },
];
const MIN_REM = Object.fromEntries(PANEL_DEFS.map((p) => [p.id, p.minRem])) as Record<PanelId, number>;
const DEFAULT_LAYOUT = { sidebar: 18, chat: 34, docs: 22, tools: 26 };
const FALLBACK_SEPARATOR_PX = 6;
const MAX_LAYOUT_REAPPLY = 6;
const COLLAPSED_KEY = "zandor.layout.userCollapsed";
const LAYOUT_ID = "zandor-main";

export const TOOL_ICONS: Record<ToolId, ReactNode> = {
  terminal: <TerminalIcon />,
  git: <GitIcon />,
  browser: <GlobeIcon />,
  info: <InfoIcon />,
  log: <ListIcon />,
};

function PanelEmpty({ title, body }: { title: string; body: string }) {
  const { t } = useI18n();
  return (
    <div className="shell-empty">
      <Badge tone="outline">{t("empty.soon")}</Badge>
      <h2 className="z-h4">{title}</h2>
      <p className="z-p shell-empty__body">{body}</p>
    </div>
  );
}

function rootFontPx(): number {
  return Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
}

function loadCollapsed(): Set<PanelId> {
  try {
    const raw = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return new Set((Array.isArray(raw) ? raw : []).filter((x): x is PanelId => PANEL_IDS.includes(x)));
  } catch {
    return new Set();
  }
}

export function ShellLayout() {
  const { t } = useI18n();
  const { panels: bridge, commands, logger, settings } = useServices();
  const [remember] = useSetting("rememberLayout");
  const [scale] = useSetting("uiScale");
  const panelState = usePanelState();

  const group = useGroupRef();
  const sidebarRef = usePanelRef();
  const chatRef = usePanelRef();
  const docsRef = usePanelRef();
  const toolsRef = usePanelRef();
  const refs = useMemo(
    () => ({ sidebar: sidebarRef, chat: chatRef, docs: docsRef, tools: toolsRef }),
    [sidebarRef, chatRef, docsRef, toolsRef],
  );
  const groupEl = useRef<HTMLDivElement>(null);

  // Pilihan pengguna (tombol, perintah, seret). Hanya ini yang dipertahankan; sisanya dihitung perencana.
  const userCollapsed = useRef<Set<PanelId>>(settings.get("rememberLayout") ? loadCollapsed() : new Set());
  const protectedId = useRef<PanelId | null>(null);
  const lastApplied = useRef<Set<string>>(new Set());

  const rememberRef = useRef(remember);
  rememberRef.current = remember;

  // Penyimpanan tata letak mengikuti pengaturan "ingat tata letak".
  const storage = useMemo(
    () => ({
      getItem: (k: string) => (rememberRef.current ? localStorage.getItem(k) : null),
      setItem: (k: string, v: string) => {
        if (rememberRef.current) localStorage.setItem(k, v);
      },
    }),
    [],
  );
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: LAYOUT_ID, storage });

  const persistCollapsed = useCallback(() => {
    try {
      if (rememberRef.current) localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...userCollapsed.current]));
      else localStorage.removeItem(COLLAPSED_KEY);
    } catch {
      // penyimpanan tidak tersedia
    }
  }, []);

  /** Pengaman: terapkan-ulang akibat perubahan layout berturut-turut dibatasi (mencegah berputar bila pustaka menolak). */
  const layoutChurn = useRef(0);

  /**
   * Hitung ulang panel yang harus terlipat lalu terapkan ke pustaka. Idempotent.
   * `fromLayout`: dipicu perubahan tata letak (bukan ukuran jendela/aksi pengguna).
   */
  const apply = useCallback(
    (fromLayout = false) => {
      if (fromLayout) {
        if (layoutChurn.current++ >= MAX_LAYOUT_REAPPLY) return;
      } else {
        layoutChurn.current = 0;
      }
      const el = groupEl.current;
      if (!el) return;
      const root = rootFontPx();
      const sep = el.querySelector("[data-separator]")?.getBoundingClientRect().width || FALLBACK_SEPARATOR_PX;
      const specs: PanelSpec[] = PANEL_DEFS.map((p) => ({
        id: p.id,
        minPx: remToPx(MIN_REM[p.id], root),
        collapsible: p.collapsible,
        priority: p.priority,
      }));
      const desired = desiredCollapsed({
        availablePx: el.getBoundingClientRect().width,
        separatorPx: sep,
        panels: specs,
        userCollapsed: userCollapsed.current,
        protectedId: protectedId.current,
      });
      for (const def of PANEL_DEFS) {
        if (!def.collapsible) continue;
        const ref = refs[def.id].current;
        if (!ref) continue;
        const isCollapsed = ref.isCollapsed();
        if (desired.has(def.id) && !isCollapsed) ref.collapse();
        else if (!desired.has(def.id) && isCollapsed) ref.expand();
      }
      lastApplied.current = desired;
      const collapsed = Object.fromEntries(PANEL_IDS.map((id) => [id, desired.has(id)])) as Record<PanelId, boolean>;
      bridge.update({ collapsed });
    },
    [bridge, refs],
  );

  const toggle = useCallback(
    (id: PanelId) => {
      const def = PANEL_DEFS.find((p) => p.id === id);
      if (!def?.collapsible) return;
      const isCollapsed = lastApplied.current.has(id);
      if (isCollapsed) {
        userCollapsed.current.delete(id);
        protectedId.current = id; // yang baru dibuka pengguna tidak boleh langsung dilipat otomatis
      } else {
        userCollapsed.current.add(id);
        if (protectedId.current === id) protectedId.current = null;
      }
      persistCollapsed();
      apply();
    },
    [apply, persistCollapsed],
  );

  /** Tampilkan panel bila sedang terlipat (tanpa menutupnya bila sudah terbuka). */
  const reveal = useCallback(
    (id: PanelId) => {
      if (lastApplied.current.has(id)) toggle(id);
    },
    [toggle],
  );

  const openTool = useCallback(
    (tool: ToolId) => {
      const sameAndOpen = bridge.state().activeTool === tool && !lastApplied.current.has("tools");
      if (sameAndOpen) {
        toggle("tools"); // klik ikon yang sedang aktif = tutup panel
        return;
      }
      bridge.update({ activeTool: tool });
      if (lastApplied.current.has("tools")) toggle("tools");
    },
    [bridge, toggle],
  );

  const reset = useCallback(() => {
    userCollapsed.current.clear();
    protectedId.current = null;
    persistCollapsed();
    group.current?.setLayout(DEFAULT_LAYOUT);
    apply();
  }, [apply, group, persistCollapsed]);

  // Pasang aksi ke jembatan + daftarkan perintah panel selama tata letak terpasang.
  useEffect(() => {
    bridge.setActions({ toggle, openTool, reveal, reset });
    const defs = [
      { id: "view.toggleSidebar", titleKey: "cmd.toggleSidebar", shortcut: "Ctrl+B", run: () => toggle("sidebar") },
      { id: "view.toggleDocs", titleKey: "cmd.toggleDocs", shortcut: "Ctrl+Alt+B", run: () => toggle("docs") },
      { id: "view.toggleTools", titleKey: "cmd.toggleTools", shortcut: "Ctrl+J", run: () => toggle("tools") },
      { id: "view.resetLayout", titleKey: "cmd.resetLayout", run: reset },
    ];
    const offs = defs.map((d) => commands.register({ ...d, group: "view" }));
    return () => {
      for (const off of offs) off();
      bridge.setActions(null);
    };
  }, [bridge, commands, toggle, openTool, reveal, reset]);

  // Hitung ulang saat ukuran grup berubah, skala UI berubah, atau saat pertama muncul.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `scale` memicu perhitungan ulang (rem berubah)
  useEffect(() => {
    apply();
    const el = groupEl.current;
    if (!el) return;
    const ro = new ResizeObserver(() => apply());
    ro.observe(el);
    const raf = requestAnimationFrame(() => apply());
    return () => {
      ro.disconnect();
      cancelAnimationFrame(raf);
    };
  }, [apply, scale]);

  // Perubahan yang dimulai pengguna (seret separator) dicatat sebagai pilihan pengguna.
  const onChanged = useCallback(
    (layout: Record<string, number>, meta: { isUserInteraction: boolean }) => {
      onLayoutChanged(layout, meta as never);
      if (!meta.isUserInteraction) {
        // Pustaka bisa melipat/membuka panel sendiri (mis. jendela menyempit); koreksi sesuai rencana.
        apply(true);
        return;
      }
      for (const def of PANEL_DEFS) {
        if (!def.collapsible) continue;
        const actual = refs[def.id].current?.isCollapsed() ?? false;
        if (actual === lastApplied.current.has(def.id)) continue;
        if (actual) userCollapsed.current.add(def.id);
        else {
          userCollapsed.current.delete(def.id);
          protectedId.current = def.id;
        }
      }
      persistCollapsed();
      apply();
    },
    [onLayoutChanged, apply, persistCollapsed, refs],
  );

  const guarded = (id: PanelId, scope: string, children: ReactNode) => (
    <ErrorBoundary name={`panel.${id}`} logger={logger}>
      <ShortcutScope name={scope}>{children}</ShortcutScope>
    </ErrorBoundary>
  );

  const toolLabel = (tool: ToolId) => t(`tools.${tool}` as never);
  const toolContent = (tool: ToolId): ReactNode => {
    switch (tool) {
      case "terminal":
        return <TerminalPanel />;
      case "git":
        return <GitPanel />;
      case "browser":
        return <BrowserPanel />;
      case "info":
        return <InfoPanel />;
      case "log":
        return <ActivityPanel />;
    }
  };

  return (
    <div className="shell-body">
      <Group
        id={LAYOUT_ID}
        groupRef={group}
        elementRef={groupEl}
        defaultLayout={defaultLayout}
        onLayoutChanged={onChanged}
        className="shell-group"
      >
        <Panel
          id="sidebar"
          panelRef={refs.sidebar}
          collapsible
          minSize={`${MIN_REM.sidebar}rem`}
          defaultSize={PANEL_DEFS[0]?.defaultSize}
          className="shell-panel"
        >
          <section aria-label={t("panel.sidebar")} className="shell-panel__inner">
            {guarded("sidebar", "panel.sidebar", <Sidebar />)}
          </section>
        </Panel>
        <Separator id="sep-1" className="shell-sep" />
        <Panel
          id="chat"
          panelRef={refs.chat}
          minSize={`${MIN_REM.chat}rem`}
          defaultSize={PANEL_DEFS[1]?.defaultSize}
          className="shell-panel"
        >
          <section aria-label={t("panel.chat")} className="shell-panel__inner">
            {guarded(
              "chat",
              "panel.chat",
              <ChatPanel empty={<PanelEmpty title={t("empty.p2.title")} body={t("empty.p2.body")} />} />,
            )}
          </section>
        </Panel>
        <Separator id="sep-2" className="shell-sep" />
        <Panel
          id="docs"
          panelRef={refs.docs}
          collapsible
          minSize={`${MIN_REM.docs}rem`}
          defaultSize={PANEL_DEFS[2]?.defaultSize}
          className="shell-panel"
        >
          <section aria-label={t("panel.docs")} className="shell-panel__inner">
            {guarded(
              "docs",
              "panel.docs",
              <DocPanel empty={<PanelEmpty title={t("empty.p3.title")} body={t("empty.p3.body")} />} />,
            )}
          </section>
        </Panel>
        <Separator id="sep-3" className="shell-sep" />
        <Panel
          id="tools"
          panelRef={refs.tools}
          collapsible
          minSize={`${MIN_REM.tools}rem`}
          defaultSize={PANEL_DEFS[3]?.defaultSize}
          className="shell-panel"
        >
          <section aria-label={t("panel.tools")} className="shell-panel__inner">
            {guarded(
              "tools",
              "panel.tools",
              <Tabs
                aria-label={t("panel.tools")}
                value={panelState.activeTool}
                onValueChange={(v) => bridge.update({ activeTool: v as ToolId })}
                items={TOOL_IDS.map((tool) => ({
                  value: tool,
                  label: toolLabel(tool),
                  content: toolContent(tool),
                }))}
              />,
            )}
          </section>
        </Panel>
      </Group>

      <nav className="shell-rail" aria-label={t("panel.rail")}>
        {TOOL_IDS.map((tool) => (
          <IconButton
            key={tool}
            label={toolLabel(tool)}
            icon={TOOL_ICONS[tool]}
            aria-pressed={panelState.activeTool === tool && !panelState.collapsed.tools}
            onClick={() => openTool(tool)}
          />
        ))}
      </nav>
    </div>
  );
}

/** Teks pintasan untuk tooltip/menu. */
export const shortcutText = (combo: string | undefined) => (combo ? formatCombo(combo) : undefined);

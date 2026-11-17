// Lima panel (ADR-015/025): P1 Sidebar | P2 Chat | P3 Dokumen | P4 Tools | P5 Rail.
// Auto-collapse ditentukan perencana murni (@zandor/core `desiredCollapsed`), bukan reaksi pustaka UI.
import { canHidePanel, desiredCollapsed, formatCombo, type PanelSpec, remToPx, smartLayout } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  ActivityIcon,
  DiffIcon,
  ErrorBoundary,
  FilesIcon,
  GitIcon,
  GlobeIcon,
  IconButton,
  InfoIcon,
  ListIcon,
  ServerIcon,
  ShortcutScope,
  TerminalIcon,
  TrajectoryIcon,
  useToast,
} from "@zandor/ui";
import { type ReactNode, useCallback, useEffect, useMemo, useRef } from "react";
import { Group, Panel, Separator, useDefaultLayout, useGroupRef, usePanelRef } from "react-resizable-panels";
import { ChatPanel } from "./ChatPanel";
import { DocPanel } from "./docs/DocPanel";
import { PanelTabBar } from "./PanelTabBar";
import { Sidebar } from "./Sidebar";
import { PANEL_IDS, type PanelId, TOOL_IDS, type ToolId, usePanelState, useServices, useSetting } from "./services";
import { ActivityPanel } from "./tools/ActivityPanel";
import { BrowserPanel } from "./tools/BrowserPanel";
import { PANEL_ANIM_EVENT } from "./tools/browser-bridge";
import { ChangesPanel } from "./tools/ChangesPanel";
import { GitPanel } from "./tools/GitPanel";
import { InfoPanel } from "./tools/InfoPanel";
import { JobsPanel } from "./tools/JobsPanel";
import { SessionActivityPanel } from "./tools/SessionActivityPanel";
import { SessionFilesPanel } from "./tools/SessionFilesPanel";
import { TerminalPanel } from "./tools/TerminalPanel";
import { TrajectoryPanel } from "./tools/TrajectoryPanel";

/** Panel yang dirender pustaka resizable (rail di kanan hanya bisa ditampilkan/disembunyikan). */
type ResizableId = Exclude<PanelId, "rail">;
type PanelDef = { id: ResizableId; minRem: number; collapsible: boolean; priority: number; defaultSize: string };

/** Lebar minimum dalam rem (ikut --ui-scale). Chat tidak bisa dilipat (ADR-015). */
export const PANEL_DEFS: PanelDef[] = [
  { id: "sidebar", minRem: 12, collapsible: true, priority: 2, defaultSize: "18%" },
  { id: "chat", minRem: 22, collapsible: true, priority: 99, defaultSize: "34%" },
  { id: "docs", minRem: 16, collapsible: true, priority: 1, defaultSize: "22%" },
  { id: "tools", minRem: 18, collapsible: true, priority: 3, defaultSize: "26%" },
];
const MIN_REM = Object.fromEntries(PANEL_DEFS.map((p) => [p.id, p.minRem])) as Record<ResizableId, number>;
const DEFAULT_LAYOUT = { sidebar: 18, chat: 34, docs: 22, tools: 26 };
/** Smart resize: lebar sidebar "secukupnya" (rem) dan batas proporsinya terhadap lebar grup. */
const SIDEBAR_SMART_REM = 16;
const SIDEBAR_SMART_MAX_RATIO = 0.22;
const MAIN_IDS: ResizableId[] = ["chat", "docs", "tools"];

/** Pintasan panel bernomor (1 Sidebar, 2 Chat, 3 Dokumen, 4 Tools, 5 Rel) dan smart resize (ADR-066). */
export const PANEL_SHORTCUTS: Record<PanelId, string> = {
  sidebar: "Ctrl+1",
  chat: "Ctrl+2",
  docs: "Ctrl+3",
  tools: "Ctrl+4",
  rail: "Ctrl+5",
};
export const SMART_RESIZE_SHORTCUT = "Ctrl+Alt+0";
const FALLBACK_SEPARATOR_PX = 6;
const MAX_LAYOUT_REAPPLY = 6;
/** Lama penanda animasi tata letak; sedikit lebih panjang dari transisi CSS. */
const LAYOUT_ANIM_MS = 320;
const COLLAPSED_KEY = "zandor.layout.userCollapsed";
const LAYOUT_ID = "zandor-main";

export const TOOL_ICONS: Record<ToolId, ReactNode> = {
  terminal: <TerminalIcon />,
  jobs: <ServerIcon />,
  git: <GitIcon />,
  changes: <DiffIcon />,
  files: <FilesIcon />,
  browser: <GlobeIcon />,
  info: <InfoIcon />,
  session: <ActivityIcon />,
  trajectory: <TrajectoryIcon />,
  log: <ListIcon />,
};

/** Pegangan batang tipis di dalam celah antarpanel, muncul saat separator disorot/diseret (tanda bisa di-resize). */
function SepGrip() {
  return <span className="shell-sep__grip" aria-hidden="true" />;
}

function PanelEmpty({ title, body }: { title: string; body: string }) {
  return (
    <div className="shell-empty">
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
  const toast = useToast();
  const ui = useRef({ toast, t });
  ui.current = { toast, t };

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

  /** Rencana smart resize (ADR-066) untuk himpunan panel terlihat tertentu; null bila tak ada panel utama. */
  const computeSmart = useCallback((visible: Set<string>) => {
    const el = groupEl.current;
    if (!el) return null;
    const root = rootFontPx();
    const sep = el.querySelector("[data-separator]:not([data-hidden])")?.getBoundingClientRect().width ?? 0;
    return smartLayout({
      availablePx: el.getBoundingClientRect().width,
      separatorPx: sep || FALLBACK_SEPARATOR_PX,
      visible,
      side: {
        id: "sidebar",
        targetPx: remToPx(SIDEBAR_SMART_REM, root),
        minPx: remToPx(MIN_REM.sidebar, root),
        maxRatio: SIDEBAR_SMART_MAX_RATIO,
      },
      main: MAIN_IDS.map((id) => ({ id, minPx: remToPx(MIN_REM[id], root) })),
    });
  }, []);

  /**
   * Animasi tampil/sembunyi panel (ADR-066). Tata letak akhir dihitung SEKALI lalu semua panel
   * bertransisi bersamaan ke sana (tanpa tahap "lipat dulu, bagi ulang kemudian"). Hanya panel yang
   * MENYEMPIT yang isinya dibekukan pada lebar px awal (terpotong, bukan terhimpit); panel yang
   * melebar mengikuti lebarnya langsung sehingga tidak ada lompatan di akhir. Panel yang ditutup memudar.
   */
  const animTimer = useRef<number | undefined>(undefined);
  /** Catat lebar isi panel + layout saat ini; panggil SEBELUM tata letak diubah. */
  const snapshot = useCallback(() => {
    const widths = new Map<HTMLElement, number>();
    for (const inner of groupEl.current?.querySelectorAll<HTMLElement>(".shell-panel__inner") ?? []) {
      widths.set(inner, inner.getBoundingClientRect().width);
    }
    return { widths, layout: group.current?.getLayout() ?? {} };
  }, [group]);
  const startAnim = useCallback(
    (snap: ReturnType<typeof snapshot>, target: Record<string, number> | null, closing: PanelId | null) => {
      const el = groupEl.current;
      if (!el) return;
      window.clearTimeout(animTimer.current);
      // Lebar AKHIR tiap panel (px) dari rencana: isi langsung ditata pada ukuran akhirnya satu kali,
      // lalu selama transisi panel hanya membuka/menutup tirai (overflow hidden) — tidak ada isi yang
      // terhimpit selama animasi maupun "lompat" di akhir, baik saat panel ditutup maupun dimunculkan.
      const visibleCount = target ? Object.values(target).filter((v) => v > 0).length : 0;
      const sep =
        el.querySelector("[data-separator]:not([data-hidden])")?.getBoundingClientRect().width || FALLBACK_SEPARATOR_PX;
      const content = el.getBoundingClientRect().width - sep * Math.max(0, visibleCount - 1);
      for (const [inner, w] of snap.widths) {
        const id = inner.dataset.zPanel ?? "";
        const pct = target?.[id] ?? 0;
        if (pct > 0) inner.style.width = `${(pct / 100) * content}px`;
        else if (w > 0) inner.style.width = `${w}px`; // panel yang ditutup: pertahankan lebar lamanya
        // panel yang ditutup (termasuk yang ikut terlipat otomatis) memudar
        inner.toggleAttribute("data-closing", id === closing || (!!target && w > 0 && pct === 0));
      }
      el.dataset.animating = "";
      window.dispatchEvent(new CustomEvent(PANEL_ANIM_EVENT, { detail: { active: true } }));
      animTimer.current = window.setTimeout(() => {
        delete el.dataset.animating;
        window.dispatchEvent(new CustomEvent(PANEL_ANIM_EVENT, { detail: { active: false } }));
        for (const inner of el.querySelectorAll<HTMLElement>(".shell-panel__inner")) {
          inner.style.width = "";
          inner.removeAttribute("data-closing");
        }
      }, LAYOUT_ANIM_MS);
    },
    [],
  );

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
      // Aturan: minimal satu panel utama (Chat/Dokumen/Tools) terlihat — sidebar/rel tidak boleh sendirian.
      if (MAIN_IDS.every((id) => desired.has(id))) {
        desired.delete("chat");
        userCollapsed.current.delete("chat");
      }
      for (const def of PANEL_DEFS) {
        if (!def.collapsible) continue;
        const ref = refs[def.id].current;
        if (!ref) continue;
        const isCollapsed = ref.isCollapsed();
        if (desired.has(def.id) && !isCollapsed) ref.collapse();
        else if (!desired.has(def.id) && isCollapsed) ref.expand();
      }
      lastApplied.current = desired;
      const collapsed = Object.fromEntries(
        PANEL_IDS.map((id) => [id, id === "rail" ? userCollapsed.current.has("rail") : desired.has(id)]),
      ) as Record<PanelId, boolean>;
      bridge.update({ collapsed });
    },
    [bridge, refs],
  );

  /** Panel resizable yang saat ini terlihat (menurut rencana terakhir yang diterapkan). */
  const visibleNow = useCallback(
    () => new Set<string>(PANEL_DEFS.map((p) => p.id).filter((id) => !lastApplied.current.has(id))),
    [],
  );

  const warnLastMain = useCallback(() => {
    ui.current.toast.show({ title: ui.current.t("panel.lastMain"), tone: "warning" });
  }, []);

  /** Tampilkan/sembunyikan panel resizable dengan animasi ke tata letak smart akhir. */
  const animateTo = useCallback(
    (closing: PanelId | null) => {
      const snap = snapshot();
      apply(); // rencana lipat (termasuk auto-collapse karena sempit) → lastApplied
      const target = computeSmart(visibleNow());
      startAnim(snap, target, closing);
      // timpa hasil lipat/buka pustaka dengan tata letak akhir di tick yang sama → satu transisi
      if (target) group.current?.setLayout(target);
    },
    [apply, computeSmart, group, snapshot, startAnim, visibleNow],
  );

  const toggle = useCallback(
    (id: PanelId) => {
      if (id === "rail") {
        // rail bukan panel resizable: cukup tampil/sembunyi sesuai pilihan pengguna
        if (userCollapsed.current.has("rail")) userCollapsed.current.delete("rail");
        else userCollapsed.current.add("rail");
        persistCollapsed();
        apply();
        // lebar grup baru final setelah rel dirender: bagi ulang di frame berikutnya, beranimasi
        requestAnimationFrame(() => animateTo(null));
        return;
      }
      const def = PANEL_DEFS.find((p) => p.id === id);
      if (!def?.collapsible) return;
      const isCollapsed = refs[id]?.current?.isCollapsed() ?? lastApplied.current.has(id);
      if (!isCollapsed && !canHidePanel(id, visibleNow())) {
        warnLastMain();
        return;
      }
      if (isCollapsed) {
        userCollapsed.current.delete(id);
        protectedId.current = id; // yang baru dibuka pengguna tidak boleh langsung dilipat otomatis
      } else {
        userCollapsed.current.add(id);
        if (protectedId.current === id) protectedId.current = null;
      }
      persistCollapsed();
      animateTo(isCollapsed ? null : id);
    },
    [animateTo, apply, persistCollapsed, refs, visibleNow, warnLastMain],
  );

  const closePanel = useCallback(
    (id: Exclude<PanelId, "rail">) => {
      if (!canHidePanel(id, visibleNow())) {
        warnLastMain();
        return;
      }
      userCollapsed.current.add(id);
      if (protectedId.current === id) protectedId.current = null;
      persistCollapsed();
      animateTo(id);
    },
    [animateTo, persistCollapsed, visibleNow, warnLastMain],
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

  /** Smart resize: bagi ulang lebar panel yang sedang terlihat menurut aturan `smartLayout` (ADR-066). */
  const smartResize = useCallback(() => {
    const layout = computeSmart(visibleNow());
    if (!layout || !group.current) return; // tidak terjadi: apply() selalu menjaga satu panel utama terlihat
    group.current.setLayout(layout);
    apply();
  }, [apply, computeSmart, group, visibleNow]);

  // Pasang aksi ke jembatan + daftarkan perintah panel selama tata letak terpasang.
  useEffect(() => {
    bridge.setActions({ toggle, close: closePanel, openTool, reveal, reset, smartResize });
    const panelCmd = (id: string, titleKey: string, panel: PanelId) => ({
      id,
      titleKey,
      shortcut: PANEL_SHORTCUTS[panel],
      run: () => toggle(panel),
    });
    const defs: Array<{ id: string; titleKey: string; shortcut?: string; run: () => void }> = [
      panelCmd("view.toggleSidebar", "cmd.toggleSidebar", "sidebar"),
      panelCmd("view.toggleChat", "cmd.toggleChat", "chat"),
      panelCmd("view.toggleDocs", "cmd.toggleDocs", "docs"),
      panelCmd("view.toggleTools", "cmd.toggleTools", "tools"),
      panelCmd("view.toggleRail", "cmd.toggleRail", "rail"),
      { id: "view.smartResize", titleKey: "cmd.smartResize", shortcut: SMART_RESIZE_SHORTCUT, run: smartResize },
      { id: "view.resetLayout", titleKey: "cmd.resetLayout", run: reset },
    ];
    const offs = defs.map((d) => commands.register({ ...d, group: "view" }));
    return () => {
      for (const off of offs) off();
      bridge.setActions(null);
    };
  }, [bridge, commands, toggle, closePanel, openTool, reveal, reset, smartResize]);

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
      case "jobs":
        return <JobsPanel />;
      case "git":
        return <GitPanel />;
      case "changes":
        return <ChangesPanel />;
      case "files":
        return <SessionFilesPanel />;
      case "browser":
        return <BrowserPanel />;
      case "session":
        return <SessionActivityPanel />;
      case "trajectory":
        return <TrajectoryPanel />;
      case "info":
        return <InfoPanel />;
      case "log":
        return <ActivityPanel />;
    }
  };

  // satu celah antarpanel: separator hanya tampil bila panel di kirinya terlihat dan ada panel terlihat sesudahnya
  const order: ResizableId[] = ["sidebar", "chat", "docs", "tools"];
  const hideSep = (i: number) => {
    const here = order[i];
    if (!here || panelState.collapsed[here]) return true;
    return order.slice(i + 1).every((id) => panelState.collapsed[id]);
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
          // panel tersembunyi tidak boleh terbuka akibat seret separator di sebelahnya
          disabled={panelState.collapsed.sidebar}
          minSize={`${MIN_REM.sidebar}rem`}
          defaultSize={PANEL_DEFS[0]?.defaultSize}
          className="shell-panel"
        >
          <section aria-label={t("panel.sidebar")} className="shell-panel__inner" data-z-panel="sidebar">
            {guarded("sidebar", "panel.sidebar", <Sidebar />)}
          </section>
        </Panel>
        <Separator id="sep-1" className="shell-sep" data-hidden={hideSep(0) ? "" : undefined}>
          <SepGrip />
        </Separator>
        <Panel
          id="chat"
          panelRef={refs.chat}
          collapsible
          // panel tersembunyi tidak boleh terbuka akibat seret separator di sebelahnya
          disabled={panelState.collapsed.chat}
          minSize={`${MIN_REM.chat}rem`}
          defaultSize={PANEL_DEFS[1]?.defaultSize}
          className="shell-panel"
        >
          <section aria-label={t("panel.chat")} className="shell-panel__inner" data-z-panel="chat" data-panel="chat">
            {guarded("chat", "panel.chat", <ChatPanel />)}
          </section>
        </Panel>
        <Separator id="sep-2" className="shell-sep" data-hidden={hideSep(1) ? "" : undefined}>
          <SepGrip />
        </Separator>
        <Panel
          id="docs"
          panelRef={refs.docs}
          collapsible
          // panel tersembunyi tidak boleh terbuka akibat seret separator di sebelahnya
          disabled={panelState.collapsed.docs}
          minSize={`${MIN_REM.docs}rem`}
          defaultSize={PANEL_DEFS[2]?.defaultSize}
          className="shell-panel"
        >
          <section aria-label={t("panel.docs")} className="shell-panel__inner" data-z-panel="docs">
            {guarded(
              "docs",
              "panel.docs",
              <DocPanel empty={<PanelEmpty title={t("empty.p3.title")} body={t("empty.p3.body")} />} />,
            )}
          </section>
        </Panel>
        <Separator id="sep-3" className="shell-sep" data-hidden={hideSep(2) ? "" : undefined}>
          <SepGrip />
        </Separator>
        <Panel
          id="tools"
          panelRef={refs.tools}
          collapsible
          // panel tersembunyi tidak boleh terbuka akibat seret separator di sebelahnya
          disabled={panelState.collapsed.tools}
          minSize={`${MIN_REM.tools}rem`}
          defaultSize={PANEL_DEFS[3]?.defaultSize}
          className="shell-panel"
        >
          <section aria-label={t("panel.tools")} className="shell-panel__inner" data-z-panel="tools">
            {guarded(
              "tools",
              "panel.tools",
              <div className="tools">
                <PanelTabBar closeLabel={t("panel.close.tools")} onClose={() => closePanel("tools")}>
                  <span className="tools__icon" aria-hidden="true">
                    {TOOL_ICONS[panelState.activeTool]}
                  </span>
                  <h2 className="z-label tools__title">{toolLabel(panelState.activeTool)}</h2>
                </PanelTabBar>
                {/* Alat dipilih lewat rail di kanan (tanpa tab di dalam panel) */}
                <div className="tools__body">{toolContent(panelState.activeTool)}</div>
              </div>,
            )}
          </section>
        </Panel>
      </Group>

      {!panelState.collapsed.rail && (
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
      )}
    </div>
  );
}

/** Teks pintasan untuk tooltip/menu. */
export const shortcutText = (combo: string | undefined) => (combo ? formatCombo(combo) : undefined);

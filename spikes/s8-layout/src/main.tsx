// Spike S8 — layout 5 panel + child webview + zoom `--ui-scale`. Halaman uji, bukan UI produk.
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { Group, Panel, Separator, useDefaultLayout, useGroupRef, usePanelRef } from "react-resizable-panels";
import { runSelftest } from "./selftest";
import { invoke } from "./tauri";

export const SCALES = [0.75, 0.875, 1, 1.125, 1.25, 1.375, 1.5];
/** Lebar minimum (rem → ikut --ui-scale). */
export const MIN = { p1: 12, p2: 22, p3: 16, p4: 18 } as const;
/** Urutan auto-collapse saat chat terlalu sempit. */
const COLLAPSE_ORDER = ["p3", "p1", "p4"] as const;
type PanelKey = keyof typeof MIN;

const readScale = () => {
  try {
    const v = Number(localStorage.getItem("zandor-s8-scale"));
    return SCALES.includes(v) ? v : 1;
  } catch {
    return 1;
  }
};

export type AppApi = {
  setScale: (s: number) => void;
  scale: () => number;
  group: ReturnType<typeof useGroupRef>;
  panels: Record<PanelKey, ReturnType<typeof usePanelRef>>;
  setOverlay: (open: boolean) => void;
  browserHost: () => HTMLDivElement | null;
  syncBounds: () => Promise<void>;
  autoCollapsed: string[];
};

function App() {
  const [scale, setScaleState] = useState(readScale);
  const [overlay, setOverlay] = useState(false);
  const group = useGroupRef();
  const panels = { p1: usePanelRef(), p2: usePanelRef(), p3: usePanelRef(), p4: usePanelRef() };
  const hostRef = useRef<HTMLDivElement>(null);
  const autoCollapsed = useRef<string[]>([]);
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: "zandor-s8-main", storage: localStorage });

  // --- zoom UI: token --ui-scale, root font-size → semua ukuran rem ikut berskala ---
  useLayoutEffect(() => {
    document.documentElement.style.setProperty("--ui-scale", String(scale));
    try { localStorage.setItem("zandor-s8-scale", String(scale)); } catch {}
  }, [scale]);
  const setScale = useCallback((s: number) => setScaleState(Math.min(1.5, Math.max(0.75, s))), []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey) return;
      const i = SCALES.indexOf(scale);
      if (e.key === "=" || e.key === "+") { e.preventDefault(); setScale(SCALES[Math.min(SCALES.length - 1, i + 1)]); }
      if (e.key === "-") { e.preventDefault(); setScale(SCALES[Math.max(0, i - 1)]); }
      if (e.key === "0") { e.preventDefault(); setScale(1); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [scale, setScale]);

  // --- sinkron bounds child webview dengan host di Panel 4 ---
  const syncBounds = useCallback(async () => {
    const el = hostRef.current;
    const p4Collapsed = panels.p4.current?.isCollapsed() ?? false;
    const r = el?.getBoundingClientRect();
    const visible = !!r && r.width > 2 && r.height > 2 && !p4Collapsed && !overlay;
    await invoke("browser_set_visible", { visible });
    if (visible && r) await invoke("browser_set_bounds", { x: r.left, y: r.top, w: r.width, h: r.height });
  }, [overlay]);
  useEffect(() => {
    let raf = 0;
    const schedule = () => { cancelAnimationFrame(raf); raf = requestAnimationFrame(() => void syncBounds()); };
    const ro = new ResizeObserver(schedule);
    if (hostRef.current) ro.observe(hostRef.current);
    window.addEventListener("resize", schedule);
    schedule();
    return () => { ro.disconnect(); window.removeEventListener("resize", schedule); cancelAnimationFrame(raf); };
  }, [syncBounds]);

  // --- auto-collapse: chat (P2) tidak boleh di bawah minimum ---
  const groupEl = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const check = () => {
      const remPx = parseFloat(getComputedStyle(document.documentElement).fontSize);
      const width = groupEl.current?.getBoundingClientRect().width ?? 0;
      const open = (Object.keys(MIN) as PanelKey[]).filter((k) => !panels[k].current?.isCollapsed());
      let need = open.reduce((s, k) => s + MIN[k] * remPx, 0) + 3 * 6;
      for (const k of COLLAPSE_ORDER) {
        if (need <= width) break;
        if (open.includes(k)) {
          panels[k].current?.collapse();
          autoCollapsed.current.push(k);
          need -= MIN[k] * remPx;
        }
      }
    };
    const ro = new ResizeObserver(() => requestAnimationFrame(check));
    if (groupEl.current) ro.observe(groupEl.current);
    return () => ro.disconnect();
  }, [scale]);

  const api: AppApi = {
    setScale,
    scale: () => scale,
    group,
    panels,
    setOverlay,
    browserHost: () => hostRef.current,
    syncBounds,
    autoCollapsed: autoCollapsed.current,
  };
  (window as any).__s8 = api;

  const toggle = (k: PanelKey) => {
    const p = panels[k].current;
    if (!p) return;
    p.isCollapsed() ? p.expand() : p.collapse();
  };

  return (
    <div className="app">
      <header className="topbar">
        <button title="Kembali">←</button>
        <button title="Maju">→</button>
        <div className="menu-anchor">
          <button onClick={() => setOverlay((o) => !o)} aria-expanded={overlay}>Menu</button>
        </div>
        <input className="omni" placeholder="Cari atau jalankan perintah…" />
        <span className="zoom">{Math.round(scale * 100)}%</span>
        {(["p1", "p3", "p4"] as PanelKey[]).map((k) => (
          <button key={k} onClick={() => toggle(k)}>{k.toUpperCase()}</button>
        ))}
      </header>
      {overlay && (
        <div className="overlay-menu" role="menu">
          <div className="item">Item menu yang menimpa Panel 4</div>
          <div className="item">Browser harus disembunyikan</div>
          <button onClick={() => setOverlay(false)}>Tutup</button>
        </div>
      )}
      <div className="body">
        <Group
          id="main"
          groupRef={group}
          elementRef={groupEl}
          defaultLayout={defaultLayout}
          onLayoutChanged={onLayoutChanged}
          className="group"
        >
          <Panel id="p1" panelRef={panels.p1} collapsible minSize={`${MIN.p1}rem`} defaultSize="18%" className="panel">
            <div className="pane">P1 · Sidebar</div>
          </Panel>
          <Separator className="sep" id="s1" />
          <Panel id="p2" panelRef={panels.p2} minSize={`${MIN.p2}rem`} defaultSize="34%" className="panel">
            <div className="pane">P2 · Chat</div>
          </Panel>
          <Separator className="sep" id="s2" />
          <Panel id="p3" panelRef={panels.p3} collapsible minSize={`${MIN.p3}rem`} defaultSize="22%" className="panel">
            <div className="pane">P3 · Dokumen</div>
          </Panel>
          <Separator className="sep" id="s3" />
          <Panel id="p4" panelRef={panels.p4} collapsible minSize={`${MIN.p4}rem`} defaultSize="26%" className="panel">
            <div className="pane p4">
              <div className="pane-head">P4 · Browser</div>
              <div className="browser-host" ref={hostRef} />
            </div>
          </Panel>
        </Group>
        <nav className="rail">
          <button onClick={() => toggle("p4")} title="Tools">⌘</button>
        </nav>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
invoke<string | null>("selftest_path").then((p) => p && setTimeout(() => runSelftest(), 300));

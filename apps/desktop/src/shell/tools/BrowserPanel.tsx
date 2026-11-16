// Panel 4 — Browser: child webview native (ADR-020), satu webview per tab (ADR-058). Webview anak berada DI ATAS UI, jadi
// hanya tab aktif yang tampil, diposisikan mengikuti elemen pemandu, dan disembunyikan saat ada overlay UI
// (dialog/menu/palet) agar tidak menutupinya. Konten remote tidak punya IPC.
import {
  addBrowserTab,
  type BrowserTabs,
  browserTabLabel,
  closeBrowserTab,
  initialBrowserTabs,
  MAX_BROWSER_TABS,
  normalizeBrowserUrl,
  remToPx,
  selectBrowserTab,
  setBrowserTabUrl,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  CloseIcon,
  ExternalLinkIcon,
  IconButton,
  Input,
  PlusIcon,
  RefreshIcon,
  useToast,
} from "@zandor/ui";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";
import { BROWSER_OPEN_EVENT, takePendingBrowserUrl } from "./browser-bridge";
import { ToolNotice } from "./ToolNotice";

type Core = typeof import("@tauri-apps/api/core");
type Mode = "checking" | "native" | "system" | "none";

// Bertahan antar pemasangan ulang panel (ganti tab alat): webview tidak dibuat ulang dan daftar tab diingat.
let store: BrowserTabs = initialBrowserTabs();
const created = new Set<string>();

const OVERLAY =
  '[data-shortcut-scope="modal"],[data-shortcut-scope="menu"],[data-shortcut-scope="palette"],[role="alertdialog"],.z-backdrop,.z-popup';
/** Di bawah ukuran ini (rem) webview disembunyikan, mis. saat panel Tools dilipat. */
const MIN_VISIBLE_REM = 3;

// Toast sengaja TIDAK termasuk: ia tidak memblokir dan muncul tiap notifikasi; menyembunyikan webview selama 6–10 detik
// membuat halaman yang sedang dijelajahi menghilang/ter-render ulang. Konsekuensinya toast yang beririsan dengan webview
// tertutup webview (notifikasi sistem dan suara tetap berjalan).

export function BrowserPanel() {
  const { t } = useI18n();
  const toast = useToast();
  const host = useRef<HTMLDivElement>(null);
  const core = useRef<Core | null>(null);
  const [mode, setMode] = useState<Mode>("checking");
  const [tabs, setTabsState] = useState<BrowserTabs>(store);
  const activeTab = tabs.tabs.find((x) => x.id === tabs.active);
  const [input, setInput] = useState(activeTab?.url ?? "");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const activeRef = useRef(tabs.active);
  activeRef.current = tabs.active;

  const setTabs = useCallback((next: BrowserTabs) => {
    store = next;
    setTabsState(next);
  }, []);

  const call = useCallback(async (cmd: string, args?: Record<string, unknown>) => {
    const c = core.current;
    if (!c) throw new Error("IPC belum siap");
    return c.invoke(cmd, args);
  }, []);

  const bounds = useCallback(() => {
    const r = host.current?.getBoundingClientRect();
    return r ? { x: r.left, y: r.top, w: r.width, h: r.height } : null;
  }, []);

  // pilih mode
  useEffect(() => {
    let alive = true;
    void (async () => {
      const c = await import("@tauri-apps/api/core");
      if (!alive) return;
      if (!c.isTauri()) {
        setMode("none");
        return;
      }
      core.current = c;
      try {
        const m = await c.invoke<string>("browser_mode");
        if (alive) setMode(m === "child-webview" ? "native" : "system");
      } catch {
        if (alive) setMode("system");
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  // tampilkan/buat webview tab aktif (tab lain disembunyikan oleh Rust)
  // biome-ignore lint/correctness/useExhaustiveDependencies: hanya mode dan tab aktif yang relevan
  useEffect(() => {
    if (mode !== "native") return;
    const id = tabs.active;
    const tab = store.tabs.find((x) => x.id === id);
    setInput(tab?.url ?? "");
    setError(null);
    const b0 = bounds();
    if (!b0 || !tab) return;
    let alive = true;
    const wanted = takePendingBrowserUrl(); // URL yang diminta bagian lain aplikasi (mis. pratinjau HTML)
    void call("browser_open", { tab: id, url: wanted ?? (created.has(id) ? null : tab.url || null), ...b0 })
      .then(() => {
        created.add(id);
        const b = bounds();
        if (b && alive) void call("browser_set_bounds", { tab: id, ...b }).catch(() => {}); // mitigasi race cold-start (ADR-020)
      })
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, [mode, tabs.active]);

  // posisi, overlay (airspace), URL masuk, dan peristiwa muat halaman
  // biome-ignore lint/correctness/useExhaustiveDependencies: efek dipasang sekali per mode native
  useEffect(() => {
    if (mode !== "native") return;
    let alive = true;
    let raf = 0;
    // Webview native selalu di atas HTML: saat panel dilipat/disempitkan sampai hampir nol, host masih punya beberapa
    // piksel dan webview akan tampil sebagai potongan tipis. Sembunyikan bila host terlalu kecil atau ada overlay.
    let hidden = false; // tampak/tersembunyi yang terakhir dikirim ke Rust
    let overlay = false;
    let tiny = false;
    const applyVisible = () => {
      const want = !(overlay || tiny);
      if (want === !hidden) return;
      hidden = !want;
      void call("browser_visible", { tab: activeRef.current, visible: want }).catch(() => {});
    };
    const sync = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const b = bounds();
        if (!b || !alive) return;
        const rootPx = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
        const minPx = remToPx(MIN_VISIBLE_REM, rootPx);
        tiny = b.w < minPx || b.h < minPx;
        applyVisible();
        if (!tiny) void call("browser_set_bounds", { tab: activeRef.current, ...b }).catch(() => {});
      });
    };
    const ro = new ResizeObserver(sync);
    if (host.current) ro.observe(host.current);
    window.addEventListener("resize", sync);
    // permintaan buka URL saat panel sudah terpasang → tab aktif
    const onOpen = () => {
      const u = takePendingBrowserUrl();
      if (u)
        void call("browser_navigate", { tab: activeRef.current, url: u }).catch((e) => alive && setError(String(e)));
    };
    window.addEventListener(BROWSER_OPEN_EVENT, onOpen);
    // overlay UI → sembunyikan webview aktif (airspace)
    const mo = new MutationObserver(() => {
      overlay = document.querySelector(OVERLAY) !== null;
      applyVisible();
    });
    mo.observe(document.body, { childList: true, subtree: true });
    let off: (() => void) | null = null;
    void import("@tauri-apps/api/event").then(async ({ listen }) => {
      const un = await listen<{ tab: string; url: string; finished: boolean }>("browser-load", (e) => {
        if (!alive) return;
        setTabs(setBrowserTabUrl(store, e.payload.tab, e.payload.url));
        if (e.payload.tab === activeRef.current) {
          setInput(e.payload.url);
          setLoading(!e.payload.finished);
          sync();
        }
      });
      if (alive) off = un;
      else un();
    });
    return () => {
      alive = false;
      cancelAnimationFrame(raf);
      ro.disconnect();
      mo.disconnect();
      window.removeEventListener("resize", sync);
      window.removeEventListener(BROWSER_OPEN_EVENT, onOpen);
      off?.();
      void call("browser_visible", { tab: activeRef.current, visible: false }).catch(() => {});
    };
  }, [mode]);

  const go = async (e?: FormEvent) => {
    e?.preventDefault();
    setError(null);
    try {
      if (mode === "native") {
        const final = await call("browser_navigate", { tab: tabs.active, url: input });
        setTabs(setBrowserTabUrl(store, tabs.active, String(final)));
      } else {
        const n = normalizeBrowserUrl(input);
        if (!n.ok) throw new Error(`${t("br.badUrl")} (${n.error})`);
        if (mode === "system") await call("open_external", { url: n.url });
        else window.open(n.url, "_blank", "noopener,noreferrer");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };
  const history = (action: "back" | "forward" | "reload") =>
    void call("browser_history", { tab: tabs.active, action }).catch((e) =>
      toast.show({ title: String(e), tone: "danger" }),
    );
  const external = () => {
    const u = activeTab?.url || input;
    if (!/^https:\/\//i.test(u)) return;
    void call("open_external", { url: u }).catch((e) => toast.show({ title: String(e), tone: "danger" }));
  };
  const newTab = () => setTabs(addBrowserTab(store));
  const closeTab = (id: string) => {
    void call("browser_close", { tab: id }).catch(() => {});
    created.delete(id);
    setTabs(closeBrowserTab(store, id));
  };

  return (
    <div className="brw">
      {mode === "native" && (
        <nav className="stabs brw__tabs" aria-label={t("br.tabs")}>
          {tabs.tabs.map((tab) => {
            const title = browserTabLabel(tab.url) || t("br.newTab");
            const active = tab.id === tabs.active;
            return (
              <div key={tab.id} className="stabs__tab" data-active={active ? "" : undefined}>
                <button
                  type="button"
                  className="stabs__label"
                  aria-current={active ? "true" : undefined}
                  onClick={() => setTabs(selectBrowserTab(store, tab.id))}
                >
                  {title}
                </button>
                <button
                  type="button"
                  className="stabs__close"
                  aria-label={t("br.closeTab", { title })}
                  onClick={() => closeTab(tab.id)}
                >
                  <CloseIcon size="sm" />
                </button>
              </div>
            );
          })}
          <IconButton
            label={t("br.newTabAction")}
            size="sm"
            icon={<PlusIcon />}
            disabled={tabs.tabs.length >= MAX_BROWSER_TABS}
            onClick={newTab}
          />
        </nav>
      )}
      <form className="brw__bar" onSubmit={(e) => void go(e)}>
        <IconButton
          label={t("br.back")}
          size="sm"
          icon={<ArrowLeftIcon />}
          disabled={mode !== "native"}
          onClick={() => history("back")}
        />
        <IconButton
          label={t("br.forward")}
          size="sm"
          icon={<ArrowRightIcon />}
          disabled={mode !== "native"}
          onClick={() => history("forward")}
        />
        <IconButton
          label={t("br.reload")}
          size="sm"
          icon={<RefreshIcon />}
          disabled={mode !== "native"}
          onClick={() => history("reload")}
        />
        <Input
          aria-label={t("br.address")}
          placeholder={t("br.address")}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          className="brw__url"
        />
        <IconButton
          label={t("br.external")}
          size="sm"
          icon={<ExternalLinkIcon />}
          disabled={mode === "none" || mode === "checking"}
          onClick={external}
        />
      </form>
      {/* Bilah progres tipis (tinggi tetap → webview tidak bergeser saat memuat); teks hanya untuk pembaca layar */}
      <div className="brw__progress" data-loading={loading ? "" : undefined}>
        {loading && (
          <span className="z-visually-hidden" role="status">
            {t("br.loading")}
          </span>
        )}
      </div>
      {error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {error}
        </p>
      )}
      {mode === "none" && <ToolNotice inline>{t("br.unavailable")}</ToolNotice>}
      {mode === "system" && <ToolNotice inline>{t("br.systemFallback")}</ToolNotice>}
      <div className="brw__host" ref={host} data-testid="browser-host" data-mode={mode} />
    </div>
  );
}

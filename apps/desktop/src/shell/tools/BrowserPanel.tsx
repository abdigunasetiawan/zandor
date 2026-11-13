// Panel 4 — Browser: child webview native (ADR-020). Webview anak berada DI ATAS UI, jadi diposisikan mengikuti elemen
// pemandu dan disembunyikan saat ada overlay UI (dialog/menu/palet) agar tidak menutupinya. Konten remote tidak punya IPC.
import { normalizeBrowserUrl } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { ArrowLeftIcon, ArrowRightIcon, ExternalLinkIcon, IconButton, Input, RefreshIcon, useToast } from "@zandor/ui";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

type Core = typeof import("@tauri-apps/api/core");
type Mode = "checking" | "native" | "system" | "none";

// Bertahan antar pemasangan ulang panel (ganti tab alat): webview tidak dibuat ulang dan URL terakhir diingat.
let created = false;
let lastUrl = "";

const OVERLAY =
  '[data-shortcut-scope="modal"],[data-shortcut-scope="menu"],[data-shortcut-scope="palette"],[role="alertdialog"],.z-backdrop,.z-popup,.z-toast';

export function BrowserPanel() {
  const { t } = useI18n();
  const toast = useToast();
  const host = useRef<HTMLDivElement>(null);
  const core = useRef<Core | null>(null);
  const [mode, setMode] = useState<Mode>("checking");
  const [input, setInput] = useState(lastUrl);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  // buka/posisikan webview native
  // biome-ignore lint/correctness/useExhaustiveDependencies: efek dipasang sekali per mode native
  useEffect(() => {
    if (mode !== "native") return;
    let alive = true;
    let raf = 0;
    const sync = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const b = bounds();
        if (b && alive) void call("browser_set_bounds", b).catch(() => {});
      });
    };
    const b0 = bounds();
    if (b0) {
      void call("browser_open", { url: created ? null : lastUrl || null, ...b0 })
        .then(() => {
          created = true;
          sync(); // sinkron ulang setelah dibuat (mitigasi race cold-start, ADR-020 temuan 1)
        })
        .catch((e) => alive && setError(String(e)));
    }
    const ro = new ResizeObserver(sync);
    if (host.current) ro.observe(host.current);
    window.addEventListener("resize", sync);
    // overlay UI → sembunyikan webview (airspace)
    let hidden = false;
    const mo = new MutationObserver(() => {
      const overlay = document.querySelector(OVERLAY) !== null;
      if (overlay === hidden) return;
      hidden = overlay;
      void call("browser_visible", { visible: !overlay }).catch(() => {});
    });
    mo.observe(document.body, { childList: true, subtree: true });
    let off: (() => void) | null = null;
    void import("@tauri-apps/api/event").then(async ({ listen }) => {
      const un = await listen<{ url: string; finished: boolean }>("browser-load", (e) => {
        if (!alive) return;
        lastUrl = e.payload.url;
        setInput(e.payload.url);
        setLoading(!e.payload.finished);
        sync();
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
      off?.();
      void call("browser_visible", { visible: false }).catch(() => {});
    };
  }, [mode]);

  const go = async (e?: FormEvent) => {
    e?.preventDefault();
    setError(null);
    try {
      if (mode === "native") {
        const final = await call("browser_navigate", { url: input });
        lastUrl = String(final);
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
    void call("browser_history", { action }).catch((e) => toast.show({ title: String(e), tone: "danger" }));
  const external = () => {
    const u = lastUrl || input;
    if (!/^https:\/\//i.test(u)) return;
    void call("open_external", { url: u }).catch((e) => toast.show({ title: String(e), tone: "danger" }));
  };

  return (
    <div className="brw">
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
      {loading && (
        <p className="z-small brw__note" role="status">
          {t("br.loading")}
        </p>
      )}
      {error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {error}
        </p>
      )}
      {mode === "none" && <p className="sidebar__note z-small">{t("br.unavailable")}</p>}
      {mode === "system" && <p className="sidebar__note z-small">{t("br.systemFallback")}</p>}
      <div className="brw__host" ref={host} data-testid="browser-host" data-mode={mode} />
    </div>
  );
}

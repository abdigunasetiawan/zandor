// Panel 4 — Terminal: xterm.js terhubung ke PTY engine (ADR-024). Terminal mengikuti project aktif (cwd = folder project;
// sesi tanpa project memakai folder Chat). Tab terminal hidup di engine, jadi muat ulang aplikasi memulihkannya dengan
// riwayat (engine memutar ulang output saat WebSocket tersambung).

import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { Terminal } from "@xterm/xterm";
import {
  CHAT_KEY,
  type DetectedShell,
  type PtySocket,
  pickShell,
  type ShellOption,
  shellId,
  type TerminalTab,
} from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import {
  Button,
  CloseIcon,
  FolderIcon,
  IconButton,
  PlusIcon,
  Select,
  Spinner,
  TerminalIcon,
  useToast,
} from "@zandor/ui";
import "@xterm/xterm/css/xterm.css";
import { useEffect, useMemo, useRef, useState } from "react";
import { useProjects, useServices, useSettings, useTerminals } from "../services";
import { ToolNotice } from "./ToolNotice";

const RECONNECT_MS = 1000;
const MAX_RECONNECTS = 5;

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Ikon shell: ikon asli exe (dari deteksi aplikasi desktop) atau ikon terminal generik. */
function ShellIcon({ shell }: { shell: ShellOption | undefined }) {
  if (shell?.icon) return <img className="terms__appicon" src={`data:image/png;base64,${shell.icon}`} alt="" />;
  return <TerminalIcon size="sm" />;
}

// Deteksi shell terpasang cukup sekali per jalannya aplikasi (Rust juga meng-cache; skrip butuh ~1 detik).
let detectStarted = false;
function useDetectShells() {
  const { terminals } = useServices();
  useEffect(() => {
    if (detectStarted) return;
    detectStarted = true;
    void (async () => {
      const c = await import("@tauri-apps/api/core");
      if (!c.isTauri()) return;
      try {
        terminals.setDetectedShells(await c.invoke<DetectedShell[]>("terminal_shells", { refresh: false }));
      } catch {
        detectStarted = false; // gagal: coba lagi saat panel dibuka berikutnya; daftar engine tetap dipakai
      }
    })();
  }, [terminals]);
}

/** Satu terminal xterm yang tersambung ke satu PTY. Tetap terpasang saat tab lain aktif (disembunyikan). */
function TerminalView({ tab, visible }: { tab: TerminalTab; visible: boolean }) {
  const { t } = useI18n();
  const { projects, terminals } = useServices();
  const host = useRef<HTMLDivElement>(null);
  const fitRef = useRef<{ fit: () => void } | null>(null);
  const [state, setState] = useState<"connecting" | "ready" | "reconnecting" | "ended">("connecting");
  const engineReady = useProjects().engineReady;
  const { terminalFontSize } = useSettings();
  const fontSizeRef = useRef(terminalFontSize);
  fontSizeRef.current = terminalFontSize;
  const termRef = useRef<Terminal | null>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: dibuat sekali per tab; `visible` ditangani efek lain
  useEffect(() => {
    const api = projects.engineApi();
    const el = host.current;
    if (!api || !el || tab.status === "exited") {
      if (tab.status === "exited") setState("ended");
      return;
    }
    const term = new Terminal({
      cursorBlink: true,
      allowProposedApi: true,
      fontFamily: cssVar("--z-font-terminal") || "monospace",
      fontSize: fontSizeRef.current,
      scrollback: 5000,
      theme: { background: cssVar("--z-color-bg"), foreground: cssVar("--z-color-fg"), cursor: cssVar("--z-color-fg") },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    term.open(el);
    termRef.current = term;
    fitRef.current = fit;
    let disposed = false;
    let socket: PtySocket | null = null;
    let cursor = 0;
    let attempts = 0;

    const sizeSafe = () => {
      try {
        fit.fit();
      } catch {
        // elemen belum punya ukuran (tersembunyi): diulang saat terlihat
      }
    };
    sizeSafe();

    const connect = async (replay: boolean) => {
      try {
        if (replay) term.reset(); // riwayat diputar ulang penuh oleh engine
        const s = await api.pty.connect(tab.cwd, tab.id, replay ? 0 : cursor);
        if (disposed) {
          s.close();
          return;
        }
        socket = s;
        attempts = 0;
        s.onData((c) => term.write(c));
        s.onCursor((n) => {
          cursor = n;
        });
        s.onClose(() => void onClosed());
        setState("ready");
        void api.pty.resize(tab.cwd, tab.id, term.cols, term.rows).catch(() => {});
      } catch {
        await onClosed();
      }
    };
    const onClosed = async () => {
      if (disposed) return;
      socket = null;
      // PTY sudah selesai (shell keluar) atau hanya koneksi yang putus?
      let alive = true;
      try {
        alive = (await api.pty.list(tab.cwd)).some((p) => p.id === tab.id && p.status === "running");
      } catch {
        alive = attempts < MAX_RECONNECTS;
      }
      if (disposed) return;
      if (!alive) {
        term.write(`\r\n\x1b[2m${t("tm.exited")}\x1b[0m\r\n`);
        setState("ended");
        terminals.markExited(tab.id);
        return;
      }
      if (attempts >= MAX_RECONNECTS) {
        setState("ended");
        return;
      }
      attempts++;
      setState("reconnecting");
      setTimeout(() => void connect(true), RECONNECT_MS);
    };

    const sub1 = term.onData((d) => socket?.send(d));
    const sub2 = term.onResize(({ cols, rows }) => void api.pty.resize(tab.cwd, tab.id, cols, rows).catch(() => {}));
    const ro = new ResizeObserver(() => sizeSafe());
    ro.observe(el);
    void connect(true);
    return () => {
      disposed = true;
      ro.disconnect();
      sub1.dispose();
      sub2.dispose();
      socket?.close();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, [tab.id, tab.cwd, engineReady]);

  useEffect(() => {
    if (visible) requestAnimationFrame(() => fitRef.current?.fit());
  }, [visible]);

  // ukuran huruf berubah di Pengaturan → terapkan langsung ke terminal yang sudah terbuka
  useEffect(() => {
    if (!termRef.current) return;
    termRef.current.options.fontSize = terminalFontSize;
    requestAnimationFrame(() => fitRef.current?.fit());
  }, [terminalFontSize]);

  return (
    <div className="term" data-visible={visible ? "" : undefined} hidden={!visible}>
      {state === "reconnecting" && <p className="term__note z-small">{t("tm.reconnecting")}</p>}
      {state === "connecting" && (
        <p className="term__note z-small">
          <Spinner size="sm" /> {t("tm.connecting")}
        </p>
      )}
      <div className="term__host" ref={host} data-testid="terminal" />
    </div>
  );
}

export function TerminalPanel() {
  const { t } = useI18n();
  const { terminals } = useServices();
  const toast = useToast();
  const proj = useProjects();
  const term = useTerminals();
  const [shell, setShell] = useState<string | null>(null);
  useDetectShells();

  // konteks: project terpilih, atau folder Chat
  const ctx = useMemo(() => {
    const key = proj.selection.key ?? CHAT_KEY;
    const project = proj.projects.find((p) => p.id === key);
    // sesi Chat punya folder kerja sendiri: terminal mengikuti sesi terpilih (folder Chat bila belum ada sesi)
    const chatSession =
      !project && key === CHAT_KEY && proj.selection.sessionId
        ? proj.sessions[CHAT_KEY]?.items.find((x) => x.id === proj.selection.sessionId)
        : undefined;
    const cwd = project?.path ?? (key === CHAT_KEY ? (chatSession?.directory ?? proj.chatDir) : null);
    return {
      key: project ? key : chatSession ? `chat:${chatSession.id}` : CHAT_KEY,
      cwd,
      name: project?.name ?? t("sb.chatGroup"),
    };
  }, [proj.selection.key, proj.selection.sessionId, proj.sessions, proj.projects, proj.chatDir, t]);

  // adopsi PTY yang masih hidup di engine untuk konteks ini (setelah muat ulang aplikasi)
  // biome-ignore lint/correctness/useExhaustiveDependencies: dijalankan saat konteks/engine berubah
  useEffect(() => {
    if (proj.engineReady && ctx.cwd) void terminals.adopt(ctx.key, ctx.cwd);
  }, [proj.engineReady, ctx.key, ctx.cwd]);

  const tabs = term.tabs.filter((x) => x.key === ctx.key);
  const active = term.activeByKey[ctx.key] ?? tabs.at(-1)?.id;
  const usable = term.shells.filter((s) => s.acceptable);
  const fallback = pickShell(term.shells);
  const current = usable.find((s) => shellId(s) === shell) ?? fallback ?? undefined;
  // tab → shell untuk ikon: cocokkan exe; bila exe sama untuk beberapa pilihan (distro WSL), cocokkan juga nama di judul
  const shellOfTab = (x: TerminalTab) => {
    const same = term.shells.filter((s) => x.command && s.path.toLowerCase() === x.command.toLowerCase());
    return same.find((s) => x.title.startsWith(`${s.name} `)) ?? same[0];
  };

  const create = async () => {
    if (!ctx.cwd) return;
    try {
      await terminals.create(ctx.key, ctx.cwd, current ? shellId(current) : undefined);
    } catch (e) {
      toast.show({ title: t("tm.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });
    }
  };

  if (!proj.engineReady) return <ToolNotice icon={<TerminalIcon size="lg" />}>{t("tm.noEngine")}</ToolNotice>;
  if (!ctx.cwd) return <ToolNotice icon={<TerminalIcon size="lg" />}>{t("tm.noContext")}</ToolNotice>;

  return (
    <div className="terms">
      {/* Bilah konteks: folder kerja (kiri, memotong dengan elipsis) + pemilih shell dan terminal baru (kanan) */}
      <div className="terms__head">
        <p className="terms__cwd z-small" title={ctx.cwd}>
          <FolderIcon size="sm" />
          <span className="terms__name">{ctx.name}</span>
          <span className="terms__cwdtext">{ctx.cwd}</span>
        </p>
        {usable.length > 1 && (
          <Select
            aria-label={t("tm.shell")}
            size="sm"
            className="terms__shell"
            icon={<ShellIcon shell={current} />}
            items={usable.map((s) => ({
              value: shellId(s),
              label: s.name,
              icon: <ShellIcon shell={s} />,
            }))}
            value={current ? shellId(current) : null}
            onValueChange={setShell}
          />
        )}
        <IconButton label={t("tm.new")} size="sm" icon={<PlusIcon />} onClick={() => void create()} />
      </div>
      {tabs.length > 0 && (
        <nav className="terms__tabs" aria-label={t("tm.tabs")}>
          {tabs.map((x) => (
            <div key={x.id} className="stabs__tab" data-active={x.id === active ? "" : undefined}>
              <button
                type="button"
                className="stabs__label"
                aria-current={x.id === active ? "true" : undefined}
                onClick={() => terminals.setActive(ctx.key, x.id)}
              >
                <ShellIcon shell={shellOfTab(x)} />
                <span className="stabs__text">{x.title}</span>
                {x.status === "exited" && (
                  <span className="z-small terms__exited" title={t("tm.exited")}>
                    ✕
                  </span>
                )}
              </button>
              <button
                type="button"
                className="stabs__close"
                aria-label={t("tm.close", { title: x.title })}
                onClick={() => void terminals.close(x.id)}
              >
                <CloseIcon size="sm" />
              </button>
            </div>
          ))}
        </nav>
      )}
      {term.error && (
        <p className="chat__note chat__note--error z-small" role="alert">
          {term.error}
        </p>
      )}
      <div className="terms__views">
        {tabs.length === 0 && (
          <div className="terms__empty">
            <TerminalIcon size="lg" />
            <p className="z-p">{t("tm.empty")}</p>
            <Button variant="solid" onClick={() => void create()}>
              {t("tm.new")}
            </Button>
          </div>
        )}
        {term.tabs.map((x) => (
          <TerminalView key={x.id} tab={x} visible={x.key === ctx.key && x.id === active} />
        ))}
      </div>
    </div>
  );
}

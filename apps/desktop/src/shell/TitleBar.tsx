// Top bar (frameless): back/next, menu, omnibox (command palette), toggle panel, pengaturan, kontrol jendela.
import { formatCombo } from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  Button,
  ChatIcon,
  CloseIcon,
  IconButton,
  Kbd,
  MaximizeIcon,
  Menu,
  type MenuEntry,
  MinimizeIcon,
  PanelDocIcon,
  PanelLeftIcon,
  PanelRailIcon,
  PanelRightIcon,
  RestoreIcon,
  SearchIcon,
  SettingsIcon,
  SmartLayoutIcon,
} from "@zandor/ui";
import { useEffect, useState } from "react";
import { useCommandCenter } from "./CommandCenter";
import { PANEL_SHORTCUTS, SMART_RESIZE_SHORTCUT } from "./ShellLayout";
import { type PanelId, useCommandList, useNavState, usePanelState, useServices, useWindowApi } from "./services";

function WindowControls() {
  const { t } = useI18n();
  const api = useWindowApi();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!api) return;
    let off: (() => void) | undefined;
    let alive = true;
    const sync = () => void api.isMaximized().then((m) => alive && setMaximized(m));
    sync();
    void api.onResized(sync).then((fn) => {
      if (alive) off = fn;
      else fn();
    });
    return () => {
      alive = false;
      off?.();
    };
  }, [api]);

  if (!api) return null; // di luar Tauri tidak ada jendela yang bisa dikendalikan
  return (
    <fieldset className="titlebar__window" aria-label="Window">
      <IconButton label={t("window.minimize")} icon={<MinimizeIcon />} onClick={() => void api.minimize()} />
      <IconButton
        label={maximized ? t("window.restore") : t("window.maximize")}
        icon={maximized ? <RestoreIcon /> : <MaximizeIcon />}
        onClick={() => void api.toggleMaximize()}
      />
      <IconButton
        label={t("window.close")}
        icon={<CloseIcon />}
        className="titlebar__close"
        onClick={() => void api.close()}
      />
    </fieldset>
  );
}

const MENUS: Array<{ labelKey: MessageKey; ids: Array<string | "-"> }> = [
  { labelKey: "menu.file", ids: ["app.openSettings", "-", "window.close"] },
  {
    labelKey: "menu.view",
    ids: [
      "view.toggleSidebar",
      "view.toggleChat",
      "view.toggleDocs",
      "view.toggleTools",
      "view.toggleRail",
      "view.smartResize",
      "view.resetLayout",
      "-",
      "view.zoomIn",
      "view.zoomOut",
      "view.zoomReset",
      "-",
      "view.toggleTheme",
      "view.langId",
      "view.langEn",
    ],
  },
  { labelKey: "menu.help", ids: ["app.palette", "help.about"] },
  { labelKey: "menu.dev", ids: ["dev.copyChatHtml", "dev.openDevtools", "-", "dev.designSystem"] },
];

function MenuBar() {
  const { t } = useI18n();
  const { commands } = useServices();
  const list = useCommandList(); // render ulang saat perintah terdaftar/dilepas

  const entriesFor = (ids: Array<string | "-">): MenuEntry[] => {
    const out: MenuEntry[] = [];
    ids.forEach((id, i) => {
      if (id === "-") {
        out.push({ type: "separator", id: `sep-${i}` });
        return;
      }
      const c = list.find((x) => x.id === id);
      if (!c) return; // perintah belum terdaftar (mis. panel di halaman lain): jangan tampilkan yang palsu
      out.push({
        type: "item",
        id,
        label: t(c.titleKey as MessageKey),
        shortcut: c.shortcut ? formatCombo(c.shortcut) : undefined,
        disabled: !commands.isEnabled(id),
        onSelect: () => void commands.run(id),
      });
    });
    // Rapikan pemisah di ujung/ganda akibat item yang tidak ada.
    return out.filter(
      (e, i, arr) => !(e.type === "separator" && (i === 0 || i === arr.length - 1 || arr[i - 1]?.type === "separator")),
    );
  };

  return (
    <nav className="titlebar__menus" aria-label={t("menu.bar")}>
      {MENUS.map((m) => {
        const entries = entriesFor(m.ids);
        if (entries.length === 0) return null;
        return (
          <Menu
            key={m.labelKey}
            trigger={
              <Button variant="ghost" size="sm">
                {t(m.labelKey)}
              </Button>
            }
            entries={entries}
          />
        );
      })}
    </nav>
  );
}

const PANEL_TOGGLES: Array<{ id: PanelId; icon: React.ReactNode; labelKey: MessageKey }> = [
  { id: "sidebar", icon: <PanelLeftIcon size="sm" />, labelKey: "panel.sidebar" },
  { id: "chat", icon: <ChatIcon size="sm" />, labelKey: "panel.chat" },
  { id: "docs", icon: <PanelDocIcon size="sm" />, labelKey: "panel.docs" },
  { id: "tools", icon: <PanelRightIcon size="sm" />, labelKey: "panel.tools" },
  { id: "rail", icon: <PanelRailIcon size="sm" />, labelKey: "panel.rail" },
];

export function TitleBar() {
  const { t } = useI18n();
  const { nav, panels } = useServices();
  const navState = useNavState();
  const panelState = usePanelState();
  const { openPalette } = useCommandCenter();
  const actions = panels.actions;

  return (
    <header
      className="titlebar"
      data-tauri-drag-region
      data-surface={/^\/settings(\/|$)/.test(navState.current) ? "" : undefined}
    >
      <div className="titlebar__nav">
        <IconButton
          label={t("nav.back")}
          icon={<ArrowLeftIcon />}
          disabled={!navState.canBack}
          onClick={() => void nav.back()}
        />
        <IconButton
          label={t("nav.forward")}
          icon={<ArrowRightIcon />}
          disabled={!navState.canForward}
          onClick={() => void nav.forward()}
        />
      </div>
      <MenuBar />
      <div className="titlebar__center" data-tauri-drag-region>
        {/* di halaman Pengaturan pencarian ada di sidebar halaman itu sendiri; omnibox disembunyikan */}
        {!/^\/settings(\/|$)/.test(navState.current) && (
          <button
            type="button"
            className="omnibox"
            onClick={openPalette}
            aria-label={t("palette.label")}
            aria-haspopup="dialog"
          >
            <SearchIcon size="sm" />
            <span className="omnibox__text">{t("palette.placeholder")}</span>
            <Kbd keys="Ctrl+K" />
          </button>
        )}
      </div>
      <div className="titlebar__tools">
        {panelState.ready && actions && (
          <fieldset className="titlebar__toggles" aria-label={t("panel.toggles")}>
            <IconButton
              label={`${t("panel.smartResize")} (${formatCombo(SMART_RESIZE_SHORTCUT)})`}
              icon={<SmartLayoutIcon size="sm" />}
              size="sm"
              onClick={() => actions.smartResize()}
            />
            {PANEL_TOGGLES.map((p) => (
              <IconButton
                key={p.id}
                label={`${t(p.labelKey)} (${formatCombo(PANEL_SHORTCUTS[p.id])})`}
                icon={p.icon}
                size="sm"
                aria-pressed={!panelState.collapsed[p.id]}
                onClick={() => actions.toggle(p.id)}
              />
            ))}
          </fieldset>
        )}
        <IconButton label={t("settings.title")} icon={<SettingsIcon />} onClick={() => nav.push("/settings")} />
      </div>
      <WindowControls />
    </header>
  );
}

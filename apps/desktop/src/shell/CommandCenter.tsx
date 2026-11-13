// Pusat perintah: mendaftarkan perintah global, mengikat pintasannya, dan menampung palette + dialog Tentang.
import { type CommandDef, formatCombo, type Logger, searchCommands } from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import { Dialog, Palette, type PaletteItem, stepScale, useShortcut, useShortcutRegistry } from "@zandor/ui";
import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { build } from "../build";
import { useCommandList, useServices, useWindowApi } from "./services";

type CommandCenterValue = { openPalette: () => void };
const CommandCenterContext = createContext<CommandCenterValue | null>(null);

export function useCommandCenter(): CommandCenterValue {
  const v = useContext(CommandCenterContext);
  if (!v) throw new Error("harus dipakai di dalam <CommandCenter>");
  return v;
}

/** Mengikat `shortcut` tiap perintah ke ShortcutRegistry (scope global). Diperbarui saat daftar berubah. */
function useCommandShortcuts(logger: Logger) {
  const registry = useShortcutRegistry();
  const { commands } = useServices();
  const list = useCommandList();
  useEffect(() => {
    const offs: Array<() => void> = [];
    for (const c of list) {
      if (!c.shortcut) continue;
      try {
        offs.push(
          registry.register({
            id: `cmd:${c.id}`,
            combo: c.shortcut,
            scope: "global",
            description: c.titleKey,
            allowInInput: true,
            run: () => void commands.run(c.id),
          }),
        );
      } catch (e) {
        logger.error("pintasan perintah bentrok", { id: c.id, error: e });
      }
    }
    return () => {
      for (const off of offs) off();
    };
  }, [list, registry, commands, logger]);
}

export function CommandCenter({ children }: { children: ReactNode }) {
  const { t } = useI18n();
  const { commands, settings, nav, logger } = useServices();
  const windowApi = useWindowApi();
  const list = useCommandList();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [aboutOpen, setAboutOpen] = useState(false);

  const openPalette = useCallback(() => {
    setQuery("");
    setPaletteOpen(true);
  }, []);

  // Perintah global.
  useEffect(() => {
    const defs: CommandDef[] = [
      {
        id: "app.openSettings",
        titleKey: "cmd.openSettings",
        group: "app",
        shortcut: "Ctrl+,",
        run: () => nav.push("/settings"),
      },
      { id: "app.providers", titleKey: "cmd.providers", group: "app", run: () => nav.push("/providers") },
      { id: "app.memory", titleKey: "cmd.memory", group: "app", run: () => nav.push("/memory") },
      { id: "app.goHome", titleKey: "cmd.goHome", group: "app", run: () => nav.push("/") },
      {
        id: "nav.back",
        titleKey: "cmd.back",
        group: "app",
        shortcut: "Alt+ArrowLeft",
        enabled: () => nav.state().canBack,
        run: () => void nav.back(),
      },
      {
        id: "nav.forward",
        titleKey: "cmd.forward",
        group: "app",
        shortcut: "Alt+ArrowRight",
        enabled: () => nav.state().canForward,
        run: () => void nav.forward(),
      },
      { id: "app.palette", titleKey: "cmd.palette", group: "app", shortcut: "Ctrl+K", run: openPalette },
      {
        id: "view.zoomIn",
        titleKey: "cmd.zoomIn",
        group: "view",
        shortcut: "Ctrl+=",
        run: () => settings.set("uiScale", stepScale(settings.get("uiScale"), 1)),
      },
      {
        id: "view.zoomOut",
        titleKey: "cmd.zoomOut",
        group: "view",
        shortcut: "Ctrl+-",
        run: () => settings.set("uiScale", stepScale(settings.get("uiScale"), -1)),
      },
      {
        id: "view.zoomReset",
        titleKey: "cmd.zoomReset",
        group: "view",
        shortcut: "Ctrl+0",
        run: () => settings.set("uiScale", 1),
      },
      {
        id: "view.toggleTheme",
        titleKey: "cmd.toggleTheme",
        group: "view",
        shortcut: "Ctrl+Alt+T",
        run: () => {
          const theme = settings.get("theme");
          const effective =
            theme === "system"
              ? window.matchMedia("(prefers-color-scheme: light)").matches
                ? "light"
                : "dark"
              : theme;
          settings.set("theme", effective === "dark" ? "light" : "dark");
        },
      },
      { id: "view.langId", titleKey: "cmd.langId", group: "view", run: () => settings.set("language", "id") },
      { id: "view.langEn", titleKey: "cmd.langEn", group: "view", run: () => settings.set("language", "en") },
      { id: "dev.designSystem", titleKey: "cmd.designSystem", group: "dev", run: () => nav.push("/design-system") },
      { id: "help.about", titleKey: "cmd.about", group: "help", run: () => setAboutOpen(true) },
    ];
    if (windowApi) {
      defs.push({ id: "window.close", titleKey: "cmd.closeWindow", group: "app", run: () => void windowApi.close() });
    }
    const offs = defs.map((d) => commands.register(d));
    return () => {
      for (const off of offs) off();
    };
  }, [commands, settings, nav, openPalette, windowApi]);

  // Pintasan tambahan yang tidak punya perintah sendiri (alias).
  useShortcut({
    id: "alias.palette",
    combo: "Ctrl+Shift+P",
    scope: "global",
    description: "cmd.palette",
    allowInInput: true,
    run: openPalette,
  });
  useShortcut({
    id: "alias.zoomIn",
    combo: "Ctrl+Shift++",
    scope: "global",
    description: "cmd.zoomIn",
    allowInInput: true,
    run: () => settings.set("uiScale", stepScale(settings.get("uiScale"), 1)),
  });

  useCommandShortcuts(logger);

  const translate = useCallback((key: string) => t(key as MessageKey), [t]);

  const items = useMemo<PaletteItem[]>(() => {
    if (!paletteOpen) return [];
    return searchCommands(list, query, translate).map((h) => ({
      id: h.command.id,
      label: h.title,
      group: t(`group.${h.command.group}` as MessageKey),
      shortcut: h.command.shortcut ? formatCombo(h.command.shortcut) : undefined,
      disabled: !h.enabled,
      matchIndices: h.match.indices,
    }));
  }, [paletteOpen, list, query, translate, t]);

  return (
    <CommandCenterContext.Provider value={{ openPalette }}>
      {children}
      <Palette
        aria-label={t("palette.label")}
        placeholder={t("palette.placeholder")}
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        query={query}
        onQueryChange={setQuery}
        items={items}
        onSelect={(id) => void commands.run(id)}
      />
      <Dialog
        open={aboutOpen}
        onOpenChange={setAboutOpen}
        title={t("about.title")}
        description={t("about.body", { version: build.version })}
      />
    </CommandCenterContext.Provider>
  );
}

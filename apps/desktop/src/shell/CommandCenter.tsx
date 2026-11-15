// Pusat perintah: mendaftarkan perintah global, mengikat pintasannya, dan menampung palette + dialog Tentang.
import { CHAT_KEY, type CommandDef, formatCombo, type Logger, parseMessageLink, searchCommands } from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import {
  Dialog,
  Palette,
  type PaletteItem,
  stepScale,
  useShortcut,
  useShortcutList,
  useShortcutRegistry,
  useToast,
} from "@zandor/ui";
import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { build } from "../build";
import { buildPanelDump } from "./dev-dump";
import { requestMessageFocus } from "./message-focus";
import { useAppearance, useCommandList, useServices, useWindowApi } from "./services";

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
  const { commands, settings, nav, logger, projects, chat } = useServices();
  const toast = useToast();
  // nilai terbaru untuk perintah berumur panjang (tanpa mendaftarkan ulang perintah tiap render)
  const ui = useRef({ toast, t });
  ui.current = { toast, t };
  const windowApi = useWindowApi();
  const list = useCommandList();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [aboutOpen, setAboutOpen] = useState(false);
  const registry = useShortcutRegistry();
  const bindings = useShortcutList(); // render ulang saat pintasan diubah (palet menampilkan combo efektif)
  const appearance = useAppearance();
  // Override pintasan dari halaman Tampilan (bentrok ditolak registry dan jatuh ke combo bawaan).
  useEffect(() => {
    registry.applyOverrides(appearance.shortcuts);
  }, [registry, appearance.shortcuts]);

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
      {
        id: "chat.new",
        titleKey: "cmd.newChat",
        group: "app",
        shortcut: "Ctrl+N",
        run: () => {
          const sel = projects.state().selection.key;
          void projects.createSession(sel ?? CHAT_KEY).catch((e) => logger.error("chat baru gagal", { error: e }));
        },
      },
      { id: "app.providers", titleKey: "cmd.providers", group: "app", run: () => nav.push("/settings/models") },
      { id: "app.memory", titleKey: "cmd.memory", group: "app", run: () => nav.push("/memory") },
      { id: "app.automation", titleKey: "cmd.automation", group: "app", run: () => nav.push("/automation") },
      { id: "app.appearance", titleKey: "cmd.appearance", group: "app", run: () => nav.push("/appearance") },
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
      {
        id: "dev.copyChatHtml",
        titleKey: "cmd.copyChatHtml",
        group: "dev",
        run: async () => {
          const root = document.querySelector<HTMLElement>('[data-panel="chat"]');
          if (!root) {
            ui.current.toast.show({ title: ui.current.t("dev.copyChat.noPanel"), tone: "danger" });
            return;
          }
          const sessionId = projects.state().selection.sessionId;
          const session = sessionId ? chat.state().sessions[sessionId] : undefined;
          try {
            const html = buildPanelDump({
              doc: document,
              root,
              title: "Zandor — dump Panel Chat",
              meta: {
                waktu: new Date().toISOString(),
                versi: build.version,
                userAgent: navigator.userAgent,
                sesi: sessionId,
                jumlahPesan: session?.messages.length ?? 0,
                skalaUi: settings.get("uiScale"),
                tema: settings.get("theme"),
              },
              data: session ? { sessionId, messages: session.messages } : undefined,
            });
            await navigator.clipboard.writeText(html);
            ui.current.toast.show({
              title: ui.current.t("dev.copyChat.done", { kb: String(Math.round(html.length / 1024)) }),
              tone: "success",
            });
          } catch (e) {
            logger.error("salin HTML Panel Chat gagal", { error: e });
            ui.current.toast.show({ title: ui.current.t("dev.copyChat.fail", { error: String(e) }), tone: "danger" });
          }
        },
      },
      { id: "help.about", titleKey: "cmd.about", group: "help", run: () => setAboutOpen(true) },
    ];
    if (windowApi) {
      defs.push({
        id: "dev.openDevtools",
        titleKey: "cmd.openDevtools",
        group: "dev",
        shortcut: "F12",
        run: async () => {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("open_devtools");
        },
      });
      defs.push({ id: "window.close", titleKey: "cmd.closeWindow", group: "app", run: () => void windowApi.close() });
    }
    const offs = defs.map((d) => commands.register(d));
    return () => {
      for (const off of offs) off();
    };
  }, [commands, settings, nav, openPalette, windowApi, projects, logger, chat]);

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

  const openMessageLink = (text: string) => {
    const link = parseMessageLink(text);
    if (!link) return;
    const group = projects.state().sessions[link.key];
    if (group?.status === "ready" && !group.items.some((s) => s.id === link.sessionId)) {
      toast.show({ title: t("msg.linkMissing"), tone: "warning" });
      return;
    }
    projects.select(link.key, link.sessionId);
    requestMessageFocus(link.messageId);
  };

  const items = useMemo<PaletteItem[]>(() => {
    if (!paletteOpen) return [];
    // tautan pesan yang ditempel → satu butir "Buka pesan"
    if (parseMessageLink(query)) {
      return [{ id: "msg-link:open", label: t("msg.openLink"), group: t("group.app" as MessageKey) }];
    }
    return searchCommands(list, query, translate).map((h) => ({
      id: h.command.id,
      label: h.title,
      group: t(`group.${h.command.group}` as MessageKey),
      shortcut: h.command.shortcut
        ? ((bindings.some((x) => x.id === `cmd:${h.command.id}`) ? registry.comboOf(`cmd:${h.command.id}`) : null) ??
          formatCombo(h.command.shortcut))
        : undefined,
      disabled: !h.enabled,
      matchIndices: h.match.indices,
    }));
  }, [paletteOpen, list, query, translate, t, bindings, registry.comboOf]);

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
        onSelect={(id) => {
          if (id === "msg-link:open") return openMessageLink(query);
          void commands.run(id);
        }}
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

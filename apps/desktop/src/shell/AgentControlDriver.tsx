// Menjalankan permintaan kontrol dari agent (ADR-077) selama aplikasi terbuka: mengambil antrean lewat Rust tiap
// POLL_MS, mengeksekusinya (agent-control.ts), lalu menjawab. Aksi yang mengubah tampilan diberi toast singkat supaya
// pengguna tahu agent yang melakukannya. Tidak merender apa pun.
import type { ModelPrefs } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { useToast } from "@zandor/ui";
import { useEffect, useRef } from "react";
import { parseRoute } from "../App";
import { type ControlDeps, type ControlRequest, executeControl, type ImageSize } from "./agent-control";
import { type ControlHost, loadControlHost } from "./control-host";
import { type Services, useServices } from "./services";
import { openInZandorBrowser, showBrowserTool } from "./tools/browser-bridge";

const POLL_MS = 700;
/** Aksi baca-saja: tanpa toast. */
const QUIET = new Set(["status", "list_commands", "list_terminals", "get_settings"]);

/** Generate gambar dengan model gambar bawaan. Melempar galat yang bisa ditampilkan. */
export async function generateWithDefault(
  host: ControlHost,
  modelPrefs: ModelPrefs,
  input: { prompt: string; size: ImageSize; root: string; rel: string },
): Promise<string> {
  const ref = modelPrefs.state().defaultImage;
  if (!ref)
    throw new Error(
      "Model gambar bawaan belum dipilih (Pengaturan → Provider dan model → Model generate gambar bawaan).",
    );
  return host.generateImage({ providerID: ref.providerID, modelID: ref.modelID, ...input });
}

/** Folder yang dikenal aplikasi: project + folder Chat. */
export function knownRoots(projects: Services["projects"]): string[] {
  const s = projects.state();
  return [...s.projects.map((p) => p.path), ...(s.chatDir ? [s.chatDir] : [])];
}

export function AgentControlDriver() {
  const services = useServices();
  const { t } = useI18n();
  const toast = useToast();
  const ui = useRef({ t, toast });
  ui.current = { t, toast };

  useEffect(() => {
    const { settings, commands, panels, nav, docs, projects, terminals, modelPrefs } = services;
    let host: ControlHost | null = null;
    let busy = false;
    let stopped = false;

    const openFile = (path: string, root: string) => {
      const roots = knownRoots(projects);
      const base = roots.find((r) => r.toLowerCase() === root.toLowerCase()) ?? null;
      if (!base) return false;
      if (/^([a-z]:[\\/]|\/)/i.test(path)) {
        const norm = path.replace(/\\/g, "/").toLowerCase();
        const owner = roots.find((r) => norm.startsWith(`${r.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase()}/`));
        if (!owner) return false;
        docs.open(owner, path.replace(/\\/g, "/").slice(owner.replace(/[\\/]+$/, "").length + 1));
      } else {
        if (path.split(/[\\/]/).includes("..")) return false;
        docs.open(base, path);
      }
      panels.actions?.reveal("docs");
      return true;
    };

    const deps: ControlDeps = {
      enabled: () => settings.get("agentControl"),
      status: () => {
        const p = panels.state();
        const sel = projects.selected();
        return {
          panels: Object.fromEntries(Object.entries(p.collapsed).map(([k, c]) => [k, c ? "closed" : "open"])),
          activeTool: p.activeTool,
          page: nav.state().current,
          project: sel?.project?.name ?? (sel ? "Chat" : null),
          directory: sel?.session.directory ?? null,
          openDocuments: docs.state().tabs.map((d) => d.path),
        };
      },
      listCommands: () =>
        commands.list().map((c) => ({
          id: c.id,
          title: ui.current.t(c.titleKey as never),
          enabled: commands.isEnabled(c.id),
        })),
      runCommand: (id) => commands.run(id),
      panel: (panel, state) => {
        const a = panels.actions;
        if (!a) throw new Error("Tata letak panel belum siap.");
        const closed = panels.state().collapsed[panel];
        if (state === "toggle" || (state === "open" && closed) || (state === "close" && !closed)) {
          if (state === "open") a.reveal(panel);
          else a.toggle(panel);
        }
      },
      openTool: (tool) => {
        if (tool === "browser") showBrowserTool(panels);
        else if (panels.state().activeTool === tool) panels.actions?.reveal("tools");
        else panels.actions?.openTool(tool);
      },
      openUrl: (url) => openInZandorBrowser(panels, url),
      openFile,
      isRoute: (path) => path === "/" || parseRoute(path).page !== "home",
      navigate: (path) => nav.push(path),
      listTerminals: () =>
        terminals.state().tabs.map((x) => ({ id: x.id, title: x.title, cwd: x.cwd, status: x.status })),
      closeTerminal: async (id) => {
        if (!terminals.state().tabs.some((x) => x.id === id)) return false;
        await terminals.close(id);
        return true;
      },
      getSettings: () => ({ ...settings.all() }),
      setSetting: (key, value) => settings.set(key, value as never),
      generateImage: (prompt, size, root, rel) => {
        if (!host) throw new Error("Generate gambar hanya tersedia di aplikasi desktop.");
        ui.current.toast.show({ title: ui.current.t("agent.imageBusy"), timeoutMs: 4000 });
        return generateWithDefault(host, modelPrefs, { prompt, size, root, rel });
      },
      knownRoots: () => knownRoots(projects),
    };

    const handle = async (req: ControlRequest) => {
      const res = await executeControl(req, deps);
      const { t: tr, toast: ts } = ui.current;
      if (!res.ok) {
        ts.show({ title: tr("agent.failed", { error: res.error }), tone: "danger", timeoutMs: 6000 });
      } else if (!QUIET.has(req.action) && typeof res.result === "string") {
        ts.show({ title: tr("agent.toast", { what: res.result }), timeoutMs: 3500 });
      }
      await host?.reply(req.id, res);
    };

    const tick = async () => {
      if (!host || busy || stopped) return;
      busy = true;
      try {
        // Tidak ditunggu: generate gambar bisa lama dan tidak boleh menahan permintaan lain.
        for (const req of await host.take()) void handle(req).catch(() => undefined);
      } catch {
        // antrean tidak terbaca (folder belum ada) → coba lagi pada detak berikutnya
      } finally {
        busy = false;
      }
    };

    void loadControlHost().then((h) => {
      host = h;
    });
    const id = setInterval(() => void tick(), POLL_MS);
    return () => {
      stopped = true;
      clearInterval(id);
    };
  }, [services]);

  return null;
}

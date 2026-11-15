// Tautan http(s) di respons model: klik (atau Enter/Spasi) membuka menu kecil yang menempel pada tautan — buka di browser
// Zandor (Panel 4), buka di browser eksternal (browser bawaan sistem), atau salin tautan. Navigasi bawaan <a> dicegah agar
// tautan dari model tidak pernah dibuka diam-diam. URL sudah disaring `SAFE_LINK` oleh RichMarkdown.
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useI18n } from "@zandor/i18n/react";
import { CopyIcon, ExternalLinkIcon, GlobeIcon, Menu, type MenuEntry, useToast } from "@zandor/ui";
import type { ReactNode } from "react";
import { useServices } from "../services";
import { openInZandorBrowser } from "../tools/browser-bridge";

const WEB_LINK = /^https?:\/\//i;

export function isWebLink(href: string): boolean {
  return WEB_LINK.test(href);
}

export function ChatLink({ href, children }: { href: string; children?: ReactNode }) {
  const { t } = useI18n();
  const toast = useToast();
  const { panels } = useServices();
  const desktop = isTauri();
  // perintah Rust `open_external` hanya menerima https (lib.rs)
  const externalOk = !desktop || /^https:\/\//i.test(href);
  const fail = (e: unknown) =>
    toast.show({ title: t("sb.error", { error: e instanceof Error ? e.message : String(e) }), tone: "danger" });

  const openExternal = async () => {
    try {
      if (desktop) await invoke("open_external", { url: href });
      else window.open(href, "_blank", "noopener,noreferrer");
    } catch (e) {
      fail(e);
    }
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(href);
      toast.show({ title: t("link.copied"), tone: "success", timeoutMs: 1500 });
    } catch (e) {
      fail(e);
    }
  };

  const entries: MenuEntry[] = [
    {
      type: "item",
      id: "zandor",
      label: desktop ? t("link.openZandor") : t("link.openZandorDesktop"),
      icon: <GlobeIcon size="sm" />,
      disabled: !desktop,
      onSelect: () => openInZandorBrowser(panels, href),
    },
    {
      type: "item",
      id: "external",
      label: externalOk ? t("link.openExternal") : t("link.openExternalHttps"),
      icon: <ExternalLinkIcon size="sm" />,
      disabled: !externalOk,
      onSelect: () => void openExternal(),
    },
    { type: "separator", id: "sep" },
    { type: "item", id: "copy", label: t("link.copy"), icon: <CopyIcon size="sm" />, onSelect: () => void copy() },
  ];

  return (
    <Menu
      nativeButton={false}
      entries={entries}
      trigger={
        <a href={href} title={href} onClick={(e) => e.preventDefault()}>
          {children}
        </a>
      }
    />
  );
}

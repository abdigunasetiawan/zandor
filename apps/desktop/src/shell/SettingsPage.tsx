// Halaman Pengaturan: navigasi kategori di kiri, pencarian, dan baris pengaturan dirender dari SETTINGS_SCHEMA (JSON Schema)
// sehingga setiap pengaturan punya skema terdokumentasi (PRD §4.7). Bagian khusus (suara kustom, izin notifikasi, data,
// pintasan ke halaman lain) ditambahkan di sekitar baris-baris skema. Semua hal yang bisa diatur dijangkau dari sini.
import { SETTING_KEYS, SETTINGS_SCHEMA, type SettingKey, type SettingProperty, type Settings } from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowLeftIcon,
  Button,
  IconButton,
  SearchField,
  Select,
  SettingsIcon,
  Switch,
  useConfirm,
  useToast,
} from "@zandor/ui";
import { type ReactNode, useMemo, useRef, useState } from "react";
import { NotificationExtras } from "./NotificationSettings";
import { useServices, useSettings } from "./services";

const GROUPS = ["general", "appearance", "chat", "notifications", "tools", "layout"] as const;
type Group = (typeof GROUPS)[number];
const EXTRA_SECTIONS = ["more", "data"] as const;
type Section = Group | (typeof EXTRA_SECTIONS)[number];

function SettingControl({
  settingKey,
  prop,
  value,
}: {
  settingKey: SettingKey;
  prop: SettingProperty;
  value: unknown;
}) {
  const { t } = useI18n();
  const { settings } = useServices();
  const meta = prop["x-zandor"];
  const label = t(meta.titleKey as MessageKey);
  const set = (v: unknown) => settings.set(settingKey, v as never);

  if (prop.type === "boolean") {
    return <Switch label="" aria-label={label} checked={value as boolean} onCheckedChange={set} />;
  }
  const values: Array<string | number> = prop.enum ? [...prop.enum] : [];
  const items = values.map((v) => ({
    value: String(v),
    label: meta.enumLabelKeys?.[String(v)]
      ? t(meta.enumLabelKeys[String(v)] as MessageKey)
      : prop.type === "number" && settingKey === "notifyVolume"
        ? `${Math.round(Number(v) * 100)}%`
        : prop.type === "number" && settingKey === "uiScale"
          ? `${Math.round(Number(v) * 100)}%`
          : String(v),
  }));
  return (
    <Select
      aria-label={label}
      value={String(value)}
      onValueChange={(v) => set(prop.type === "number" ? Number(v) : v)}
      items={items}
    />
  );
}

function download(name: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export function SettingsPage() {
  const { t } = useI18n();
  const { settings, nav, panels } = useServices();
  const all: Settings = useSettings();
  const confirm = useConfirm();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState<Section>("general");
  const importRef = useRef<HTMLInputElement>(null);
  const q = query.trim().toLowerCase();

  const rowsOf = useMemo(() => {
    const out = {} as Record<Group, SettingKey[]>;
    for (const g of GROUPS) {
      out[g] = SETTING_KEYS.filter((k) => {
        const meta = (SETTINGS_SCHEMA.properties[k] as SettingProperty)["x-zandor"];
        if (meta.group !== g) return false;
        if (!q) return true;
        const text = `${t(meta.titleKey as MessageKey)} ${meta.descriptionKey ? t(meta.descriptionKey as MessageKey) : ""} ${k}`;
        return text.toLowerCase().includes(q);
      });
    }
    return out;
  }, [q, t]);

  const goto = (s: Section) => {
    setActive(s);
    document.getElementById(`settings-${s}`)?.scrollIntoView?.({ block: "start", behavior: "smooth" });
  };

  const matchesText = (...texts: string[]) => !q || texts.join(" ").toLowerCase().includes(q);
  const resetAll = async () => {
    const ok = await confirm({
      title: t("settings.resetAllTitle"),
      description: t("settings.resetAllBody"),
      confirmLabel: t("settings.resetAll"),
      tone: "danger",
    });
    if (!ok) return;
    for (const k of SETTING_KEYS) settings.reset(k);
    toast.show({ title: t("settings.resetAllDone"), tone: "success", timeoutMs: 2000 });
  };
  const importJson = async (file: File | undefined) => {
    if (!file) return;
    try {
      const raw = JSON.parse(await file.text()) as Record<string, unknown>;
      if (raw === null || typeof raw !== "object" || Array.isArray(raw)) throw new Error(t("settings.importBad"));
      let applied = 0;
      let skipped = 0;
      for (const [k, v] of Object.entries(raw)) {
        try {
          if (!(SETTING_KEYS as string[]).includes(k)) throw new Error("tidak dikenal");
          settings.set(k as SettingKey, v as never);
          applied++;
        } catch {
          skipped++;
        }
      }
      toast.show({ title: t("settings.importDone", { applied, skipped }), tone: skipped ? "warning" : "success" });
    } catch (e) {
      toast.show({
        title: t("settings.importError", { error: e instanceof Error ? e.message : String(e) }),
        tone: "danger",
      });
    }
  };

  const linkCards: Array<{ path: string; title: MessageKey; body: MessageKey }> = [
    { path: "/appearance", title: "settings.more.appearance", body: "settings.more.appearanceBody" },
    { path: "/providers", title: "settings.more.providers", body: "settings.more.providersBody" },
    { path: "/memory", title: "settings.more.memory", body: "settings.more.memoryBody" },
    { path: "/automation", title: "settings.more.automation", body: "settings.more.automationBody" },
  ];

  const navItems: Array<{ id: Section; label: string }> = [
    ...GROUPS.map((g) => ({ id: g as Section, label: t(`settings.${g}` as MessageKey) })),
    { id: "more", label: t("settings.more") },
    { id: "data", label: t("settings.data") },
  ];

  const groupExtra = (g: Group): ReactNode => {
    if (g === "appearance")
      return matchesText(t("settings.more.appearance"), t("settings.more.appearanceBody")) ? (
        <div className="settings__row" data-setting="appearanceLink">
          <div className="settings__text">
            <span className="z-label">{t("settings.more.appearance")}</span>
            <span className="z-small">{t("settings.more.appearanceBody")}</span>
          </div>
          <div className="settings__control">
            <Button size="sm" onClick={() => nav.push("/appearance")}>
              {t("settings.open")}
            </Button>
          </div>
        </div>
      ) : null;
    if (g === "notifications")
      return !q || matchesText(t("settings.notifications"), t("notif.custom.title")) ? <NotificationExtras /> : null;
    if (g === "layout")
      return matchesText(t("settings.layout.reset"), t("settings.layout.resetBody")) ? (
        <div className="settings__row" data-setting="layoutReset">
          <div className="settings__text">
            <span className="z-label">{t("settings.layout.reset")}</span>
            <span className="z-small">{t("settings.layout.resetBody")}</span>
          </div>
          <div className="settings__control">
            <Button size="sm" onClick={() => panels.actions?.reset()}>
              {t("settings.layout.resetButton")}
            </Button>
          </div>
        </div>
      ) : null;
    return null;
  };

  return (
    <main className="settings" aria-labelledby="settings-title">
      <header className="settings__head">
        <IconButton label={t("mem.back")} icon={<ArrowLeftIcon />} onClick={() => nav.push("/")} />
        <h1 id="settings-title" className="z-h2">
          {t("settings.title")}
        </h1>
        <SearchField
          value={query}
          onValueChange={setQuery}
          placeholder={t("settings.search")}
          aria-label={t("settings.search")}
          className="settings__search"
        />
      </header>
      <div className="settings__body">
        <nav className="settings__nav" aria-label={t("settings.categories")}>
          {navItems.map((n) => (
            <button
              key={n.id}
              type="button"
              className="settings__navitem"
              aria-current={active === n.id ? "true" : undefined}
              onClick={() => goto(n.id)}
            >
              {n.label}
            </button>
          ))}
        </nav>
        <div className="settings__content">
          <p className="z-small settings__note">{t("settings.schemaNote")}</p>

          {GROUPS.map((group) => {
            const keys = rowsOf[group];
            const extra = groupExtra(group);
            if (keys.length === 0 && !extra) return null;
            return (
              <section
                key={group}
                className="settings__group"
                id={`settings-${group}`}
                aria-labelledby={`settings-h-${group}`}
              >
                <h2 id={`settings-h-${group}`} className="z-h4">
                  {t(`settings.${group}` as MessageKey)}
                </h2>
                {keys.map((k) => {
                  const prop = SETTINGS_SCHEMA.properties[k] as SettingProperty;
                  const meta = prop["x-zandor"];
                  return (
                    <div key={k} className="settings__row" data-setting={k}>
                      <div className="settings__text">
                        <span className="z-label">{t(meta.titleKey as MessageKey)}</span>
                        {meta.descriptionKey && <span className="z-small">{t(meta.descriptionKey as MessageKey)}</span>}
                      </div>
                      <div className="settings__control">
                        <SettingControl settingKey={k} prop={prop} value={all[k]} />
                      </div>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={!settings.isExplicit(k)}
                        onClick={() => settings.reset(k)}
                      >
                        {t("settings.reset")}
                      </Button>
                    </div>
                  );
                })}
                {extra}
              </section>
            );
          })}

          {(!q || matchesText(t("settings.more"), ...linkCards.map((c) => t(c.title)))) && (
            <section className="settings__group" id="settings-more" aria-labelledby="settings-h-more">
              <h2 id="settings-h-more" className="z-h4">
                {t("settings.more")}
              </h2>
              <div className="settings__cards">
                {linkCards
                  .filter((c) => matchesText(t(c.title), t(c.body)))
                  .map((c) => (
                    <button key={c.path} type="button" className="settings__card" onClick={() => nav.push(c.path)}>
                      <SettingsIcon size="sm" />
                      <span className="z-label">{t(c.title)}</span>
                      <span className="z-small">{t(c.body)}</span>
                    </button>
                  ))}
              </div>
            </section>
          )}

          {(!q ||
            matchesText(t("settings.data"), t("settings.export"), t("settings.import"), t("settings.resetAll"))) && (
            <section className="settings__group" id="settings-data" aria-labelledby="settings-h-data">
              <h2 id="settings-h-data" className="z-h4">
                {t("settings.data")}
              </h2>
              <div className="settings__row">
                <div className="settings__text">
                  <span className="z-label">{t("settings.export")}</span>
                  <span className="z-small">{t("settings.exportBody")}</span>
                </div>
                <div className="settings__control settings__actions">
                  <Button
                    size="sm"
                    onClick={() =>
                      download("zandor-settings.json", `${JSON.stringify(explicitOnly(settings, all), null, 2)}\n`)
                    }
                  >
                    {t("settings.exportButton")}
                  </Button>
                </div>
              </div>
              <div className="settings__row">
                <div className="settings__text">
                  <span className="z-label">{t("settings.import")}</span>
                  <span className="z-small">{t("settings.importBody")}</span>
                </div>
                <div className="settings__control settings__actions">
                  <input
                    ref={importRef}
                    type="file"
                    accept="application/json,.json"
                    hidden
                    aria-label={t("settings.import")}
                    onChange={(e) => {
                      void importJson(e.target.files?.[0]);
                      e.target.value = "";
                    }}
                  />
                  <Button size="sm" onClick={() => importRef.current?.click()}>
                    {t("settings.importButton")}
                  </Button>
                </div>
              </div>
              <div className="settings__row">
                <div className="settings__text">
                  <span className="z-label">{t("settings.resetAll")}</span>
                  <span className="z-small">{t("settings.resetAllHelp")}</span>
                </div>
                <div className="settings__control settings__actions">
                  <Button size="sm" variant="danger" onClick={() => void resetAll()}>
                    {t("settings.resetAll")}
                  </Button>
                </div>
              </div>
              <h3 id="settings-json" className="z-label settings__sub">
                {t("settings.current")}
              </h3>
              <pre className="settings__json z-code" data-testid="settings-json">
                {JSON.stringify(all, null, 2)}
              </pre>
            </section>
          )}
        </div>
      </div>
    </main>
  );
}

/** Hanya nilai yang ditetapkan eksplisit (yang lain mengikuti default) — itulah yang berguna dipindahkan antar perangkat. */
function explicitOnly(store: { isExplicit(k: SettingKey): boolean }, all: Settings): Partial<Settings> {
  const out: Record<string, unknown> = {};
  for (const k of SETTING_KEYS) if (store.isExplicit(k)) out[k] = all[k];
  return out as Partial<Settings>;
}

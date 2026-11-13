// Halaman Pengaturan: dirender dari SETTINGS_SCHEMA (JSON Schema) sehingga setiap pengaturan punya
// skema terdokumentasi (PRD §4.7). Tidak ada kontrol yang ditulis tangan per pengaturan.
import { SETTING_KEYS, SETTINGS_SCHEMA, type SettingKey, type SettingProperty, type Settings } from "@zandor/core";
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import { Button, Select, Switch } from "@zandor/ui";
import { useServices, useSettings } from "./services";

const GROUPS = ["general", "appearance", "layout"] as const;

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
    return <Switch label={label} checked={value as boolean} onCheckedChange={set} />;
  }
  const values: Array<string | number> = prop.enum ? [...prop.enum] : [];
  const items = values.map((v) => ({
    value: String(v),
    label:
      prop.type === "number"
        ? `${Math.round(Number(v) * 100)}%`
        : meta.enumLabelKeys?.[String(v)]
          ? t(meta.enumLabelKeys[String(v)] as MessageKey)
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

export function SettingsPage() {
  const { t } = useI18n();
  const { settings } = useServices();
  const all: Settings = useSettings();

  return (
    <main className="settings" aria-labelledby="settings-title">
      <h1 id="settings-title" className="z-h2">
        {t("settings.title")}
      </h1>
      <p className="z-small settings__note">{t("settings.schemaNote")}</p>

      {GROUPS.map((group) => {
        const keys = SETTING_KEYS.filter((k) => SETTINGS_SCHEMA.properties[k]["x-zandor"].group === group);
        if (keys.length === 0) return null;
        return (
          <section key={group} className="settings__group" aria-labelledby={`settings-${group}`}>
            <h2 id={`settings-${group}`} className="z-h4">
              {t(`settings.${group}` as MessageKey)}
            </h2>
            {keys.map((k) => {
              const prop = SETTINGS_SCHEMA.properties[k] as SettingProperty;
              const meta = prop["x-zandor"];
              return (
                <div key={k} className="settings__row" data-setting={k}>
                  <div className="settings__text">
                    {prop.type !== "boolean" && <span className="z-label">{t(meta.titleKey as MessageKey)}</span>}
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
          </section>
        );
      })}

      <section className="settings__group" aria-labelledby="settings-json">
        <h2 id="settings-json" className="z-h4">
          {t("settings.current")}
        </h2>
        <pre className="settings__json z-code" data-testid="settings-json">
          {JSON.stringify(all, null, 2)}
        </pre>
      </section>
    </main>
  );
}

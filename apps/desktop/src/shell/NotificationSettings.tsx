// Bagian khusus di Pengaturan > Notifikasi dan suara: izin notifikasi sistem dan, per peristiwa, uji suara + berkas suara kustom.
import type { SoundSetting } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, useToast } from "@zandor/ui";
import { useEffect, useRef, useState } from "react";
import { useServices, useSettings } from "./services";
import {
  type CustomSound,
  clearCustomSound,
  loadCustomSound,
  playSound,
  SOUND_EVENTS,
  type SoundEvent,
  saveCustomSound,
} from "./sound";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function SystemPermission() {
  const { t } = useI18n();
  const { notify } = useServices();
  const toast = useToast();
  const [granted, setGranted] = useState<boolean | null>(null);
  useEffect(() => {
    let live = true;
    void notify.granted().then((g) => live && setGranted(g));
    return () => {
      live = false;
    };
  }, [notify]);
  return (
    <div className="settings__row" data-setting="notifyPermission">
      <div className="settings__text">
        <span className="z-label">{t("notif.permission.title")}</span>
        <span className="z-small">
          {!notify.available
            ? t("notif.permission.unavailable")
            : granted
              ? t("notif.permission.granted")
              : t("notif.permission.help")}
        </span>
      </div>
      <div className="settings__control">
        <Button
          size="sm"
          disabled={!notify.available || granted === true}
          onClick={() =>
            void notify.request().then(
              (g) => {
                setGranted(g);
                if (!g) toast.show({ title: t("notif.permission.denied"), tone: "warning" });
              },
              (e) => toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" }),
            )
          }
        >
          {t("notif.permission.request")}
        </Button>
      </div>
    </div>
  );
}

function SoundRow({ ev }: { ev: SoundEvent }) {
  const { t } = useI18n();
  const toast = useToast();
  const s = useSettings();
  const input = useRef<HTMLInputElement>(null);
  const [custom, setCustom] = useState<CustomSound | null>(() => loadCustomSound(ev));
  const choice: SoundSetting = ev === "done" ? s.soundDone : ev === "failed" ? s.soundFailed : s.soundAction;

  const pick = async (file: File | undefined) => {
    if (!file) return;
    try {
      setCustom(await saveCustomSound(ev, file));
      toast.show({ title: t("notif.custom.saved", { name: file.name }), tone: "success", timeoutMs: 2000 });
    } catch (e) {
      toast.show({ title: t("notif.custom.error", { error: errText(e) }), tone: "danger" });
    }
  };
  const test = async () => {
    const played = await playSound(choice, s.notifyVolume, ev);
    if (!played && choice !== "none")
      toast.show({ title: choice === "custom" ? t("notif.custom.missing") : t("notif.test.failed"), tone: "warning" });
  };

  return (
    <div className="settings__row" data-setting={`soundCustom.${ev}`}>
      <div className="settings__text">
        <span className="z-label">{t(`notif.event.${ev}` as never)}</span>
        <span className="z-small">
          {custom ? t("notif.custom.current", { name: custom.name }) : t("notif.custom.none")}
        </span>
      </div>
      <div className="settings__control settings__actions">
        <input
          ref={input}
          type="file"
          accept="audio/*"
          hidden
          aria-label={t("notif.custom.pick", { event: t(`notif.event.${ev}` as never) })}
          onChange={(e) => {
            void pick(e.target.files?.[0]);
            e.target.value = "";
          }}
        />
        <Button size="sm" onClick={() => input.current?.click()}>
          {t("notif.custom.choose")}
        </Button>
        {custom && (
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              clearCustomSound(ev);
              setCustom(null);
            }}
          >
            {t("notif.custom.remove")}
          </Button>
        )}
        <Button size="sm" onClick={() => void test()}>
          {t("notif.test")}
        </Button>
      </div>
    </div>
  );
}

function TestNotification() {
  const { t } = useI18n();
  const { notify } = useServices();
  const toast = useToast();
  const s = useSettings();
  const run = async () => {
    const title = t("notify.done");
    const body = t("notif.testAll.body");
    // toast selalu tampil sebagai umpan balik uji; saluran lain mengikuti pengaturan
    toast.show({ title, description: body, tone: "success", timeoutMs: 6000 });
    if (s.notifySystem && notify.available) {
      try {
        await notify.notify(title, body);
      } catch (e) {
        toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
      }
    }
    if (s.notifySound && s.soundDone !== "none") {
      const played = await playSound(s.soundDone, s.notifyVolume, "done");
      if (!played)
        toast.show({
          title: s.soundDone === "custom" ? t("notif.custom.missing") : t("notif.test.failed"),
          tone: "warning",
        });
    }
  };
  return (
    <div className="settings__row" data-setting="notifyTest">
      <div className="settings__text">
        <span className="z-label">{t("notif.testAll.title")}</span>
        <span className="z-small">{t("notif.testAll.help")}</span>
      </div>
      <div className="settings__control">
        <Button size="sm" onClick={() => void run()}>
          {t("notif.testAll.run")}
        </Button>
      </div>
    </div>
  );
}

export function NotificationExtras() {
  const { t } = useI18n();
  return (
    <>
      <SystemPermission />
      <TestNotification />
      <h3 className="z-label settings__sub">{t("notif.custom.title")}</h3>
      <p className="z-small settings__subnote">{t("notif.custom.help")}</p>
      {SOUND_EVENTS.map((ev) => (
        <SoundRow key={ev} ev={ev} />
      ))}
    </>
  );
}

// Tur pengenalan pertama kali: nama → model → tempat kerja → pintasan. Muncul sekali saat aplikasi dibuka tanpa project
// (pengaturan `onboarding`), mati setelah selesai atau dilewati, dan bisa dibuka lagi dari palet ("Tur pengenalan").
// Semua status berasal dari data nyata (model engine, project terdaftar); tidak ada langkah palsu.
import { CHAT_KEY, MAX_PROFILE_NAME } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, CheckIcon, Dialog, Field, Input, Kbd, Spinner } from "@zandor/ui";
import { useState } from "react";
import { AddProject } from "./Sidebar";
import { useAppearance, useChat, useProjects, useServices } from "./services";
import { useOsUser } from "./use-os-user";

const STEPS = ["name", "model", "workspace", "keys"] as const;
type Step = (typeof STEPS)[number];

const KEYS: Array<{ keys: string; label: string }> = [
  { keys: "Ctrl+K", label: "tour.keys.palette" },
  { keys: "Ctrl+N", label: "tour.keys.newChat" },
  { keys: "Ctrl+Shift+F", label: "tour.keys.search" },
  { keys: "Ctrl+1", label: "tour.keys.panels" },
  { keys: "Ctrl+,", label: "tour.keys.settings" },
];

export function Onboarding({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { t } = useI18n();
  const { settings, appearance, nav, projects } = useServices();
  const profile = useAppearance().profile;
  const { models, modelsStatus } = useChat();
  const state = useProjects();
  const osName = useOsUser();
  const [step, setStep] = useState<Step>("name");
  const [nameError, setNameError] = useState<string | null>(null);
  const i = STEPS.indexOf(step);

  const finish = () => {
    settings.set("onboarding", false);
    onOpenChange(false);
    setStep("name");
  };
  const goSettings = (path: string) => {
    finish();
    nav.push(path);
  };

  let body: React.ReactNode;
  if (step === "name") {
    body = (
      <Field label={t("profile.name")} description={t("tour.name.hint")} error={nameError ?? undefined}>
        {(c) => (
          <Input
            {...c}
            autoFocus
            value={profile.name ?? ""}
            maxLength={MAX_PROFILE_NAME}
            placeholder={osName ?? t("sb.user.fallback")}
            onChange={(e) => setNameError(appearance.setProfileName(e.target.value))}
          />
        )}
      </Field>
    );
  } else if (step === "model") {
    body =
      modelsStatus !== "ready" ? (
        <p className="z-p tour__status" role="status">
          <Spinner size="sm" /> {t("tour.model.loading")}
        </p>
      ) : models.length > 0 ? (
        <p className="z-p tour__status tour__status--ok" role="status">
          <CheckIcon size="sm" /> {t("tour.model.ready", { n: models.length })}
        </p>
      ) : (
        <div className="tour__stack">
          <p className="z-p tour__status" role="status">
            {t("tour.model.none")}
          </p>
          <Button variant="solid" onClick={() => goSettings("/settings/models")}>
            {t("tour.model.connect")}
          </Button>
        </div>
      );
  } else if (step === "workspace") {
    body = (
      <div className="tour__stack">
        {state.projects.length > 0 && (
          <p className="z-p tour__status tour__status--ok" role="status">
            <CheckIcon size="sm" /> {t("tour.workspace.added", { name: state.projects.at(-1)?.name ?? "" })}
          </p>
        )}
        <div className="tour__row">
          <AddProject labelled />
          <Button
            onClick={() => {
              projects.select(CHAT_KEY, null);
              finish();
            }}
          >
            {t("tour.workspace.chat")}
          </Button>
        </div>
      </div>
    );
  } else {
    body = (
      <ul className="tour__keys">
        {KEYS.map((k) => (
          <li key={k.keys}>
            <Kbd keys={k.keys} />
            <span className="z-p">{t(k.label as never)}</span>
          </li>
        ))}
      </ul>
    );
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => (o ? onOpenChange(true) : finish())}
      title={t(`tour.${step}.title` as never)}
      description={t(`tour.${step}.body` as never)}
      className="tour"
      footer={
        <>
          <span className="z-small tour__count">{t("tour.step", { i: i + 1, n: STEPS.length })}</span>
          <Button variant="ghost" onClick={finish}>
            {t("tour.skip")}
          </Button>
          {i > 0 && <Button onClick={() => setStep(STEPS[i - 1] as Step)}>{t("tour.back")}</Button>}
          {i < STEPS.length - 1 ? (
            <Button variant="solid" onClick={() => setStep(STEPS[i + 1] as Step)}>
              {t("tour.next")}
            </Button>
          ) : (
            <Button variant="solid" onClick={finish}>
              {t("tour.done")}
            </Button>
          )}
        </>
      }
    >
      {body}
    </Dialog>
  );
}

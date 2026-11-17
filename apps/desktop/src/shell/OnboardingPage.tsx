// Halaman onboarding (rute `#/welcome`): fokus utama aplikasi saat pertama dipasang (pengaturan `onboarding`).
// Langkah: Sambutan → Profil (nama + avatar) → Provider → Pemeriksaan sistem → Perilaku memori & skill →
// Impor memori dari AI lain (konflik antar-AI dipilih pengguna, tidak diputuskan diam-diam) → Terima kasih.
// Semua status dari data nyata; di luar aplikasi desktop pemeriksaan program dijelaskan tidak tersedia.
import type { MessageKey } from "@zandor/i18n";
import { useI18n } from "@zandor/i18n/react";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  Badge,
  Button,
  CheckIcon,
  CopyIcon,
  ExternalLinkIcon,
  RefreshIcon,
  Spinner,
  useToast,
} from "@zandor/ui";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { ConnectionPanel } from "./ConnectionPanel";
import type { SystemCheck } from "./engine-host";
import { ImportStep, type LeaveGuard, MemoryStep, ThanksStep, WelcomeStep } from "./onboarding-steps";
import { ProfileSettings } from "./ProfileSettings";
import { useChat, useEngineState, useProviders, useServices } from "./services";

export const ONBOARDING_STEPS = ["welcome", "profile", "provider", "system", "memory", "import", "thanks"] as const;
type Step = (typeof ONBOARDING_STEPS)[number];

/** Cara memasang alat yang belum ada: perintah winget + situs resmi. */
const INSTALL: Record<string, { cmd?: string; url: string }> = {
  python: { cmd: "winget install -e --id Python.Python.3.12", url: "https://www.python.org/downloads/" },
  pythonOffice: { cmd: "python -m pip install python-docx openpyxl python-pptx pypdf", url: "https://pypi.org/" },
  git: { cmd: "winget install -e --id Git.Git", url: "https://git-scm.com/download/win" },
  node: { cmd: "winget install -e --id OpenJS.NodeJS.LTS", url: "https://nodejs.org/" },
  pip: { cmd: "python -m ensurepip --upgrade", url: "https://pip.pypa.io/en/stable/installation/" },
  uv: { cmd: "winget install -e --id astral-sh.uv", url: "https://docs.astral.sh/uv/" },
  gh: { cmd: "winget install -e --id GitHub.cli", url: "https://cli.github.com/" },
  npm: { cmd: "winget install -e --id OpenJS.NodeJS.LTS", url: "https://nodejs.org/" },
  bun: { cmd: "winget install -e --id Oven-sh.Bun", url: "https://bun.sh/" },
  opencode: { cmd: "npm install -g opencode-ai", url: "https://opencode.ai/" },
  libreoffice: { cmd: "winget install -e --id TheDocumentFoundation.LibreOffice", url: "https://www.libreoffice.org/" },
  winget: { url: "https://aka.ms/getwinget" },
};

/** Kelompok tampilan pemeriksaan sistem, berurutan. */
const GROUPS = ["core", "python", "dev", "docs", "install"] as const;
type Group = (typeof GROUPS)[number];

type Status = "ok" | "missing" | "optional" | "checking" | "unavailable";
type Row = {
  id: string;
  group: Group;
  status: Status;
  version?: string | null;
  note?: string | null;
  install?: (typeof INSTALL)[string];
};

export function OnboardingPage() {
  const { t } = useI18n();
  const { settings, nav } = useServices();
  const [step, setStep] = useState<Step>("welcome");
  const i = ONBOARDING_STEPS.indexOf(step);

  const finish = (to = "/") => {
    settings.set("onboarding", false);
    nav.push(to);
  };

  // Langkah boleh memasang penjaga (impor memori menyimpan otomatis sebelum pindah).
  const guard = useRef<LeaveGuard | null>(null);
  const setGuard = useCallback((g: LeaveGuard | null) => {
    guard.current = g;
  }, []);
  const go = async (n: number) => {
    if (guard.current && !(await guard.current())) return;
    setStep(ONBOARDING_STEPS[n] as Step);
  };
  const last = i === ONBOARDING_STEPS.length - 1;

  if (step === "welcome") return <WelcomeStep onNext={() => go(1)} onSkip={() => finish()} />;

  let body: ReactNode;
  if (step === "profile") body = <ProfileSettings heading={false} />;
  else if (step === "provider") body = <ProviderStep />;
  else if (step === "system") body = <SystemStep />;
  else if (step === "memory") body = <MemoryStep />;
  else if (step === "import") body = <ImportStep onLeave={setGuard} />;
  else body = <ThanksStep />;

  // Bilah progres hanya untuk langkah setup (tanpa sambutan).
  const setup = ONBOARDING_STEPS.slice(1);
  const at = i - 1;

  return (
    <main className="onb" aria-label={t("onb.title")}>
      <div className="onb__frame">
        <ol className="onb__progress" aria-label={t("tour.step", { i: at + 1, n: setup.length })}>
          {setup.map((s, n) => (
            <li key={s}>
              <button
                type="button"
                className="onb__dot"
                aria-label={t(`onb.${s}.title` as MessageKey)}
                aria-current={s === step ? "step" : undefined}
                data-done={n < at ? "" : undefined}
                onClick={() => go(n + 1)}
              />
            </li>
          ))}
        </ol>
        <section className="onb__content" aria-labelledby="onb-step-title">
          <span className="z-small onb__muted">{t("tour.step", { i: at + 1, n: setup.length })}</span>
          <h1 id="onb-step-title" className="z-h2 onb__title">
            {t(`onb.${step}.title` as MessageKey)}
          </h1>
          <p className="z-p onb__muted onb__lead">{t(`onb.${step}.body` as MessageKey)}</p>
          {body}
        </section>
        <footer className="onb__foot">
          <Button onClick={() => go(i - 1)}>
            <ArrowLeftIcon size="sm" /> {t("tour.back")}
          </Button>
          <span className="onb__spacer" />
          {last ? (
            <Button variant="solid" onClick={() => finish()}>
              {t("onb.finish")} <ArrowRightIcon size="sm" />
            </Button>
          ) : (
            <Button variant="solid" onClick={() => go(i + 1)}>
              {t("tour.next")} <ArrowRightIcon size="sm" />
            </Button>
          )}
        </footer>
      </div>
    </main>
  );
}

function ProviderStep() {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const { models, modelsStatus } = useChat();
  const prov = useProviders();
  const [adding, setAdding] = useState(false);
  const byProvider = new Map<string, number>();
  for (const m of models) byProvider.set(m.providerName, (byProvider.get(m.providerName) ?? 0) + 1);

  if (adding)
    return (
      <ConnectionPanel
        editing={null}
        onClose={() => setAdding(false)}
        onSaved={() => {
          setAdding(false);
          void store.refresh();
        }}
      />
    );
  return (
    <div className="onb__stack">
      {modelsStatus !== "ready" ? (
        <p className="z-p onb__status" role="status">
          <Spinner size="sm" /> {t("tour.model.loading")}
        </p>
      ) : models.length > 0 ? (
        <p className="z-p onb__status" role="status" data-tone="ok">
          <CheckIcon size="sm" /> {t("tour.model.ready", { n: models.length })}
        </p>
      ) : (
        <p className="z-p onb__status" role="status">
          {t("tour.model.none")}
        </p>
      )}
      {byProvider.size > 0 && (
        <ul className="onb__list" aria-label={t("onb.provider.list")}>
          {[...byProvider].map(([name, n]) => (
            <li key={name} className="onb__item">
              <span className="z-label">{name}</span>
              <Badge>{t("onb.provider.models", { n })}</Badge>
            </li>
          ))}
        </ul>
      )}
      {prov.error && (
        <p className="z-small onb__error" role="alert">
          {prov.error}
        </p>
      )}
      <div className="onb__row">
        <Button variant="solid" onClick={() => setAdding(true)}>
          {t("onb.provider.add")}
        </Button>
      </div>
      <p className="z-small onb__muted">{t("onb.provider.hint")}</p>
    </div>
  );
}

function useSystemRows(): { rows: Row[]; recheck: () => void; native: boolean | null } {
  const { host } = useServices();
  const engine = useEngineState();
  const { models, modelsStatus } = useChat();
  const prov = useProviders();
  const [checks, setChecks] = useState<SystemCheck[] | null | undefined>(undefined);

  const recheck = useCallback(() => {
    setChecks(undefined);
    host.systemCheck().then(setChecks, () => setChecks(null));
  }, [host]);
  useEffect(recheck, [recheck]);

  const fromCheck = (group: Group, id: string, optional = false): Row => ({ group, ...checkRow(id, optional) });
  const checkRow = (id: string, optional: boolean): Omit<Row, "group"> => {
    if (checks === undefined) return { id, status: "checking" };
    if (checks === null) return { id, status: "unavailable" };
    const c = checks.find((x) => x.id === id);
    if (!c) return { id, status: "unavailable" };
    const install = INSTALL[id];
    if (id === "pythonOffice" && install) {
      const py = checks.find((x) => x.id === "python")?.detail ?? "python";
      return {
        id,
        status: c.found ? "ok" : optional ? "optional" : "missing",
        note: c.found ? null : c.detail,
        install: { ...install, cmd: install.cmd?.replace(/^python/, py) },
      };
    }
    return {
      id,
      status: c.found ? "ok" : optional ? "optional" : "missing",
      version: c.version,
      note: id === "python" && c.found ? c.detail : null,
      ...(c.found || !install ? {} : { install }),
    };
  };

  const opencode = models.filter((m) => m.providerID === "opencode").length;
  const routers = prov.providers.filter((p) => p.id === "openrouter" || p.kind === "local" || p.kind === "custom");
  const rows: Row[] = [
    {
      group: "core",
      id: "engine",
      status: engine.status === "ready" ? "ok" : engine.status === "error" ? "missing" : "checking",
      version: engine.version,
      note: engine.error,
    },
    {
      group: "core",
      id: "opencodeModels",
      status: modelsStatus !== "ready" ? "checking" : opencode > 0 ? "ok" : "optional",
      version: opencode > 0 ? String(opencode) : null,
    },
    {
      group: "core",
      id: "router",
      status: !prov.loaded ? "checking" : routers.length > 0 ? "ok" : "optional",
      note: routers.map((p) => p.name).join(", ") || null,
    },
    fromCheck("core", "opencode", true),
    fromCheck("python", "python"),
    fromCheck("python", "pip"),
    fromCheck("python", "pythonOffice", true),
    fromCheck("python", "uv", true),
    fromCheck("dev", "git"),
    fromCheck("dev", "gh", true),
    fromCheck("dev", "node", true),
    fromCheck("dev", "npm", true),
    fromCheck("dev", "bun", true),
    fromCheck("docs", "libreoffice", true),
    fromCheck("install", "winget", true),
  ];
  return { rows, recheck, native: checks === undefined ? null : checks !== null };
}

function SystemStep() {
  const { t } = useI18n();
  const toast = useToast();
  const { providers } = useServices();
  const { rows, recheck, native } = useSystemRows();
  const copy = (cmd: string) =>
    navigator.clipboard.writeText(cmd).then(
      () => toast.show({ title: t("onb.sys.copied"), timeoutMs: 2500 }),
      () => toast.show({ title: t("onb.sys.copyFailed"), tone: "danger" }),
    );
  return (
    <div className="onb__stack">
      {native === false && (
        <p className="z-small onb__muted" role="status">
          {t("onb.sys.webOnly")}
        </p>
      )}
      {GROUPS.map((g) => (
        <section key={g} className="onb__stack">
          <h2 className="z-h4 onb__group">{t(`onb.sys.group.${g}` as MessageKey)}</h2>
          <ul className="onb__list" aria-label={t(`onb.sys.group.${g}` as MessageKey)}>
            {rows
              .filter((r) => r.group === g)
              .map((r) => (
                <li key={r.id} className="onb__check" data-status={r.status}>
                  <div className="onb__checkhead">
                    <span className="z-label">{t(`onb.sys.${r.id}` as MessageKey)}</span>
                    <StatusBadge row={r} />
                  </div>
                  <p className="z-small onb__muted">{t(`onb.sys.${r.id}.why` as MessageKey)}</p>
                  {r.note && <p className="z-small onb__note">{r.note}</p>}
                  {r.install && (r.status === "missing" || r.status === "optional") && (
                    <div className="onb__install">
                      {r.install.cmd && <code className="onb__cmd">{r.install.cmd}</code>}
                      <div className="onb__row">
                        {r.install.cmd && (
                          <Button size="sm" onClick={() => void copy(r.install?.cmd ?? "")}>
                            <CopyIcon size="sm" /> {t("onb.sys.copy")}
                          </Button>
                        )}
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => void providers.openExternal(r.install?.url ?? "")}
                        >
                          <ExternalLinkIcon size="sm" /> {t("onb.sys.site")}
                        </Button>
                      </div>
                    </div>
                  )}
                </li>
              ))}
          </ul>
        </section>
      ))}
      <div className="onb__row">
        <Button onClick={recheck}>
          <RefreshIcon size="sm" /> {t("onb.sys.recheck")}
        </Button>
      </div>
    </div>
  );
}

function StatusBadge({ row }: { row: Row }) {
  const { t } = useI18n();
  if (row.status === "checking")
    return (
      <span className="z-small onb__muted onb__checking">
        <Spinner size="sm" /> {t("onb.sys.checking")}
      </span>
    );
  const label =
    row.status === "ok"
      ? row.version
        ? t("onb.sys.okVersion", { v: row.version })
        : t("onb.sys.ok")
      : t(`onb.sys.${row.status}` as MessageKey);
  return <Badge tone={row.status === "ok" ? undefined : "outline"}>{label}</Badge>;
}

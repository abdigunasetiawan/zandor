// Pengaturan → Engine remote: sambungkan Zandor ke engine opencode yang sudah berjalan di mesin lain (URL + Basic Auth).
// Zandor tidak memasang/menjalankan engine di sana. Status berasal dari cek health nyata; galat menampilkan sebabnya.
import { type RemoteEngine, type RemoteStatus, remoteUrlProblem } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Badge, Button, Dialog, Field, GlobeIcon, Input, Spinner, useConfirm, useToast } from "@zandor/ui";
import { type FormEvent, useState } from "react";
import { useProjects, useServices } from "./services";

type Draft = Omit<RemoteEngine, "id">;
const EMPTY: Draft = { name: "", url: "", username: "opencode", password: "" };

export function RemoteStatusBadge({ status }: { status: RemoteStatus | undefined }) {
  const { t } = useI18n();
  if (!status || status.status === "checking") {
    return (
      <span className="z-small remote__checking">
        <Spinner size="sm" /> {t("remote.checking")}
      </span>
    );
  }
  return status.status === "ready" ? (
    <Badge tone="success">{t("remote.ready", { version: status.version ?? "?" })}</Badge>
  ) : (
    <Badge tone="danger">{t("remote.failed")}</Badge>
  );
}

function RemoteForm({
  open,
  initial,
  onClose,
  onSave,
}: {
  open: boolean;
  initial: Draft;
  onClose: () => void;
  onSave: (d: Draft) => Promise<void>;
}) {
  const { t } = useI18n();
  const [d, setD] = useState<Draft>(initial);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const problem = d.url.trim() ? remoteUrlProblem(d.url) : null;
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (problem || !d.url.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await onSave(d);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t(initial.url ? "remote.editTitle" : "remote.addTitle")}
      description={t("remote.formBody")}
      footer={
        <>
          <Button onClick={onClose}>{t("ui.cancel")}</Button>
          <Button variant="solid" type="submit" form="remote-form" disabled={busy || !d.url.trim() || !!problem}>
            {t("remote.save")}
          </Button>
        </>
      }
    >
      <form id="remote-form" className="remote__form" onSubmit={(e) => void submit(e)}>
        <Field label={t("remote.name")}>
          {(c) => (
            <Input
              {...c}
              value={d.name}
              placeholder={t("remote.namePlaceholder")}
              onChange={(e) => setD({ ...d, name: e.target.value })}
            />
          )}
        </Field>
        <Field
          label={t("remote.url")}
          description={t("remote.urlHint")}
          error={problem ? t(problem === "insecure" ? "remote.urlInsecure" : "remote.urlInvalid") : undefined}
        >
          {(c) => (
            <Input
              {...c}
              autoFocus
              value={d.url}
              inputMode="url"
              autoComplete="off"
              placeholder="https://"
              onChange={(e) => setD({ ...d, url: e.target.value })}
            />
          )}
        </Field>
        <Field label={t("remote.username")}>
          {(c) => (
            <Input
              {...c}
              value={d.username}
              autoComplete="off"
              onChange={(e) => setD({ ...d, username: e.target.value })}
            />
          )}
        </Field>
        <Field label={t("remote.password")} description={t("remote.passwordHint")}>
          {(c) => (
            <Input
              {...c}
              type="password"
              autoComplete="off"
              value={d.password}
              onChange={(e) => setD({ ...d, password: e.target.value })}
            />
          )}
        </Field>
        {error && (
          <p className="z-small remote__error" role="alert">
            {error}
          </p>
        )}
      </form>
    </Dialog>
  );
}

export function RemoteEngines() {
  const { t } = useI18n();
  const { projects } = useServices();
  const state = useProjects();
  const confirm = useConfirm();
  const toast = useToast();
  const [form, setForm] = useState<{ id: string | null; initial: Draft } | null>(null);

  const remove = async (r: RemoteEngine) => {
    const ok = await confirm({
      title: t("remote.removeTitle", { name: r.name }),
      description: t("remote.removeBody"),
      confirmLabel: t("remote.remove"),
      tone: "danger",
    });
    if (!ok) return;
    await projects.removeRemote(r.id).catch((e) => {
      toast.show({ title: e instanceof Error ? e.message : String(e), tone: "danger" });
    });
  };

  return (
    <section className="settings__group" id="settings-engines" aria-labelledby="settings-h-engines">
      <h2 id="settings-h-engines" className="z-h4">
        {t("settings.engines")}
      </h2>
      <p className="z-small settings__note">{t("remote.intro")}</p>
      {state.remotes.length === 0 ? (
        <p className="z-small remote__muted">{t("remote.empty")}</p>
      ) : (
        <ul className="remote__list" aria-label={t("settings.engines")}>
          {state.remotes.map((r) => {
            const st = state.remoteStatus[r.id];
            const used = state.projects.filter((p) => p.engine === r.id).length;
            return (
              <li key={r.id} className="remote__row">
                <GlobeIcon size="sm" />
                <div className="remote__text">
                  <span className="z-label">{r.name}</span>
                  <span className="z-small remote__muted">
                    {r.url} · {t("remote.projects", { n: used })}
                  </span>
                  {st?.status === "error" && st.error && <span className="z-small remote__error">{st.error}</span>}
                </div>
                <RemoteStatusBadge status={st} />
                <Button size="sm" variant="ghost" onClick={() => void projects.checkRemote(r.id)}>
                  {t("remote.check")}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() =>
                    setForm({
                      id: r.id,
                      initial: { name: r.name, url: r.url, username: r.username, password: r.password },
                    })
                  }
                >
                  {t("remote.edit")}
                </Button>
                <Button size="sm" variant="danger" onClick={() => void remove(r)}>
                  {t("remote.remove")}
                </Button>
              </li>
            );
          })}
        </ul>
      )}
      <div>
        <Button onClick={() => setForm({ id: null, initial: EMPTY })}>{t("remote.add")}</Button>
      </div>
      {form && (
        <RemoteForm
          open
          initial={form.initial}
          onClose={() => setForm(null)}
          onSave={async (d) => {
            if (form.id) await projects.updateRemote(form.id, d);
            else await projects.addRemote(d);
          }}
        />
      )}
    </section>
  );
}

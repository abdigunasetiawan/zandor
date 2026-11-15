// Akun/kunci ganda pada satu provider (ADR-056): daftar akun, pilih yang aktif, ganti nama, hapus, dan tambah akun baru.
// Kunci hanya lewat host (keychain); UI tidak menyimpannya setelah dikirim.
import type { ProviderAccount, ProviderView } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Badge, Button, EditIcon, IconButton, Input, PlusIcon, TrashIcon, useConfirm, useToast } from "@zandor/ui";
import { useState } from "react";
import { useServices } from "./services";

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Nama akun untuk tampilan: label bila ada, "Akun utama" untuk akun lama, selain itu "Akun N". */
export function accountName(t: ReturnType<typeof useI18n>["t"], a: ProviderAccount, index: number): string {
  if (a.label) return a.label;
  return a.id === "main" ? t("acc.main") : t("acc.unnamed", { n: index + 1 });
}

export function AccountList({
  provider,
  name,
  withAdd = true,
}: {
  provider: ProviderView;
  /** Nama tampilan provider (untuk provider katalog yang baru dibuat). */
  name?: string;
  withAdd?: boolean;
}) {
  const { t } = useI18n();
  const { providers: store } = useServices();
  const confirm = useConfirm();
  const toast = useToast();
  const accounts = provider.accounts ?? [];
  const [renaming, setRenaming] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [adding, setAdding] = useState(false);
  const [label, setLabel] = useState("");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);

  const run = async (fn: () => Promise<unknown>, ok?: string): Promise<boolean> => {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.show({ title: ok, tone: "success" });
      return true;
    } catch (e) {
      toast.show({ title: t("sb.error", { error: errText(e) }), tone: "danger" });
      return false;
    } finally {
      setBusy(false);
    }
  };

  const remove = async (a: ProviderAccount, display: string) => {
    const ok = await confirm({
      title: t("acc.confirmTitle", { name: display }),
      description: t("acc.confirmBody"),
      confirmLabel: t("acc.remove"),
      tone: "danger",
    });
    if (ok) await run(() => store.removeAccount(provider.id, a.id));
  };

  return (
    <section className="acct" aria-label={`${t("acc.title")} ${provider.name}`}>
      <div className="acct__head">
        <span className="z-label acct__title">{t("acc.title")}</span>
        <span className="z-small acct__count">{t("acc.count", { count: accounts.length })}</span>
        {withAdd && !adding && (
          <Button size="sm" onClick={() => setAdding(true)} aria-label={`${t("acc.add")} ${provider.name}`}>
            <PlusIcon size="sm" /> {t("acc.add")}
          </Button>
        )}
      </div>
      <ul className="acct__list">
        {accounts.map((a, i) => {
          const display = accountName(t, a, i);
          return (
            <li key={a.id} className="acct__row" aria-label={display} data-active={a.active || undefined}>
              {renaming === a.id ? (
                <form
                  className="acct__edit"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void run(() => store.renameAccount(provider.id, a.id, draft)).then((ok) => {
                      if (ok) setRenaming(null);
                    });
                  }}
                >
                  <Input
                    aria-label={t("acc.renameFor", { name: display })}
                    value={draft}
                    maxLength={80}
                    onChange={(e) => setDraft(e.target.value)}
                  />
                  <Button size="sm" variant="solid" type="submit" disabled={busy}>
                    {t("pv.save")}
                  </Button>
                  <Button size="sm" type="button" onClick={() => setRenaming(null)}>
                    {t("ow.cancel")}
                  </Button>
                </form>
              ) : (
                <>
                  <span className="acct__name">{display}</span>
                  {a.active ? (
                    <Badge tone="success">{t("acc.active")}</Badge>
                  ) : (
                    <Button
                      size="sm"
                      disabled={busy}
                      aria-label={t("acc.useFor", { name: display })}
                      onClick={() =>
                        void run(() => store.activateAccount(provider.id, a.id), t("acc.switched", { name: display }))
                      }
                    >
                      {t("acc.use")}
                    </Button>
                  )}
                  <IconButton
                    size="sm"
                    label={t("acc.renameFor", { name: display })}
                    icon={<EditIcon />}
                    onClick={() => {
                      setDraft(a.label);
                      setRenaming(a.id);
                    }}
                  />
                  <IconButton
                    size="sm"
                    label={t("acc.removeFor", { name: display })}
                    icon={<TrashIcon />}
                    disabled={busy}
                    onClick={() => void remove(a, display)}
                  />
                </>
              )}
            </li>
          );
        })}
      </ul>
      {adding && (
        <form
          className="acct__add"
          aria-label={t("acc.add")}
          onSubmit={(e) => {
            e.preventDefault();
            void run(async () => {
              await store.addAccount({ providerId: provider.id, name: name ?? provider.name, label, apiKey: key });
              setAdding(false);
              setLabel("");
              setKey("");
            }, t("pv.saved"));
          }}
        >
          <Input
            aria-label={t("acc.label")}
            placeholder={t("acc.labelHint")}
            value={label}
            maxLength={80}
            onChange={(e) => setLabel(e.target.value)}
          />
          <Input
            type="password"
            autoComplete="off"
            aria-label={`${t("pv.apiKey")} ${provider.name}`}
            placeholder={t("pv.apiKey")}
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
          <Button variant="solid" type="submit" disabled={busy || !key.trim()}>
            {t("pv.save")}
          </Button>
          <Button type="button" onClick={() => setAdding(false)}>
            {t("ow.cancel")}
          </Button>
        </form>
      )}
    </section>
  );
}

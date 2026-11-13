// Pemilih model penuh (Fase 6): dialog dengan pencarian yang langsung fokus, grup "Disematkan" lalu per provider,
// sematkan model, dan uji (ping) model dari koneksi kustom/lokal milik pengguna. Status ping jujur dan selalu berakhir.
import { type ModelOption, modelKey, pingKey } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Badge, Button, Dialog, IconButton, PingIcon, PinIcon, SearchField, Spinner } from "@zandor/ui";
import { useMemo, useState } from "react";
import { pingLabel } from "./ProvidersPage";
import { useChat, useProviders, useServices } from "./services";

const matches = (m: ModelOption, q: string) =>
  !q ||
  m.name.toLowerCase().includes(q) ||
  m.providerName.toLowerCase().includes(q) ||
  m.modelID.toLowerCase().includes(q);

export function ModelPicker() {
  const { t } = useI18n();
  const { chat, nav, providers: store } = useServices();
  const s = useChat();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const pv = useProviders();

  const current = s.models.find((m) => s.model && m.providerID === s.model.providerID && m.modelID === s.model.modelID);
  const owned = useMemo(
    () => new Map(pv.providers.filter((p) => p.kind !== "builtin").map((p) => [p.id, p])),
    [pv.providers],
  );

  const q = query.trim().toLowerCase();
  const _match = (m: ModelOption) =>
    !q ||
    m.name.toLowerCase().includes(q) ||
    m.providerName.toLowerCase().includes(q) ||
    m.modelID.toLowerCase().includes(q);

  const groups = useMemo(() => {
    const byProvider = new Map<string, { name: string; models: ModelOption[] }>();
    for (const m of s.models) {
      if (!matches(m, q)) continue;
      const g = byProvider.get(m.providerID) ?? { name: m.providerName, models: [] };
      g.models.push(m);
      byProvider.set(m.providerID, g);
    }
    return [...byProvider.entries()].sort((a, b) => a[1].name.localeCompare(b[1].name));
  }, [s.models, q]);
  const pinned = s.pinned
    .map((k) => s.models.find((m) => modelKey(m) === k))
    .filter((m): m is ModelOption => !!m && matches(m, q));

  if (s.modelsStatus === "error")
    return <Badge tone="danger">{t("chat.modelsError", { error: s.modelsError ?? "" })}</Badge>;
  if (s.models.length === 0) return null;

  const choose = (m: ModelOption) => {
    chat.selectModel({ providerID: m.providerID, modelID: m.modelID });
    setOpen(false);
    setQuery("");
  };

  const row = (m: ModelOption, keyPrefix: string) => {
    const isPinned = s.pinned.includes(modelKey(m));
    const ownedProvider = owned.get(m.providerID);
    const ping = ownedProvider ? pv.pings[pingKey(m.providerID, m.modelID)] : undefined;
    const selected = s.model?.providerID === m.providerID && s.model?.modelID === m.modelID;
    return (
      <li key={`${keyPrefix}${modelKey(m)}`} className="mrow" data-selected={selected ? "" : undefined}>
        <button
          type="button"
          className="mrow__pick"
          aria-current={selected ? "true" : undefined}
          onClick={() => choose(m)}
        >
          <span className="mrow__name">{m.name}</span>
          <span className="mrow__provider z-small">{m.providerName}</span>
        </button>
        <span className="mrow__status z-small" role="status">
          {ping?.status === "pending" && <Spinner size="sm" />}
          {ping?.status === "done" && (
            <span data-tone={ping.result.ok ? "ok" : "bad"}>{pingLabel(t, ping.result)}</span>
          )}
        </span>
        {ownedProvider && (
          <IconButton
            label={t("pv.ping", { name: m.name })}
            size="sm"
            icon={<PingIcon />}
            disabled={ping?.status === "pending"}
            onClick={() => void store.ping(m.providerID, m.modelID)}
          />
        )}
        <IconButton
          label={isPinned ? t("mp.unpin", { name: m.name }) : t("mp.pin", { name: m.name })}
          size="sm"
          icon={<PinIcon />}
          aria-pressed={isPinned}
          onClick={() => chat.togglePin({ providerID: m.providerID, modelID: m.modelID })}
        />
      </li>
    );
  };

  return (
    <>
      <Button
        size="sm"
        className="chat__model"
        aria-label={t("mp.open", { name: current?.name ?? "" })}
        onClick={() => setOpen(true)}
      >
        <span className="chat__model-name">{current?.name ?? t("chat.model")}</span>
      </Button>
      <Dialog
        open={open}
        onOpenChange={(o) => {
          setOpen(o);
          if (!o) setQuery("");
        }}
        title={t("mp.title")}
        footer={
          <>
            <Button
              onClick={() => {
                setOpen(false);
                nav.push("/providers");
              }}
            >
              {t("mp.manage")}
            </Button>
            <Button onClick={() => setOpen(false)}>{t("ui.close")}</Button>
          </>
        }
      >
        <div className="mpicker">
          <SearchField
            aria-label={t("mp.search")}
            placeholder={t("mp.search")}
            value={query}
            onValueChange={setQuery}
            autoFocus
          />
          <div className="mpicker__scroll">
            {pinned.length === 0 && groups.length === 0 && <p className="z-small">{t("mp.none")}</p>}
            {pinned.length > 0 && (
              <section aria-label={t("mp.pinned")}>
                <h3 className="z-label mpicker__group">{t("mp.pinned")}</h3>
                <ul className="mlist">{pinned.map((m) => row(m, "pin:"))}</ul>
              </section>
            )}
            {groups.length > 0 && (
              <section aria-label={t("mp.all")}>
                <h3 className="z-label mpicker__group">{t("mp.all")}</h3>
                {groups.map(([id, g]) => (
                  <section key={id} aria-label={g.name}>
                    <h4 className="z-small mpicker__provider">{g.name}</h4>
                    <ul className="mlist">{g.models.map((m) => row(m, "all:"))}</ul>
                  </section>
                ))}
              </section>
            )}
          </div>
        </div>
      </Dialog>
    </>
  );
}

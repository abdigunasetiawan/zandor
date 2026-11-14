// Tab "Uji (ping)" pemilih model: uji semua model, per provider, atau per model. Hanya koneksi kustom/lokal milik pengguna
// yang bisa diuji dari klien; model provider katalog (mis. opencode) tetap DITAMPILKAN dengan status "Tidak bisa diuji"
// beserta alasannya, bukan disembunyikan. Menampilkan status, waktu respons, kode HTTP, jumlah model yang dilaporkan, waktu
// uji terakhir, dan galat nyata; bisa diurutkan dan dikelompokkan.
import { type ModelOption, pingKey } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Button, IconButton, ModelLogo, PingIcon, Select, Spinner } from "@zandor/ui";
import { useMemo, useState } from "react";
import { groupRows, type PingGroup, type PingRow, type PingSort, rowStatus, sortRows, summarize } from "./model-ping";
import { useChat, useProviders, useServices } from "./services";

const clock = (at: number, locale: string) => new Date(at).toLocaleTimeString(locale === "id" ? "id-ID" : "en-US");

export function ModelPingTab() {
  const { t, locale } = useI18n();
  const { host, providers: store } = useServices();
  const { models } = useChat();
  const pv = useProviders();
  const [sort, setSort] = useState<PingSort>("status");
  const [group, setGroup] = useState<PingGroup>("provider");

  const ownedIds = useMemo(
    () => new Set(pv.providers.filter((p) => p.kind !== "builtin").map((p) => p.id)),
    [pv.providers],
  );
  const rows: PingRow[] = useMemo(
    () =>
      models.map((m) => ({
        model: m,
        state: pv.pings[pingKey(m.providerID, m.modelID)],
        pingable: host.native || ownedIds.has(m.providerID),
      })),
    [models, ownedIds, pv.pings, host.native],
  );
  const groups = useMemo(() => groupRows(sortRows(rows, sort), group), [rows, sort, group]);
  const sum = summarize(rows);
  const pingableModels = rows.filter((r) => r.pingable).map((r) => r.model);
  const busy = sum.pending > 0;

  const run = (list: ModelOption[]) => {
    void (async () => {
      for (const m of list) await store.ping(m.providerID, m.modelID);
    })();
  };

  const statusLabel = (k: string) =>
    k === "untested"
      ? t("mp.st.untested")
      : k === "unsupported"
        ? t("mp.st.unsupported")
        : k === "pending"
          ? t("pv.pinging")
          : k === "ok"
            ? t("mp.st.ok")
            : t(`pv.ping.${k}` as never);

  return (
    <div className="mping">
      <div className="mping__bar">
        <Button
          size="sm"
          variant="solid"
          disabled={busy || pingableModels.length === 0}
          onClick={() => run(pingableModels)}
        >
          {t("mp.ping.all", { n: pingableModels.length })}
        </Button>
        <Select
          aria-label={t("mp.ping.sort")}
          size="sm"
          items={(["status", "name", "provider", "latency"] as PingSort[]).map((v) => ({
            value: v,
            label: `${t("mp.sort.short")}: ${t(`mp.sort.${v}` as never)}`,
          }))}
          value={sort}
          onValueChange={(v) => setSort(v as PingSort)}
        />
        <Select
          aria-label={t("mp.ping.group")}
          size="sm"
          items={(["provider", "status", "none"] as PingGroup[]).map((v) => ({
            value: v,
            label: `${t("mp.group.short")}: ${t(`mp.group.${v}` as never)}`,
          }))}
          value={group}
          onValueChange={(v) => setGroup(v as PingGroup)}
        />
      </div>
      <p className="z-small mping__summary" role="status">
        {t("mp.ping.summary", { ok: sum.ok, bad: sum.failed, todo: sum.untested + sum.pending, na: sum.unsupported })}
      </p>
      {sum.unsupported > 0 && <p className="z-small mping__note">{t("mp.pingOnlyOwn")}</p>}
      <p className="z-small mping__note">{t("mp.ping.cost")}</p>
      <div className="mpicker__scroll">
        {groups.map((g) => {
          const list = g.rows.filter((r) => r.pingable).map((r) => r.model);
          const title =
            group === "provider"
              ? (g.rows[0]?.model.providerName ?? g.id)
              : group === "status"
                ? statusLabel(g.id)
                : "";
          return (
            <section key={g.id || "all"} aria-label={title || t("mp.all")} className="mgroup">
              {group !== "none" && (
                <header className="mgroup__head">
                  <span className="mgroup__toggle mgroup__toggle--static">
                    {group === "provider" && <ModelLogo providerID={g.id} name={title} />}
                    <span className="mgroup__name">{title}</span>
                    <span className="mgroup__count z-small">({g.rows.length})</span>
                  </span>
                  {list.length > 0 && (
                    <IconButton
                      label={
                        group === "provider"
                          ? t("mp.ping.provider", { name: title })
                          : t("mp.ping.group.run", { name: title })
                      }
                      size="sm"
                      icon={<PingIcon />}
                      disabled={g.rows.some((r) => r.state?.status === "pending")}
                      onClick={() => run(list)}
                    />
                  )}
                </header>
              )}
              <ul className="mlist mping__list">
                {g.rows.map((r) => {
                  const { model: m, state } = r;
                  const k = rowStatus(r);
                  const tone =
                    k === "ok" ? "ok" : k === "untested" || k === "pending" || k === "unsupported" ? "idle" : "bad";
                  return (
                    <li key={`${m.providerID}/${m.modelID}`} className="mping__row" data-status={k}>
                      <ModelLogo providerID={m.providerID} modelID={m.modelID} name={m.providerName} />
                      <span className="mrow__text">
                        <span className="mrow__name">{m.name}</span>
                        <span className="mrow__provider z-small">
                          {group === "provider" ? m.modelID : `${m.providerName} · ${m.modelID}`}
                        </span>
                        {state?.status === "done" &&
                          (() => {
                            const meta = [
                              state.result.status !== null ? `HTTP ${state.result.status}` : null,
                              t("mp.ping.models", { n: state.result.models.length }),
                              t("mp.ping.checked", { time: clock(state.at, locale) }),
                              state.result.error,
                            ]
                              .filter(Boolean)
                              .join(" · ");
                            // satu baris saja; teks lengkap (mis. pesan galat panjang) ada di tooltip
                            return (
                              <span className="mping__meta z-small" title={meta}>
                                {meta}
                              </span>
                            );
                          })()}
                      </span>
                      <span className="mping__status z-small" data-tone={tone} role="status">
                        {k === "pending" ? <Spinner size="sm" /> : statusLabel(k)}
                        {state?.status === "done" && (
                          <span className="mping__latency">{t("mp.ping.latency", { ms: state.result.latencyMs })}</span>
                        )}
                      </span>
                      {r.pingable && (
                        <IconButton
                          label={t("pv.ping", { name: m.name })}
                          size="sm"
                          icon={<PingIcon />}
                          disabled={state?.status === "pending"}
                          onClick={() => void store.ping(m.providerID, m.modelID)}
                        />
                      )}
                    </li>
                  );
                })}
              </ul>
            </section>
          );
        })}
      </div>
    </div>
  );
}

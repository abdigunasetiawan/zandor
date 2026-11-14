// Baris model di pemilih model: pilih, status ping (bila koneksi milik pengguna), uji, dan sematkan.
import { type ModelOption, modelKey, pingKey } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { IconButton, ModelLogo, PingIcon, PinIcon, Spinner } from "@zandor/ui";
import { pingLabel } from "./ProvidersPage";
import { useChat, useProviders, useServices } from "./services";

export function ModelRow({
  model: m,
  showProvider,
  onChoose,
}: {
  model: ModelOption;
  showProvider: boolean;
  onChoose: (m: ModelOption) => void;
}) {
  const { t } = useI18n();
  const { chat, host, providers: store } = useServices();
  const s = useChat();
  const pv = useProviders();
  const isPinned = s.pinned.includes(modelKey(m));
  // koneksi kustom/lokal selalu bisa diuji; provider katalog bisa diuji di aplikasi desktop (Rust membaca endpoint dari engine)
  const pingable = host.native || pv.providers.some((p) => p.id === m.providerID && p.kind !== "builtin");
  const ping = pingable ? pv.pings[pingKey(m.providerID, m.modelID)] : undefined;
  const selected = s.model?.providerID === m.providerID && s.model?.modelID === m.modelID;
  return (
    <li className="mrow" data-selected={selected ? "" : undefined}>
      <button
        type="button"
        className="mrow__pick"
        aria-current={selected ? "true" : undefined}
        onClick={() => onChoose(m)}
      >
        <ModelLogo providerID={m.providerID} modelID={m.modelID} name={m.providerName} />
        <span className="mrow__text">
          <span className="mrow__name">{m.name}</span>
          {showProvider && <span className="mrow__provider z-small">{m.providerName}</span>}
        </span>
      </button>
      <span className="mrow__status z-small" role="status">
        {ping?.status === "pending" && <Spinner size="sm" />}
        {ping?.status === "done" && (
          <span data-tone={ping.result.ok ? "ok" : "bad"} title={pingLabel(t, ping.result)}>
            {ping.result.ok
              ? t("pv.ping.ok", { ms: ping.result.latencyMs })
              : t(`pv.ping.${ping.result.kind}` as never)}
          </span>
        )}
      </span>
      {pingable && (
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
}

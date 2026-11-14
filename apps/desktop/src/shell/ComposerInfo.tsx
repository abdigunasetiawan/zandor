// Info pemakaian di composer: cincin kecil yang menunjukkan seberapa penuh jendela konteks; hover/klik membuka popup rincian
// (model, konteks, token, biaya bila provider melaporkan, dsb). Semua angka dihitung dari pesan sesi nyata; batas dari
// metadata model engine. Kuota/limit tarif provider tidak diekspos engine, jadi tidak ditampilkan (bukan angka palsu).
// Batas model tidak diketahui → cincin kosong bergaris putus (tanpa persentase palsu).
import { formatTokens, type SessionChat, sessionUsage, usageLevel } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { Popover } from "@zandor/ui";
import { useChat } from "./services";

const duration = (ms: number) => {
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s} dtk` : `${Math.floor(s / 60)} mnt ${s % 60} dtk`;
};

// Geometri cincin dalam satuan viewBox (bukan piksel layar): jari-jari 8, tebal 2.
const RADIUS = 8;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export function ComposerInfo({ session }: { session: SessionChat }) {
  const { t, locale } = useI18n();
  const { models, model, variant, permissionMode } = useChat();

  const last = [...session.messages].reverse().find((m) => m.info.role === "assistant" && m.info.modelID);
  const used =
    models.find((m) => m.providerID === last?.info.providerID && m.modelID === last?.info.modelID) ??
    models.find((m) => m.providerID === model?.providerID && m.modelID === model?.modelID);
  const u = sessionUsage(session.messages, used?.contextLimit ?? null, used?.outputLimit ?? null);
  const level = usageLevel(u.contextPercent);
  const fmt = (n: number) => formatTokens(n, locale);
  const modelLabel = used?.name ?? (model ? model.modelID : t("usage.noModel"));

  const contextText =
    u.contextTokens === null
      ? t("usage.contextNone")
      : u.contextLimit
        ? `${fmt(u.contextTokens)} / ${fmt(u.contextLimit)} (${(u.contextPercent ?? 0).toFixed(0)}%)`
        : `${fmt(u.contextTokens)} (${t("info.contextUnknown")})`;
  const pct = u.contextPercent === null ? null : Math.min(100, Math.max(0, u.contextPercent));

  const ring = (
    <button
      type="button"
      className="cring"
      aria-label={`${t("usage.toggle")}: ${pct === null ? t("usage.contextNone") : `${pct.toFixed(0)}%`}`}
    >
      <svg className="cring__svg" viewBox="0 0 20 20" aria-hidden="true" focusable="false">
        <circle className="cring__track" cx="10" cy="10" r={RADIUS} data-unknown={pct === null ? "" : undefined} />
        {pct !== null && pct > 0 && (
          <circle
            className="cring__value"
            cx="10"
            cy="10"
            r={RADIUS}
            strokeDasharray={`${(pct / 100) * CIRCUMFERENCE} ${CIRCUMFERENCE}`}
          />
        )}
      </svg>
    </button>
  );

  return (
    <div className="cinfo" data-level={level} data-testid="composer-info">
      <Popover trigger={ring} title={modelLabel} openOnHover>
        <div className="cinfo__body" data-level={level}>
          <p className="cinfo__line z-small">
            <span>{t("usage.context")}</span>{" "}
            <span className="cinfo__ctx" data-testid="cinfo-context">
              {contextText}
            </span>
          </p>
          {pct !== null && <progress className="cinfo__meter" max={100} value={pct} aria-label={t("usage.context")} />}
          {level !== "ok" && (
            <p className="cinfo__warn z-small" role="status">
              {level === "high" ? t("usage.warnHigh") : t("usage.warnSoon")}
            </p>
          )}
          {u.tokens.total > 0 && (
            <p className="cinfo__line z-small" data-testid="cinfo-tokens">
              ↑{fmt(u.tokens.input)} ↓{fmt(u.tokens.output)}
              {u.cost !== null && <span data-testid="cinfo-cost"> · ${u.cost.toFixed(4)}</span>}
            </p>
          )}
          <dl className="cinfo__detail z-small" aria-label={t("usage.detail")}>
            <dt>{t("usage.provider")}</dt>
            <dd>{used ? `${used.providerName} (${used.providerID})` : "—"}</dd>
            <dt>{t("usage.modelId")}</dt>
            <dd>{used?.modelID ?? model?.modelID ?? "—"}</dd>
            <dt>{t("usage.remaining")}</dt>
            <dd>{u.contextRemaining === null ? "—" : fmt(u.contextRemaining)}</dd>
            <dt>{t("usage.outputLimit")}</dt>
            <dd>{u.outputLimit === null ? "—" : fmt(u.outputLimit)}</dd>
            <dt>{t("usage.tokens")}</dt>
            <dd>
              {t("info.tokensValue", {
                input: fmt(u.tokens.input),
                output: fmt(u.tokens.output),
                reasoning: fmt(u.tokens.reasoning),
              })}
            </dd>
            <dt>{t("usage.cost")}</dt>
            <dd>{u.cost === null ? t("info.costNone") : `$${u.cost.toFixed(4)}`}</dd>
            <dt>{t("usage.messages")}</dt>
            <dd>{t("info.messagesValue", { user: u.messages.user, assistant: u.messages.assistant })}</dd>
            <dt>{t("usage.tools")}</dt>
            <dd>{u.toolCalls.total}</dd>
            <dt>{t("usage.work")}</dt>
            <dd>{duration(u.workMs)}</dd>
            <dt>{t("usage.permission")}</dt>
            <dd>{t(`chat.mode.${permissionMode}` as never)}</dd>
            <dt>{t("usage.variant")}</dt>
            <dd>{variant ?? t("chat.variant.default")}</dd>
          </dl>
          <p className="cinfo__note z-small">{t("usage.quotaNote")}</p>
        </div>
      </Popover>
    </div>
  );
}

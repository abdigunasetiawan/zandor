// Bar info di composer (Fase 12): model aktif, pemakaian jendela konteks (dengan peringatan bila hampir penuh), token,
// biaya (hanya bila provider melaporkan), dan rincian lengkap saat dibuka. Semua angka dihitung dari pesan sesi nyata;
// batas dari metadata model engine. Kuota/limit tarif provider tidak diekspos engine, jadi tidak ditampilkan (bukan angka palsu).
import { formatTokens, type SessionChat, sessionUsage, usageLevel } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { useState } from "react";
import { useChat } from "./services";

const duration = (ms: number) => {
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s} dtk` : `${Math.floor(s / 60)} mnt ${s % 60} dtk`;
};

export function ComposerInfo({ session }: { session: SessionChat }) {
  const { t, locale } = useI18n();
  const { models, model, variant, permissionMode } = useChat();
  const [open, setOpen] = useState(false);

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

  return (
    <div className="cinfo" data-level={level} data-testid="composer-info">
      <button
        type="button"
        className="cinfo__bar"
        aria-expanded={open}
        aria-label={t("usage.toggle")}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="cinfo__model">{modelLabel}</span>
        <span className="cinfo__sep" aria-hidden="true">
          ·
        </span>
        <span className="cinfo__ctx" data-testid="cinfo-context">
          {t("usage.context")} {contextText}
        </span>
        {u.contextPercent !== null && (
          <progress className="cinfo__meter" max={100} value={u.contextPercent} aria-label={t("usage.context")} />
        )}
        {u.tokens.total > 0 && (
          <span className="cinfo__tok" data-testid="cinfo-tokens">
            ↑{fmt(u.tokens.input)} ↓{fmt(u.tokens.output)}
          </span>
        )}
        {u.cost !== null && <span data-testid="cinfo-cost">${u.cost.toFixed(4)}</span>}
      </button>
      {level !== "ok" && (
        <p className="cinfo__warn z-small" role="status">
          {level === "high" ? t("usage.warnHigh") : t("usage.warnSoon")}
        </p>
      )}
      {open && (
        <dl className="cinfo__detail z-small" aria-label={t("usage.detail")}>
          <dt>{t("usage.provider")}</dt>
          <dd>{used ? `${used.providerName} (${used.providerID})` : "—"}</dd>
          <dt>{t("usage.modelId")}</dt>
          <dd>{used?.modelID ?? model?.modelID ?? "—"}</dd>
          <dt>{t("usage.context")}</dt>
          <dd>{contextText}</dd>
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
          <dd className="cinfo__note">{t("usage.quotaNote")}</dd>
        </dl>
      )}
    </div>
  );
}

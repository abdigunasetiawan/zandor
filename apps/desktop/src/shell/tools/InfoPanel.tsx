// Panel 4 — Info sesi: model, penggunaan token/konteks, biaya (bila dilaporkan engine), tool call, waktu.
import { sessionTitle, sessionUsage } from "@zandor/core";
import { useI18n } from "@zandor/i18n/react";
import { useChat, useProjects, useServices } from "../services";

const nf = new Intl.NumberFormat();

function Row({ label, children, testId }: { label: string; children: React.ReactNode; testId?: string }) {
  return (
    <>
      <dt className="z-small">{label}</dt>
      <dd data-testid={testId}>{children}</dd>
    </>
  );
}

function duration(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} dtk`;
  const m = Math.floor(s / 60);
  return `${m} mnt ${s % 60} dtk`;
}

export function InfoPanel() {
  const { t } = useI18n();
  const { projects } = useServices();
  const state = useProjects();
  const chat = useChat();
  const sel = state.selection.sessionId ? projects.selected() : null;
  const session = sel ? chat.sessions[sel.session.id] : undefined;
  if (!sel || !session) return <p className="sidebar__note z-small">{t("info.empty")}</p>;

  const model = chat.models.find((m) => m.providerID === chat.model?.providerID && m.modelID === chat.model?.modelID);
  const last = [...session.messages].reverse().find((m) => m.info.role === "assistant" && m.info.modelID);
  const usedModel =
    chat.models.find((m) => m.providerID === last?.info.providerID && m.modelID === last?.info.modelID) ?? model;
  const u = sessionUsage(session.messages, usedModel?.contextLimit ?? null);

  return (
    <div className="infop">
      <h3 className="z-label">{sessionTitle(sel.session) ?? t("sb.untitled")}</h3>
      <dl className="infop__grid">
        <Row label={t("info.project")}>{sel.project?.name ?? t("chat.noProject")}</Row>
        <Row label={t("info.dir")}>
          <code className="infop__path">{sel.session.directory}</code>
        </Row>
        <Row label={t("info.model")} testId="info-model">
          {u.model.modelID ? `${u.model.modelID} (${u.model.providerID})` : (usedModel?.name ?? "—")}
        </Row>
        <Row label={t("info.messages")}>
          {t("info.messagesValue", { user: u.messages.user, assistant: u.messages.assistant })}
        </Row>
        <Row label={t("info.context")} testId="info-context">
          {u.contextTokens === null
            ? "—"
            : u.contextLimit
              ? `${nf.format(u.contextTokens)} / ${nf.format(u.contextLimit)} (${(u.contextPercent ?? 0).toFixed(1)}%)`
              : `${nf.format(u.contextTokens)} (${t("info.contextUnknown")})`}
          {u.contextPercent !== null && (
            <progress className="infop__bar" max={100} value={u.contextPercent} aria-label={t("info.context")} />
          )}
        </Row>
        <Row label={t("info.tokens")} testId="info-tokens">
          {t("info.tokensValue", {
            input: nf.format(u.tokens.input),
            output: nf.format(u.tokens.output),
            reasoning: nf.format(u.tokens.reasoning),
          })}
        </Row>
        <Row label={t("info.cost")} testId="info-cost">
          {u.cost === null ? t("info.costNone") : `$${u.cost.toFixed(4)}`}
        </Row>
        <Row label={t("info.tools")} testId="info-tools">
          {u.toolCalls.total === 0
            ? "—"
            : `${u.toolCalls.total} (${t("info.toolsValue", { ok: u.toolCalls.completed, failed: u.toolCalls.failed, running: u.toolCalls.running })})`}
        </Row>
        {Object.keys(u.toolCalls.byTool).length > 0 && (
          <Row label={t("info.byTool")}>
            {Object.entries(u.toolCalls.byTool)
              .sort((a, b) => b[1] - a[1])
              .map(([k, n]) => `${k} ×${n}`)
              .join(", ")}
          </Row>
        )}
        <Row label={t("info.work")}>{duration(u.workMs)}</Row>
        {u.firstAt !== null && <Row label={t("info.first")}>{new Date(u.firstAt).toLocaleString()}</Row>}
        <Row label={t("info.session")}>
          <code>{sel.session.id}</code>
        </Row>
      </dl>
    </div>
  );
}
